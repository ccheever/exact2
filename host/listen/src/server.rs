//! One listener's thread: it binds (or waits, D2), then polls its socket,
//! its connections, its directory and a wake pipe. Lines go to the sink in
//! arrival order, each connection's in order (D1, D3); what the executor
//! sends is queued per connection and written as the connection takes it.
//! A connection is never half-open: the peer's end of input ends it.
use super::{
    address::{self, Failure, Resolved},
    event, publish, registry, reply,
    watch::Watch,
    Options, Sink,
};
use std::{
    collections::HashMap,
    ffi::c_void,
    io::{ErrorKind, Read},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::net::{UnixListener, UnixStream},
    },
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// What another listener, or the registry, asks of this one (D2).
pub(crate) enum Poke {
    /// A newer listener of this process took the address over.
    Displaced,
    /// Bind: a waiting listener's turn, or a displaced one's address back.
    Bind,
}

/// What the executor's calls and the registry share with the thread. Both
/// ends of the wake pipe live here, so a poke after the thread has gone
/// writes to a pipe that is still open: no `SIGPIPE`.
pub(crate) struct Shared {
    wake: OwnedFd,
    wake_read: OwnedFd,
    state: Mutex<State>,
}

/// A lock that a panicked listener thread left poisoned still opens: the
/// state is plain data, and the C ABI must not panic.
fn guard<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Default)]
struct State {
    stop: bool,
    pokes: Vec<Poke>,
    /// Each open connection's unwritten bytes and whether the app ended it.
    out: HashMap<u64, Out>,
}

#[derive(Default)]
struct Out {
    bytes: Vec<u8>,
    end: bool,
}

impl Shared {
    pub(crate) fn poke(&self, poke: Poke) {
        guard(&self.state).pokes.push(poke);
        self.wake();
    }

    fn wake(&self) {
        let byte = 1u8;
        // A full pipe already wakes the thread.
        unsafe { libc::write(self.wake.as_raw_fd(), (&byte as *const u8).cast(), 1) };
    }
}

struct Handle {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

static LISTENERS: Mutex<Vec<(u64, Handle)>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

fn pipe() -> std::io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [r.as_raw_fd(), w.as_raw_fd()] {
        unsafe {
            libc::fcntl(
                fd,
                libc::F_SETFL,
                libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK,
            );
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
        }
    }
    Ok((r, w))
}

/// Start listener `options` on a thread of its own.
pub(crate) fn open(options: Options, sink: Sink, ctx: usize) -> u64 {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let failed = |why: String| {
        let text = format!("Network: {why}");
        sink(
            ctx as *mut c_void,
            event::ERROR,
            0,
            text.as_ptr(),
            text.len(),
        );
        id
    };
    let (wake_r, wake_w) = match pipe() {
        Ok(fds) => fds,
        Err(e) => return failed(format!("making the listener's wake pipe: {e}")),
    };
    let shared = Arc::new(Shared {
        wake: wake_w,
        wake_read: wake_r,
        state: Mutex::new(State::default()),
    });
    // The path is resolved and joined here, on the caller's thread, so that
    // of two listeners on one path the one opened later is the newer (D2).
    let start = address::prepare(&options.address, &options.grants).map(|resolved| {
        let joined = registry::join(&resolved.path, id, options.takeover, shared.clone());
        (resolved, joined)
    });
    let joined = start.is_ok();
    let mut listener = Listener {
        id,
        options,
        sink,
        ctx,
        shared: shared.clone(),
        start: Some(start),
        resolved: None,
        joined,
        bound: None,
        watch: None,
        conns: Vec::new(),
        next_connection: 1,
        gone: false,
        paused: None,
    };
    let thread = std::thread::Builder::new()
        .name(format!("exact-listen-{id}"))
        .spawn(move || {
            // A panic ends the listener with an error the executor waits on,
            // and its socket file and place on the path still go.
            let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener.run()));
            if run.is_err() {
                let text = "Network: the listener panicked";
                sink(
                    ctx as *mut c_void,
                    event::ERROR,
                    0,
                    text.as_ptr(),
                    text.len(),
                );
                let tidy = std::panic::AssertUnwindSafe(|| listener.teardown());
                let _ = std::panic::catch_unwind(tidy);
            }
        });
    match thread {
        Ok(thread) => {
            let handle = Handle {
                shared,
                thread: Some(thread),
            };
            guard(&LISTENERS).push((id, handle));
            id
        }
        Err(e) => failed(format!("starting the listener's thread: {e}")),
    }
}

/// Queue `bytes` and a newline on `connection`, or end it after what is
/// queued (`None`).
pub(crate) fn send(handle: u64, connection: u64, bytes: Option<&[u8]>) -> i32 {
    let listeners = guard(&LISTENERS);
    let Some((_, h)) = listeners.iter().find(|(id, _)| *id == handle) else {
        return 2;
    };
    let mut state = guard(&h.shared.state);
    let Some(out) = state.out.get_mut(&connection).filter(|out| !out.end) else {
        return 1;
    };
    match bytes {
        Some(bytes) => {
            out.bytes.extend_from_slice(bytes);
            out.bytes.push(b'\n');
        }
        None => out.end = true,
    }
    drop(state);
    h.shared.wake();
    0
}

/// `connection`'s unwritten bytes.
pub(crate) fn buffered(handle: u64, connection: u64) -> u64 {
    let listeners = guard(&LISTENERS);
    let Some((_, h)) = listeners.iter().find(|(id, _)| *id == handle) else {
        return u64::MAX;
    };
    let state = guard(&h.shared.state);
    state
        .out
        .get(&connection)
        .map_or(u64::MAX, |out| out.bytes.len() as u64)
}

/// Stop listener `handle` and wait for its thread.
pub(crate) fn close(handle: u64) {
    let found = {
        let mut listeners = guard(&LISTENERS);
        let at = listeners.iter().position(|(id, _)| *id == handle);
        at.map(|at| listeners.remove(at).1)
    };
    let Some(mut h) = found else { return };
    guard(&h.shared.state).stop = true;
    h.shared.wake();
    if let Some(thread) = h.thread.take() {
        let _ = thread.join();
    }
}

/// One connection, on the listener's thread.
struct Conn {
    id: u64,
    stream: UnixStream,
    input: Vec<u8>,
    /// Until its first complete line: when it ends `timeout`.
    deadline: Option<Instant>,
}

/// How a connection ended, as the `END` event says.
#[derive(Clone, Copy)]
enum End {
    Peer,
    App,
    Timeout,
    Overflow,
}

impl End {
    fn word(self) -> &'static str {
        match self {
            End::Peer => "peer",
            End::App => "app",
            End::Timeout => "timeout",
            End::Overflow => "overflow",
        }
    }
}

struct Listener {
    id: u64,
    options: Options,
    sink: Sink,
    ctx: usize,
    shared: Arc<Shared>,
    /// What `open` found of the path, taken by `start`.
    start: Option<Result<(Resolved, registry::Joined), Failure>>,
    resolved: Option<Resolved>,
    joined: bool,
    bound: Option<(UnixListener, u64)>,
    watch: Option<Watch>,
    conns: Vec<Conn>,
    next_connection: u64,
    /// The sink wants no more, or the listener failed: it idles until closed.
    gone: bool,
    /// Accepting failed for want of descriptors: the socket is not polled
    /// until then, or a readable socket would spin the thread.
    paused: Option<Instant>,
}

/// What `send` passes so a peer that left raises no `SIGPIPE`: a host whose
/// `main` is not Rust's does not ignore it.
#[cfg(any(target_os = "linux", target_os = "android"))]
const SEND_FLAGS: i32 = libc::MSG_NOSIGNAL;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const SEND_FLAGS: i32 = 0;

impl Listener {
    fn emit(&mut self, kind: u32, connection: u64, data: &[u8]) -> u32 {
        if self.gone {
            return reply::GONE;
        }
        let answer = (self.sink)(
            self.ctx as *mut c_void,
            kind,
            connection,
            data.as_ptr(),
            data.len(),
        );
        if answer == reply::GONE {
            self.gone = true;
        }
        answer
    }

    fn fail(&mut self, failure: Failure) {
        let text = format!("{}: {}", failure.kind, failure.message);
        self.emit(event::ERROR, 0, text.as_bytes());
        self.gone = true;
    }

    fn address(&self) -> String {
        String::from_utf8_lossy(&self.options.address).into_owned()
    }

    fn run(&mut self) {
        if let Err(failure) = self.start() {
            self.fail(failure);
        }
        while self.turn() {}
        self.teardown();
    }

    fn start(&mut self) -> Result<(), Failure> {
        let (resolved, joined) = self.start.take().expect("started once")?;
        self.resolved = Some(resolved);
        match joined {
            registry::Joined::Bind => self.bind(),
            registry::Joined::Wait => {
                let address = self.address();
                self.emit(event::WAITING, 0, address.as_bytes());
                Ok(())
            }
        }
    }

    /// Bind and publish, unless a newer listener of this process took the
    /// path over while this one was on its way (D2): checked under the
    /// path's lock, so the newest publishes last. Then this one waits,
    /// displaced, for the address back.
    fn bind(&mut self) -> Result<(), Failure> {
        let r = self.resolved.clone().expect("prepared");
        let id = self.id;
        let holder = || registry::role(id) == Some(registry::Role::Holder);
        let bound = match publish::bind(&r, self.options.takeover, holder) {
            Ok(Some(bound)) => bound,
            Ok(None) => {
                let address = self.address();
                self.emit(event::WAITING, 0, address.as_bytes());
                return Ok(());
            }
            Err(failure) => {
                registry::leave(self.id);
                self.joined = false;
                return Err(failure);
            }
        };
        self.watch = Watch::new(&r.parent).ok();
        self.bound = Some((bound.listener, bound.ino));
        let listening = format!(
            "{{\"address\":{},\"port\":0,\"tookOver\":{}}}",
            json_string(&self.address()),
            bound.took_over
        );
        self.emit(event::LISTENING, 0, listening.as_bytes());
        Ok(())
    }

    /// Bind again where the path is free, keeping the connections.
    fn take_back(&mut self) {
        let Some(r) = self.resolved.clone() else {
            return;
        };
        if self
            .bound
            .as_ref()
            .is_some_and(|(_, ino)| publish::is(&r, *ino))
        {
            return;
        }
        if let Ok(Some(bound)) = publish::relink(&r) {
            self.bound = Some((bound.listener, bound.ino));
            self.emit(event::RESUMED, 0, &[]);
        }
    }

    /// One poll and what it found; false once stopped.
    fn turn(&mut self) -> bool {
        let mut fds = vec![poll_fd(self.shared.wake_read.as_raw_fd(), libc::POLLIN)];
        let now = Instant::now();
        self.paused = self.paused.filter(|until| *until > now);
        let listening = !self.gone && self.bound.is_some() && self.paused.is_none();
        if let (true, Some((listener, _))) = (listening, &self.bound) {
            fds.push(poll_fd(listener.as_raw_fd(), libc::POLLIN));
        }
        let watching = !self.gone && self.watch.is_some();
        if let (true, Some(watch)) = (watching, &self.watch) {
            fds.push(poll_fd(watch.fd(), libc::POLLIN));
        }
        // Gone, it polls only the wake pipe: a closed peer would poll
        // `POLLHUP` on every turn.
        let first_conn = fds.len();
        if !self.gone {
            let state = guard(&self.shared.state);
            for conn in &self.conns {
                let writing = state.out.get(&conn.id).is_some_and(|o| !o.bytes.is_empty());
                let events = libc::POLLIN | if writing { libc::POLLOUT } else { 0 };
                fds.push(poll_fd(conn.stream.as_raw_fd(), events));
            }
        }
        // Rounded up: a deadline under a millisecond away waits, not spins.
        let deadlines = self
            .conns
            .iter()
            .filter_map(|c| c.deadline)
            .chain(self.paused);
        let timeout = deadlines.min().map_or(-1, |at| {
            (at.saturating_duration_since(now).as_micros().div_ceil(1000)).min(60_000) as i32
        });
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
        if n < 0 && std::io::Error::last_os_error().kind() != ErrorKind::Interrupted {
            self.fail(Failure::os("polling", std::io::Error::last_os_error()));
        }
        // The wake pipe: stop, pokes, and sends to write.
        let mut sink = [0u8; 64];
        let wake = self.shared.wake_read.as_raw_fd();
        while unsafe { libc::read(wake, sink.as_mut_ptr().cast(), sink.len()) } > 0 {}
        let pokes = {
            let mut state = guard(&self.shared.state);
            if state.stop {
                return false;
            }
            std::mem::take(&mut state.pokes)
        };
        for poke in pokes {
            match poke {
                // One displaced before it bound says it waits instead (`bind`).
                Poke::Displaced if self.bound.is_some() => {
                    self.emit(event::DISPLACED, 0, &[]);
                }
                Poke::Displaced => {}
                Poke::Bind if self.bound.is_some() => self.take_back(),
                Poke::Bind => {
                    if let Err(failure) = self.bind() {
                        self.fail(failure);
                    }
                }
            }
        }
        let ready = |i: usize, flag: i16| fds.get(i).is_some_and(|f| f.revents & flag != 0);
        let mut at = 1;
        if listening {
            if ready(at, libc::POLLIN) {
                self.accept();
            }
            at += 1;
        }
        if watching && ready(at, libc::POLLIN | libc::POLLHUP) {
            self.watched();
        }
        // Each connection polled, by id: accepting above may have added some.
        let polled: Vec<(u64, bool)> = (self.conns.iter().enumerate())
            .map(|(i, c)| {
                let f = fds.get(first_conn + i).map_or(0, |f| f.revents);
                (
                    c.id,
                    f & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0,
                )
            })
            .collect();
        for (id, readable) in polled {
            if readable && !self.gone {
                self.read(id);
            }
        }
        self.write();
        self.expire();
        true
    }

    fn accept(&mut self) {
        loop {
            let Some((listener, _)) = &self.bound else {
                return;
            };
            let stream = match listener.accept() {
                Ok((stream, _)) => stream,
                Err(e) if e.kind() == ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                // A connection that failed before it was taken, or the
                // process out of descriptors: the listener goes on, and
                // tries again in a moment.
                Err(_) => {
                    self.paused = Some(Instant::now() + Duration::from_millis(100));
                    return;
                }
            };
            if self.gone || self.conns.len() >= self.options.max_connections {
                continue;
            }
            if stream.set_nonblocking(true).is_err() {
                continue;
            }
            no_sigpipe(stream.as_raw_fd());
            let id = self.next_connection;
            self.next_connection += 1;
            guard(&self.shared.state).out.insert(id, Out::default());
            match self.emit(event::CONNECTION, id, &[]) {
                reply::TAKEN => {
                    let timeout = self.options.first_line_timeout_ms;
                    self.conns.push(Conn {
                        id,
                        stream,
                        input: Vec::new(),
                        deadline: (timeout > 0)
                            .then(|| Instant::now() + Duration::from_millis(timeout.into())),
                    });
                }
                _ => {
                    guard(&self.shared.state).out.remove(&id);
                }
            }
        }
    }

    /// The directory changed: when the path vanished and this listener
    /// holds the address in this process, it binds again (D4).
    fn watched(&mut self) {
        if let Some(watch) = &self.watch {
            watch.drain();
        }
        let vanished = self.resolved.as_ref().is_some_and(publish::vanished);
        if vanished && registry::role(self.id) == Some(registry::Role::Holder) {
            self.take_back();
        }
    }

    fn read(&mut self, id: u64) {
        let Some(at) = self.conns.iter().position(|c| c.id == id) else {
            return;
        };
        let mut chunk = [0u8; 16 << 10];
        let mut total = 0;
        let ended = loop {
            match self.conns[at].stream.read(&mut chunk) {
                Ok(0) => break Some(End::Peer),
                Ok(n) => {
                    self.conns[at].input.extend_from_slice(&chunk[..n]);
                    total += n;
                    if let Some(end) = self.lines(at) {
                        break Some(end);
                    }
                    // Fair to the other connections: the rest next turn.
                    if total >= 64 << 10 {
                        break None;
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break None,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break Some(End::Peer),
            }
        };
        if self.gone {
            return;
        }
        if let Some(End::Peer) = ended {
            // A last line the peer did not terminate is still a line.
            let rest = std::mem::take(&mut self.conns[at].input);
            let rest = rest.strip_suffix(b"\r").unwrap_or(&rest);
            if rest.len() > self.options.max_line_bytes {
                return self.end(id, End::Overflow);
            }
            if !rest.is_empty() {
                let line = String::from_utf8_lossy(rest).into_owned();
                match self.emit(event::MESSAGE, id, line.as_bytes()) {
                    reply::GONE => return,
                    reply::OVERFLOW => return self.end(id, End::Overflow),
                    _ => {}
                }
            }
            self.flush_one(id);
        }
        if let Some(end) = ended {
            self.end(id, end);
        }
    }

    /// The complete lines in connection `at`'s input, each to the sink.
    fn lines(&mut self, at: usize) -> Option<End> {
        let max = self.options.max_line_bytes;
        loop {
            let conn = &mut self.conns[at];
            let Some(pos) = conn.input.iter().position(|&b| b == b'\n') else {
                let len = conn.input.len();
                let cr = conn.input.last() == Some(&b'\r');
                return (len > max + usize::from(cr)).then_some(End::Overflow);
            };
            let mut line: Vec<u8> = conn.input.drain(..=pos).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.len() > max {
                return Some(End::Overflow);
            }
            conn.deadline = None;
            let id = conn.id;
            let text = String::from_utf8_lossy(&line).into_owned();
            match self.emit(event::MESSAGE, id, text.as_bytes()) {
                reply::OVERFLOW => return Some(End::Overflow),
                reply::GONE => return None,
                _ => {}
            }
        }
    }

    /// Write what each connection has queued, as far as it takes it, and
    /// end the connections the app ended once theirs is written.
    fn write(&mut self) {
        let ids: Vec<u64> = self.conns.iter().map(|c| c.id).collect();
        for id in ids {
            match self.flush_one(id) {
                Some(true) => self.end(id, End::App),
                Some(false) => {}
                None => self.end(id, End::Peer),
            }
        }
    }

    /// Write connection `id`'s queue: `Some(ended)` where `ended` is an app's
    /// end now due, `None` when the peer is gone.
    fn flush_one(&mut self, id: u64) -> Option<bool> {
        let conn = self.conns.iter().find(|c| c.id == id)?;
        let fd = conn.stream.as_raw_fd();
        let mut state = guard(&self.shared.state);
        let out = state.out.get_mut(&id)?;
        while !out.bytes.is_empty() {
            let n =
                unsafe { libc::send(fd, out.bytes.as_ptr().cast(), out.bytes.len(), SEND_FLAGS) };
            if n > 0 {
                out.bytes.drain(..n as usize);
                continue;
            }
            let e = std::io::Error::last_os_error();
            match e.kind() {
                ErrorKind::WouldBlock => return Some(false),
                ErrorKind::Interrupted => continue,
                _ => return None,
            }
        }
        Some(out.end)
    }

    fn expire(&mut self) {
        let now = Instant::now();
        let due: Vec<u64> = (self.conns.iter())
            .filter(|c| c.deadline.is_some_and(|at| at <= now))
            .map(|c| c.id)
            .collect();
        for id in due {
            self.end(id, End::Timeout);
        }
    }

    /// Close connection `id` (both directions) and say so.
    fn end(&mut self, id: u64, end: End) {
        let Some(at) = self.conns.iter().position(|c| c.id == id) else {
            return;
        };
        let conn = self.conns.remove(at);
        let _ = conn.stream.shutdown(std::net::Shutdown::Both);
        drop(conn);
        guard(&self.shared.state).out.remove(&id);
        self.emit(event::END, id, end.word().as_bytes());
    }

    /// Closed: connections end without events, the socket file goes while
    /// it is this listener's, and the address passes on (D2).
    fn teardown(&mut self) {
        self.gone = true;
        for conn in self.conns.drain(..) {
            let _ = conn.stream.shutdown(std::net::Shutdown::Both);
        }
        guard(&self.shared.state).out.clear();
        if let (Some((listener, ino)), Some(r)) = (self.bound.take(), &self.resolved) {
            drop(listener);
            publish::remove(r, ino);
        }
        if self.joined {
            registry::leave(self.id);
        }
    }
}

fn poll_fd(fd: RawFd, events: i16) -> libc::pollfd {
    libc::pollfd {
        fd,
        events,
        revents: 0,
    }
}

/// Apple's per-socket `SIGPIPE` refusal (`SO_NOSIGPIPE`); Linux passes
/// `MSG_NOSIGNAL` on each send instead.
fn no_sigpipe(fd: RawFd) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    unsafe {
        let one: libc::c_int = 1;
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE,
            (&one as *const libc::c_int).cast(),
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    let _ = fd;
}

/// `text` as a JSON string.
fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
