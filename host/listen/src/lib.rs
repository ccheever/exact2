//! A local listener (LLP 1133.005): a `unix:` socket bound under D4's
//! rules, each connection read as lines within D3's bounds and written as
//! the app sends, and each address held by one listener of the process at a
//! time (D2). One thread per listener runs a `poll` loop over the socket,
//! its connections, the directory watch and a wake pipe.
//!
//! The caller hears events through a sink, which may refuse one ([`Taken`]):
//! the executor bounds what waits for the runner (D3). Sends arrive from
//! another thread ([`Listener::send`]) and are written by the loop.
//!
//! @ref LLP 1133.005 D1 (events, options) / D2 (the hold) / D3 (bounds) /
//! D4 (paths, `takeover`, the lock, the watch) / D13 (a separate artifact)
#![cfg(unix)]

pub mod abi;
pub mod bind;
mod hold;
pub mod loopback;
mod watch;

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Bytes a connection may hold unwritten (D3).
pub const SEND_BYTES: usize = 1 << 20;

/// What to listen on, and how (D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The address as the app asked: `unix:<path>`.
    pub address: String,
    /// The path, its existing ancestors resolved.
    pub path: PathBuf,
    /// Newest wins (D2, D4).
    pub takeover: bool,
    /// How long a connection has for its first complete line.
    pub first_line: Option<Duration>,
    /// The longest line.
    pub max_line: usize,
    /// Connections open at once.
    pub max_connections: usize,
}

/// One event for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    /// The address is bound: always the first.
    Listening,
    /// A peer connected.
    Connection(u64),
    /// One line from a connection, without its terminator.
    Line(u64, &'a [u8]),
    /// A connection ended: `peer`, `app`, `timeout` or `overflow`.
    End(u64, &'static str),
    /// Where the address stands, no mapper's: `waiting`, `displaced`,
    /// `holding` (held again) or `took over`.
    Standing(&'static str),
    /// The listener could not bind, or stopped working: its last event.
    Error(&'a bind::Failure),
}

/// What became of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    /// Queued for the runner.
    Queued,
    /// Nobody wants the listener any more: it closes.
    Gone,
    /// Past a bound (D3): a connection is closed at once, with no event; a
    /// line ends its connection `overflow`.
    Full,
}

/// The caller's end of the events.
pub type Sink = Box<dyn FnMut(Event) -> Taken + Send>;

/// Why a send was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The connection has ended: the send is dropped.
    Gone,
    /// The connection would hold more than [`SEND_BYTES`] unwritten.
    Quota,
}

/// One connection's sending side, shared with the senders.
#[derive(Default)]
struct Outgoing {
    bytes: Vec<u8>,
    ending: bool,
    received: u64,
    sent: u64,
}

struct Shared {
    out: Mutex<HashMap<u64, Outgoing>>,
    signal: Arc<hold::Signal>,
    wake: RawFd,
}

/// A listener: its loop runs until [`Listener::close`] or its error.
pub struct Listener {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
    wake_read: RawFd,
}

fn wake(fd: RawFd) {
    // SAFETY: a one-byte write to the pipe's nonblocking write end; a full
    // pipe already wakes the loop.
    unsafe { libc::write(fd, [1u8].as_ptr().cast(), 1) };
}

fn pipe() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0; 2];
    // SAFETY: `fds` holds the two descriptors pipe returns.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    for fd in fds {
        // SAFETY: descriptors pipe just made.
        unsafe {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK);
        }
    }
    Ok((fds[0], fds[1]))
}

impl Listener {
    /// Start listening as `config` says, telling `sink` what happens.
    pub fn open(config: Config, sink: Sink) -> io::Result<Listener> {
        let (read, write) = pipe()?;
        let signal = Arc::new(hold::Signal::new(move || wake(write)));
        let shared = Arc::new(Shared {
            out: Mutex::default(),
            signal,
            wake: write,
        });
        let looped = Loop {
            config,
            sink,
            shared: shared.clone(),
            wake: read,
            bound: None,
            ever_bound: false,
            hold: None,
            watch: None,
            conns: HashMap::new(),
            next: 1,
            stopped: false,
        };
        let thread = std::thread::Builder::new()
            .name("exact-listen".into())
            .spawn(move || looped.run())?;
        Ok(Listener {
            shared,
            thread: Some(thread),
            wake_read: read,
        })
    }

    /// Queue `line` and a `\n` on `connection`, and then end it if `end`.
    /// An empty `line` with `end` only ends it.
    pub fn send(&self, connection: u64, line: Option<&[u8]>, end: bool) -> Result<(), Refused> {
        let mut out = self.shared.out.lock().unwrap();
        let o = out.get_mut(&connection).ok_or(Refused::Gone)?;
        if o.ending {
            return Err(Refused::Gone);
        }
        if let Some(line) = line {
            if o.bytes.len() + line.len() + 1 > SEND_BYTES {
                return Err(Refused::Quota);
            }
            o.bytes.extend_from_slice(line);
            o.bytes.push(b'\n');
            o.sent += 1;
        }
        o.ending |= end;
        drop(out);
        wake(self.shared.wake);
        Ok(())
    }

    /// The bytes `connection` holds unwritten, if it is open.
    pub fn buffered(&self, connection: u64) -> Option<usize> {
        let out = self.shared.out.lock().unwrap();
        out.get(&connection).map(|o| o.bytes.len())
    }

    /// Each open connection: its id, the lines received and those sent.
    pub fn view(&self) -> Vec<(u64, u64, u64)> {
        let out = self.shared.out.lock().unwrap();
        let mut view: Vec<_> = out
            .iter()
            .map(|(id, o)| (*id, o.received, o.sent))
            .collect();
        view.sort_unstable();
        view
    }

    /// Stop: every connection closes without an event, the socket file is
    /// removed while it is still this listener's, and the next listener of
    /// the process on the address takes it.
    pub fn close(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.shared.signal.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop();
        // SAFETY: the pipe's ends are this listener's, its loop joined.
        unsafe {
            libc::close(self.wake_read);
            libc::close(self.shared.wake);
        }
    }
}

/// One connection, as the loop reads it.
struct Conn {
    stream: UnixStream,
    input: Vec<u8>,
    /// Until its first complete line, when `first_line` is set.
    deadline: Option<Instant>,
}

struct Loop {
    config: Config,
    sink: Sink,
    shared: Arc<Shared>,
    wake: RawFd,
    bound: Option<bind::Bound>,
    ever_bound: bool,
    hold: Option<hold::Hold>,
    watch: Option<watch::Watch>,
    conns: HashMap<u64, Conn>,
    next: u64,
    stopped: bool,
}

impl Loop {
    fn emit(&mut self, event: Event) -> Taken {
        let taken = (self.sink)(event);
        if taken == Taken::Gone {
            self.stopped = true;
        }
        taken
    }

    fn fail(&mut self, failure: bind::Failure) {
        self.emit(Event::Error(&failure));
        self.stopped = true;
    }

    fn run(mut self) {
        let signal = self.shared.signal.clone();
        let (hold, start) = hold::enter(&self.config.path, self.config.takeover, signal.clone());
        self.hold = Some(hold);
        match start {
            hold::Start::Wait => {
                self.emit(Event::Standing("waiting"));
            }
            hold::Start::Bind => self.bind(false),
            hold::Start::TakeOver => self.bind(true),
        }
        while !self.stopped && !signal.stop.load(Ordering::SeqCst) {
            self.turn();
        }
        self.teardown();
    }

    /// Bind as the process's holder (`over` the one it displaced): the
    /// first `listening`, or `holding` once it has listened before. A first
    /// bind that fails ends it.
    fn bind(&mut self, over: bool) {
        match bind::publish(&self.config.path, self.config.takeover, over) {
            Ok(bound) => {
                let took_over = bound.took_over;
                self.bound = Some(bound);
                if self.watch.is_none() {
                    let dir = self
                        .config
                        .path
                        .parent()
                        .unwrap_or(std::path::Path::new("/"));
                    self.watch = watch::Watch::new(dir).ok();
                }
                if self.ever_bound {
                    self.emit(Event::Standing("holding"));
                } else {
                    self.ever_bound = true;
                    self.emit(Event::Listening);
                }
                if took_over {
                    self.emit(Event::Standing("took over"));
                }
            }
            Err(failure) if !self.ever_bound => self.fail(failure),
            // Held again only when the path is free: otherwise it stays displaced.
            Err(_) => {}
        }
    }

    /// No new connections: a newer listener holds the address.
    fn displace(&mut self) {
        if self.bound.take().is_some() {
            self.emit(Event::Standing("displaced"));
        }
    }

    /// The directory changed: when this listener holds the address in the
    /// process, see whether its path is still its socket.
    fn look(&mut self) {
        if !self.hold.as_ref().is_some_and(hold::Hold::holding) {
            return;
        }
        let now = bind::identity(&self.config.path);
        match &self.bound {
            Some(bound) if now == Some(bound.identity) => {}
            // Removed under it: bound again where it can be.
            Some(_) if now.is_none() => {
                self.bound = None;
                if let Ok(bound) = bind::publish(&self.config.path, false, false) {
                    self.bound = Some(bound);
                } else {
                    self.emit(Event::Standing("displaced"));
                }
            }
            // Another process took it over.
            Some(_) => self.displace(),
            // That process let go: take it back while it is free.
            None if now.is_none() && self.ever_bound => self.bind(false),
            None => {}
        }
    }

    fn turn(&mut self) {
        let mut fds = vec![libc::pollfd {
            fd: self.wake,
            events: libc::POLLIN,
            revents: 0,
        }];
        let watched = self.watch.is_some();
        if let Some(watch) = &self.watch {
            fds.push(libc::pollfd {
                fd: watch.fd(),
                events: libc::POLLIN,
                revents: 0,
            });
        }
        let listening = self.bound.as_ref().map(|b| b.listener.as_raw_fd());
        if let Some(fd) = listening {
            fds.push(libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            });
        }
        let ids: Vec<u64> = self.conns.keys().copied().collect();
        {
            let out = self.shared.out.lock().unwrap();
            for id in &ids {
                let writes = out.get(id).is_some_and(|o| !o.bytes.is_empty() || o.ending);
                fds.push(libc::pollfd {
                    fd: self.conns[id].stream.as_raw_fd(),
                    events: libc::POLLIN | if writes { libc::POLLOUT } else { 0 },
                    revents: 0,
                });
            }
        }
        let now = Instant::now();
        let soonest = self.conns.values().filter_map(|c| c.deadline).min();
        let timeout = soonest.map_or(-1, |d| {
            d.saturating_duration_since(now)
                .as_millis()
                .min(i32::MAX as u128) as i32
                + 1
        });
        // SAFETY: `fds` is a live array of its own length.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                self.fail(bind::Failure::Network(format!("poll: {e}")));
            }
            return;
        }
        let mut at = 0;
        if fds[at].revents != 0 {
            let mut buf = [0u8; 64];
            // SAFETY: reading the pipe's nonblocking read end into the stack.
            while unsafe { libc::read(self.wake, buf.as_mut_ptr().cast(), buf.len()) } > 0 {}
        }
        at += 1;
        let signal = self.shared.signal.clone();
        if signal.displaced.swap(false, Ordering::SeqCst) {
            self.displace();
        }
        if signal.acquire.swap(false, Ordering::SeqCst) {
            self.bind(false);
        }
        if watched {
            if fds[at].revents != 0 {
                if let Some(watch) = &self.watch {
                    watch.drain();
                }
                self.look();
            }
            at += 1;
        }
        if let Some(fd) = listening {
            if fds[at].revents != 0
                && self.bound.as_ref().map(|b| b.listener.as_raw_fd()) == Some(fd)
            {
                self.accept();
            }
            at += 1;
        }
        for (i, id) in ids.iter().enumerate() {
            let revents = fds[at + i].revents;
            if self.stopped || !self.conns.contains_key(id) {
                continue;
            }
            if revents & libc::POLLOUT != 0 {
                self.flush(*id);
            }
            if revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                self.read(*id);
            }
        }
        let now = Instant::now();
        let late: Vec<u64> = (self.conns.iter())
            .filter(|(_, c)| c.deadline.is_some_and(|d| d <= now))
            .map(|(id, _)| *id)
            .collect();
        for id in late {
            self.end(id, "timeout");
        }
        // A send arrives through the wake pipe: write what is waiting.
        let ending: Vec<u64> = {
            let out = self.shared.out.lock().unwrap();
            (out.iter())
                .filter(|(_, o)| !o.bytes.is_empty() || o.ending)
                .map(|(id, _)| *id)
                .collect()
        };
        for id in ending {
            if self.conns.contains_key(&id) {
                self.flush(id);
            }
        }
    }

    fn accept(&mut self) {
        loop {
            let Some(bound) = &self.bound else { return };
            let stream = match bound.listener.accept() {
                Ok((stream, _)) => stream,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                // A connection that failed as it was accepted is no event.
                Err(e) if e.kind() == io::ErrorKind::ConnectionAborted => continue,
                Err(e) => return self.fail(bind::Failure::Network(format!("accept: {e}"))),
            };
            // Past `maxConnections`: closed at once, as a full backlog refuses.
            if self.conns.len() >= self.config.max_connections
                || stream.set_nonblocking(true).is_err()
            {
                continue;
            }
            let id = self.next;
            self.next += 1;
            match self.emit(Event::Connection(id)) {
                Taken::Queued => {}
                Taken::Full => continue,
                Taken::Gone => return,
            }
            let deadline = self.config.first_line.map(|d| Instant::now() + d);
            self.shared
                .out
                .lock()
                .unwrap()
                .insert(id, Outgoing::default());
            let input = Vec::new();
            self.conns.insert(
                id,
                Conn {
                    stream,
                    input,
                    deadline,
                },
            );
        }
    }

    /// Read what `id` has sent, delivering each complete line.
    fn read(&mut self, id: u64) {
        let mut buf = [0u8; 65536];
        loop {
            let Some(conn) = self.conns.get_mut(&id) else {
                return;
            };
            let n = match conn.stream.read(&mut buf) {
                Ok(0) => return self.end(id, "peer"),
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return self.end(id, "peer"),
            };
            conn.input.extend_from_slice(&buf[..n]);
            if !self.lines(id) {
                return;
            }
        }
    }

    /// Deliver `id`'s complete lines; false once it has ended.
    fn lines(&mut self, id: u64) -> bool {
        loop {
            let Some(conn) = self.conns.get_mut(&id) else {
                return false;
            };
            let Some(at) = conn.input.iter().position(|b| *b == b'\n') else {
                if conn.input.len() > self.config.max_line {
                    self.end(id, "overflow");
                    return false;
                }
                return true;
            };
            let mut line: Vec<u8> = conn.input.drain(..=at).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            conn.deadline = None;
            if line.len() > self.config.max_line {
                self.end(id, "overflow");
                return false;
            }
            match self.emit(Event::Line(id, &line)) {
                Taken::Queued => {
                    let mut out = self.shared.out.lock().unwrap();
                    if let Some(o) = out.get_mut(&id) {
                        o.received += 1;
                    }
                }
                Taken::Full => {
                    self.end(id, "overflow");
                    return false;
                }
                Taken::Gone => return false,
            }
        }
    }

    /// Write what `id` has waiting; once it is written, an `end()` ends it.
    fn flush(&mut self, id: u64) {
        let mut out = self.shared.out.lock().unwrap();
        let Some(o) = out.get_mut(&id) else { return };
        let Some(conn) = self.conns.get_mut(&id) else {
            return;
        };
        while !o.bytes.is_empty() {
            match conn.stream.write(&o.bytes) {
                Ok(0) => break,
                Ok(n) => {
                    o.bytes.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(_) => {
                    drop(out);
                    return self.end(id, "peer");
                }
            }
        }
        let done = o.ending && o.bytes.is_empty();
        drop(out);
        if done {
            self.end(id, "app");
        }
    }

    /// End `id` for `reason`: closed both ways, its record dropped, said.
    fn end(&mut self, id: u64, reason: &'static str) {
        let Some(conn) = self.conns.remove(&id) else {
            return;
        };
        let _ = conn.stream.shutdown(std::net::Shutdown::Both);
        drop(conn);
        self.shared.out.lock().unwrap().remove(&id);
        self.emit(Event::End(id, reason));
    }

    fn teardown(&mut self) {
        for (_, conn) in self.conns.drain() {
            let _ = conn.stream.shutdown(std::net::Shutdown::Both);
        }
        self.shared.out.lock().unwrap().clear();
        if let Some(bound) = self.bound.take() {
            bind::unpublish(&self.config.path, bound.identity);
        }
        self.watch = None;
        if let Some(hold) = self.hold.take() {
            hold.leave();
        }
    }
}

#[cfg(test)]
mod tests;
