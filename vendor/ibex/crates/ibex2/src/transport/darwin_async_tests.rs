//! The lane's tasks against a local HTTP/1.1 server.
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// One connection: reads the head (and `body` more bytes), then `handler`.
fn server(
    body: usize,
    handler: impl FnOnce(TcpStream, Vec<u8>) + Send + 'static,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        let mut payload = vec![0; body];
        stream.read_exact(&mut payload).unwrap();
        request.extend_from_slice(&payload);
        handler(stream, request);
    });
    (url, worker)
}

#[derive(Debug)]
enum Event {
    Head(u16, Option<u64>),
    Data(Vec<u8>),
    End(Result<(), HostError>),
    Metrics(HopMetrics),
}

/// A sink that reports every event and answers `head` with `flow`.
struct Recorder {
    events: mpsc::Sender<Event>,
    flow: Flow,
}

impl Sink for Recorder {
    fn head(&mut self, head: Head) -> Flow {
        let _ = self
            .events
            .send(Event::Head(head.status, head.declared_length));
        self.flow
    }
    fn data(&mut self, bytes: &[u8]) -> Flow {
        let _ = self.events.send(Event::Data(bytes.to_vec()));
        Flow::Continue
    }
    fn end(&mut self, result: Result<(), HostError>) {
        let _ = self.events.send(Event::End(result));
    }
    fn metrics(&mut self, metrics: HopMetrics) {
        let _ = self.events.send(Event::Metrics(metrics));
    }
}

fn get(url: &str) -> HopRequest {
    HopRequest {
        method: "GET".into(),
        url: url.into(),
        headers: Headers::new(),
        body: None,
        max_body: 1 << 20,
        timeout: Some(Duration::from_secs(5)),
    }
}

fn start(lane: &DarwinLane, request: HopRequest, flow: Flow) -> (Exchange, mpsc::Receiver<Event>) {
    let (events, received) = mpsc::channel();
    let exchange = lane.start(request, Box::new(Recorder { events, flow }));
    (exchange, received)
}

/// Every event until the end, then whatever else comes within a second.
fn until_end(events: &mpsc::Receiver<Event>) -> Vec<Event> {
    let mut seen = Vec::new();
    loop {
        let event = events.recv_timeout(Duration::from_secs(5)).expect("an end");
        let end = matches!(event, Event::End(_));
        seen.push(event);
        if end {
            break;
        }
    }
    while let Ok(event) = events.recv_timeout(Duration::from_millis(1200)) {
        seen.push(event);
    }
    seen
}

#[test]
fn a_response_arrives_as_head_data_end_and_metrics() {
    let (url, server) = server(0, |mut stream, _| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello")
            .unwrap();
    });
    let lane = DarwinLane::new(0);
    assert!(!lane.created(), "the session waits for the first exchange");
    let (_exchange, events) = start(&lane, get(&url), Flow::Continue);
    assert!(lane.created());
    let seen = until_end(&events);
    assert!(matches!(seen[0], Event::Head(200, Some(5))), "{seen:?}");
    let body: Vec<u8> = seen
        .iter()
        .filter_map(|e| match e {
            Event::Data(d) => Some(d.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(body, b"hello");
    assert!(seen.iter().any(|e| matches!(e, Event::End(Ok(())))));
    let metrics = seen.iter().find_map(|e| match e {
        Event::Metrics(m) => Some(m.clone()),
        _ => None,
    });
    let metrics = metrics.expect("metrics, before or after the end");
    assert_eq!(metrics.protocol.as_deref(), Some("http/1.1"));
    assert!(metrics.remote.is_some_and(|r| r.contains("127.0.0.1")));
    server.join().unwrap();
}

#[test]
fn the_platform_follows_no_redirect() {
    let (url, server) = server(0, |mut stream, _| {
        stream
            .write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/elsewhere\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
    });
    let lane = DarwinLane::new(0);
    let (_exchange, events) = start(&lane, get(&url), Flow::Skip);
    let seen = until_end(&events);
    assert!(matches!(seen[0], Event::Head(302, _)), "{seen:?}");
    server.join().unwrap();
}

#[test]
fn a_stop_at_the_head_delivers_no_body_and_still_ends() {
    let (url, server) = server(0, |mut stream, _| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\n");
        let _ = stream.write_all(&vec![b'x'; 1_000_000]);
    });
    let lane = DarwinLane::new(0);
    let (_exchange, events) = start(&lane, get(&url), Flow::Stop);
    let seen = until_end(&events);
    assert!(matches!(seen[0], Event::Head(200, Some(1_000_000))));
    assert!(
        !seen.iter().any(|e| matches!(e, Event::Data(_))),
        "{seen:?}"
    );
    assert!(seen.iter().any(|e| matches!(e, Event::End(Err(_)))));
    server.join().unwrap();
}

#[test]
fn a_cancel_ends_the_exchange() {
    let (accepted, wait) = mpsc::channel();
    let (url, server) = server(0, move |mut stream, _| {
        accepted.send(()).unwrap();
        let mut byte = [0];
        let _ = stream.read(&mut byte);
    });
    let lane = DarwinLane::new(0);
    let (exchange, events) = start(&lane, get(&url), Flow::Continue);
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    exchange.cancel();
    let seen = until_end(&events);
    assert!(
        seen.iter().any(|e| matches!(e, Event::End(Err(_)))),
        "{seen:?}"
    );
    server.join().unwrap();
}

#[test]
fn an_upload_is_lent_and_released_after_the_exchange() {
    let (url, server) = server(4, |mut stream, request| {
        assert!(
            request.ends_with(b"\r\n\r\nbody"),
            "{:?}",
            String::from_utf8_lossy(&request)
        );
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
    });
    let released = Arc::new(AtomicUsize::new(0));
    let flag = released.clone();
    let upload = Upload::new(
        b"body".to_vec(),
        Some(Box::new(move || {
            flag.fetch_add(1, Ordering::SeqCst);
        })),
    );
    let lane = DarwinLane::new(0);
    let mut request = get(&url);
    request.method = "POST".into();
    request.body = Some(upload);
    let (exchange, events) = start(&lane, request, Flow::Continue);
    let seen = until_end(&events);
    assert!(
        seen.iter().any(|e| matches!(e, Event::End(Ok(())))),
        "{seen:?}"
    );
    // The task's request holds the body until the task goes.
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(exchange);
    for _ in 0..200 {
        if released.load(Ordering::SeqCst) == 1 {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        released.load(Ordering::SeqCst),
        1,
        "the platform let the buffer go"
    );
    server.join().unwrap();
}

#[test]
fn dropping_the_lane_cancels_its_tasks() {
    let (accepted, wait) = mpsc::channel();
    let (url, server) = server(0, move |mut stream, _| {
        accepted.send(()).unwrap();
        let mut byte = [0];
        let _ = stream.read(&mut byte);
    });
    let lane = DarwinLane::new(0);
    let (_exchange, events) = start(&lane, get(&url), Flow::Continue);
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(lane);
    let seen = until_end(&events);
    assert!(
        seen.iter().any(|e| matches!(e, Event::End(Err(_)))),
        "{seen:?}"
    );
    server.join().unwrap();
}

#[test]
fn a_fetch_task_follows_a_redirect_over_the_lane() {
    use crate::grant::{Grant, GrantSet, Origin};
    use crate::stdlib::abort::AbortController;
    use crate::stdlib::fetch::Request;
    use crate::stdlib::fetch_async::{Collected, FetchEvents, FetchTask, HeaderCaps};

    let (target, second) = server(0, |mut stream, _| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .unwrap();
    });
    let location = format!("Location: {target}there\r\n");
    let (url, first) = server(0, move |mut stream, _| {
        let head = format!("HTTP/1.1 302 Found\r\n{location}Content-Length: 4\r\n\r\nskip");
        stream.write_all(head.as_bytes()).unwrap();
    });
    let port = |u: &str| {
        u.rsplit(':')
            .next()
            .unwrap()
            .trim_end_matches('/')
            .parse::<u16>()
            .unwrap()
    };
    let grants = GrantSet::none()
        .with(Grant::Fetch(Origin::new("http", "127.0.0.1", port(&url))))
        .with(Grant::Fetch(Origin::new(
            "http",
            "127.0.0.1",
            port(&target),
        )));

    struct Done(mpsc::Sender<Result<Collected, HostError>>, AtomicUsize);
    impl FetchEvents for Done {
        fn outcome(&self, result: Result<Collected, HostError>) {
            let _ = self.0.send(result);
        }
        fn hop_ended(&self) {
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (sent, outcome) = mpsc::channel();
    let events = Arc::new(Done(sent, AtomicUsize::new(0)));
    let abort = AbortController::new();
    FetchTask::start(
        Arc::new(DarwinLane::new(0)),
        Arc::new(grants),
        Request::get(&url),
        None,
        abort.signal(),
        HeaderCaps {
            bytes: 64 << 10,
            entries: 1024,
        },
        events.clone(),
    );
    let done = outcome
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(
        (done.status, &done.body[..], done.redirected),
        (200, &b"ok"[..], true)
    );
    assert_eq!(done.url, format!("{target}there"));
    assert_eq!(events.1.load(Ordering::SeqCst), 2, "both hops ended");
    first.join().unwrap();
    second.join().unwrap();
}
