//! The socket transports against a local WebSocket peer (LLP 1069.004 slice
//! 3): upgrade, text frames (fragmented and extended-length), a ping, the
//! closing handshake, an over-limit frame, a binary frame, a refused
//! handshake, a connection dropped without a close, and an abort.
use super::*;
use crate::stdlib::abort::AbortController;
use std::sync::mpsc::{channel, Receiver, Sender};

/// A server frame (never masked).
fn frame(fin: bool, opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![if fin { 0x80 } else { 0 } | opcode];
    match payload.len() {
        n if n < 126 => out.push(n as u8),
        n if n < 65536 => {
            out.push(126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(payload);
    out
}

/// One client frame, unmasked: (opcode, payload), or `None` at the end.
fn client_frame(s: &mut TcpStream) -> Option<(u8, Vec<u8>)> {
    let mut head = [0u8; 2];
    s.read_exact(&mut head).ok()?;
    assert!(head[1] & 0x80 != 0, "a client frame is masked");
    let len = (head[1] & 0x7f) as usize;
    let mut mask = [0u8; 4];
    s.read_exact(&mut mask).ok()?;
    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload).ok()?;
    for (i, b) in payload.iter_mut().enumerate() {
        *b ^= mask[i % 4];
    }
    Some((head[0] & 0x0f, payload))
}

/// A local peer; what it saw from the client arrives on the receiver.
pub(crate) fn peer() -> (u16, Receiver<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (saw, seen) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let (stream, saw) = (stream.unwrap(), saw.clone());
            std::thread::spawn(move || serve(stream, saw));
        }
    });
    (port, seen)
}

fn serve(mut s: TcpStream, saw: Sender<String>) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if s.read(&mut byte).unwrap_or(0) == 0 {
            return;
        }
        head.push(byte[0]);
    }
    let head = String::from_utf8(head).unwrap();
    let path = head.split(' ').nth(1).unwrap_or("").to_string();
    if path == "/refuse" {
        let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return;
    }
    let key = head
        .lines()
        .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
        .unwrap()
        .trim();
    let accept = crate::stdlib::websocket::accept_key(key);
    let _ = write!(
        s,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    let mut out = Vec::new();
    match path.as_str() {
        "/three" => {
            out.extend(frame(true, 1, b"one"));
            out.extend(frame(false, 1, b"t"));
            out.extend(frame(true, 9, b"are you there"));
            out.extend(frame(true, 0, b"wo"));
            out.extend(frame(true, 1, &[b'3'; 300]));
            let mut close = 1000u16.to_be_bytes().to_vec();
            close.extend_from_slice(b"bye");
            out.extend(frame(true, 8, &close));
        }
        "/big" => out.extend(frame(true, 1, &[b'x'; 2000])),
        "/binary" => out.extend(frame(true, 2, &[1, 2, 3])),
        "/drop" => out.extend(frame(true, 1, b"x")),
        _ => out.extend(frame(true, 1, b"held")),
    }
    let _ = s.write_all(&out);
    if path == "/drop" {
        let _ = s.shutdown(Shutdown::Both);
        return;
    }
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    while let Some((opcode, payload)) = client_frame(&mut s) {
        let text = String::from_utf8_lossy(&payload).into_owned();
        let _ = saw.send(match opcode {
            0xA => format!("{path} pong {text}"),
            0x8 if payload.len() >= 2 => format!(
                "{path} close {}",
                u16::from_be_bytes([payload[0], payload[1]])
            ),
            other => format!("{path} opcode {other}"),
        });
    }
    let _ = saw.send(format!("{path} gone"));
}

fn open_on(
    transport: &dyn SocketTransport,
    port: u16,
    path: &str,
    signal: &AbortSignal,
) -> Result<Box<dyn MessageSource>, HostError> {
    let url = url::Url::parse(&format!("ws://127.0.0.1:{port}{path}")).unwrap();
    transport.connect(&url, 1024, signal)
}

fn text(s: &str) -> Incoming {
    Incoming::Text(s.into())
}

fn wait(seen: &Receiver<String>) -> String {
    seen.recv_timeout(Duration::from_secs(5))
        .expect("the peer reports")
}

/// The whole conversation, on whichever transport: shared by the Rust
/// transport's test and the Darwin one's.
pub(crate) fn conversation(transport: &dyn SocketTransport) {
    let (port, seen) = peer();
    let none = AbortSignal::default();
    let mut s = open_on(transport, port, "/three", &none).unwrap();
    assert_eq!(s.next().unwrap(), text("one"));
    assert_eq!(
        s.next().unwrap(),
        text("two"),
        "fragments join; a ping between"
    );
    assert_eq!(
        s.next().unwrap(),
        text(&"3".repeat(300)),
        "an extended length"
    );
    assert_eq!(
        s.next().unwrap(),
        Incoming::Closed {
            code: 1000,
            reason: "bye".into()
        }
    );
    assert_eq!(wait(&seen), "/three pong are you there");
    assert_eq!(
        wait(&seen),
        "/three close 1000",
        "the closing handshake is answered"
    );
    drop(s);

    let mut s = open_on(transport, port, "/big", &none).unwrap();
    assert_eq!(s.next().unwrap(), Incoming::TooLarge);
    drop(s);
    let mut s = open_on(transport, port, "/binary", &none).unwrap();
    assert!(matches!(s.next().unwrap(), Incoming::Binary(_)));
    drop(s);
    let mut s = open_on(transport, port, "/drop", &none).unwrap();
    assert_eq!(s.next().unwrap(), text("x"));
    assert!(matches!(
        s.next().unwrap(),
        Incoming::Closed { code: 1006, .. }
    ));
    drop(s);
    let refused = open_on(transport, port, "/refuse", &none).err().unwrap();
    assert!(refused.to_string().contains("did not open"), "{refused}");

    // Aborting ends a blocked read, and the peer sees the connection go.
    let abort = AbortController::new();
    let mut s = open_on(transport, port, "/hold", &abort.signal()).unwrap();
    assert_eq!(s.next().unwrap(), text("held"));
    let aborter = abort.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        aborter.abort();
    });
    assert!(s.next().is_err(), "an abort is an error, not a close");
    drop(s);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match seen.recv_timeout(left).expect("the peer saw the socket go") {
            gone if gone == "/hold gone" => break,
            _ => {}
        }
    }
}

#[test]
fn the_rust_transport_holds_the_whole_conversation() {
    conversation(&TcpSocketTransport::new());
}

#[test]
fn sha1_is_sha1() {
    let hex = |b: [u8; 20]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    assert_eq!(
        hex(crate::stdlib::websocket::sha1(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    assert_eq!(
        hex(crate::stdlib::websocket::sha1(b"")),
        "da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
    // RFC 6455 §1.3's worked example.
    assert_eq!(
        crate::stdlib::websocket::accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}
