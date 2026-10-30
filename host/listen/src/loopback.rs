//! A loopback redirect for `openAuthSession` (LLP 1133.005 D9, RFC 8252
//! §7.3): bound on `127.0.0.1:<port>` (or `[::1]`) before the browser
//! opens, it answers the one `GET` whose path is the callback's and whose
//! query carries `state` exactly once, and exactly one `code` or `error`
//! (RFC 6749 §4.1.2, §4.1.2.1), with a short page, and hands back the full
//! callback URL; anything else gets a 400 and the receiver stays open.
//! Each connection has 5 s to send its head, of at most 32 KiB; at most 8
//! are open at once, and while they are, new ones wait in the backlog; a
//! reply that cannot be written within 5 s is abandoned.
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const HEAD_BYTES: usize = 32 << 10;
const HEAD_TIME: Duration = Duration::from_secs(5);
const OPEN_AT_ONCE: usize = 8;

/// What the receiver answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    /// `127.0.0.1` or `[::1]`, as the callback spells it.
    pub host: String,
    /// The port the callback names.
    pub port: u16,
    /// The callback's path.
    pub path: String,
    /// The `state` the request carries.
    pub state: String,
    /// The app's name, for the page.
    pub app: String,
}

/// Why the receiver could not start.
#[derive(Debug)]
pub enum Failed {
    /// The port is in use.
    PortInUse,
    /// Anything else.
    Other(io::Error),
}

/// A running receiver: dropping it stops it and frees the port.
pub struct Receiver {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Bind and start answering; `done` is called once, from the receiver's
/// thread, with the full callback URL.
pub fn open(callback: Callback, done: Box<dyn FnOnce(String) + Send>) -> Result<Receiver, Failed> {
    let ip = if callback.host == "[::1]" {
        Ipv6Addr::LOCALHOST.into()
    } else {
        Ipv4Addr::LOCALHOST.into()
    };
    let listener = bind(SocketAddr::new(ip, callback.port)).map_err(|e| {
        if e.kind() == io::ErrorKind::AddrInUse {
            Failed::PortInUse
        } else {
            Failed::Other(e)
        }
    })?;
    listener.set_nonblocking(true).map_err(Failed::Other)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let thread = std::thread::Builder::new()
        .name("exact-auth-loopback".into())
        .spawn(move || serve(listener, callback, stopping, done))
        .map_err(Failed::Other)?;
    Ok(Receiver {
        stop,
        thread: Some(thread),
    })
}

/// Bind `address`, refusing a port another process listens on through
/// the wildcard address. `SO_REUSEADDR` stays, so a port the last receiver
/// left in TIME_WAIT (it closes first) binds again at once; with it alone,
/// macOS would also bind `127.0.0.1:P` beside a listener on `0.0.0.0:P` or
/// `[::]:P`, so a probe binds the wildcard first, without listening: it
/// fails for such a holder, with or without its `SO_REUSEADDR`, and not for
/// TIME_WAIT. `socket2` sets `SO_NOSIGPIPE` on Apple, which accepted
/// sockets inherit: this runs inside an app that does not ignore SIGPIPE.
fn bind(address: SocketAddr) -> io::Result<TcpListener> {
    use socket2::{Domain, Socket, Type};
    let wildcard: SocketAddr = match address {
        SocketAddr::V4(a) => (Ipv4Addr::UNSPECIFIED, a.port()).into(),
        SocketAddr::V6(a) => (Ipv6Addr::UNSPECIFIED, a.port()).into(),
    };
    let probe = Socket::new(Domain::for_address(wildcard), Type::STREAM, None)?;
    probe.set_reuse_address(true)?;
    if wildcard.is_ipv6() {
        probe.set_only_v6(false)?;
    }
    probe.bind(&wildcard.into())?;
    drop(probe);
    let socket = Socket::new(Domain::for_address(address), Type::STREAM, None)?;
    socket.set_reuse_address(true)?;
    socket.bind(&address.into())?;
    socket.listen(128)?; // std's backlog
    Ok(socket.into())
}

/// One connection, reading its head.
struct Pending {
    stream: TcpStream,
    head: Vec<u8>,
    since: Instant,
}

fn serve(
    listener: TcpListener,
    callback: Callback,
    stop: Arc<AtomicBool>,
    done: Box<dyn FnOnce(String) + Send>,
) {
    let mut open: Vec<Pending> = Vec::new();
    let mut done = Some(done);
    // After a failed `accept` (out of descriptors), when to try again.
    let mut paused: Option<Instant> = None;
    while !stop.load(Ordering::SeqCst) {
        paused = paused.filter(|until| *until > Instant::now());
        // While every place is taken nothing is accepted, so a new
        // connection waits in the backlog: idle ones cannot shut the
        // browser's callback out, only delay it until their 5 s pass.
        while open.len() < OPEN_AT_ONCE && paused.is_none() {
            match listener.accept() {
                Ok((stream, _)) if stream.set_nonblocking(true).is_ok() => open.push(Pending {
                    stream,
                    head: Vec::new(),
                    since: Instant::now(),
                }),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                // A peer that left before it was taken: the next one.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::ConnectionAborted
                    ) => {}
                Err(_) => paused = Some(Instant::now() + Duration::from_millis(100)),
            }
        }
        let mut i = 0;
        while i < open.len() {
            match step(&mut open[i], &callback) {
                Step::Waiting if open[i].since.elapsed() < HEAD_TIME => i += 1,
                Step::Waiting => drop(open.swap_remove(i)),
                Step::Answered(None) => drop(open.swap_remove(i)),
                Step::Answered(Some(url)) => {
                    if let Some(done) = done.take() {
                        done(url);
                    }
                    return;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

enum Step {
    Waiting,
    /// Answered: with the callback URL when it was the callback.
    Answered(Option<String>),
}

fn step(p: &mut Pending, callback: &Callback) -> Step {
    let mut buf = [0u8; 4096];
    loop {
        match p.stream.read(&mut buf) {
            Ok(0) => return Step::Answered(None),
            Ok(n) => p.head.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Step::Answered(None),
        }
        if p.head.len() > HEAD_BYTES {
            reply(&mut p.stream, 431, "The request's head is too large.", None);
            return Step::Answered(None);
        }
    }
    let Some(end) = p.head.windows(4).position(|w| w == b"\r\n\r\n") else {
        if p.head.len() > HEAD_BYTES {
            reply(&mut p.stream, 431, "The request's head is too large.", None);
            return Step::Answered(None);
        }
        return Step::Waiting;
    };
    let head = String::from_utf8_lossy(&p.head[..end]).into_owned();
    match check(&head, callback) {
        Ok(target) => {
            reply(&mut p.stream, 200, "", Some(&callback.app));
            let url = format!("http://{}:{}{}", callback.host, callback.port, target);
            Step::Answered(Some(url))
        }
        Err(why) => {
            reply(&mut p.stream, 400, why, None);
            Step::Answered(None)
        }
    }
}

/// The request target when the head is the callback, or a plain sentence.
fn check<'a>(head: &'a str, callback: &Callback) -> Result<&'a str, &'static str> {
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    if method != "GET" {
        return Err("This address answers only the sign-in's return.");
    }
    let want = format!("{}:{}", callback.host, callback.port);
    let host = lines
        .filter_map(|l| l.split_once(':'))
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim())
        .collect::<Vec<_>>();
    if host.as_slice() != [want.as_str()] {
        return Err("This request is not for this address.");
    }
    // A request target carries no fragment (RFC 9112 §3.2), and a browser
    // sends none. Drop one as the runner's URL parse does, so both read the
    // same query.
    let target = target.split_once('#').map_or(target, |(before, _)| before);
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != callback.path {
        return Err("This address answers only the sign-in's return.");
    }
    // The runner's rules (`exact_runner::auth::accept` and
    // `one_code_or_error`): nonempty `&`-separated pairs, name and value
    // percent-decoded.
    let values = |want: &str| -> Vec<String> {
        (query.split('&').filter(|p| !p.is_empty()))
            .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
            .filter(|(name, _)| decode(name) == want)
            .map(|(_, value)| decode(value))
            .collect()
    };
    let states = values("state");
    if states.len() != 1 || !same(&states[0], &callback.state) {
        return Err("This is not the sign-in this app is waiting for.");
    }
    match (values("code").as_slice(), values("error").as_slice()) {
        ([one], []) | ([], [one]) if !one.is_empty() => Ok(target),
        _ => Err("This return carries no single code or error."),
    }
}

/// `exact_runner::auth`'s decoding: `%` and two hex digits as a byte, `+`
/// a space, anything else as written.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| Some(hex(*bytes.get(i + 1)?)? << 4 | hex(*bytes.get(i + 2)?)?))
            .flatten();
        match (bytes[i], escaped) {
            (_, Some(b)) => {
                out.push(b);
                i += 2;
            }
            (b'+', _) => out.push(b' '),
            (b, _) => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `a` and `b` are equal, in time that depends on their lengths only.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Write the reply within 5 s, or abandon it with its connection.
fn reply(stream: &mut TcpStream, status: u16, sentence: &str, app: Option<&str>) {
    let (reason, kind, body) = match app {
        Some(app) => {
            let app = escape(app);
            let page = format!(
                "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Return to {app}</title>\
                 <style>body{{font:16px system-ui,sans-serif;margin:4em auto;max-width:30em;padding:0 1em}}</style>\
                 <h1>Return to {app}</h1><p>You can close this tab.</p></html>"
            );
            ("OK", "text/html; charset=utf-8", page)
        }
        None => {
            let reason = if status == 431 {
                "Request Header Fields Too Large"
            } else {
                "Bad Request"
            };
            (reason, "text/plain; charset=utf-8", format!("{sentence}\n"))
        }
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    let bytes = [head.as_bytes(), body.as_bytes()].concat();
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_write_timeout(Some(HEAD_TIME));
    let _ = stream.write_all(&bytes);
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn ask(port: u16, request: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    #[ignore = "async lane: binds a real loopback port"]
    fn the_one_callback_is_answered_and_everything_else_refused() {
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        let callback = Callback {
            host: "127.0.0.1".into(),
            port,
            path: "/auth/callback".into(),
            state: "s 1".into(),
            app: "T3 <Code>".into(),
        };
        let (tx, rx) = mpsc::channel();
        let receiver = open(callback.clone(), Box::new(move |url| tx.send(url).unwrap())).unwrap();
        assert!(matches!(
            open(callback, Box::new(|_| {})),
            Err(Failed::PortInUse)
        ));
        let host = format!("Host: 127.0.0.1:{port}");
        for bad in [
            format!("GET /other?state=s+1 HTTP/1.1\r\n{host}\r\n\r\n"),
            format!("POST /auth/callback?state=s+1 HTTP/1.1\r\n{host}\r\n\r\n"),
            "GET /auth/callback?state=s+1 HTTP/1.1\r\nHost: evil.test\r\n\r\n".to_string(),
            format!("GET /auth/callback?state=s+2 HTTP/1.1\r\n{host}\r\n\r\n"),
            format!("GET /auth/callback?state=s+1&state=s+1 HTTP/1.1\r\n{host}\r\n\r\n"),
            // The right `state`, but no single code or error: the receiver
            // stays open for the real return.
            format!("GET /auth/callback?state=s+1 HTTP/1.1\r\n{host}\r\n\r\n"),
            format!("GET /auth/callback?code=&state=s+1 HTTP/1.1\r\n{host}\r\n\r\n"),
            format!("GET /auth/callback?code=c&error=e&state=s+1 HTTP/1.1\r\n{host}\r\n\r\n"),
        ] {
            let answer = ask(port, &bad);
            assert!(answer.starts_with("HTTP/1.1 400"), "{bad}: {answer}");
        }
        let answer = ask(
            port,
            &format!("GET /auth/callback?code=c&state=s%201 HTTP/1.1\r\n{host}\r\n\r\n"),
        );
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(answer.contains("Return to T3 &lt;Code&gt;"));
        assert!(answer.contains("Content-Security-Policy: default-src 'none'"));
        assert!(!answer.contains("code=c"), "the page never shows the code");
        let url = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            url,
            format!("http://127.0.0.1:{port}/auth/callback?code=c&state=s%201")
        );
        drop(receiver);
        assert!(
            TcpListener::bind(("127.0.0.1", port)).is_ok(),
            "the port is freed"
        );
    }

    fn callback(port: u16) -> Callback {
        Callback {
            host: "127.0.0.1".into(),
            port,
            path: "/cb".into(),
            state: "s A".into(),
            app: "App".into(),
        }
    }

    #[test]
    #[ignore = "async lane: binds a real loopback port"]
    fn a_port_held_on_the_wildcard_address_is_in_use() {
        for wildcard in ["0.0.0.0:0", "[::]:0"] {
            // With `SO_REUSEADDR` (std's) and without.
            for reuse in [true, false] {
                let address: SocketAddr = wildcard.parse().unwrap();
                let held = socket2::Socket::new(
                    socket2::Domain::for_address(address),
                    socket2::Type::STREAM,
                    None,
                )
                .unwrap();
                held.set_reuse_address(reuse).unwrap();
                held.bind(&address.into()).unwrap();
                held.listen(4).unwrap();
                let port = held.local_addr().unwrap().as_socket().unwrap().port();
                assert!(
                    matches!(
                        open(callback(port), Box::new(|_| {})),
                        Err(Failed::PortInUse)
                    ),
                    "{wildcard} reuse={reuse}"
                );
            }
        }
    }

    #[test]
    #[ignore = "async lane: binds a real loopback port"]
    fn a_receiver_opens_again_on_its_port_right_after_answering() {
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        for round in 0..2 {
            let (tx, rx) = mpsc::channel();
            let receiver = open(callback(port), Box::new(move |url| tx.send(url).unwrap()))
                .unwrap_or_else(|e| panic!("round {round}: {e:?}"));
            // The receiver answers and closes first, leaving TIME_WAIT behind.
            let answer = ask(
                port,
                &format!("GET /cb?code=c&state=s+A HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
            );
            assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
            assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
            drop(receiver);
        }
    }

    /// The receiver runs inside an app that does not ignore SIGPIPE, so a
    /// write to a connection its peer reset must fail, not signal: the
    /// listener, and each connection accepted from it, carry `SO_NOSIGPIPE`.
    #[cfg(target_vendor = "apple")]
    #[test]
    #[ignore = "async lane: binds a real loopback port"]
    fn the_listener_and_what_it_accepts_never_raise_sigpipe() {
        use std::os::fd::AsRawFd;
        let nosigpipe = |fd: i32| {
            let (mut on, mut len) = (0i32, std::mem::size_of::<i32>() as libc::socklen_t);
            // SAFETY: `fd` is open; `on` and `len` are this frame's.
            let r = unsafe {
                libc::getsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_NOSIGPIPE,
                    (&mut on as *mut i32).cast(),
                    &mut len,
                )
            };
            assert_eq!(r, 0);
            on != 0
        };
        let listener = bind("127.0.0.1:0".parse().unwrap()).unwrap();
        assert!(nosigpipe(listener.as_raw_fd()), "the listener");
        let _peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (accepted, _) = listener.accept().unwrap();
        assert!(nosigpipe(accepted.as_raw_fd()), "an accepted connection");
    }

    #[test]
    #[ignore = "async lane: binds a real loopback port"]
    fn a_peer_that_resets_mid_reply_does_not_kill_the_process() {
        // The app the receiver runs in does not ignore SIGPIPE; the test
        // harness does, so it is restored here.
        // SAFETY: setting a signal's disposition to its default.
        unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        let (tx, rx) = mpsc::channel();
        let _receiver = open(callback(port), Box::new(move |url| tx.send(url).unwrap())).unwrap();
        let head = format!("GET /cb?state=wrong HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
        for _ in 0..50 {
            let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.write_all(head.as_bytes()).unwrap();
            socket2::SockRef::from(&s)
                .set_linger(Some(Duration::ZERO))
                .unwrap();
            drop(s);
        }
        std::thread::sleep(Duration::from_millis(200));
        let answer = ask(
            port,
            &format!("GET /cb?code=c&state=s+A HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
        );
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    #[ignore = "async lane: binds a real loopback port"]
    fn idle_connections_in_every_place_delay_the_callback_but_do_not_shut_it_out() {
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        let (tx, rx) = mpsc::channel();
        let _receiver = open(callback(port), Box::new(move |url| tx.send(url).unwrap())).unwrap();
        let idle: Vec<TcpStream> = (0..OPEN_AT_ONCE)
            .map(|_| TcpStream::connect(("127.0.0.1", port)).unwrap())
            .collect();
        std::thread::sleep(Duration::from_millis(100));
        // The runner's state rule: a name is decoded too, a trailing escape
        // decodes.
        let answer = ask(
            port,
            &format!("GET /cb?st%61te=s+%41&c%6Fde=c HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
        );
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
        drop(idle);
    }

    /// Two sign-ins wait at once on their own ports and complete
    /// independently, as the reference's do ("allows two accounts to
    /// complete independently"); a stray return to one ends neither.
    #[test]
    #[ignore = "async lane: binds a real loopback port"]
    fn two_receivers_on_two_ports_complete_independently() {
        let ports: Vec<u16> = (0..2)
            .map(|_| TcpListener::bind("127.0.0.1:0").unwrap())
            .collect::<Vec<_>>()
            .iter()
            .map(|l| l.local_addr().unwrap().port())
            .collect();
        let (tx, rx) = mpsc::channel();
        let receivers: Vec<Receiver> = ports
            .iter()
            .map(|&port| {
                let tx = tx.clone();
                open(callback(port), Box::new(move |url| tx.send(url).unwrap())).unwrap()
            })
            .collect();
        let get = |port: u16, query: &str| {
            ask(
                port,
                &format!("GET /cb?{query} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
            )
        };
        assert!(get(ports[0], "state=s+A").starts_with("HTTP/1.1 400"));
        assert!(get(ports[1], "state=s+A&error=access_denied").starts_with("HTTP/1.1 200"));
        assert!(get(ports[0], "state=s+A&code=a").starts_with("HTTP/1.1 200"));
        let mut got: Vec<String> = (0..2)
            .map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect();
        got.sort();
        let mut want = vec![
            format!("http://127.0.0.1:{}/cb?state=s+A&code=a", ports[0]),
            format!(
                "http://127.0.0.1:{}/cb?state=s+A&error=access_denied",
                ports[1]
            ),
        ];
        want.sort();
        assert_eq!(got, want);
        drop(receivers);
    }

    /// The runner drops a fragment before it reads the query
    /// (`exact_runner::auth::one_code_or_error`), so the receiver does too:
    /// a `code` after `#` is none, and what follows `#` is not passed on.
    #[test]
    fn a_fragment_is_dropped_as_the_runner_drops_it() {
        let cb = callback(4333);
        let head = |target: &str| format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:4333");
        assert_eq!(
            check(&head("/cb?state=s+A&#&code=c"), &cb),
            Err("This return carries no single code or error.")
        );
        assert_eq!(
            check(&head("/cb#?state=s+A&code=c"), &cb),
            Err("This is not the sign-in this app is waiting for.")
        );
        assert_eq!(
            check(&head("/cb?state=s+A&code=c#x"), &cb),
            Ok("/cb?state=s+A&code=c")
        );
    }

    #[test]
    fn the_state_rule_is_the_runner_s() {
        assert_eq!(decode("a%41"), "aA");
        assert_eq!(decode("%+4"), "% 4");
        assert_eq!(decode("%4"), "%4");
        assert!(same("s A", "s A") && !same("s A", "s B") && !same("s", "s A"));
    }
}
