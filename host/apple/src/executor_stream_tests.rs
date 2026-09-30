//! The executor's scripted stream (LLP 1069.004 slices 1 and 2): a fake
//! transport whose body is fed by the test, so every message, gap, close
//! and abort is the test's to make.
use super::super::{Core, WORKERS};
use super::EventStream;
use exact_runner::{FailureKind, Message, Outcome, Request, RequestOut};
use ibex2::{
    boundary::HostError,
    stdlib::{
        abort::AbortSignal,
        fetch::{Body, BodySource, Headers, StreamingResponse, Transport},
    },
};
use std::{
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// The test's end of one open body: bytes to send, or a drop to close.
type Feed = Sender<Vec<u8>>;

struct Fed {
    rx: Receiver<Vec<u8>>,
    signal: AbortSignal,
}
impl BodySource for Fed {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, HostError> {
        loop {
            self.signal.check()?;
            match self.rx.recv_timeout(Duration::from_millis(5)) {
                Ok(bytes) => {
                    output[..bytes.len()].copy_from_slice(&bytes);
                    return Ok(bytes.len());
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
    }
}

/// `/events` opens an event stream the test feeds; `/plain` answers 404;
/// anything else answers `done` at once.
#[derive(Default)]
struct Script {
    feeds: Mutex<Vec<Option<Feed>>>,
    aborted: Arc<Mutex<usize>>,
}
struct Scripted(Arc<Script>);
impl Transport for Scripted {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        let mut headers = Headers::default();
        if request.url.ends_with("/events") {
            assert_eq!(request.headers.get("accept"), Some("text/event-stream"));
            headers.set_response("content-type", "text/event-stream; charset=utf-8");
            let (tx, rx) = channel();
            self.0.feeds.lock().unwrap().push(Some(tx));
            let aborted = self.0.aborted.clone();
            let registration = signal.register(move || *aborted.lock().unwrap() += 1);
            std::mem::forget(registration);
            return Ok(StreamingResponse {
                status: 200,
                status_text: "OK".into(),
                headers,
                body: Body::new(
                    Box::new(Fed {
                        rx,
                        signal: signal.clone(),
                    }),
                    request.body_limit(),
                    signal.clone(),
                ),
                url: request.url.clone(),
                redirected: false,
            });
        }
        // A held reply: until the test ends it (retirement aborts it).
        if request.url.ends_with("/hold") {
            let end = Instant::now() + Duration::from_secs(5);
            while !signal.aborted() && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let status = if request.url.ends_with("/plain") {
            404
        } else {
            200
        };
        Ok(ibex2::stdlib::fetch::Response {
            status,
            status_text: String::new(),
            headers,
            body: b"done".to_vec(),
            url: request.url.clone(),
            redirected: false,
        }
        .into_stream(request.body_limit(), signal.clone()))
    }
}

fn setup() -> (Core, Arc<Script>, Receiver<()>) {
    let script = Arc::new(Script::default());
    let grants = "net.fetch https://example.test";
    let owners = (0..WORKERS)
        .map(|_| {
            Some(
                ibex2::host::Host::with_transport(Box::new(Scripted(script.clone())))
                    .endow(ibex2::grant::GrantSet::parse(grants).unwrap()),
            )
        })
        .collect();
    let (wake, woke) = channel();
    let scripted = script.clone();
    let core = Core::with_owners(
        owners,
        grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
    )
    .streams_on(move || ibex2::host::Host::with_transport(Box::new(Scripted(scripted.clone()))));
    (core, script, woke)
}

fn stream(ticket: u64, path: &str, ceiling: u32) -> RequestOut {
    let exact_runner::Answer::Later(request) = exact_runner::Answer::stream(
        Request::get(&format!("https://example.test{path}")).independent_http(ceiling),
    ) else {
        unreachable!()
    };
    RequestOut {
        ticket,
        target: "progress".into(),
        request,
        forced: false,
    }
}

fn feed(script: &Script, at: usize) -> Feed {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(feed) = script
            .feeds
            .lock()
            .unwrap()
            .get_mut(at)
            .and_then(Option::take)
        {
            return feed;
        }
        assert!(Instant::now() < deadline, "stream {at} never opened");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn next(core: &Core, woke: &Receiver<()>) -> (u64, Outcome) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        core.begin_pump();
        if let Some(done) = core.drain().pop() {
            return (done.0, done.1);
        }
        woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("an outcome within five seconds");
    }
}

fn data(outcome: &Outcome) -> (&str, u32) {
    match outcome {
        Outcome::Message(m) => (m.data.as_str(), m.coalesced),
        other => panic!("expected a message, got {other:?}"),
    }
}

fn settled(core: &Core) -> bool {
    let state = core.shared.state.lock().unwrap();
    state.counts == [0, 0] && state.bytes == [0, 0] && state.running.is_empty()
}

#[test]
fn three_messages_then_a_close_and_the_worker_stays_free() {
    let (core, script, woke) = setup();
    core.run_owned(stream(1, "/events", 4096), None).unwrap();
    let tx = feed(&script, 0);
    for n in 1..=3 {
        tx.send(format!("id: {n}\ndata: {n}0\n\n").into_bytes())
            .unwrap();
        let (ticket, outcome) = next(&core, &woke);
        assert_eq!((ticket, data(&outcome)), (1, (format!("{n}0").as_str(), 0)));
        if let Outcome::Message(m) = outcome {
            assert_eq!(m.id, n.to_string(), "the cursor");
        }
    }
    // Both independent workers are free while the stream is open.
    for ticket in [2, 3] {
        core.run_owned(
            RequestOut {
                ticket,
                target: "read".into(),
                request: Request::get("https://example.test/read").independent_http(4096),
                forced: false,
            },
            None,
        )
        .unwrap();
        assert_eq!(next(&core, &woke).0, ticket);
    }
    drop(tx);
    let (ticket, end) = next(&core, &woke);
    assert_eq!(ticket, 1);
    assert!(
        matches!(&end, Outcome::Failed { kind: FailureKind::Network, message } if message == "the event stream ended"),
        "{end:?}"
    );
    assert!(settled(&core), "the stream's reservation ends with it");
}

#[test]
fn undelivered_messages_coalesce_to_the_newest() {
    let (core, script, woke) = setup();
    core.run_owned(stream(1, "/events", 4096), None).unwrap();
    let tx = feed(&script, 0);
    let burst: String = (1..=5).map(|n| format!("data: {n}\n\n")).collect();
    tx.send(burst.into_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    tx.send(b"data: 6\n\n".to_vec()).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let (_, newest) = next(&core, &woke);
    assert_eq!(data(&newest), ("6", 5), "five dropped for the sixth");
    tx.send(b"data: 7\n\n".to_vec()).unwrap();
    assert_eq!(data(&next(&core, &woke).1), ("7", 0));
    drop(tx);
    assert!(matches!(next(&core, &woke).1, Outcome::Failed { .. }));
}

#[test]
fn forgetting_a_stream_aborts_its_read_and_releases_it() {
    let (core, script, woke) = setup();
    core.run_owned(stream(1, "/events", 4096), None).unwrap();
    let tx = feed(&script, 0);
    tx.send(b"data: a\n\n".to_vec()).unwrap();
    assert_eq!(data(&next(&core, &woke).1), ("a", 0));
    tx.send(b"data: b\n\n".to_vec()).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    core.forget(|ticket| ticket != 1);
    assert_eq!(
        *script.aborted.lock().unwrap(),
        1,
        "the transport's read aborted"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(&core) {
        assert!(
            Instant::now() < deadline,
            "the forgotten stream kept its reservation"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = tx.send(b"data: c\n\n".to_vec());
    core.begin_pump();
    assert!(
        core.drain().is_empty(),
        "nothing after the forget is delivered"
    );
}

#[test]
fn a_response_that_is_not_an_event_stream_is_the_one_answer() {
    let (core, _, woke) = setup();
    core.run_owned(stream(1, "/plain", 4096), None).unwrap();
    let (_, outcome) = next(&core, &woke);
    assert!(
        matches!(&outcome, Outcome::Response(r) if r.status == 404 && r.body == b"done"),
        "{outcome:?}"
    );
    assert!(settled(&core));
}

#[test]
fn an_event_over_the_ceiling_ends_the_stream_refused() {
    let (core, script, woke) = setup();
    core.run_owned(stream(1, "/events", 8), None).unwrap();
    let tx = feed(&script, 0);
    tx.send(b"data: 0123456789\n\n".to_vec()).unwrap();
    let (_, outcome) = next(&core, &woke);
    assert!(
        matches!(
            &outcome,
            Outcome::Failed {
                kind: FailureKind::Refused,
                ..
            }
        ),
        "{outcome:?}"
    );
}

#[test]
fn a_stream_opens_while_every_independent_worker_holds_a_reply() {
    // Exact Live's wave: held replies occupy both independent workers until
    // release; the wave's progress stream must not wait behind them.
    let (core, script, woke) = setup();
    for ticket in [1, 2] {
        core.run_owned(
            RequestOut {
                ticket,
                target: "held".into(),
                request: Request::get("https://example.test/hold").independent_http(4096),
                forced: false,
            },
            None,
        )
        .unwrap();
    }
    std::thread::sleep(Duration::from_millis(20));
    core.run_owned(stream(3, "/events", 4096), None).unwrap();
    let tx = feed(&script, 0);
    tx.send(b"data: moving\n\n".to_vec()).unwrap();
    assert_eq!(
        next(&core, &woke),
        (
            3,
            Outcome::Message(Message {
                data: "moving".into(),
                ..Message::default()
            })
        )
    );
}

#[test]
fn streams_have_their_own_bound_beside_a_full_independent_lane() {
    let (core, _, _) = setup();
    // A lane of 128 undrained replies (Exact Live's wave) leaves room for
    // its progress stream, and a stream never takes a reply's slot.
    for ticket in 0..128 {
        let request = Request::get("https://example.test/read").independent_http(4096);
        core.run_owned(
            RequestOut {
                ticket,
                target: "read".into(),
                request,
                forced: false,
            },
            None,
        )
        .unwrap();
    }
    let read = |ticket| RequestOut {
        ticket,
        target: "read".into(),
        request: Request::get("https://example.test/read").independent_http(4096),
        forced: false,
    };
    assert!(core.run_owned(read(500), None).is_err(), "the lane is full");
    for ticket in 1000..1016 {
        core.run_owned(stream(ticket, "/plain", 4096), None)
            .unwrap();
    }
    assert!(
        core.run_owned(stream(2000, "/plain", 4096), None).is_err(),
        "sixteen streams at most"
    );
}

#[test]
fn a_stream_on_the_ordered_lane_is_refused_at_admission() {
    let (core, _, _) = setup();
    let mut ordered = stream(1, "/events", 4096);
    ordered.request.http = exact_runner::HttpScheduling::Ordered;
    assert!(core.run_owned(ordered, None).is_err());
}

fn parse(chunks: &[&[u8]]) -> Vec<Message> {
    let mut events = EventStream::new(1024);
    chunks
        .iter()
        .flat_map(|c| events.push(c).unwrap())
        .collect()
}

#[test]
fn the_parser_reads_lines_fields_and_cursors_as_html_does() {
    let got = parse(&[
        b"\xef\xbb\xbf: a comment\r",
        b"\nretry: 10\nevent: progress\nid: 7\ndata: {\"a\":\r\n",
        b"data: 1}\r\r",
        b"data\n\nid\ndata: x\n\n",
        b"data: no end yet",
    ]);
    assert_eq!(
        got,
        vec![
            Message {
                event: "progress".into(),
                id: "7".into(),
                data: "{\"a\":\n1}".into(),
                coalesced: 0
            },
            Message {
                id: "7".into(),
                ..Message::default()
            },
            Message {
                data: "x".into(),
                ..Message::default()
            },
        ]
    );
    // A blank line with no data dispatches nothing; `id:` persists.
    assert!(parse(&[b"event: x\n\n\n"]).is_empty());
    assert!(EventStream::new(4).push(b"data: 12345\n").is_err());
}

/// A local SSE peer on the platform transport (Darwin on Apple, the Linux
/// host's own otherwise): `/three` sends three events a little apart and
/// closes; `/hold` sends one and holds the connection until the client
/// goes, then reports the close on `closed`.
fn peer() -> (u16, Receiver<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (closed, gone) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let (mut stream, closed) = (stream.unwrap(), closed.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n: open\n\n"
                );
                let _ = stream.flush();
                if head.contains("/three") {
                    for n in 1..=3 {
                        std::thread::sleep(Duration::from_millis(40));
                        let _ = write!(stream, "id: {n}\ndata: {{\"n\":{n}}}\n\n");
                        let _ = stream.flush();
                    }
                    return;
                }
                let _ = write!(stream, "id: 1\ndata: held\n\n");
                let _ = stream.flush();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                while matches!(stream.read(&mut buf), Ok(n) if n > 0) {}
                let _ = closed.send(());
            });
        }
    });
    (port, gone)
}

fn platform(port: u16) -> (Core, Receiver<()>) {
    let (wake, woke) = channel();
    let core = Core::start(
        None,
        &format!("net.fetch http://127.0.0.1:{port}"),
        Box::new(move || {
            let _ = wake.send(());
        }),
    );
    (core, woke)
}

#[test]
fn a_local_event_stream_arrives_message_by_message_on_the_platform_transport() {
    let (port, _) = peer();
    let (core, woke) = platform(port);
    let mut out = stream(1, "/three", 4096);
    out.request.url = format!("http://127.0.0.1:{port}/three");
    core.run_owned(out, None).unwrap();
    let mut seen = vec![];
    loop {
        match next(&core, &woke).1 {
            Outcome::Message(m) => seen.push((m.id, m.data, m.coalesced)),
            Outcome::Failed { kind, message } => {
                assert_eq!(
                    (kind, message.as_str()),
                    (FailureKind::Network, "the event stream ended")
                );
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    let ids: Vec<_> = seen.iter().map(|(id, ..)| id.as_str()).collect();
    assert_eq!(ids, ["1", "2", "3"], "{seen:?}");
    assert!(settled(&core));
}

#[test]
fn forgetting_a_platform_stream_closes_its_connection() {
    let (port, gone) = peer();
    let (core, woke) = platform(port);
    let mut out = stream(1, "/hold", 4096);
    out.request.url = format!("http://127.0.0.1:{port}/hold");
    core.run_owned(out, None).unwrap();
    assert_eq!(data(&next(&core, &woke).1), ("held", 0));
    core.forget(|_| false);
    gone.recv_timeout(Duration::from_secs(5))
        .expect("the server saw the connection close");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(&core) {
        assert!(Instant::now() < deadline, "the reservation was kept");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A local WebSocket peer (LLP 1069.004 slice 3): `/three` sends three text
/// frames a little apart then closes; `/binary` sends a binary frame;
/// `/hold` sends one and reports on `gone` when the client goes.
fn socket_peer() -> (u16, Receiver<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (closed, gone) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let (mut s, closed) = (stream.unwrap(), closed.clone());
            std::thread::spawn(move || {
                let (mut head, mut byte) = (Vec::new(), [0u8; 1]);
                while !head.ends_with(b"\r\n\r\n") && s.read(&mut byte).unwrap_or(0) == 1 {
                    head.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&head).to_string();
                let key = head
                    .lines()
                    .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
                    .unwrap_or("")
                    .trim();
                let accept = ibex2::stdlib::websocket::accept_key(key);
                let _ = write!(s, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n");
                let frame = |opcode: u8, payload: &[u8]| {
                    let mut f = vec![0x80 | opcode, payload.len() as u8];
                    f.extend_from_slice(payload);
                    f
                };
                if head.contains("/three") {
                    for n in 1..=3 {
                        std::thread::sleep(Duration::from_millis(40));
                        let _ = s.write_all(&frame(1, format!("{{\"n\":{n}}}").as_bytes()));
                    }
                    let _ = s.write_all(&frame(8, &1000u16.to_be_bytes()));
                } else if head.contains("/binary") {
                    let _ = s.write_all(&frame(2, &[1, 2, 3]));
                } else {
                    let _ = s.write_all(&frame(1, b"held"));
                }
                s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                let mut buf = [0u8; 256];
                while matches!(s.read(&mut buf), Ok(n) if n > 0) {}
                let _ = closed.send(());
            });
        }
    });
    (port, gone)
}

fn socket(port: u16, path: &str) -> RequestOut {
    let mut out = stream(1, path, 4096);
    out.request.url = format!("ws://127.0.0.1:{port}{path}");
    out
}

fn socket_platform(port: u16, grants: &str) -> (Core, Receiver<()>) {
    let (wake, woke) = channel();
    let grants = grants.replace("{port}", &port.to_string());
    let core = Core::start(
        None,
        &grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
    );
    (core, woke)
}

#[test]
fn a_local_socket_arrives_message_by_message_and_its_close_ends_it() {
    let (port, _) = socket_peer();
    let (core, woke) = socket_platform(port, "net.websocket ws://127.0.0.1:{port}");
    core.run_owned(socket(port, "/three"), None).unwrap();
    let mut seen = vec![];
    loop {
        match next(&core, &woke).1 {
            Outcome::Message(m) => {
                assert_eq!((m.event.as_str(), m.id.as_str()), ("", ""));
                seen.push(m.data);
            }
            Outcome::Failed { kind, message } => {
                assert_eq!(
                    (kind, message.as_str()),
                    (FailureKind::Network, "the socket closed (1000)")
                );
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(seen, [r#"{"n":1}"#, r#"{"n":2}"#, r#"{"n":3}"#]);
    assert!(settled(&core));
}

#[test]
fn a_socket_needs_its_own_grant_and_takes_only_text() {
    let (port, _) = socket_peer();
    // A fetch grant for the same origin admits no socket.
    let (core, woke) = socket_platform(port, "net.fetch http://127.0.0.1:{port}");
    core.run_owned(socket(port, "/three"), None).unwrap();
    assert!(matches!(
        next(&core, &woke).1,
        Outcome::Failed { kind: FailureKind::Refused, ref message } if message.contains("net.websocket")
    ));
    let (core, woke) = socket_platform(port, "net.websocket ws://127.0.0.1:{port}");
    core.run_owned(socket(port, "/binary"), None).unwrap();
    assert!(matches!(
        next(&core, &woke).1,
        Outcome::Failed { kind: FailureKind::Refused, ref message } if message.contains("binary")
    ));
    assert!(settled(&core));
}

#[test]
fn forgetting_a_socket_closes_its_connection() {
    let (port, gone) = socket_peer();
    let (core, woke) = socket_platform(port, "net.websocket ws://127.0.0.1:{port}");
    core.run_owned(socket(port, "/hold"), None).unwrap();
    assert_eq!(data(&next(&core, &woke).1), ("held", 0));
    core.forget(|_| false);
    gone.recv_timeout(Duration::from_secs(5))
        .expect("the server saw the socket close");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(&core) {
        assert!(Instant::now() < deadline, "the reservation was kept");
        std::thread::sleep(Duration::from_millis(5));
    }
}
