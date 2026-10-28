//! A loopback redirect for `openAuthSession` (LLP 1133.005 D9, RFC 8252
//! §7.3): bound on `127.0.0.1:<port>` (or `[::1]`) before the browser
//! opens, it answers the one `GET` whose path is the callback's and whose
//! query carries `state` exactly once with a short page, and hands back the
//! full callback URL; anything else gets a 400 and the receiver stays open.
//! Each connection has 5 s to send its head, of at most 32 KiB; at most 8
//! are open at once; a reply that cannot be written within 5 s is abandoned.
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
    let listener = TcpListener::bind(SocketAddr::new(ip, callback.port)).map_err(|e| {
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
    while !stop.load(Ordering::SeqCst) {
        loop {
            match listener.accept() {
                Ok((stream, _))
                    if open.len() < OPEN_AT_ONCE && stream.set_nonblocking(true).is_ok() =>
                {
                    open.push(Pending {
                        stream,
                        head: Vec::new(),
                        since: Instant::now(),
                    })
                }
                // Past the bound: closed on accept.
                Ok(_) => {}
                Err(_) => break,
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
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != callback.path {
        return Err("This address answers only the sign-in's return.");
    }
    let states: Vec<String> = (query.split('&'))
        .filter_map(|pair| pair.split_once('=').or(Some((pair, ""))))
        .filter(|(name, _)| *name == "state")
        .map(|(_, value)| decode(value))
        .collect();
    if states.len() != 1 || states[0] != callback.state {
        return Err("This is not the sign-in this app is waiting for.");
    }
    Ok(target)
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => match text
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                Some(b) => {
                    out.push(b);
                    i += 2;
                }
                None => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
}
