//! A system name lookup off the request's own thread. The OS resolver
//! (`getaddrinfo`) can't be interrupted, so each lookup runs on a helper of its
//! own and the request waits for whichever comes first: the answer, its
//! signal's abort, or its deadline. An abort or a deadline returns the request
//! at once even while the resolver hangs; the helper finishes in the
//! background and its answer is dropped. At most [`HELPERS`] lookups run at
//! once, and a request waiting for a free helper waits interruptibly too.

use crate::stdlib::abort::AbortSignal;
use std::sync::{mpsc, Condvar, Mutex, MutexGuard};
use std::time::Instant;

/// Lookups that may run at once, per process.
pub(crate) const HELPERS: usize = 4;

/// The helpers' count, and its wake.
pub(crate) struct Pool {
    active: Mutex<usize>,
    freed: Condvar,
    most: usize,
    /// Requests waiting for a free helper now (for tests to observe).
    waiting: std::sync::atomic::AtomicUsize,
}

/// The process's lookups.
static SYSTEM: Pool = Pool::new(HELPERS);

/// Why a lookup gave no answer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Unanswered {
    Aborted,
    TimedOut,
    /// No helper could start, or one ended without an answer.
    Failed(String),
}

/// What reaches the waiting request.
enum Message<T> {
    Found(T),
    Aborted,
    /// The helper ended without an answer (its lookup panicked).
    Ended,
}

/// A helper's place in the pool: given back when the helper ends, however it
/// ends, and saying so if it ends with no answer sent.
struct Slot<T> {
    pool: &'static Pool,
    answered: Option<mpsc::Sender<Message<T>>>,
}
impl<T> Drop for Slot<T> {
    fn drop(&mut self) {
        self.pool.free();
        if let Some(answered) = self.answered.take() {
            let _ = answered.send(Message::Ended);
        }
    }
}

/// Run `lookup` on one of the process's helpers and wait for its answer,
/// `signal`'s abort, or `deadline`.
pub(crate) fn off_thread<T: Send + 'static>(
    signal: &AbortSignal,
    deadline: Option<Instant>,
    lookup: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Unanswered> {
    SYSTEM.run(signal, deadline, lookup)
}

impl Pool {
    pub(crate) const fn new(most: usize) -> Pool {
        Pool {
            active: Mutex::new(0),
            freed: Condvar::new(),
            most,
            waiting: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn count(&self) -> MutexGuard<'_, usize> {
        self.active.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn free(&self) {
        *self.count() -= 1;
        self.freed.notify_all();
    }

    /// Lookups running now.
    #[cfg(test)]
    pub(crate) fn running(&self) -> usize {
        *self.count()
    }

    /// Requests waiting for a free helper now.
    #[cfg(test)]
    pub(crate) fn waiting(&self) -> usize {
        self.waiting.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub(crate) fn run<T: Send + 'static>(
        &'static self,
        signal: &AbortSignal,
        deadline: Option<Instant>,
        lookup: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, Unanswered> {
        let expired = || deadline.is_some_and(|at| Instant::now() >= at);
        let (answered, answer) = mpsc::channel::<Message<T>>();
        let woken = answered.clone();
        // The abort wakes the wait for an answer and the wait for a helper. It
        // takes the count's lock to notify, so a waiter between its check and
        // its wait can't miss it.
        let _registration = signal.register(move || {
            let _ = woken.send(Message::Aborted);
            let _count = self.count();
            self.freed.notify_all();
        });
        {
            let mut count = self.count();
            let waited = *count >= self.most;
            if waited {
                self.waiting
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            while *count >= self.most && !signal.aborted() && !expired() {
                count = match deadline {
                    None => self.freed.wait(count).unwrap_or_else(|e| e.into_inner()),
                    Some(at) => {
                        let left = at.saturating_duration_since(Instant::now());
                        (self.freed.wait_timeout(count, left))
                            .unwrap_or_else(|e| e.into_inner())
                            .0
                    }
                };
            }
            if waited {
                self.waiting
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
            // Checked again with a helper free: one that is no longer wanted
            // starts no lookup.
            if signal.aborted() {
                return Err(Unanswered::Aborted);
            }
            if expired() {
                return Err(Unanswered::TimedOut);
            }
            *count += 1;
        }
        #[cfg(test)]
        let held = gate::held_here();
        let slot = Slot {
            pool: self,
            answered: Some(answered),
        };
        let started = std::thread::Builder::new()
            .name("ibex2-lookup".into())
            .spawn(move || {
                let mut slot = slot;
                #[cfg(test)]
                let _held = held.map(|held| held.pass());
                let found = lookup();
                if let Some(answered) = slot.answered.take() {
                    let _ = answered.send(Message::Found(found));
                }
            });
        if let Err(error) = started {
            // The closure, and its slot with it, was dropped: the place is back.
            return Err(Unanswered::Failed(format!(
                "a lookup helper could not start: {error}"
            )));
        }
        let message = match deadline {
            None => answer.recv().ok(),
            Some(at) => match answer.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(message) => Some(message),
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(Unanswered::TimedOut),
                Err(mpsc::RecvTimeoutError::Disconnected) => None,
            },
        };
        match message {
            Some(_) if signal.aborted() => Err(Unanswered::Aborted),
            // An answer that came at the deadline is not taken after it.
            Some(Message::Found(_)) if expired() => Err(Unanswered::TimedOut),
            Some(Message::Found(found)) => Ok(found),
            Some(Message::Aborted) => Err(Unanswered::Aborted),
            Some(Message::Ended) | None => Err(Unanswered::Failed(
                "the lookup ended without an answer".into(),
            )),
        }
    }
}

/// Tests hold the lookups a thread starts at their beginning, as a hung
/// resolver would: only that thread's, so other tests' lookups go on.
#[cfg(test)]
pub(crate) mod gate {
    use std::cell::RefCell;
    use std::sync::{Arc, Condvar, Mutex};

    /// A gate: held until opened, and how many lookups wait at it.
    #[derive(Default)]
    pub(crate) struct Gate {
        state: Mutex<(bool, usize)>,
        opened: Condvar,
    }

    thread_local! {
        static HERE: RefCell<Option<Arc<Gate>>> = const { RefCell::new(None) };
    }

    /// Opens its gate when dropped, so a failing test leaves no lookup held.
    pub(crate) struct Opens(pub(crate) Arc<Gate>);
    impl Drop for Opens {
        fn drop(&mut self) {
            self.0.open();
        }
    }

    impl Gate {
        /// A held gate for the lookups this thread starts from now on.
        pub(crate) fn hold_here() -> Arc<Gate> {
            let gate = Arc::new(Gate::default());
            gate.state.lock().unwrap().0 = true;
            HERE.with(|here| *here.borrow_mut() = Some(gate.clone()));
            gate
        }
        pub(crate) fn open(&self) {
            self.state.lock().unwrap().0 = false;
            self.opened.notify_all();
        }
        /// Lookups waiting at it now.
        pub(crate) fn waiting(&self) -> usize {
            self.state.lock().unwrap().1
        }
        pub(super) fn pass(&self) {
            let mut state = self.state.lock().unwrap();
            state.1 += 1;
            while state.0 {
                state = self.opened.wait(state).unwrap();
            }
            state.1 -= 1;
        }
    }

    pub(super) fn held_here() -> Option<Arc<Gate>> {
        HERE.with(|here| here.borrow().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::abort::AbortController;
    use std::sync::Arc;
    use std::time::Duration;

    const PROMPT: Duration = Duration::from_millis(500);
    const BOUND: Duration = Duration::from_secs(5);

    fn until(mut ready: impl FnMut() -> bool) {
        let end = Instant::now() + BOUND;
        while !ready() {
            assert!(Instant::now() < end, "timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// A hung resolver's lookup, released by `release` or when the gate drops
    /// (so a failing test never leaves helpers held).
    struct Hung(Arc<(Mutex<bool>, Condvar)>);
    impl Hung {
        fn new() -> Hung {
            Hung(Arc::new((Mutex::new(false), Condvar::new())))
        }
        fn lookup(&self) -> impl FnOnce() -> &'static str + Send + 'static {
            let gate = self.0.clone();
            move || {
                let (open, opened) = &*gate;
                let mut open = open.lock().unwrap();
                while !*open {
                    open = opened.wait(open).unwrap();
                }
                "found"
            }
        }
        fn release(&self) {
            *self.0 .0.lock().unwrap() = true;
            self.0 .1.notify_all();
        }
    }
    impl Drop for Hung {
        fn drop(&mut self) {
            self.release();
        }
    }

    /// `work` on a thread, its result awaited at most `BOUND`.
    fn bounded<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> mpsc::Receiver<T> {
        let (done, result) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(work());
        });
        result
    }

    #[test]
    fn an_abort_mid_lookup_returns_at_once_while_the_lookup_still_runs() {
        static POOL: Pool = Pool::new(HELPERS);
        let hung = Hung::new();
        let controller = AbortController::new();
        let signal = controller.signal();
        let lookup = hung.lookup();
        let waiting = bounded(move || POOL.run(&signal, None, lookup));
        until(|| POOL.running() == 1);
        let aborted = Instant::now();
        controller.abort();
        assert_eq!(
            waiting.recv_timeout(BOUND).expect("returned"),
            Err(Unanswered::Aborted)
        );
        assert!(aborted.elapsed() < PROMPT, "returned at the abort");
        assert_eq!(POOL.running(), 1, "the lookup itself still runs");
        hung.release();
        until(|| POOL.running() == 0);
    }

    #[test]
    fn a_deadline_mid_lookup_times_out_while_the_lookup_still_runs() {
        static POOL: Pool = Pool::new(HELPERS);
        let hung = Hung::new();
        let signal = AbortController::new().signal();
        let begun = Instant::now();
        let result = POOL.run(
            &signal,
            Some(begun + Duration::from_millis(100)),
            hung.lookup(),
        );
        assert_eq!(result, Err(Unanswered::TimedOut));
        assert!(begun.elapsed() < Duration::from_millis(100) + PROMPT);
        assert_eq!(POOL.running(), 1);
        hung.release();
        until(|| POOL.running() == 0);
    }

    #[test]
    fn with_every_helper_held_a_fifth_lookup_still_answers_its_abort_and_deadline() {
        static POOL: Pool = Pool::new(HELPERS);
        let hung = Hung::new();
        let four: Vec<_> = (0..HELPERS)
            .map(|_| {
                let signal = AbortController::new().signal();
                let lookup = hung.lookup();
                bounded(move || POOL.run(&signal, None, lookup))
            })
            .collect();
        until(|| POOL.running() == HELPERS);
        // The fifth waits for a free helper (seen waiting); its abort returns it at once.
        let controller = AbortController::new();
        let signal = controller.signal();
        let fifth = bounded(move || POOL.run(&signal, None, || "found"));
        until(|| POOL.waiting() == 1);
        let aborted = Instant::now();
        controller.abort();
        assert_eq!(
            fifth.recv_timeout(BOUND).expect("returned"),
            Err(Unanswered::Aborted)
        );
        assert!(aborted.elapsed() < PROMPT);
        // And a deadline while it waits.
        let signal = AbortController::new().signal();
        let begun = Instant::now();
        let result = POOL.run(&signal, Some(begun + Duration::from_millis(100)), || {
            "found"
        });
        assert_eq!(result, Err(Unanswered::TimedOut));
        assert!(begun.elapsed() < Duration::from_millis(100) + PROMPT);
        assert_eq!(POOL.running(), HELPERS, "no fifth helper started");
        assert_eq!(POOL.waiting(), 0);
        // Released, the four answer, and the helpers are free again.
        hung.release();
        for lookup in four {
            assert_eq!(lookup.recv_timeout(BOUND).expect("answered"), Ok("found"));
        }
        until(|| POOL.running() == 0);
        let signal = AbortController::new().signal();
        assert_eq!(POOL.run(&signal, None, || "again"), Ok("again"));
    }

    #[test]
    fn a_deadline_already_passed_when_a_helper_frees_starts_no_lookup() {
        static POOL: Pool = Pool::new(1);
        let hung = Hung::new();
        let signal = AbortController::new().signal();
        let lookup = hung.lookup();
        let first = bounded(move || POOL.run(&signal, None, lookup));
        until(|| POOL.running() == 1);
        // The second's deadline passes while it waits; the helper frees after.
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ran = started.clone();
        let signal = AbortController::new().signal();
        let second = bounded(move || {
            POOL.run(
                &signal,
                Some(Instant::now() + Duration::from_millis(50)),
                move || {
                    ran.store(true, std::sync::atomic::Ordering::SeqCst);
                },
            )
        });
        assert_eq!(
            second.recv_timeout(BOUND).expect("returned"),
            Err(Unanswered::TimedOut)
        );
        hung.release();
        assert_eq!(first.recv_timeout(BOUND).expect("answered"), Ok("found"));
        until(|| POOL.running() == 0);
        assert!(
            !started.load(std::sync::atomic::Ordering::SeqCst),
            "no lookup for a request past its deadline"
        );
    }

    #[test]
    fn a_lookup_that_panics_gives_its_helper_back_and_says_it_ended() {
        static POOL: Pool = Pool::new(1);
        let signal = AbortController::new().signal();
        let result = bounded(move || {
            POOL.run(&signal, None, || -> &'static str {
                panic!("the resolver failed")
            })
        });
        let result = result.recv_timeout(BOUND).expect("returned");
        assert_eq!(
            result,
            Err(Unanswered::Failed(
                "the lookup ended without an answer".into()
            ))
        );
        let signal = AbortController::new().signal();
        until(|| POOL.running() == 0);
        assert_eq!(POOL.run(&signal, None, || "again"), Ok("again"));
    }
}
