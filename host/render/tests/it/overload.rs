//! The server under overload (LLP 1048.000 D10): a connection it turns away
//! gets its 503 whole, and holds neither the accept loop nor a render
//! worker, however slowly its peer sends.
//!
//! These assert on what happens, never on how long it took: a wait here is
//! [`BOUND`], the hang bound, and a scenario a loaded machine reordered is
//! run again. The worker is held by a request that stops short, not by a
//! slow render: the test says when it is freed, so nothing races a render's
//! deadline.

use super::serve::{get, start, BOUND};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

fn connect(addr: SocketAddr) -> TcpStream {
    let stream =
        TcpStream::connect_timeout(&addr, BOUND).unwrap_or_else(|e| panic!("no connection: {e}"));
    stream.set_read_timeout(Some(BOUND)).unwrap();
    stream.set_write_timeout(Some(BOUND)).unwrap();
    stream
}

/// Status and body of an answer read to the end of its stream.
fn answer(bytes: &[u8]) -> (u16, String) {
    let text = String::from_utf8_lossy(bytes);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("no whole answer: {text:?}"));
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|status| status.parse().ok())
        .unwrap_or_else(|| panic!("no status: {head:?}"));
    (status, body.to_string())
}

/// A server's one worker, held: a request whose head stops short, which the
/// worker waits for (as long as it gives a head) while every other
/// connection finds the queue full.
struct Hold(TcpStream);

impl Hold {
    fn take(addr: SocketAddr) -> Hold {
        // The worker is up and free: until it is, every connection is
        // turned away, as this one would be.
        let until = Instant::now() + BOUND;
        while get(addr, "/.exact/health").0 != 200 {
            assert!(Instant::now() < until, "no free worker in {BOUND:?}");
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut stream = connect(addr);
        stream
            .write_all(b"GET /.exact/health HTTP/1.1\r\nHost: blog.test\r\n")
            .unwrap();
        Hold(stream)
    }

    /// The server has answered it, or closed it: it holds no worker. Its
    /// head took longer than a worker gives one, or another connection had
    /// the worker when it came.
    fn lost(&self) -> bool {
        use std::io::ErrorKind::{TimedOut, WouldBlock};
        let _ = self.0.set_read_timeout(Some(Duration::from_millis(20)));
        !matches!(self.0.peek(&mut [0u8]), Err(e) if matches!(e.kind(), WouldBlock | TimedOut))
    }

    /// Free the worker: the rest of the request. True when its answer is
    /// the health check's, so the worker was held from `take` to now.
    fn release(mut self) -> bool {
        let _ = self.0.set_read_timeout(Some(BOUND));
        let mut bytes = Vec::new();
        self.0.write_all(b"Connection: close\r\n\r\n").is_ok()
            && self.0.read_to_end(&mut bytes).is_ok()
            && answer(&bytes) == (200, "ok\n".to_string())
    }
}

/// A peer that never finishes its request: the start of a head, then one
/// more byte of it every 50 ms (well inside any idle timeout a read might
/// have) until it is dropped, or the server takes no more.
struct Dripper {
    stream: TcpStream,
    stop: Arc<AtomicBool>,
    refused: Arc<AtomicBool>,
    dripping: Option<JoinHandle<()>>,
}

impl Dripper {
    fn open(addr: SocketAddr) -> Dripper {
        let mut stream = connect(addr);
        stream
            .write_all(b"GET /post/7 HTTP/1.1\r\nHost: blog.test\r\nX-Slow: ")
            .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let refused = Arc::new(AtomicBool::new(false));
        let mut writer = stream.try_clone().unwrap();
        let (stopped, failed) = (stop.clone(), refused.clone());
        let dripping = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                if writer.write_all(b"a").is_err() {
                    failed.store(true, Ordering::SeqCst);
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        Dripper {
            stream,
            stop,
            refused,
            dripping: Some(dripping),
        }
    }

    /// What the server answers it, read to the end of the stream while its
    /// request is still arriving.
    fn answer(&mut self) -> (u16, String) {
        let mut bytes = Vec::new();
        if let Err(e) = self.stream.read_to_end(&mut bytes) {
            panic!(
                "no whole answer to a dripping request within {BOUND:?}: {e} after {:?}",
                String::from_utf8_lossy(&bytes)
            );
        }
        answer(&bytes)
    }

    /// The server has let the connection go: a write to it failed.
    fn refused(&self) -> bool {
        self.refused.load(Ordering::SeqCst)
    }
}

impl Drop for Dripper {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(dripping) = self.dripping.take() {
            let _ = dripping.join();
        }
    }
}

#[test]
fn a_turned_away_peer_that_drips_its_request_holds_nothing() {
    for attempt in 1.. {
        // One render at a time, and nothing may wait for it.
        let served = start("drip", 1, 0, 300);
        let addr = served.addr;
        let hold = Hold::take(addr);
        // A peer that drips its request, while the worker is held…
        let mut dripper = Dripper::open(addr);
        let answered = dripper.answer();
        // …and the next connection, while it drips.
        let (next, _, _) = get(addr, "/.exact/health");
        if !hold.release() {
            // A loaded machine took longer over this than a worker gives a
            // request's head, so the worker wasn't held throughout: the
            // scenario again, on a new server.
            assert!(attempt < 5, "the worker was never held throughout");
            continue;
        }
        // Turned away with its request unfinished, the dripper had its 503
        // whole and the end of the stream; and the accept loop, not held by
        // it, answered the connection after it.
        assert_eq!(answered, (503, "busy\n".to_string()));
        assert_eq!(next, 503);
        // The freed worker is reached at once, past the dripper.
        let (status, _, body) = get(addr, "/.exact/health");
        assert_eq!((status, body.as_str()), (200, "ok\n"));
        assert_eq!(get(addr, "/post/7").0, 200);
        // The server lets the dripper's connection go in a bounded time,
        // though its peer never stops sending.
        let until = Instant::now() + BOUND;
        while !dripper.refused() {
            assert!(
                Instant::now() < until,
                "the server still takes a turned-away peer's bytes after {BOUND:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        return;
    }
}

/// One request sent in two parts, `pause` apart: its answer, read to a clean
/// end of stream. A write the server refuses, or a reset in place of the
/// end, fails the test.
fn in_two_parts(addr: SocketAddr, pause: Duration) -> (u16, String) {
    let mut stream = connect(addr);
    stream
        .write_all(b"GET /.exact/health HTTP/1.1\r\nHost: blog.test\r\n")
        .unwrap_or_else(|e| panic!("the request's first part: {e}"));
    std::thread::sleep(pause);
    stream
        .write_all(b"Connection: close\r\n\r\n")
        .unwrap_or_else(|e| panic!("the request's second part: {e}"));
    let mut bytes = Vec::new();
    if let Err(e) = stream.read_to_end(&mut bytes) {
        panic!(
            "the answer ended in {:?} after {:?}",
            e.kind(),
            String::from_utf8_lossy(&bytes)
        );
    }
    answer(&bytes)
}

#[test]
fn a_turned_away_answer_is_whole_while_its_request_still_arrives() {
    const CLIENTS: usize = 4;
    const EACH: usize = 25;
    let served = start("reset", 1, 0, 300);
    let addr = served.addr;
    // The worker is held until every client has been turned away often
    // enough: taken again whenever the hold is lost.
    let done = Arc::new(AtomicBool::new(false));
    let holder = {
        let done = done.clone();
        std::thread::spawn(move || {
            while !done.load(Ordering::SeqCst) {
                let hold = Hold::take(addr);
                while !done.load(Ordering::SeqCst) && !hold.lost() {}
            }
        })
    };
    let clients: Vec<_> = (0..CLIENTS)
        .map(|client| {
            std::thread::spawn(move || {
                let until = Instant::now() + BOUND;
                let mut turned_away = 0;
                for round in client.. {
                    // The rest of the request arrives with the 503, just
                    // after it, or well after it.
                    let pause = Duration::from_millis(round as u64 % 4 * 5);
                    match in_two_parts(addr, pause) {
                        (503, body) => {
                            assert_eq!(body, "busy\n");
                            turned_away += 1;
                        }
                        // The worker was free: not yet held, or its hold lost.
                        (200, body) => assert_eq!(body, "ok\n"),
                        (status, body) => panic!("{status}: {body}"),
                    }
                    if turned_away == EACH {
                        break;
                    }
                    assert!(
                        Instant::now() < until,
                        "{turned_away} of {EACH} turned away in {BOUND:?}"
                    );
                }
            })
        })
        .collect();
    let ended: Vec<_> = clients.into_iter().map(JoinHandle::join).collect();
    done.store(true, Ordering::SeqCst);
    let held = holder.join();
    for client in ended {
        if let Err(panic) = client {
            std::panic::resume_unwind(panic);
        }
    }
    held.unwrap();
}
