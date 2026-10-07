#![cfg_attr(feature = "test-support", allow(dead_code))]
//! The socket transports against a local WebSocket peer (LLP 0059.000
//! §3.12): upgrade, text frames (fragmented and extended-length), a ping, the
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

/// One client frame, unmasked: (fin, opcode, payload), or `None` at the end.
fn client_frame(s: &mut impl Read) -> Option<(bool, u8, Vec<u8>)> {
    let mut head = [0u8; 2];
    s.read_exact(&mut head).ok()?;
    assert!(head[1] & 0x80 != 0, "a client frame is masked");
    let len = match head[1] & 0x7f {
        126 => {
            let mut bytes = [0u8; 2];
            s.read_exact(&mut bytes).ok()?;
            u16::from_be_bytes(bytes) as usize
        }
        127 => {
            let mut bytes = [0u8; 8];
            s.read_exact(&mut bytes).ok()?;
            usize::try_from(u64::from_be_bytes(bytes)).ok()?
        }
        len => len as usize,
    };
    let mut mask = [0u8; 4];
    s.read_exact(&mut mask).ok()?;
    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload).ok()?;
    for (i, b) in payload.iter_mut().enumerate() {
        *b ^= mask[i % 4];
    }
    Some((head[0] & 0x80 != 0, head[0] & 0x0f, payload))
}

/// A local peer; what it saw from the client arrives on the receiver.
pub fn peer() -> (u16, Receiver<String>) {
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
    let selected = if path == "/protocol" && head.contains("Sec-WebSocket-Protocol: chat") {
        "Sec-WebSocket-Protocol: chat\r\n"
    } else {
        ""
    };
    let _ = write!(s, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n{selected}\r\n");
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
        "/legacy-large" => {
            out.extend([0x82, 127]);
            out.extend_from_slice(&(64u64 << 20).to_be_bytes());
        }
        "/legacy-fragment" => {
            out.extend(frame(false, 2, &[1, 2, 3]));
            out.extend(frame(true, 0, &[4; 64]));
        }
        "/drop" => out.extend(frame(true, 1, b"x")),
        "/peer-close" => {
            let mut close = 1000u16.to_be_bytes().to_vec();
            close.extend_from_slice(b"peer");
            out.extend(frame(true, 8, &close));
        }
        "/echo" | "/protocol" | "/drain" | "/never" | "/no-read" => {}
        _ => out.extend(frame(true, 1, b"held")),
    }
    let _ = s.write_all(&out);
    if path == "/drop" {
        let _ = s.shutdown(Shutdown::Both);
        return;
    }
    if path == "/no-read" {
        std::thread::sleep(Duration::from_millis(300));
        let _ = s.shutdown(Shutdown::Both);
        return;
    }
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    if path == "/drain" {
        std::thread::sleep(Duration::from_millis(100));
    }
    let mut message: Option<(u8, Vec<u8>)> = None;
    while let Some((fin, opcode, payload)) = client_frame(&mut s) {
        if matches!(opcode, 1 | 2) {
            message = Some((opcode, Vec::new()));
        }
        if matches!(opcode, 0..=2) {
            let Some((_kind, whole)) = message.as_mut() else {
                return;
            };
            whole.extend_from_slice(&payload);
            if fin {
                let (kind, whole) = message.take().unwrap();
                let _ = saw.send(format!("{path} message {kind} {}", whole.len()));
                if matches!(path.as_str(), "/echo" | "/protocol") {
                    let _ = s.write_all(&frame(true, kind, &whole));
                }
            }
            continue;
        }
        let text = String::from_utf8_lossy(&payload).into_owned();
        let report = match opcode {
            0xA => format!("{path} pong {text}"),
            0x8 if payload.len() >= 2 => format!(
                "{path} close {} {}",
                u16::from_be_bytes([payload[0], payload[1]]),
                String::from_utf8_lossy(&payload[2..])
            ),
            other => format!("{path} opcode {other}"),
        };
        let _ = saw.send(report);
        if opcode == 0x8 {
            if matches!(path.as_str(), "/echo" | "/protocol" | "/drain") {
                let _ = s.write_all(&frame(true, 8, &payload));
            }
            if path == "/peer-close" {
                s.set_read_timeout(Some(Duration::from_millis(100)))
                    .unwrap();
                while let Some((_, later_opcode, later_payload)) = client_frame(&mut s) {
                    let _ = saw.send(format!(
                        "{path} after-close {later_opcode} {}",
                        later_payload.len()
                    ));
                }
            }
            break;
        }
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
        "/three close 1000 bye",
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

    let url = url::Url::parse(&format!("ws://127.0.0.1:{port}/protocol")).unwrap();
    let mut s = transport
        .connect_with_protocols(&url, 128 << 10, &none, &["chat".into()])
        .unwrap();
    assert_eq!(s.protocol(), "chat");
    s.send_text("hello").unwrap();
    assert_eq!(s.next().unwrap(), text("hello"));
    assert_eq!(wait(&seen), "/protocol message 1 5");
    s.send_binary(&[1, 2, 3, 4]).unwrap();
    assert_eq!(
        s.next_event().unwrap(),
        Event::Message(Message::Binary(vec![1, 2, 3, 4]))
    );
    assert_eq!(wait(&seen), "/protocol message 2 4");
    let fragmented = "x".repeat((FRAGMENT * 2) + 7);
    s.send_text(&fragmented).unwrap();
    assert_eq!(s.next().unwrap(), text(&fragmented));
    assert_eq!(
        wait(&seen),
        format!("/protocol message 1 {}", fragmented.len())
    );
    s.close(3001, "done").unwrap();
    assert_eq!(
        s.next().unwrap(),
        Incoming::Closed {
            code: 3001,
            reason: "done".into()
        }
    );
    assert_eq!(wait(&seen), "/protocol close 3001 done");
    let before = s.buffered_amount();
    s.send_text("discarded").unwrap();
    assert_eq!(s.buffered_amount(), before + "discarded".len());
    drop(s);
    assert_eq!(wait(&seen), "/protocol gone");

    // The close and a following send race at both transport boundaries. The
    // data is accounted per WHATWG, but a close already handed to the
    // platform wins and the peer never receives the later message.
    let mut s = open_on(transport, port, "/echo", &none).unwrap();
    s.sender().unwrap().close(None, "").unwrap();
    let before = s.buffered_amount();
    s.send_text("after-close").unwrap();
    assert_eq!(s.buffered_amount(), before + "after-close".len());
    assert!(matches!(
        s.next().unwrap(),
        Incoming::Closed {
            code: 1000 | 1005,
            reason
        } if reason.is_empty()
    ));
    assert!(matches!(
        wait(&seen).as_str(),
        "/echo opcode 8" | "/echo close 1000 "
    ));
    assert_eq!(wait(&seen), "/echo gone");

    // Race sends with a peer-initiated close. Frames accepted before the peer
    // close may precede our close reply, but nothing may follow that reply.
    let mut s = open_on(transport, port, "/peer-close", &none).unwrap();
    let sender = s.sender().unwrap();
    let racer = Arc::clone(&sender);
    let sending = std::thread::spawn(move || {
        for _ in 0..4 {
            racer.send_text("racing").unwrap();
            std::thread::yield_now();
        }
    });
    assert_eq!(
        s.next().unwrap(),
        Incoming::Closed {
            code: 1000,
            reason: "peer".into()
        }
    );
    sending.join().unwrap();
    sender.send_text("after-peer-close").unwrap();
    assert!(
        sender.buffered_amount() >= "after-peer-close".len(),
        "the discarded post-close payload remains accounted even while earlier racing sends complete"
    );
    loop {
        let report = wait(&seen);
        if report == "/peer-close close 1000 peer" {
            break;
        }
        assert!(report.starts_with("/peer-close message 1 "), "{report}");
    }
    loop {
        let report = wait(&seen);
        if report == "/peer-close gone" {
            break;
        }
        assert_eq!(
            report, "/peer-close after-close 1 6",
            "only sends admitted before the peer-close callback may still be in flight"
        );
    }

    let mut s = open_on(transport, port, "/drain", &none).unwrap();
    let payload = vec![7; 8 << 20];
    s.send_binary(&payload).unwrap();
    assert!(
        s.buffered_amount() > 0,
        "queued bytes are accounted immediately"
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while s.buffered_amount() != 0 && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        s.buffered_amount(),
        0,
        "sent bytes drain from bufferedAmount"
    );
    assert_eq!(wait(&seen), format!("/drain message 2 {}", payload.len()));
    s.close(1000, "").unwrap();
    assert!(matches!(
        s.next().unwrap(),
        Incoming::Closed { code: 1000, .. }
    ));

    // A non-reading peer cannot make the native send queue grow without
    // bound. Repeated data eventually fails the connection at the per-socket
    // byte/message ceiling; further sends only affect bufferedAmount.
    let mut s = open_on(transport, port, "/no-read", &none).unwrap();
    let chunk = vec![9; 1 << 20];
    for _ in 0..32 {
        s.send_binary(&chunk).unwrap();
    }
    assert!(matches!(s.next(), Ok(Incoming::Closed { .. }) | Err(_)));
}

#[test]
fn legacy_binary_next_returns_the_first_frames_declared_length_without_payload() {
    let (port, _seen) = peer();
    let transport = TcpSocketTransport::new();
    let none = AbortSignal::default();

    let mut large = open_on(&transport, port, "/legacy-large", &none).unwrap();
    assert_eq!(large.next().unwrap(), Incoming::Binary(64 << 20));
    drop(large);

    let mut fragmented = open_on(&transport, port, "/legacy-fragment", &none).unwrap();
    assert_eq!(fragmented.next().unwrap(), Incoming::Binary(3));
}

#[test]
fn an_ipv6_literal_is_bracketed_in_the_host_header() {
    let listener = std::net::TcpListener::bind("[::1]:0").expect("IPv6 loopback");
    let port = listener.local_addr().unwrap().port();
    let (reported, host) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8(head).unwrap();
        reported
            .send(
                head.lines()
                    .find(|line| line.starts_with("Host: "))
                    .unwrap()
                    .to_string(),
            )
            .unwrap();
        let key = head
            .lines()
            .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap()
            .trim();
        let accept = crate::stdlib::websocket::accept_key(key);
        write!(stream, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").unwrap();
        stream.write_all(&frame(true, 8, &[])).unwrap();
    });
    let url = url::Url::parse(&format!("ws://[::1]:{port}/")).unwrap();
    let mut socket = TcpSocketTransport::new()
        .connect(&url, 1024, &AbortSignal::default())
        .unwrap();
    assert!(matches!(socket.next(), Ok(Incoming::Closed { .. })));
    assert_eq!(host.recv().unwrap(), format!("Host: [::1]:{port}"));
    peer.join().unwrap();
}

#[test]
fn the_rust_transport_holds_the_whole_conversation() {
    conversation(&TcpSocketTransport::new());
}

#[test]
fn a_ping_flood_from_a_non_reading_peer_fails_cleanly() {
    const PINGS: usize = 4_096;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (release_peer, released) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8(head).unwrap();
        let key = head
            .lines()
            .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap()
            .trim();
        let accept = crate::stdlib::websocket::accept_key(key);
        write!(
            stream,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
        )
        .unwrap();

        let ping = frame(true, 0x9, &[7; 125]);
        for _ in 0..PINGS {
            if stream.write_all(&ping).is_err() {
                break;
            }
        }
        // Deliberately never read a pong. Keep the peer open long enough that
        // only the client's own bounded-queue failure can finish next().
        let _ = released.recv_timeout(Duration::from_secs(5));
    });

    let writer_gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let mut socket = open_on(
        &TcpSocketTransport::with_writer_gate(Arc::clone(&writer_gate)),
        port,
        "/ping-flood",
        &AbortSignal::default(),
    )
    .unwrap();
    let (reported, report) = channel();
    let reader = std::thread::spawn(move || {
        let _ = reported.send(socket.next());
    });
    let result = report.recv_timeout(Duration::from_secs(2));
    let _ = release_peer.send(());
    peer.join().unwrap();
    {
        let (lock, ready) = &*writer_gate;
        *lock.lock().unwrap() = true;
        ready.notify_one();
    }
    reader.join().unwrap();

    let error = result
        .expect("the ping flood must fail before the peer closes")
        .expect_err("command-capacity exhaustion is an abrupt failure");
    assert!(
        error.to_string().contains("outbound command queue is full"),
        "unexpected ping-flood failure: {error}"
    );
}

#[test]
fn a_stalled_write_to_a_non_reading_peer_fails_within_the_stall_bound() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (release_peer, released) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8(head).unwrap();
        let key = head
            .lines()
            .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap()
            .trim();
        let accept = crate::stdlib::websocket::accept_key(key);
        write!(
            stream,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
        )
        .unwrap();
        // Never read again. Let the client's large write fill the buffers and
        // stall first, then ask to close: a reader starved by the stalled
        // writer cannot see it. Then hold the connection open.
        std::thread::sleep(Duration::from_millis(300));
        let _ = stream.write_all(&frame(true, 0x8, &1000u16.to_be_bytes()));
        let _ = released.recv_timeout(Duration::from_secs(10));
    });

    let mut socket = open_on(
        &TcpSocketTransport::new(),
        port,
        "/stall",
        &AbortSignal::default(),
    )
    .unwrap();
    // Several MiB cannot fit in the kernel buffers of a peer that never reads,
    // so the writer stalls inside a frame.
    socket.send_binary(&vec![5u8; 8 * 1024 * 1024]).unwrap();
    let (reported, report) = channel();
    let reader = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut last = None;
        for _ in 0..4 {
            match socket.next() {
                Ok(Incoming::Closed { .. }) | Err(_) => {
                    last = Some(started.elapsed());
                    break;
                }
                Ok(_) => continue,
            }
        }
        let _ = reported.send(last);
    });
    // The bound is 500 ms in tests; allow generous scheduling slack.
    let finished = report.recv_timeout(Duration::from_secs(10));
    let _ = release_peer.send(());
    peer.join().unwrap();
    // A reader still blocked behind a stalled writer would never be joined;
    // fail instead of hanging the suite.
    let elapsed = finished
        .expect("next() must finish while the peer keeps the connection open")
        .expect("the socket must report a close or a failure");
    reader.join().unwrap();
    assert!(
        elapsed < Duration::from_secs(8),
        "a stalled write pinned the reader for {elapsed:?}"
    );
}

const LOCAL_CERT: &str = "MIIBcDCCARagAwIBAgIJAL/L9Qemvq28MAoGCCqGSM49BAMCMBQxEjAQBgNVBAMMCWxvY2FsaG9zdDAeFw0yNjEwMDQxMzQ2MTBaFw0yNzEwMDQxMzQ2MTBaMBQxEjAQBgNVBAMMCWxvY2FsaG9zdDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABB+9b/H/REalNbaY5CeIowEsLfdmeVL8M/iQgCo4BrJM+IgYXRIUDI6EdvgZkkyBFTr8dIRFr/5u/AX/0vRU3p2jUTBPMBoGA1UdEQQTMBGCCWxvY2FsaG9zdIcEfwAAATAMBgNVHRMBAf8EAjAAMA4GA1UdDwEB/wQEAwIHgDATBgNVHSUEDDAKBggrBgEFBQcDATAKBggqhkjOPQQDAgNIADBFAiBU7Mu0QDVetJW9tm7u7aoPrVQcEqkO0IUkZ0aMgPA6GwIhAPMuBqpj21v+kfb7/bCjL94nmzgQkNzpdDPei6+PzVpa";
const LOCAL_KEY: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgNuGe4B07FBTDauLEJyJRWafyt3Hlvuh33z/wS96uBu2hRANCAAQfvW/x/0RGpTW2mOQniKMBLC33ZnlS/DP4kIAqOAayTPiIGF0SFAyOhHb4GZJMgRU6/HSERa/+bvwF/9L0VN6d";

fn local_tls() -> (
    Vec<u8>,
    Arc<rustls::ClientConfig>,
    Arc<rustls::ServerConfig>,
) {
    use base64::Engine as _;
    let cert_bytes = base64::engine::general_purpose::STANDARD
        .decode(LOCAL_CERT)
        .unwrap();
    let cert = rustls::pki_types::CertificateDer::from(cert_bytes.clone());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        base64::engine::general_purpose::STANDARD
            .decode(LOCAL_KEY)
            .unwrap(),
    ));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.clone()).unwrap();
    let client = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    (cert_bytes, Arc::new(client), Arc::new(server))
}

pub(crate) fn tls_echo_peer() -> (
    u16,
    Vec<u8>,
    Arc<rustls::ClientConfig>,
    std::thread::JoinHandle<()>,
) {
    let (certificate, client, server) = local_tls();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peer = std::thread::spawn(move || {
        let tcp = listener.accept().unwrap().0;
        tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let connection = rustls::ServerConnection::new(server).unwrap();
        let mut wire = rustls::StreamOwned::new(connection, tcp);
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            wire.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8(head).unwrap();
        let key = head
            .lines()
            .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap()
            .trim();
        let accept = crate::stdlib::websocket::accept_key(key);
        write!(wire, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").unwrap();
        let (fin, opcode, payload) = client_frame(&mut wire).unwrap();
        assert!(fin);
        assert_eq!(opcode, 1);
        wire.write_all(&frame(true, 1, &payload)).unwrap();
        let (fin, opcode, payload) = client_frame(&mut wire).unwrap();
        assert!(fin);
        assert_eq!(opcode, 8);
        wire.write_all(&frame(true, 8, &payload)).unwrap();
    });
    (port, certificate, client, peer)
}

#[test]
fn the_rust_transport_echoes_and_closes_over_local_tls() {
    let (port, _certificate, client, peer) = tls_echo_peer();
    let transport = TcpSocketTransport::with_tls(client);
    let url = url::Url::parse(&format!("wss://localhost:{port}/echo")).unwrap();
    let mut socket = transport
        .connect(&url, 1024, &AbortSignal::default())
        .unwrap();
    socket.send_text("secure").unwrap();
    assert_eq!(socket.next().unwrap(), text("secure"));
    socket.close(1000, "tls done").unwrap();
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 1000,
            reason: "tls done".into(),
        }
    );
    peer.join().unwrap();
}

/// Go through the production connection path, then inspect and read the socket
/// actually retained by Wire. Configuring a shutdown clone instead regresses
/// both progress and the writer bound on Windows.
#[test]
fn retained_plain_and_tls_sockets_have_bounded_frame_io() {
    for secure in [false, true] {
        let (transport, url, peer) = if secure {
            let (port, _, client, peer) = tls_echo_peer();
            (
                TcpSocketTransport::with_tls(client),
                format!("wss://localhost:{port}/echo"),
                Some(peer),
            )
        } else {
            let (port, _seen) = peer();
            (
                TcpSocketTransport::new(),
                format!("ws://127.0.0.1:{port}/echo"),
                None,
            )
        };
        let mut socket = transport
            .open_socket(
                &url::Url::parse(&url).unwrap(),
                1024,
                &AbortSignal::default(),
                &[],
            )
            .unwrap();
        {
            let wire = socket.wire.wire.lock().unwrap();
            let tcp = match &*wire {
                Wire::Plain(tcp) => tcp,
                Wire::Tls(tls) => &tls.sock,
            };
            // An OS may round options to its timer granularity. Neither the
            // old 15 s read nor an unbounded write fits these retained bounds.
            let read = tcp.read_timeout().unwrap().expect("bounded read");
            let write = tcp.write_timeout().unwrap().expect("bounded write");
            assert!((Duration::from_millis(25)..Duration::from_millis(125)).contains(&read));
            assert!(
                (WRITE_STALL_TIMEOUT..WRITE_STALL_TIMEOUT + Duration::from_millis(100))
                    .contains(&write)
            );
        }
        // The peer remains open and waits for our first frame. Keep a generous
        // watchdog so a lost read bound fails instead of hanging the suite.
        let shutdown = socket.shutdown.try_clone().unwrap();
        let (done, finished) = channel();
        let watchdog = std::thread::spawn(move || {
            if finished.recv_timeout(Duration::from_secs(2)).is_err() {
                let _ = shutdown.shutdown(Shutdown::Both);
            }
        });
        let idle = socket.wire.read(&mut [0; 1]);
        let _ = done.send(());
        watchdog.join().unwrap();
        assert!(
            matches!(idle, Err(ref error) if matches!(error.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock)),
            "idle retained Wire did not time out: {idle:?}"
        );
        // A receive slice is not a failed conversation: writing and receiving
        // after it must still work, including the shared rustls state.
        socket.send_text("after idle").unwrap();
        assert_eq!(socket.next().unwrap(), text("after idle"));
        socket.close(1000, "done").unwrap();
        assert_eq!(
            socket.next().unwrap(),
            Incoming::Closed {
                code: 1000,
                reason: "done".into()
            }
        );
        if let Some(peer) = peer {
            peer.join().unwrap();
        }
    }
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
