//! The graceful close (LLP 1048.000 D10), off the threads that serve.
//!
//! A connection that has its answer closes in two steps: the server's half
//! at once, so the client reads the answer and the end of the stream, and
//! the socket itself when the client has closed too. Dropping it sooner,
//! with bytes of the request unread or still on their way, resets the
//! connection, and the client can find the reset where the answer's end
//! should be. So what the client still sends is read and dropped until it
//! closes (RFC 9112 §9.6, the staged close).
//!
//! That wait is the peer's to stretch, so no thread that serves does it.
//! The accept loop read a rejected request and closed it this way itself,
//! and a peer sending a byte every 100 ms held it — no health check, no
//! connection for a worker just freed — for as long as it kept sending; a
//! worker closing its last connection was held the same way while it
//! counted as free. One thread waits on all of them at once (`poll`)
//! instead, and nothing it does is seen by a client: the answer and the end
//! of the stream have gone before a socket reaches it.
//!
//! What it holds is bounded: each socket for [`HOLD`] from its hand-off,
//! however its peer paces what it sends; to [`MOST`] bytes read; and
//! [`HELD`] sockets at once (and as many handed off and not yet taken), the
//! oldest dropped for a new one. A socket dropped at a bound may be reset:
//! its peer had the answer, and the time to read it.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a closing socket is held for its peer to close.
const HOLD: Duration = Duration::from_secs(2);
/// The most read from one before it is dropped.
const MOST: usize = 64 << 10;
/// How many are held at once.
const HELD: usize = 256;

/// A socket whose answer and end of stream are sent.
struct Held {
    stream: TcpStream,
    until: Instant,
    read: usize,
}

/// Add `new` to `held`, the oldest first: at `most`, the oldest is dropped
/// for it.
fn admit(held: &mut VecDeque<Held>, new: Held, most: usize) {
    if held.len() >= most {
        held.pop_front();
    }
    held.push_back(new);
}

/// Read and drop what `held`'s peer has sent so far, never waiting for
/// more: false once the peer has closed, the socket failed, or it sent more
/// than [`MOST`].
fn absorb(held: &mut Held) -> bool {
    let mut sink = [0u8; 4096];
    loop {
        match held.stream.read(&mut sink) {
            Ok(0) => return false,
            Ok(n) => {
                held.read += n;
                if held.read > MOST {
                    return false;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => return true,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
}

/// What the hand-offs and the thread share.
struct Inbox {
    /// Handed off, not yet taken by the thread.
    arrived: VecDeque<Held>,
    /// A byte is in the wake socket: the thread's `poll` returns for it.
    woken: bool,
    /// Take what is held to its end, then end.
    finish: bool,
    /// The thread has ended: a socket handed off now is dropped.
    gone: bool,
}

/// Hands connections to the closing thread.
#[derive(Clone)]
pub(crate) struct Closer(Arc<(Mutex<Inbox>, UnixStream)>);

impl Closer {
    /// Close `stream`, whose answer is written: the server's half now, the
    /// rest by the closing thread. Never waits on the peer.
    pub(crate) fn close(&self, stream: TcpStream) {
        if stream.set_nonblocking(true).is_ok() {
            self.hand_off(stream);
        }
    }

    /// Answer a connection the server won't serve and close it, without
    /// reading its request or waiting on its peer: an answer may go before
    /// its request is whole, and what the peer sends after it is the closing
    /// thread's to read. `answer` goes in one write to a send buffer nothing
    /// has filled; a socket that doesn't take it whole is dropped.
    pub(crate) fn refuse(&self, stream: TcpStream, answer: &[u8]) {
        if stream.set_nonblocking(true).is_err() {
            return;
        }
        if matches!((&stream).write(answer), Ok(sent) if sent == answer.len()) {
            self.hand_off(stream);
        }
    }

    /// `stream` is nonblocking: end the server's half and give it away.
    fn hand_off(&self, stream: TcpStream) {
        let _ = stream.shutdown(Shutdown::Write);
        let (inbox, wake) = &*self.0;
        let mut inbox = inbox.lock().unwrap();
        if inbox.gone {
            return;
        }
        let held = Held {
            stream,
            until: Instant::now() + HOLD,
            read: 0,
        };
        admit(&mut inbox.arrived, held, HELD);
        wake_thread(&mut inbox, wake);
    }
}

/// Wake the thread from its `poll`: one byte, while none is already there.
fn wake_thread(inbox: &mut Inbox, mut wake: &UnixStream) {
    if !inbox.woken {
        inbox.woken = wake.write(&[0]).is_ok();
    }
}

/// The closing thread, for as long as its server runs.
pub(crate) struct Linger {
    closer: Closer,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Linger {
    pub(crate) fn start() -> std::io::Result<Linger> {
        let (wake, woken) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        woken.set_nonblocking(true)?;
        let inbox = Inbox {
            arrived: VecDeque::new(),
            woken: false,
            finish: false,
            gone: false,
        };
        let closer = Closer(Arc::new((Mutex::new(inbox), wake)));
        let shared = closer.0.clone();
        let thread = std::thread::Builder::new()
            .name("exact-render-close".into())
            .spawn(move || run(&shared.0, &woken))?;
        Ok(Linger {
            closer,
            thread: Some(thread),
        })
    }

    pub(crate) fn closer(&self) -> Closer {
        self.closer.clone()
    }
}

/// A drain: the thread takes what it holds to its end — each peer closes,
/// or its hold runs out — and ends, so every answer handed to it had its
/// graceful close before the server returns.
impl Drop for Linger {
    fn drop(&mut self) {
        let (inbox, wake) = &*self.closer.0;
        {
            let mut inbox = inbox.lock().unwrap();
            inbox.finish = true;
            wake_thread(&mut inbox, wake);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(inbox: &Mutex<Inbox>, mut woken: &UnixStream) {
    let mut held = VecDeque::new();
    let mut fds = Vec::new();
    loop {
        {
            let mut inbox = inbox.lock().unwrap();
            if inbox.woken {
                // The lock orders this with the write: one byte is there.
                let _ = woken.read(&mut [0u8; 8]);
                inbox.woken = false;
            }
            for new in inbox.arrived.drain(..) {
                admit(&mut held, new, HELD);
            }
            if inbox.finish && held.is_empty() {
                inbox.gone = true;
                return;
            }
        }
        // Until a peer sends or closes, a hand-off wakes it, or the oldest
        // socket's hold runs out (they are held in the order they came).
        let now = Instant::now();
        let wait = held.front().map_or(-1, |oldest| {
            let left = oldest.until.saturating_duration_since(now).as_millis();
            left as libc::c_int + 1
        });
        let listen = |fd| libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        fds.clear();
        fds.push(listen(woken.as_raw_fd()));
        fds.extend(held.iter().map(|held| listen(held.stream.as_raw_fd())));
        // SAFETY: `fds` is a live array of `fds.len()` entries, and each
        // names a socket this thread keeps open across the call.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, wait) };
        if ready < 0 {
            // Interrupted, or out of memory: the holds still run out.
            std::thread::sleep(Duration::from_millis(10));
        }
        let now = Instant::now();
        let mut at = 0;
        held.retain_mut(|held| {
            at += 1;
            (ready <= 0 || fds[at].revents == 0 || absorb(held)) && now < held.until
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A connection on loopback: the server's end, the client's.
    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        (server, client)
    }

    #[test]
    fn the_oldest_is_dropped_for_a_new_one_at_the_bound() {
        let mut held = VecDeque::new();
        let mut clients = Vec::new();
        for _ in 0..5 {
            let (stream, client) = pair();
            let until = Instant::now() + HOLD;
            let new = Held {
                stream,
                until,
                read: 0,
            };
            admit(&mut held, new, 3);
            clients.push(client);
        }
        // The three newest stay, oldest first.
        let kept: Vec<_> = held
            .iter()
            .map(|held| held.stream.peer_addr().unwrap())
            .collect();
        let newest: Vec<_> = clients[2..]
            .iter()
            .map(|client| client.local_addr().unwrap())
            .collect();
        assert_eq!(kept, newest);
        // The two before them are closed.
        for client in &mut clients[..2] {
            client.set_read_timeout(Some(BOUND)).unwrap();
            assert_eq!(client.read(&mut [0u8; 8]).unwrap(), 0);
        }
    }

    /// A hang bound, never a deadline.
    const BOUND: Duration = Duration::from_secs(60);

    #[test]
    fn what_a_peer_sends_is_read_and_dropped_until_it_closes_or_sends_too_much() {
        let hold = |stream: TcpStream| {
            stream.set_nonblocking(true).unwrap();
            let until = Instant::now() + HOLD;
            Held {
                stream,
                until,
                read: 0,
            }
        };
        // Absorb until `done`, as the thread does each time `poll` wakes it.
        let absorbed = |held: &mut Held, done: &dyn Fn(bool, usize) -> bool| {
            let until = Instant::now() + BOUND;
            loop {
                let open = absorb(held);
                if done(open, held.read) {
                    return open;
                }
                assert!(open, "let go after {} bytes", held.read);
                assert!(Instant::now() < until, "{} bytes in {BOUND:?}", held.read);
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        let (stream, mut client) = pair();
        let mut held = hold(stream);
        // Nothing sent yet: kept, without waiting for any.
        assert!(absorb(&mut held));
        client.write_all(&[b'a'; 1000]).unwrap();
        assert!(absorbed(&mut held, &|_, read| read == 1000));
        // The peer closes: let go.
        drop(client);
        assert!(!absorbed(&mut held, &|open, _| !open));
        // A peer that sends past the cap is let go while it is still open.
        let (stream, mut client) = pair();
        let mut held = hold(stream);
        let sending = std::thread::spawn(move || {
            let _ = client.write_all(&vec![b'a'; MOST + 1]);
            client
        });
        assert!(!absorbed(&mut held, &|open, _| !open));
        assert!(held.read > MOST, "{}", held.read);
        drop(sending.join().unwrap());
    }

    #[test]
    fn an_answer_is_whole_to_a_peer_still_sending_its_request() {
        let linger = Linger::start().unwrap();
        let mut clients = Vec::new();
        // Answered before any of its request is read, some of it already
        // here; and answered after its request was read, as a worker's is.
        for refused in [true, false] {
            let (server, mut client) = pair();
            if refused {
                client.write_all(b"GET / HTTP/1.1\r\n").unwrap();
                linger.closer().refuse(server, b"busy\n");
            } else {
                (&server).write_all(b"busy\n").unwrap();
                linger.closer().close(server);
            }
            clients.push(client);
        }
        for mut client in clients {
            client.set_read_timeout(Some(BOUND)).unwrap();
            // The server's half is closed; the client's still takes bytes.
            for _ in 0..4 {
                client.write_all(b"X-More: of the request\r\n").unwrap();
                std::thread::sleep(Duration::from_millis(5));
            }
            let mut answer = String::new();
            client.read_to_string(&mut answer).unwrap();
            assert_eq!(answer, "busy\n");
        }
        // Each peer has closed, so the thread holds nothing more: it ends.
        drop(linger);
    }

    #[test]
    fn a_socket_handed_off_after_the_thread_ended_is_dropped() {
        let linger = Linger::start().unwrap();
        let closer = linger.closer();
        drop(linger);
        let (server, mut client) = pair();
        closer.close(server);
        client.set_read_timeout(Some(BOUND)).unwrap();
        assert_eq!(client.read(&mut [0u8; 8]).unwrap(), 0);
        assert!(closer.0 .0.lock().unwrap().arrived.is_empty());
    }
}
