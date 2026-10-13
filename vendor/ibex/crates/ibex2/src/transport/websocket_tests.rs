#![cfg_attr(feature = "test-support", allow(dead_code))]
//! The socket transports against a local WebSocket peer (LLP 0059.000
//! §3.12): upgrade, text frames (fragmented and extended-length), a ping, the
//! closing handshake, an over-limit frame, a binary frame, a refused
//! handshake, a connection dropped without a close, and an abort.
use super::*;
use crate::stdlib::abort::AbortController;
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender};

// The production outbound quota is also the largest single message these
// regressions can admit. It is comfortably larger than Linux's observed
// multi-megabyte autotuned send-ahead and spans 1,024 wire fragments.
const LARGE_SEND: usize = MAX_OUTBOUND_BYTES;

/// Set the peer's advertised receive window before `accept`. Linux inherits
/// this listener option while negotiating the child socket, whereas changing
/// an accepted socket is too late to constrain the initial window.
fn slow_peer_listener() -> std::net::TcpListener {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    socket2::SockRef::from(&listener)
        .set_recv_buffer_size(FRAGMENT)
        .unwrap();
    listener
}

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
    assert_eq!(head[0] & 0x70, 0, "client frames do not set RSV bits");
    assert!(head[1] & 0x80 != 0, "a client frame is masked");
    let fin = head[0] & 0x80 != 0;
    let opcode = head[0] & 0x0f;
    assert!(
        matches!(opcode, 0x0..=0x2 | 0x8..=0xA),
        "reserved client opcode {opcode:#x}"
    );
    let encoded_len = head[1] & 0x7f;
    let len = match encoded_len {
        126 => {
            let mut bytes = [0u8; 2];
            s.read_exact(&mut bytes).ok()?;
            let len = u16::from_be_bytes(bytes) as usize;
            assert!(len >= 126, "client frame length is not minimally encoded");
            len
        }
        127 => {
            let mut bytes = [0u8; 8];
            s.read_exact(&mut bytes).ok()?;
            let len = u64::from_be_bytes(bytes);
            assert!(
                len >= 65_536,
                "client frame length is not minimally encoded"
            );
            assert_eq!(len >> 63, 0, "client frame length sets the reserved bit");
            usize::try_from(len).ok()?
        }
        len => len as usize,
    };
    if opcode >= 0x8 {
        assert!(fin, "a client control frame is not final");
        assert!(len <= 125, "a client control frame is oversized");
        assert!(
            encoded_len < 126,
            "a client control frame uses an extended length"
        );
    }
    let mut mask = [0u8; 4];
    s.read_exact(&mut mask).ok()?;
    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload).ok()?;
    for (i, b) in payload.iter_mut().enumerate() {
        *b ^= mask[i % 4];
    }
    Some((fin, opcode, payload))
}

#[derive(Default)]
struct FragmentedBinary {
    fragments: usize,
    bytes: usize,
}

impl FragmentedBinary {
    fn observe(&mut self, fin: bool, opcode: u8, payload: &[u8], awaited: &str) {
        assert_eq!(
            opcode,
            if self.fragments == 0 { 0x2 } else { 0x0 },
            "unexpected data opcode before {awaited}"
        );
        assert!(!fin, "{awaited} followed the message's final fragment");
        assert_eq!(
            payload.len(),
            FRAGMENT,
            "non-final data fragments have the transport fragment size"
        );
        self.fragments += 1;
        self.bytes += payload.len();
    }

    fn read_one(&mut self, stream: &mut impl Read, awaited: &str) {
        let (fin, opcode, payload) = client_frame(stream).expect("the client sent a data frame");
        self.observe(fin, opcode, &payload, awaited);
    }

    fn read_control(
        &mut self,
        stream: &mut impl Read,
        expected_opcode: u8,
        expected_payload: &[u8],
        awaited: &str,
    ) {
        loop {
            let (fin, opcode, payload) =
                client_frame(stream).unwrap_or_else(|| panic!("the client sent {awaited}"));
            if opcode == expected_opcode {
                assert!(fin);
                assert_eq!(payload, expected_payload);
                assert!(self.fragments > 0, "{awaited} preceded the large message");
                assert!(
                    self.bytes < LARGE_SEND,
                    "{awaited} followed the entire large message"
                );
                return;
            }
            self.observe(fin, opcode, &payload, awaited);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
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

/// Wait until the pump's last write found the kernel full and no write has
/// progressed for a while. One `WouldBlock` is not enough: macOS grows an
/// autotuned send buffer as earlier segments are acknowledged, so a socket that
/// once refused bytes can accept the rest of a fragment and a control frame
/// moments later. Once the non-reading peer's window is closed, nothing more
/// is acknowledged and the buffer stops growing.
#[cfg(test)]
fn wait_for_kernel_backpressure(observer: &PumpObserver) {
    const QUIET: Duration = Duration::from_millis(200);
    let watchdog = std::time::Instant::now() + Duration::from_secs(10);
    let mut quiet_since: Option<(std::time::Instant, usize)> = None;
    while std::time::Instant::now() < watchdog {
        let bytes = observer.network_bytes.load(Ordering::Acquire);
        let blocked = observer.write_blocked.load(Ordering::Acquire)
            && observer.parked.load(Ordering::Acquire);
        match quiet_since {
            Some((since, seen)) if blocked && seen == bytes => {
                if since.elapsed() >= QUIET {
                    return;
                }
            }
            _ => quiet_since = blocked.then(|| (std::time::Instant::now(), bytes)),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the maximum-size message did not fill the kernel buffers");
}

/// The pump's terminal-latch transitions so far, read under the lock the pump
/// records them with.
#[cfg(test)]
fn terminal_history(observer: &PumpObserver) -> Vec<TerminalStep> {
    observer
        .send_state
        .get()
        .expect("the pump registered its state")
        .lock()
        .expect("WebSocket sender poisoned")
        .history
        .clone()
}

/// Wait for the pump's publish-or-defer decision and return it.
#[cfg(test)]
fn wait_for_terminal_decision(observer: &PumpObserver) -> TerminalStep {
    let watchdog = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(first) = terminal_history(observer).first() {
            return *first;
        }
        assert!(
            std::time::Instant::now() < watchdog,
            "the pump made no terminal decision"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A terminal result that generated a Close was deferred when latched and
/// became deliverable only when that Close was handed to TCP.
#[cfg(test)]
fn assert_deferred_until_close_sent(observer: &PumpObserver) {
    assert_eq!(
        terminal_history(observer),
        [
            TerminalStep::LatchedDeferred,
            TerminalStep::CloseSent,
            TerminalStep::Deliverable
        ],
        "the terminal result became deliverable before its Close was sent"
    );
}

fn server_handshake<W: Read + Write + ?Sized>(wire: &mut W) {
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
    wire.flush().unwrap();
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
fn close_and_fin_between_receives_preserve_the_peer_close_and_reply() {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (close_sent, close_observed) = channel();
    let (reported, report) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        server_handshake(&mut stream);
        stream.write_all(&frame(true, 1, b"first")).unwrap();
        let mut message = FragmentedBinary::default();
        message.read_one(&mut stream, "the peer Close reply");
        let mut close = 3001u16.to_be_bytes().to_vec();
        close.extend_from_slice(b"between");
        stream.write_all(&frame(true, 0x8, &close)).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        close_sent.send(()).unwrap();
        message.read_control(&mut stream, 0x8, &close, "the peer Close reply");
        reported.send(message.bytes).unwrap();
    });

    let mut socket = open_on(
        &TcpSocketTransport::new(),
        port,
        "/close-fin",
        &AbortSignal::default(),
    )
    .unwrap();
    assert_eq!(socket.next().unwrap(), text("first"));
    socket.send_binary(&vec![7; LARGE_SEND]).unwrap();
    close_observed.recv().unwrap();
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 3001,
            reason: "between".into(),
        }
    );
    let data_before_close = report
        .recv_timeout(Duration::from_secs(10))
        .expect("the peer received the close reply");
    assert!(
        data_before_close < LARGE_SEND,
        "the buffered Close reply followed the whole queued message"
    );
    peer.join().unwrap();
}

#[test]
fn a_watch_does_not_publish_close_until_its_blocked_reply_drains() {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (close_now, close_requested) = channel();
    let (close_sent, close_observed) = channel();
    let (drain_now, drain_requested) = channel();
    let (started, message_started) = channel();
    let (reported, report) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        server_handshake(&mut stream);
        let mut message = FragmentedBinary::default();
        message.read_one(&mut stream, "the watched Close reply");
        started.send(()).unwrap();
        close_requested.recv().unwrap();
        let mut close = 3010u16.to_be_bytes().to_vec();
        close.extend_from_slice(b"watched");
        stream.write_all(&frame(true, 0x8, &close)).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        close_sent.send(()).unwrap();
        drain_requested.recv().unwrap();
        message.read_control(&mut stream, 0x8, &close, "the watched Close reply");
        reported.send(close).unwrap();
    });

    let observer = Arc::new(PumpObserver::default());
    let transport = Arc::new(TcpSocketTransport::with_pump_observer(Arc::clone(
        &observer,
    )));
    let grants = Arc::new(
        crate::grant::GrantSet::parse(&format!("net.websocket ws://127.0.0.1:{port}\n")).unwrap(),
    );
    let (connection, events, _subscription) = crate::stdlib::websocket::watch(
        transport,
        grants,
        format!("ws://127.0.0.1:{port}/watch-close-fin"),
        Vec::new(),
        1024,
    );
    assert!(matches!(events.recv().unwrap(), Event::Open { .. }));
    connection.send_binary(&vec![8; LARGE_SEND]).unwrap();
    message_started.recv().unwrap();
    wait_for_kernel_backpressure(&observer);
    close_now.send(()).unwrap();
    close_observed.recv().unwrap();
    // Observe the pump's publish-or-defer decision itself, not the earlier
    // parse flag. The peer Close generated a reply, so it is latched
    // undeliverable; while that remains so, the watch cannot have it.
    assert_eq!(
        wait_for_terminal_decision(&observer),
        TerminalStep::LatchedDeferred,
        "the peer Close was published before its reply could drain"
    );
    assert!(observer.close_received.load(Ordering::Acquire));
    {
        let state = observer.send_state.get().unwrap().lock().unwrap();
        if !state.history.contains(&TerminalStep::Deliverable) {
            assert!(
                matches!(events.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)),
                "the watch published Close before its reply could drain"
            );
        }
    }
    drain_now.send(()).unwrap();
    assert_eq!(
        report.recv_timeout(Duration::from_secs(10)).unwrap(),
        [3010u16.to_be_bytes().as_slice(), b"watched"].concat()
    );
    assert_eq!(
        events.recv_timeout(Duration::from_secs(10)).unwrap(),
        Event::Close {
            code: 3010,
            reason: "watched".into(),
            was_clean: true,
        }
    );
    assert_deferred_until_close_sent(&observer);
    peer.join().unwrap();
}

/// An oversized inbound message supersedes an admitted but unstarted local
/// Close with 1009, with or without the peer's FIN behind the oversized
/// frame. The peer reads nothing until the pump has latched TooLarge as
/// undeliverable, so 1009 is generated while the data fragment is blocked.
/// Afterwards the client half-closes: the peer reads 1009 and then a clean
/// EOF, not a reset that would have discarded the queued 1009.
#[cfg(test)]
fn too_large_supersedes_an_admitted_close(peer_fin: bool) {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (send_oversize, oversize_requested) = channel();
    let (oversize_sent, oversize_observed) = channel();
    let (message_started, started) = channel();
    let (drain_now, drain_requested) = channel();
    let (reported, report) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        server_handshake(&mut stream);
        let mut message = FragmentedBinary::default();
        message.read_one(&mut stream, "the 1009 Close");
        message_started.send(()).unwrap();
        oversize_requested.recv().unwrap();
        stream.write_all(&frame(true, 0x1, &[b'x'; 128])).unwrap();
        if peer_fin {
            stream.shutdown(Shutdown::Write).unwrap();
        }
        oversize_sent.send(()).unwrap();
        drain_requested.recv().unwrap();
        message.read_control(&mut stream, 0x8, &1009u16.to_be_bytes(), "the 1009 Close");
        let mut after = [0u8; 1];
        let end = stream.read(&mut after).map_err(|error| error.kind());
        reported.send((message.bytes, end)).unwrap();
    });

    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let url = url::Url::parse(&format!("ws://127.0.0.1:{port}/oversize-close")).unwrap();
    let mut socket = transport
        .connect(&url, 64, &AbortSignal::default())
        .unwrap();
    socket.send_binary(&vec![9; LARGE_SEND]).unwrap();
    socket.close(3008, "queued").unwrap();
    started.recv().unwrap();
    wait_for_kernel_backpressure(&observer);
    send_oversize.send(()).unwrap();
    oversize_observed.recv().unwrap();
    let receiving = std::thread::spawn(move || {
        let received = socket.next();
        (socket, received)
    });
    assert_eq!(
        wait_for_terminal_decision(&observer),
        TerminalStep::LatchedDeferred,
        "TooLarge was published before its 1009 could drain"
    );
    drain_now.send(()).unwrap();
    let (socket, received) = receiving.join().unwrap();
    assert_eq!(received.unwrap(), Incoming::TooLarge);
    assert_deferred_until_close_sent(&observer);
    let (data_before_close, end) = report.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(data_before_close < LARGE_SEND);
    assert_eq!(
        end,
        Ok(0),
        "the client reset instead of half-closing after 1009"
    );
    drop(socket);
    peer.join().unwrap();
}

#[test]
fn too_large_supersedes_an_admitted_close_with_receive_demand() {
    too_large_supersedes_an_admitted_close(false);
}

#[test]
fn too_large_supersedes_an_admitted_close_before_a_peer_fin() {
    too_large_supersedes_an_admitted_close(true);
}

/// A peer FIN is the in-order end of the byte stream. Messages ahead of it are
/// still delivered to later receive demand even when the pump observed the
/// FIN's readiness first, and only then does EOF report 1006.
#[test]
fn messages_before_a_peer_fin_survive_fin_readiness_without_demand() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (reported, report) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        server_handshake(&mut stream);
        let mut inbound = frame(true, 0x1, b"before fin");
        inbound.extend(frame(true, 0x2, &[4; 300]));
        stream.write_all(&inbound).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut after = [0u8; 1];
        reported
            .send(stream.read(&mut after).map_err(|error| error.kind()))
            .unwrap();
    });

    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let mut socket = open_on(&transport, port, "/data-fin", &AbortSignal::default()).unwrap();
    let watchdog = std::time::Instant::now() + Duration::from_secs(10);
    while !observer.peer_fin.load(Ordering::Acquire) && std::time::Instant::now() < watchdog {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(observer.peer_fin.load(Ordering::Acquire));
    assert_eq!(socket.next().unwrap(), text("before fin"));
    assert_eq!(
        socket.next_event().unwrap(),
        Event::Message(Message::Binary(vec![4; 300]))
    );
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 1006,
            reason: String::new(),
        }
    );
    assert_eq!(
        report.recv_timeout(Duration::from_secs(10)).unwrap(),
        Ok(0),
        "the client did not half-close after EOF"
    );
    drop(socket);
    peer.join().unwrap();
}

#[test]
fn read_fin_preserves_an_admitted_close_behind_a_blocked_fragment() {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (finish_reading, finish_requested) = channel();
    let (reported, report) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        server_handshake(&mut stream);
        finish_requested.recv().unwrap();
        let mut message = FragmentedBinary::default();
        message.read_one(&mut stream, "the admitted Close");
        stream.shutdown(Shutdown::Write).unwrap();
        let close = [3009u16.to_be_bytes().as_slice(), b"retain"].concat();
        message.read_control(&mut stream, 0x8, &close, "the admitted Close");
        reported.send(message.bytes).unwrap();
    });

    let mut socket = open_on(
        &TcpSocketTransport::new(),
        port,
        "/fin-local-close",
        &AbortSignal::default(),
    )
    .unwrap();
    socket.send_binary(&vec![3; LARGE_SEND]).unwrap();
    socket.close(3009, "retain").unwrap();
    finish_reading.send(()).unwrap();
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 1006,
            reason: String::new(),
        }
    );
    assert!(report.recv_timeout(Duration::from_secs(10)).unwrap() < LARGE_SEND);
    peer.join().unwrap();
}

#[test]
fn write_failure_drains_unread_data_before_publishing_a_peer_close() {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (reset_sent, reset_observed) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        server_handshake(&mut stream);
        let mut inbound = frame(true, 0x1, &[b'd'; 64 << 10]);
        let mut close = 3011u16.to_be_bytes().to_vec();
        close.extend_from_slice(b"behind data");
        inbound.extend(frame(true, 0x8, &close));
        stream.write_all(&inbound).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        socket2::SockRef::from(&stream)
            .set_linger(Some(Duration::ZERO))
            .unwrap();
        reset_sent.send(()).unwrap();
    });

    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let url = url::Url::parse(&format!("ws://127.0.0.1:{port}/write-dead-close")).unwrap();
    let mut socket = transport
        .connect(&url, 128 << 10, &AbortSignal::default())
        .unwrap();
    socket.send_binary(&vec![2; LARGE_SEND]).unwrap();
    reset_observed.recv().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !observer.terminal_draining.load(Ordering::Acquire)
        && std::time::Instant::now() < deadline
    {
        std::thread::yield_now();
    }
    assert!(observer.terminal_draining.load(Ordering::Acquire));
    let terminal = socket.next();
    // Linux and macOS keep bytes that arrived before a RST readable, so the
    // drain finds the peer Close. Windows discards a reset connection's unread
    // receive buffer: the Close is gone, and whichever direction observes the
    // reset first reports the abnormal end (1006 or the write error).
    #[cfg(not(windows))]
    assert_eq!(
        terminal.unwrap(),
        Incoming::Closed {
            code: 3011,
            reason: "behind data".into(),
        }
    );
    #[cfg(windows)]
    match terminal {
        Ok(Incoming::Closed { code: 1006, .. }) => {}
        Err(HostError::Failed(error)) if error.contains("the socket write failed") => {}
        other => panic!("a Windows reset did not end abnormally: {other:?}"),
    }
    wait_for_pump_exit(&observer, "write-dead terminal drain");
    peer.join().unwrap();
}

#[test]
fn a_peer_close_survives_a_following_reset_during_a_blocked_send() {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (reset_now, reset_requested) = channel();
    let (close_sent, close_observed) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        server_handshake(&mut stream);
        reset_requested.recv().unwrap();
        let mut message = FragmentedBinary::default();
        message.read_one(&mut stream, "the peer Close");
        let mut close = 3002u16.to_be_bytes().to_vec();
        close.extend_from_slice(b"keep me");
        stream.write_all(&frame(true, 0x8, &close)).unwrap();
        close_sent.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        socket2::SockRef::from(&stream)
            .set_linger(Some(Duration::ZERO))
            .unwrap();
    });

    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let mut socket = open_on(&transport, port, "/close-reset", &AbortSignal::default()).unwrap();
    socket.send_binary(&vec![6; LARGE_SEND]).unwrap();
    reset_now.send(()).unwrap();
    close_observed.recv().unwrap();
    wait_for_pump_exit(&observer, "close followed by reset");
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 3002,
            reason: "keep me".into(),
        }
    );
    peer.join().unwrap();
}

#[test]
fn too_large_close_is_not_followed_by_queued_pongs() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (reported, report) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        server_handshake(&mut stream);
        let mut inbound = frame(true, 0x9, b"first");
        inbound.extend(frame(true, 0x9, b"second"));
        inbound.extend(frame(true, 0x1, &[b'x'; 128]));
        stream.write_all(&inbound).unwrap();
        loop {
            let (_, opcode, payload) = client_frame(&mut stream).expect("the client sent 1009");
            if opcode == 0x8 {
                assert_eq!(payload, 1009u16.to_be_bytes());
                break;
            }
            assert_eq!(opcode, 0xA, "only pre-Close pongs may precede 1009");
        }
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        reported.send(client_frame(&mut stream)).unwrap();
    });

    let url = url::Url::parse(&format!("ws://127.0.0.1:{port}/too-large-pongs")).unwrap();
    let mut socket = TcpSocketTransport::new()
        .connect(&url, 64, &AbortSignal::default())
        .unwrap();
    assert_eq!(socket.next().unwrap(), Incoming::TooLarge);
    assert_eq!(
        report.recv_timeout(Duration::from_secs(2)).unwrap(),
        None,
        "queued pongs followed the 1009 Close"
    );
    peer.join().unwrap();
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
fn control_frames_interleave_with_a_large_send_to_a_slow_reader() {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (reported, report) = channel();
    let (pong_seen, pong_observed) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        server_handshake(&mut stream);
        let mut message = FragmentedBinary::default();
        message.read_one(&mut stream, "the Pong");
        stream.write_all(&frame(true, 0x9, b"priority")).unwrap();
        message.read_control(&mut stream, 0xA, b"priority", "the Pong");
        let data_before_pong = message.bytes;
        pong_seen.send(()).unwrap();

        stream
            .write_all(&frame(true, 0x8, &1000u16.to_be_bytes()))
            .unwrap();
        message.read_control(&mut stream, 0x8, &1000u16.to_be_bytes(), "the Close reply");
        reported.send((data_before_pong, message.bytes)).unwrap();
    });

    let mut socket = open_on(
        &TcpSocketTransport::new(),
        port,
        "/slow-control",
        &AbortSignal::default(),
    )
    .unwrap();
    socket.send_binary(&vec![5; LARGE_SEND]).unwrap();
    pong_observed
        .recv_timeout(Duration::from_secs(10))
        .expect("the pump answered ping without a posted receive");
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 1000,
            reason: String::new(),
        }
    );
    let (before_pong, before_close) = report
        .recv_timeout(Duration::from_secs(10))
        .expect("the slow peer received both control replies");
    peer.join().unwrap();
    assert!(before_pong < LARGE_SEND, "pong followed the whole message");
    assert!(
        before_close < LARGE_SEND,
        "close reply followed the whole message"
    );
}

#[test]
fn an_admitted_local_close_does_not_suppress_ping_or_peer_close() {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (start_control, control_started) = channel();
    let (reported, report) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        server_handshake(&mut stream);
        control_started.recv().unwrap();
        let mut message = FragmentedBinary::default();
        message.read_one(&mut stream, "the Pong");
        stream.write_all(&frame(true, 0x9, b"still open")).unwrap();
        message.read_control(&mut stream, 0xA, b"still open", "the Pong");
        let data_before_pong = message.bytes;

        let mut close = 3003u16.to_be_bytes().to_vec();
        close.extend_from_slice(b"peer wins");
        stream.write_all(&frame(true, 0x8, &close)).unwrap();
        message.read_control(&mut stream, 0x8, &close, "the peer Close reply");
        reported.send((data_before_pong, message.bytes)).unwrap();
    });

    let mut socket = open_on(
        &TcpSocketTransport::new(),
        port,
        "/local-close-control",
        &AbortSignal::default(),
    )
    .unwrap();
    socket.send_binary(&vec![4; LARGE_SEND]).unwrap();
    socket.close(3000, "local").unwrap();
    start_control.send(()).unwrap();
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 3003,
            reason: "peer wins".into(),
        }
    );
    let (before_pong, before_close) = report.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(before_pong < LARGE_SEND);
    assert!(before_close < LARGE_SEND);
    peer.join().unwrap();
}

#[cfg(test)]
fn stalled_peer(
    secure: bool,
    flood_pongs: bool,
) -> (
    TcpSocketTransport,
    String,
    Sender<()>,
    Arc<PumpObserver>,
    std::thread::JoinHandle<()>,
) {
    let listener = slow_peer_listener();
    let port = listener.local_addr().unwrap().port();
    let (release_peer, released) = channel();
    let observer = Arc::new(PumpObserver::default());
    let (transport, server) = if secure {
        let (_, client, server) = local_tls();
        (
            TcpSocketTransport::with_tls_and_pump_observer(client, Arc::clone(&observer)),
            Some(server),
        )
    } else {
        (
            TcpSocketTransport::with_pump_observer(Arc::clone(&observer)),
            None,
        )
    };
    let peer = std::thread::spawn(move || {
        let tcp = listener.accept().unwrap().0;
        tcp.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
        let hold = |wire: &mut (dyn ReadWrite + '_)| {
            server_handshake(wire);
            if flood_pongs {
                let pong = frame(true, 0xA, &[7; 125]);
                while wire.write_all(&pong).is_ok() {}
            } else {
                // After the handshake, neither read nor write until the
                // client's own no-progress deadline has failed the socket.
                let _ = released.recv_timeout(Duration::from_secs(5));
            }
        };
        if let Some(server) = server {
            let connection = rustls::ServerConnection::new(server).unwrap();
            let mut wire = rustls::StreamOwned::new(connection, tcp);
            hold(&mut wire);
        } else {
            let mut wire = tcp;
            hold(&mut wire);
        }
    });

    let scheme = if secure { "wss" } else { "ws" };
    (
        transport,
        format!("{scheme}://localhost:{port}/stall"),
        release_peer,
        observer,
        peer,
    )
}

#[cfg(test)]
trait ReadWrite: Read + Write {}
#[cfg(test)]
impl<T: Read + Write> ReadWrite for T {}

#[cfg(test)]
fn assert_stall_deadline(secure: bool, flood_pongs: bool) {
    let (transport, url, release_peer, observer, peer) = stalled_peer(secure, flood_pongs);
    let mut socket = transport
        .connect(
            &url::Url::parse(&url).unwrap(),
            1024,
            &AbortSignal::default(),
        )
        .unwrap();
    socket.send_binary(&vec![5u8; LARGE_SEND]).unwrap();
    let error = socket
        .next()
        .expect_err("the client's no-progress deadline must fail the socket");
    assert!(
        error
            .to_string()
            .contains("the socket write made no progress before its deadline"),
        "unexpected stalled-write failure: {error}"
    );
    assert!(
        observer.network_bytes.load(Ordering::Acquire) > 0,
        "the test must partially write the frame before it stalls"
    );
    let last_progress = observer
        .last_write_progress
        .lock()
        .expect("WebSocket progress observer poisoned")
        .expect("a partial write records its progress instant");
    let stalled_for = last_progress.elapsed();
    assert!(
        stalled_for >= WRITE_STALL_TIMEOUT && stalled_for < Duration::from_secs(3),
        "the {:?} write made no progress for {stalled_for:?}, expected at least {WRITE_STALL_TIMEOUT:?}",
        if secure { "TLS" } else { "plaintext" }
    );
    eprintln!(
        "{} stalled write failed after {stalled_for:?} without progress, with {} network bytes written{}",
        if secure { "TLS" } else { "plaintext" },
        observer.network_bytes.load(Ordering::Acquire),
        if flood_pongs {
            " during a pong flood"
        } else {
            ""
        }
    );
    let _ = release_peer.send(());
    drop(socket);
    peer.join().unwrap();
}

#[test]
fn silent_peer_stalls_plaintext_and_partial_tls_writes_at_the_deadline() {
    assert_stall_deadline(false, false);
    assert_stall_deadline(true, false);
}

#[test]
fn pong_flood_cannot_starve_the_stalled_write_deadline() {
    assert_stall_deadline(false, true);
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

#[test]
fn zero_from_write_tls_is_an_abnormal_close() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (_, client, server) = local_tls();
    let peer = std::thread::spawn(move || {
        let tcp = listener.accept().unwrap().0;
        tcp.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let connection = rustls::ServerConnection::new(server).unwrap();
        let mut wire = rustls::StreamOwned::new(connection, tcp);
        server_handshake(&mut wire);
        let mut byte = [0; 1];
        let _ = wire.read(&mut byte);
    });

    let transport = TcpSocketTransport::with_tls_zero_socket_writes(client);
    let url = url::Url::parse(&format!("wss://localhost:{port}/zero-write")).unwrap();
    let mut socket = transport
        .connect(&url, 1024, &AbortSignal::default())
        .unwrap();
    socket.send_text("drive rustls through the pump").unwrap();
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 1006,
            reason: String::new(),
        }
    );
    drop(socket);
    peer.join().unwrap();
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

/// Go through the production connection path, then inspect and read a clone of
/// the retained socket. Nonblocking mode is a property of the underlying
/// socket on every supported platform, including Windows' duplicated handle.
#[test]
fn retained_plain_and_tls_sockets_are_nonblocking_after_the_handshake() {
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
        let mut retained = socket.shutdown.try_clone().unwrap();
        assert_eq!(retained.read_timeout().unwrap(), None);
        assert_eq!(retained.write_timeout().unwrap(), None);
        let idle = retained.read(&mut [0; 1]);
        assert!(
            matches!(idle, Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock),
            "idle retained socket was not nonblocking: {idle:?}"
        );
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
fn an_idle_connection_blocks_without_periodic_wakeups() {
    let (port, _seen) = peer();
    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let socket = open_on(&transport, port, "/never", &AbortSignal::default()).unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !observer.parked.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(
        observer.parked.load(Ordering::Acquire),
        "the pump never entered its readiness wait"
    );
    // The socket's one initial writable edge can arrive after open returns.
    // Let setup readiness settle before measuring the idle interval.
    std::thread::sleep(Duration::from_millis(50));
    while !observer.parked.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    let returns = observer.returns.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(150));
    assert!(observer.parked.load(Ordering::Acquire));
    assert_eq!(
        observer.returns.load(Ordering::Acquire),
        returns,
        "an idle pump returned from readiness without a command or socket event"
    );
    drop(socket);
}

#[cfg(test)]
fn idle_peer_for_exit_test() -> (u16, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        server_handshake(&mut stream);
        let mut byte = [0u8; 1];
        while stream.read(&mut byte).unwrap_or(0) != 0 {}
    });
    (port, peer)
}

#[cfg(test)]
fn wait_for_pump_exit(observer: &PumpObserver, what: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !observer.exited.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(
        observer.exited.load(Ordering::Acquire),
        "the {what} pump did not exit"
    );
}

#[test]
fn abort_wakes_and_ends_an_idle_pump() {
    let (port, peer) = idle_peer_for_exit_test();
    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let controller = AbortController::new();
    let mut socket = open_on(&transport, port, "/idle-abort", &controller.signal()).unwrap();
    controller.abort();
    wait_for_pump_exit(&observer, "aborted idle");
    assert!(socket.next().is_err(), "abort remains an error, not EOF");
    drop(socket);
    peer.join().unwrap();
}

#[test]
fn dropping_the_last_handles_ends_an_idle_pump() {
    let (port, peer) = idle_peer_for_exit_test();
    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let socket = open_on(&transport, port, "/idle-drop", &AbortSignal::default()).unwrap();
    let sender = socket.sender().unwrap();
    drop(sender);
    drop(socket);
    wait_for_pump_exit(&observer, "last-handle drop");
    peer.join().unwrap();
}

#[test]
fn a_real_peer_reset_closes_the_idle_pump_without_spinning() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (reset_now, reset_requested) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        server_handshake(&mut stream);
        reset_requested.recv().unwrap();
        socket2::SockRef::from(&stream)
            .set_linger(Some(Duration::ZERO))
            .unwrap();
    });
    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let mut socket = transport
        .open_socket(
            &url::Url::parse(&format!("ws://127.0.0.1:{port}/never")).unwrap(),
            1024,
            &AbortSignal::default(),
            &[],
        )
        .unwrap();

    reset_now.send(()).unwrap();
    let started = std::time::Instant::now();
    wait_for_pump_exit(&observer, "reset");
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(
        socket.next().unwrap(),
        Incoming::Closed {
            code: 1006,
            reason: String::new(),
        }
    );
    assert!(!observer.parked.load(Ordering::Acquire));
    peer.join().unwrap();
}

/// Guards the linger cap under a peer that keeps sending after the close
/// handshake. On loopback the pump usually out-reads the peer and so also
/// stops at a WouldBlock; the per-read deadline check in `linger_close` is
/// what bounds a peer that never lets reads block (reviewed, 2026-10-07).
#[test]
fn a_peer_flooding_after_the_close_cannot_hold_the_pump_past_its_linger_cap() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (closed_tx, closed_rx) = channel();
    let peer = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        server_handshake(&mut stream);
        // The client's masked Close: 2-byte header, 4-byte mask, short payload.
        let mut header = [0u8; 2];
        stream.read_exact(&mut header).unwrap();
        assert_eq!(header[0], 0x88, "the client sent Close");
        let mut rest = vec![0u8; 4 + usize::from(header[1] & 0x7f)];
        stream.read_exact(&mut rest).unwrap();
        closed_tx.send(()).unwrap();
        stream.write_all(&[0x88, 0x02, 0x03, 0xe8]).unwrap();
        // The client half-closes when it starts lingering; flood only then.
        let mut byte = [0u8; 1];
        while stream.read(&mut byte).unwrap_or(0) != 0 {}
        let junk = [0u8; 16 << 10];
        while stream.write_all(&junk).is_ok() {}
    });
    let observer = Arc::new(PumpObserver::default());
    let transport = TcpSocketTransport::with_pump_observer(Arc::clone(&observer));
    let socket = open_on(
        &transport,
        port,
        "/flood-after-close",
        &AbortSignal::default(),
    )
    .unwrap();
    let sender = socket.sender().unwrap();
    sender.close(Some(1000), "bye").unwrap();
    closed_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the peer received the Close");
    drop(sender);
    drop(socket);
    let deadline = std::time::Instant::now() + CLOSE_LINGER + Duration::from_secs(2);
    while !observer.exited.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        observer.exited.load(Ordering::Acquire),
        "the pump outlived its {CLOSE_LINGER:?} linger cap under a post-close flood"
    );
    peer.join().unwrap();
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
