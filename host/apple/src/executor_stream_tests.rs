//! The executor's scripted stream (LLP 1069.004 slices 1 and 2): a fake
//! transport whose body is fed by the test, so every message, gap, close
//! and abort is the test's to make.
use super::super::{Core, INDEPENDENT, WORKERS};
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

/// `/events` opens an event stream the test feeds, `/bytes` a binary body
/// it feeds; `/plain` answers 404, `/empty` 204; anything else answers
/// `done` at once.
#[derive(Default)]
struct Script {
    feeds: Mutex<Vec<Option<Feed>>>,
    aborted: Arc<Mutex<usize>>,
    /// Told when `/upload-burst` has reported all its bytes sent.
    reported: Mutex<Option<Sender<()>>>,
    /// The platform timeout each `/upload-timed` was given.
    timeouts: Mutex<Vec<Option<Duration>>>,
    /// `/upload`s going out now, and the most at once.
    going: Mutex<(usize, usize)>,
}
struct Scripted(Arc<Script>);
impl Transport for Scripted {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        let mut headers = Headers::default();
        let bytes = request.url.ends_with("/bytes");
        if request.url.ends_with("/events") || bytes {
            assert_eq!(request.headers.get("accept"), Some("text/event-stream"));
            let kind = if bytes {
                "application/octet-stream"
            } else {
                "text/event-stream; charset=utf-8"
            };
            headers.set_response("content-type", kind);
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
        // An upload (LLP 1016.000 D10), reporting its bytes sent as a
        // platform does: `/upload` a quarter, a half and the whole, each when
        // the test feeds a step; `-burst` all three at once, then its reply
        // when fed; `-fails` a half, then a network failure; `-empty` none;
        // `-timed` none, keeping the platform timeout it was given. Each
        // replies JSON, which is no event stream.
        if let Some(kind) = request
            .url
            .rsplit('/')
            .next()
            .filter(|k| k.starts_with("upload"))
        {
            assert_eq!(
                request.headers.get("accept"),
                None,
                "an upload asks for no event stream"
            );
            let sent = request.sent.clone().expect("an upload's hook");
            let total = request.body.as_ref().map_or(0, Vec::len) as u64;
            let aborted = self.0.aborted.clone();
            std::mem::forget(signal.register(move || *aborted.lock().unwrap() += 1));
            let step = |rx: &Receiver<Vec<u8>>| {
                rx.recv()
                    .map_err(|_| HostError::Failed("the test let go".into()))
            };
            let fed = || {
                let (tx, rx) = channel();
                self.0.feeds.lock().unwrap().push(Some(tx));
                rx
            };
            match kind {
                "upload" => {
                    let rx = fed();
                    {
                        let mut going = self.0.going.lock().unwrap();
                        going.0 += 1;
                        going.1 = going.1.max(going.0);
                    }
                    let steps = (|| {
                        for loaded in [total / 4, total / 2, total] {
                            step(&rx)?;
                            (sent.0)(loaded, total);
                        }
                        Ok(())
                    })();
                    self.0.going.lock().unwrap().0 -= 1;
                    steps?;
                }
                "upload-burst" => {
                    let rx = fed();
                    for loaded in [total / 4, total / 2, total] {
                        (sent.0)(loaded, total);
                    }
                    let _ = self.0.reported.lock().unwrap().take().map(|tx| tx.send(()));
                    step(&rx)?;
                }
                "upload-fails" => {
                    (sent.0)(total / 2, total);
                    return Err(HostError::Failed(
                        "TypeError: Failed to fetch — the connection was reset".into(),
                    ));
                }
                "upload-timed" => self.0.timeouts.lock().unwrap().push(request.timeout),
                _ => assert_eq!(total, 0, "{kind}"),
            }
            headers.set_response("content-type", "application/json");
            return Ok(ibex2::stdlib::fetch::Response {
                status: 201,
                status_text: String::new(),
                headers,
                body: br#"{"id":"a1"}"#.to_vec(),
                url: request.url.clone(),
                redirected: false,
            }
            .into_stream(request.body_limit(), signal.clone()));
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
        } else if request.url.ends_with("/empty") {
            204
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
    setup_with("net.fetch https://example.test")
}

fn setup_with(grants: &str) -> (Core, Arc<Script>, Receiver<()>) {
    let script = Arc::new(Script::default());
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

/// An upload (LLP 1016.000 D10): a POST stream of `size` bytes.
fn upload(ticket: u64, path: &str, size: usize) -> RequestOut {
    let mut request = Request::get(&format!("https://example.test{path}"))
        .independent_http(4096)
        .upload_progress();
    request.method = "POST".into();
    request.body = vec![7; size];
    let exact_runner::Answer::Later(request) = exact_runner::Answer::stream(request) else {
        unreachable!()
    };
    RequestOut {
        ticket,
        target: "upload".into(),
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

/// The next outcome but an upload's start (`UPLOAD_SENDING`), which comes
/// before its transport opens and a later message may fold in (D4).
fn after_start(core: &Core, woke: &Receiver<()>) -> (u64, Outcome) {
    loop {
        match next(core, woke) {
            (_, Outcome::Message(m)) if m.upload_started() => {}
            other => return other,
        }
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
    // The terminal outcome can drain before the async transport's physical
    // end is processed. Its reservation belongs to that later event.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(&core) {
        assert!(
            Instant::now() < deadline,
            "the ended stream kept its reservation"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "async lane: waits 50 ms of wall clock for the reader to fold each message; bun scripts/async.mjs runs it"]
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
    // Exact Live's wave: held replies occupy every independent worker until
    // release; the wave's progress stream must not wait behind them.
    let (core, script, woke) = setup();
    for ticket in 1..=INDEPENDENT as u64 {
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
    core.run_owned(stream(100, "/events", 4096), None).unwrap();
    let tx = feed(&script, 0);
    tx.send(b"data: moving\n\n".to_vec()).unwrap();
    assert_eq!(
        next(&core, &woke),
        (
            100,
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
                coalesced: 0,
                binary: None,
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
#[ignore = "async lane: a real socket under the platform transport (URLSession), timing-sensitive on a loaded machine; bun scripts/async.mjs runs it"]
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
#[ignore = "async lane: a real socket under the platform transport (URLSession), timing-sensitive on a loaded machine; bun scripts/async.mjs runs it"]
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
#[ignore = "async lane: a real socket under the platform transport (URLSession), timing-sensitive on a loaded machine; bun scripts/async.mjs runs it"]
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
#[ignore = "async lane: a real socket under the platform transport (URLSession), timing-sensitive on a loaded machine; bun scripts/async.mjs runs it"]
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
#[ignore = "async lane: a real socket under the platform transport (URLSession), timing-sensitive on a loaded machine; bun scripts/async.mjs runs it"]
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

/// A TypeScript stream's request: a body that is not events is bytes.
fn bytes_stream(ticket: u64, path: &str, ceiling: u32) -> RequestOut {
    let mut out = stream(ticket, path, ceiling);
    out.request.bytes = true;
    out
}

/// Waits until the reader has queued `n` undelivered bytes for `ticket`.
fn queued(core: &Core, ticket: u64, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let waiting: usize = {
            let state = core.shared.state.lock().unwrap();
            (state.completed.iter().flatten())
                .filter(|done| done.ticket == ticket && done.message)
                .map(|done| match &done.outcome {
                    Outcome::Message(m) => m.binary.as_ref().map_or(0, Vec::len),
                    _ => 0,
                })
                .sum()
        };
        if waiting == n {
            return;
        }
        assert!(Instant::now() < deadline, "{waiting} of {n} bytes queued");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn binary(outcome: &Outcome) -> &[u8] {
    match outcome {
        Outcome::Message(m) => m.binary.as_deref().expect("a binary message"),
        other => panic!("expected a message, got {other:?}"),
    }
}

#[test]
fn a_byte_stream_joins_reads_until_the_runner_takes_them() {
    let (core, script, woke) = setup();
    core.run_owned(bytes_stream(1, "/bytes", 4096), None)
        .unwrap();
    let tx = feed(&script, 0);
    for chunk in [&b"\x00\x01"[..], b"\x02", b"\x03\x04\x05"] {
        tx.send(chunk.to_vec()).unwrap();
    }
    queued(&core, 1, 6);
    let (ticket, first) = next(&core, &woke);
    assert_eq!(
        (ticket, binary(&first)),
        (1, &[0, 1, 2, 3, 4, 5][..]),
        "three reads, one message, in order"
    );
    tx.send(vec![6, 7]).unwrap();
    assert_eq!(binary(&next(&core, &woke).1), [6, 7]);
    drop(tx);
    let (_, end) = next(&core, &woke);
    assert!(
        matches!(&end, Outcome::Failed { kind: FailureKind::Network, message } if message == "the byte stream ended"),
        "{end:?}"
    );
    assert!(settled(&core), "the stream's reservation ends with it");
}

#[test]
fn a_byte_stream_past_its_bound_delivers_what_waits_then_ends_refused() {
    let (core, script, woke) = setup();
    core.run_owned(bytes_stream(1, "/bytes", 8), None).unwrap();
    let tx = feed(&script, 0);
    tx.send(vec![1; 5]).unwrap();
    queued(&core, 1, 5);
    tx.send(vec![2; 5]).unwrap();
    // The reader ends the stream once the second read would pass the bound.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !core.shared.state.lock().unwrap().running.is_empty() {
        assert!(Instant::now() < deadline, "the stream did not end");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        binary(&next(&core, &woke).1),
        [1; 5],
        "the queued bytes first"
    );
    let (_, end) = next(&core, &woke);
    assert!(
        matches!(&end, Outcome::Failed { kind: FailureKind::Refused, message } if message == "byte stream receive queue exceeds the response ceiling"),
        "{end:?}"
    );
}

#[test]
fn a_byte_stream_that_is_not_2xx_and_a_rust_sources_body_are_one_answer() {
    let (core, _, woke) = setup();
    core.run_owned(bytes_stream(1, "/plain", 4096), None)
        .unwrap();
    let (_, outcome) = next(&core, &woke);
    assert!(
        matches!(&outcome, Outcome::Response(r) if r.status == 404),
        "{outcome:?}"
    );
    core.run_owned(stream(2, "/other", 4096), None).unwrap();
    let (_, outcome) = next(&core, &woke);
    assert!(
        matches!(&outcome, Outcome::Response(r) if r.status == 200 && r.body == b"done"),
        "{outcome:?}"
    );
    assert!(settled(&core));
}

#[test]
fn a_byte_stream_with_no_body_is_one_answer_as_on_the_web() {
    let (core, _, woke) = setup();
    core.run_owned(bytes_stream(1, "/empty", 4096), None)
        .unwrap();
    let (_, outcome) = next(&core, &woke);
    assert!(
        matches!(&outcome, Outcome::Response(r) if r.status == 204),
        "{outcome:?}"
    );
    let mut head = bytes_stream(2, "/other", 4096);
    head.request.method = "HEAD".into();
    core.run_owned(head, None).unwrap();
    let (_, outcome) = next(&core, &woke);
    assert!(
        matches!(&outcome, Outcome::Response(r) if r.status == 200),
        "{outcome:?}"
    );
    assert!(settled(&core));
}

#[test]
#[ignore = "async lane: waits 50 ms of wall clock for the reader to fold each message; bun scripts/async.mjs runs it"]
fn a_server_event_named_as_the_long_poll_reply_is_an_ordinary_event() {
    let (core, script, woke) = setup();
    core.run_owned(stream(1, "/events", 4096), None).unwrap();
    let tx = feed(&script, 0);
    tx.send(b"event: exact:reply\ndata: 1\n\n".to_vec())
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    tx.send(b"data: 2\n\n".to_vec()).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    // Folded as any two events: the newest lands, counting the one it replaced.
    assert_eq!(data(&next(&core, &woke).1), ("2", 1));
    drop(tx);
    assert!(matches!(next(&core, &woke).1, Outcome::Failed { .. }));
}

#[test]
fn an_upload_tells_its_bytes_sent_then_its_reply_ends_it() {
    let (core, script, woke) = setup();
    core.run_owned(upload(1, "/upload", 4000), None).unwrap();
    let step = feed(&script, 0);
    // It said it was going out before its transport opened.
    assert!(matches!(next(&core, &woke), (1, Outcome::Message(m)) if m.upload_started()));
    for loaded in [1000, 2000, 4000] {
        step.send(Vec::new()).unwrap();
        let (ticket, outcome) = next(&core, &woke);
        match outcome {
            Outcome::Message(m) => assert_eq!((ticket, m.sent()), (1, Some((loaded, 4000)))),
            other => panic!("expected the bytes sent, got {other:?}"),
        }
    }
    // The reply, read whole though it is a 2xx that is no event stream.
    let (ticket, end) = next(&core, &woke);
    assert_eq!(ticket, 1);
    assert!(
        matches!(&end, Outcome::Response(r) if r.status == 201 && r.body == br#"{"id":"a1"}"#),
        "{end:?}"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(&core) {
        assert!(Instant::now() < deadline, "the upload kept its reservation");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn an_upload_is_refused_where_it_could_send_no_body() {
    let (core, _script, _woke) = setup();
    let mut get = upload(1, "/upload", 4000);
    get.request.method = "GET".into();
    get.request.body.clear();
    assert_eq!(
        core.run_owned(get, None),
        Err("exactUploadProgress: a GET or HEAD request sends no body")
    );
}

#[test]
#[ignore = "async lane: waits 50 ms of wall clock for an upload's deadline; bun scripts/async.mjs runs it"]
fn an_uploads_deadline_ends_it_as_a_timeout() {
    let (core, _script, woke) = setup();
    let mut held = upload(1, "/hold", 4000);
    held.request.timeout_ms = Some(50);
    core.run_owned(held, None).unwrap();
    let (_, end) = after_start(&core, &woke);
    assert!(
        matches!(&end, Outcome::Failed { kind: FailureKind::Timeout, message } if message == "the request timed out after 50 ms"),
        "{end:?}"
    );
}

#[test]
fn an_uploads_deadline_reaches_the_platform_a_second_later() {
    let (core, script, woke) = setup();
    let mut timed = upload(1, "/upload-timed", 4000);
    timed.request.timeout_ms = Some(60_000);
    core.run_owned(timed, None).unwrap();
    assert!(matches!(after_start(&core, &woke).1, Outcome::Response(r) if r.status == 201));
    core.run_owned(upload(2, "/upload-timed", 4000), None)
        .unwrap();
    assert!(matches!(after_start(&core, &woke).1, Outcome::Response(r) if r.status == 201));
    assert_eq!(
        *script.timeouts.lock().unwrap(),
        [Some(Duration::from_millis(61_000)), None],
        "the deadline's second later, else the platform's own"
    );
}

#[test]
fn an_uploads_bytes_sent_coalesce_while_the_runner_has_not_taken_them() {
    let (core, script, woke) = setup();
    let (tx, reported) = channel();
    *script.reported.lock().unwrap() = Some(tx);
    core.run_owned(upload(1, "/upload-burst", 4000), None)
        .unwrap();
    reported
        .recv_timeout(Duration::from_secs(5))
        .expect("the bytes sent were reported");
    // The start is a message of its own; the first two counts fold into the newest.
    match next(&core, &woke).1 {
        Outcome::Message(m) => assert!(m.upload_started() && m.coalesced == 0, "{m:?}"),
        other => panic!("expected the upload's start, got {other:?}"),
    }
    match next(&core, &woke).1 {
        Outcome::Message(m) => assert_eq!((m.sent(), m.coalesced), (Some((4000, 4000)), 2)),
        other => panic!("expected the newest bytes sent, got {other:?}"),
    }
    feed(&script, 0).send(Vec::new()).unwrap();
    assert!(matches!(next(&core, &woke).1, Outcome::Response(r) if r.status == 201));
}

#[test]
fn an_upload_that_fails_midway_ends_with_its_failure() {
    let (core, _script, woke) = setup();
    core.run_owned(upload(1, "/upload-fails", 4000), None)
        .unwrap();
    match after_start(&core, &woke).1 {
        Outcome::Message(m) => assert_eq!(m.sent(), Some((2000, 4000))),
        other => panic!("expected the bytes sent, got {other:?}"),
    }
    let (_, end) = next(&core, &woke);
    assert!(
        matches!(&end, Outcome::Failed { kind: FailureKind::Network, message } if message.contains("the connection was reset")),
        "{end:?}"
    );
}

#[test]
fn letting_go_of_an_upload_midway_aborts_it() {
    let (core, script, woke) = setup();
    core.run_owned(upload(1, "/upload", 4000), None).unwrap();
    let step = feed(&script, 0);
    step.send(Vec::new()).unwrap();
    assert!(matches!(next(&core, &woke).1, Outcome::Message(_)));
    core.forget(|_| false);
    assert_eq!(
        *script.aborted.lock().unwrap(),
        1,
        "the transport saw the abort"
    );
    drop(step);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(&core) {
        assert!(
            Instant::now() < deadline,
            "the let-go upload kept its reservation"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn an_empty_upload_tells_no_progress_and_ends_with_its_reply() {
    let (core, _script, woke) = setup();
    core.run_owned(upload(1, "/upload-empty", 0), None).unwrap();
    assert!(matches!(next(&core, &woke), (1, Outcome::Message(m)) if m.upload_started()));
    assert!(matches!(next(&core, &woke).1, Outcome::Response(r) if r.status == 201));
}

/// LLP 1016.000 D10: two bodies whose copies pass the test's 1 MiB budget
/// together; the second waits, unopened, until the first's exchange ends.
#[test]
fn an_upload_waits_for_room_in_its_executors_budget() {
    let (core, script, woke) = setup();
    core.shared.uploads.limit(1 << 20);
    core.run_owned(upload(1, "/upload", 600 << 10), None)
        .unwrap();
    let first = feed(&script, 0);
    core.run_owned(upload(2, "/upload", 600 << 10), None)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while core.shared.uploads.waiting().0 == 0 {
        assert!(Instant::now() < deadline, "the second upload never waited");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        script.feeds.lock().unwrap().len(),
        1,
        "the second waits unopened"
    );
    let finish = |step: Sender<Vec<u8>>, ticket: u64| {
        for _ in 0..3 {
            step.send(Vec::new()).unwrap();
        }
        while !matches!(next(&core, &woke), (t, Outcome::Response(_)) if t == ticket) {}
    };
    finish(first, 1);
    finish(feed(&script, 1), 2);
}

/// App directories under the system's temporary one, with one file in
/// `app:/tmp`; removed when dropped.
struct AppFiles(std::path::PathBuf);
impl AppFiles {
    fn new(name: &str, bytes: &[u8]) -> AppFiles {
        let root = std::env::temp_dir().join(format!(
            "exact-upload-{}-{}",
            std::process::id(),
            ibex2::stdlib::crypto::random_uuid().unwrap()
        ));
        for dir in ["data", "cache", "tmp"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("tmp").join(name), bytes).unwrap();
        AppFiles(root)
    }
    fn roots(&self) -> [std::path::PathBuf; 3] {
        ["data", "cache", "tmp"].map(|dir| self.0.join(dir))
    }
}
impl Drop for AppFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An upload of `app:/tmp/<name>`.
fn file_upload(ticket: u64, path: &str, name: &str) -> RequestOut {
    let mut out = upload(ticket, path, 0);
    out.request.body_from = Some(format!("app:/tmp/{name}"));
    out
}

/// LLP 1016.000 D10: a file's room is the most a file body may be until it
/// is read, then its length; a second file waits for room before it is read.
#[test]
fn an_uploads_file_is_read_once_it_has_room() {
    let (core, script, woke) = setup_with("net.fetch https://example.test\nfs.read app:/tmp");
    core.shared.uploads.limit(1 << 20);
    let files = AppFiles::new("a.bin", &[7; 4096]);
    core.set_app_roots(files.roots());
    core.run_owned(file_upload(1, "/upload", "a.bin"), None)
        .unwrap();
    let first = feed(&script, 0);
    assert_eq!(
        core.shared.uploads.waiting(),
        (0, 4096 * super::COPIES),
        "held at its length"
    );
    core.run_owned(file_upload(2, "/upload", "a.bin"), None)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while core.shared.uploads.waiting().0 == 0 {
        assert!(Instant::now() < deadline, "the second upload never waited");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        script.feeds.lock().unwrap().len(),
        1,
        "the second waits unopened"
    );
    let finish = |step: Sender<Vec<u8>>, ticket: u64| {
        for _ in 0..3 {
            step.send(Vec::new()).unwrap();
        }
        while !matches!(next(&core, &woke), (t, Outcome::Response(_)) if t == ticket) {}
    };
    finish(first, 1);
    finish(feed(&script, 1), 2);
    assert_eq!(
        core.shared.uploads.waiting(),
        (0, 0),
        "every place given back"
    );
}

/// LLP 1016.000 D10: a burst of uploads at a photo's size, in the budget a
/// device has, goes three at a time, as upstream T3 Code sends them, and
/// leaves the app's other streams their places: an event stream opened
/// before twenty 2 MB uploads and one opened after them both open at once,
/// and every upload ends with its reply.
#[test]
fn a_burst_of_photo_uploads_goes_three_at_a_time_and_leaves_other_streams_their_places() {
    let (core, script, woke) = setup();
    core.run_owned(stream(100, "/events", 1024), None).unwrap();
    let before = feed(&script, 0);
    for ticket in 1..=20 {
        core.run_owned(upload(ticket, "/upload", 2 << 20), None)
            .unwrap();
    }
    let going: Vec<Feed> = (1..=3).map(|at| feed(&script, at)).collect();
    core.run_owned(stream(101, "/events", 1024), None)
        .expect("a stream after the burst has its place");
    let after = feed(&script, 4);
    assert_eq!(
        core.shared.uploads.waiting(),
        (17, 3 * (2 << 20) * super::COPIES)
    );
    for (fed, ticket) in [(&before, 100), (&after, 101)] {
        fed.send(format!("id: {ticket}\ndata: {ticket}\n\n").into_bytes())
            .unwrap();
        while !matches!(next(&core, &woke), (t, Outcome::Message(_)) if t == ticket) {}
    }
    // Feed each upload as it opens, whatever its order, until all twenty replied.
    let (mut fed, mut replies) = (going, 0);
    let mut opened = 5;
    while replies < 20 {
        for step in fed.drain(..) {
            for _ in 0..3 {
                step.send(Vec::new()).unwrap();
            }
        }
        if let (_, Outcome::Response(_)) = next(&core, &woke) {
            replies += 1;
        }
        while let Some(more) = script
            .feeds
            .lock()
            .unwrap()
            .get_mut(opened)
            .and_then(Option::take)
        {
            fed.push(more);
            opened += 1;
        }
    }
    assert_eq!(
        script.going.lock().unwrap().1,
        3,
        "never more than three at once"
    );
    assert_eq!(core.shared.uploads.waiting(), (0, 0));
}

/// The executor records each upload that has begun to go out, and tells
/// the host which of those the runner let go (`let_go_started`); one let go
/// while it waits in the queue never started (LLP 1016.000 D10).
#[test]
fn the_uploads_let_go_that_had_started_are_told_once() {
    let (core, script, woke) = setup();
    for ticket in 1..=4 {
        core.run_owned(upload(ticket, "/upload", 4096), None)
            .unwrap();
    }
    let _going: Vec<Feed> = (0..3).map(|at| feed(&script, at)).collect();
    let mut started: Vec<u64> = (0..3)
        .map(|_| match next(&core, &woke) {
            (ticket, Outcome::Message(m)) if m.upload_started() => ticket,
            other => panic!("expected an upload's start, got {other:?}"),
        })
        .collect();
    started.sort_unstable();
    assert_eq!(core.shared.uploads.waiting().0, 1, "the fourth waits");
    assert_eq!(
        core.let_go_started(|_| true),
        Vec::<u64>::new(),
        "none let go yet"
    );
    core.forget(|_| false);
    let mut told = core.let_go_started(|_| false);
    told.sort_unstable();
    assert_eq!(told, started);
    assert!(!started.contains(&4));
    assert_eq!(
        core.let_go_started(|_| false),
        Vec::<u64>::new(),
        "told once"
    );
}

/// An upload the grants do not admit is refused before it starts: no
/// `exact:sending`, so the runner marks it as no upload asked, and
/// `refresh` asks it again (LLP 1016.000 D10, local patch 23).
#[test]
fn an_upload_the_grants_refuse_never_starts() {
    let (core, _script, woke) = setup();
    let mut away = upload(1, "/upload", 4096);
    away.request.url = "https://elsewhere.test/upload".into();
    core.run_owned(away, None).unwrap();
    let (ticket, end) = next(&core, &woke);
    assert_eq!(ticket, 1);
    assert!(
        matches!(
            &end,
            Outcome::Failed {
                kind: FailureKind::Refused,
                ..
            }
        ),
        "{end:?}"
    );
    assert!(core.let_go_started(|_| false).is_empty());
}

#[test]
fn a_reply_that_lands_as_the_deadline_passes_stands() {
    use super::timed_out;
    let reply = Outcome::Response(exact_runner::Response {
        status: 201,
        headers: Vec::new(),
        body: Vec::new(),
    });
    assert!(matches!(timed_out(reply, true, 50), Outcome::Response(r) if r.status == 201));
    let aborted = Outcome::Failed {
        kind: FailureKind::Aborted,
        message: "native request aborted".into(),
    };
    assert!(matches!(
        timed_out(aborted.clone(), true, 50),
        Outcome::Failed { kind: FailureKind::Timeout, message } if message == "the request timed out after 50 ms"
    ));
    assert_eq!(timed_out(aborted.clone(), false, 50), aborted);
}

/// URLSession end to end (local patch 23): a 16 MiB body from an app file
/// (more than loopback's socket buffers take at once) a loopback server reads
/// 256 KiB at a time, then answers 62 s later, past URLSession's 60 s without
/// data, which the upload's deadline a second later lifts.
#[test]
#[ignore = "async lane: a real socket under the platform transport (URLSession), and a reply 62 s after its body; bun scripts/async.mjs runs it"]
fn an_upload_through_urlsession_tells_its_progress_and_waits_out_a_slow_reply() {
    use std::io::{BufRead, BufReader, Read, Write};
    const SIZE: usize = 16 << 20;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = n.trim().parse().unwrap();
            }
        }
        let (mut took, mut chunk) = (0, vec![0; 256 << 10]);
        while took < length {
            let n = reader
                .read(&mut chunk[..(length - took).min(256 << 10)])
                .unwrap();
            took += n;
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_secs(62));
        let mut stream = stream;
        let _ = stream
            .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    });
    let files = AppFiles::new("big.bin", &vec![7; SIZE]);
    let (wake, woke) = channel();
    let grants = format!("net.fetch http://127.0.0.1:{port}\nfs.read app:/tmp");
    let core = Core::start(
        None,
        &grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
    );
    core.set_app_roots(files.roots());
    let mut out = file_upload(1, "/upload", "big.bin");
    out.request.url = format!("http://127.0.0.1:{port}/upload");
    out.request.timeout_ms = Some(120_000);
    core.run_owned(out, None).unwrap();
    let (mut sent, deadline) = (Vec::new(), Instant::now() + Duration::from_secs(130));
    let end = loop {
        core.begin_pump();
        match core.drain().pop() {
            Some((_, Outcome::Message(m), _)) if m.upload_started() => {}
            Some((_, Outcome::Message(m), _)) => sent.push(m.sent().expect("an upload's progress")),
            Some((_, end, _)) => break end,
            None => {
                assert!(Instant::now() < deadline, "no reply after {sent:?}");
                let _ = woke.recv_timeout(Duration::from_millis(100));
            }
        }
    };
    assert!(
        matches!(&end, Outcome::Response(r) if r.status == 201 && r.body == b"ok"),
        "{end:?}"
    );
    let total = SIZE as u64;
    assert!(sent.iter().all(|&(_, t)| t == total), "{sent:?}");
    assert!(sent.windows(2).all(|w| w[0].0 <= w[1].0), "{sent:?}");
    assert!(
        sent.iter().any(|&(n, _)| n < total),
        "nothing before the whole: {sent:?}"
    );
    assert_eq!(sent.last(), Some(&(total, total)));
}
