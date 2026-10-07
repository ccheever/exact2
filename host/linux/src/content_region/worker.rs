//! Private content font transport. Production owns ONE process-lifetime service
//! thread, never a new thread per controller or catalog. Admission remains busy
//! until all old session work/scratch has actually dropped on that thread.
use crate::wake::Stream as UnixStream;
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::{self, JoinHandle};

// Generic only for an allocation-observing test double. The sole production
// implementation owns font preparation/shaping, not arbitrary application work.
pub(super) trait TextWork: 'static {
    type Input: Send + 'static;
    type Output: Send + 'static;
    fn execute(&mut self, input: Self::Input) -> Self::Output;
}
pub(super) struct ThreadSlot<W: TextWork>(Mutex<Option<Service<W>>>);
#[cfg(test)]
impl<W: TextWork> Default for ThreadSlot<W> {
    fn default() -> Self {
        Self::new()
    }
}
struct Service<W: TextWork> {
    control: Arc<Control<W>>,
    _thread: JoinHandle<()>,
}
struct Control<W: TextWork> {
    state: Mutex<ServiceState<W>>,
    ready: Condvar,
}
struct ServiceState<W: TextWork> {
    next: Option<Session<W>>,
    current: Weak<Shared<W::Input, W::Output>>,
    occupied: bool,
    stop: bool,
}
struct Session<W: TextWork> {
    create: Box<dyn FnOnce() -> W + Send>,
    shared: Arc<Shared<W::Input, W::Output>>,
    signal: UnixStream,
}
impl<W: TextWork> ThreadSlot<W> {
    pub(super) const fn new() -> Self {
        Self(Mutex::new(None))
    }
    #[cfg(test)]
    pub(super) fn occupied(&self) -> bool {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|s| s.control.state.lock().unwrap().occupied)
    }
    pub(super) fn start(
        &self,
        create: impl FnOnce() -> W + Send + 'static,
    ) -> io::Result<Port<W::Input, W::Output>> {
        let mut slot = self.0.lock().unwrap();
        if slot.is_none() {
            let control = Arc::new(Control {
                state: Mutex::new(ServiceState {
                    next: None,
                    current: Weak::new(),
                    occupied: false,
                    stop: false,
                }),
                ready: Condvar::new(),
            });
            let worker_control = control.clone();
            let thread = thread::Builder::new()
                .name("exact-region-font".into())
                .spawn(move || serve(worker_control))?;
            *slot = Some(Service {
                control,
                _thread: thread,
            });
        }
        let service = slot.as_ref().unwrap();
        let mut state = service.control.state.lock().unwrap();
        if state.occupied {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "font session is still owned",
            ));
        }
        let (wake, signal) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        signal.set_nonblocking(true)?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                pending: None,
                completed: None,
                running: None,
                serial: 0,
                desired: None,
                closed: false,
            }),
            ready: Condvar::new(),
        });
        state.next = Some(Session {
            create: Box::new(create),
            shared: shared.clone(),
            signal,
        });
        state.current = Arc::downgrade(&shared);
        state.occupied = true;
        drop(state);
        service.control.ready.notify_one();
        Ok(Port { shared, wake })
    }
}
impl<W: TextWork> Drop for ThreadSlot<W> {
    fn drop(&mut self) {
        // Only isolated tests drop the service. Production keeps its singleton
        // for process lifetime; dropping/reloading a controller only closes Port.
        if let Some(service) = self.0.get_mut().unwrap().as_ref() {
            let current = {
                let mut state = service.control.state.lock().unwrap();
                state.stop = true;
                state.current.upgrade()
            };
            if let Some(current) = current {
                current.clear(true);
            }
            service.control.ready.notify_one();
        }
    }
}
fn serve<W: TextWork>(control: Arc<Control<W>>) {
    loop {
        let session = {
            let mut state = control.state.lock().unwrap();
            while state.next.is_none() && !state.stop {
                state = control.ready.wait(state).unwrap();
            }
            if state.stop && state.next.is_none() {
                return;
            }
            state.next.take().unwrap()
        };
        // Unexpected worker panic still retires that session and reports closure
        // through its pipe. It never spawns another service thread.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_session(session)));
        let mut state = control.state.lock().unwrap();
        state.current = Weak::new();
        state.occupied = false;
    }
}
struct SessionExit<I, O> {
    shared: Arc<Shared<I, O>>,
    signal: UnixStream,
}
impl<I, O> Drop for SessionExit<I, O> {
    fn drop(&mut self) {
        let (pending, completed) = {
            let mut state = self.shared.state.lock().unwrap();
            state.closed = true;
            state.desired = None;
            state.running = None;
            (state.pending.take(), state.completed.take())
        };
        drop(pending);
        drop(completed);
        let _ = (&self.signal).write(&[1]);
    }
}
fn run_session<W: TextWork>(session: Session<W>) {
    let Session {
        create,
        shared,
        signal,
    } = session;
    let exit = SessionExit { shared, signal };
    // Recipe construction and any new font-file copying happen here. Ordinary
    // pre-existing UI catalog startup is a separate, explicitly measured cost.
    let mut work = create();
    loop {
        let (serial, input) = {
            let mut state = exit.shared.state.lock().unwrap();
            while !state.closed && (state.pending.is_none() || state.completed.is_some()) {
                state = exit.shared.ready.wait(state).unwrap();
            }
            if state.closed {
                return;
            }
            let request = state.pending.take().unwrap();
            state.running = Some(request.0);
            request
        };
        let output = work.execute(input);
        let mut result = Some((serial, output));
        let published = {
            let mut state = exit.shared.state.lock().unwrap();
            if !state.closed && state.desired == Some(serial) {
                debug_assert!(state.completed.is_none());
                state.completed = result.take();
                true
            } else {
                false
            }
        };
        drop(result); // Stale actual allocations die before work is called idle.
        exit.shared.state.lock().unwrap().running = None;
        if published {
            let _ = (&exit.signal).write(&[1]);
        }
    }
}

struct Shared<I, O> {
    state: Mutex<State<I, O>>,
    ready: Condvar,
}
impl<I, O> Shared<I, O> {
    fn clear(&self, close: bool) {
        let (pending, completed) = {
            let mut state = self.state.lock().unwrap();
            state.closed |= close;
            state.desired = None;
            (state.pending.take(), state.completed.take())
        };
        drop(pending);
        drop(completed);
        self.ready.notify_one();
    }
}
struct State<I, O> {
    pending: Option<(u64, I)>,
    completed: Option<(u64, O)>,
    running: Option<u64>,
    serial: u64,
    desired: Option<u64>,
    closed: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Counts {
    pub running: usize,
    pub pending: usize,
    pub completed: usize,
}
pub(super) struct Port<I, O> {
    shared: Arc<Shared<I, O>>,
    wake: UnixStream,
}
impl<I, O> Port<I, O> {
    pub(super) fn submit(&self, input: I) -> Result<u64, &'static str> {
        let (serial, old_pending, old_completed) = {
            let mut state = self.shared.state.lock().unwrap();
            if state.closed {
                return Err("font controller is closed");
            }
            let serial = state
                .serial
                .checked_add(1)
                .ok_or("font request serial exhausted")?;
            state.serial = serial;
            state.desired = Some(serial);
            (
                serial,
                state.pending.replace((serial, input)),
                state.completed.take(),
            )
        };
        drop(old_pending);
        drop(old_completed);
        self.shared.ready.notify_one();
        Ok(serial)
    }
    pub(super) fn take(&self) -> Option<(u64, O)> {
        let mut bytes = [0; 32];
        while let Ok(n) = (&self.wake).read(&mut bytes) {
            if n == 0 {
                break;
            }
        }
        let result = self.shared.state.lock().unwrap().completed.take();
        self.shared.ready.notify_one();
        result
    }
    pub(super) fn cancel(&self) {
        self.clear(false);
    }
    pub(super) fn close(&self) {
        self.clear(true);
    }
    pub(super) fn is_closed(&self) -> bool {
        self.shared.state.lock().unwrap().closed
    }
    fn clear(&self, close: bool) {
        self.shared.clear(close);
    }
    pub(super) fn counts(&self) -> Counts {
        let state = self.shared.state.lock().unwrap();
        Counts {
            running: usize::from(state.running.is_some()),
            pending: usize::from(state.pending.is_some()),
            completed: usize::from(state.completed.is_some()),
        }
    }
    #[cfg(unix)]
    pub(super) fn fd(&self) -> RawFd {
        self.wake.as_raw_fd()
    }
}
impl<I, O> Drop for Port<I, O> {
    fn drop(&mut self) {
        self.clear(true);
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
