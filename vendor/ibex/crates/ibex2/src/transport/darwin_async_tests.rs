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

/// Counts the drops of the sink it is part of.
struct Dropped(Arc<AtomicUsize>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// A recorder that also counts its own drops.
struct Counted {
    recorder: Recorder,
    _dropped: Dropped,
}
impl Sink for Counted {
    fn head(&mut self, head: Head) -> Flow {
        self.recorder.head(head)
    }
    fn data(&mut self, bytes: &[u8]) -> Flow {
        self.recorder.data(bytes)
    }
    fn end(&mut self, result: Result<(), HostError>) {
        self.recorder.end(result)
    }
    fn metrics(&mut self, metrics: HopMetrics) {
        self.recorder.metrics(metrics)
    }
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
    let exchange = lane
        .start(request, Box::new(Recorder { events, flow }))
        .unwrap();
    exchange.activate();
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
    assert!(metrics
        .remote
        .as_ref()
        .is_some_and(|r| r.contains("127.0.0.1")));
    // Queued from resume, not from the task's creation: small on localhost.
    assert!(
        metrics.queued_ms.is_some_and(|q| q < 1_000.0),
        "{metrics:?}"
    );
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
    // The completed task releases its buffer while both session and control
    // handle remain alive.
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
    drop(exchange);
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

#[test]
fn nothing_is_delivered_before_activation_and_a_refusal_comes_after_it() {
    let lane = DarwinLane::new(0);
    let (events, received) = mpsc::channel();
    let mut request = get("http://127.0.0.1:1/");
    request.url = "not a url \u{0}".into();
    let exchange = lane
        .start(
            request,
            Box::new(Recorder {
                events,
                flow: Flow::Continue,
            }),
        )
        .unwrap();
    assert!(received.recv_timeout(Duration::from_millis(300)).is_err());
    exchange.activate();
    let seen = until_end(&received);
    assert!(matches!(seen[0], Event::End(Err(_))), "{seen:?}");
}

#[test]
fn exchanges_on_one_lane_run_at_once() {
    let (go, wait) = mpsc::channel::<()>();
    let wait = Arc::new(std::sync::Mutex::new(wait));
    let mut servers = Vec::new();
    let lane = DarwinLane::new(0);
    let mut receivers = Vec::new();
    for _ in 0..3 {
        let (accepted, seen) = mpsc::channel();
        let wait = wait.clone();
        let (url, server) = server(0, move |mut stream, _| {
            accepted.send(()).unwrap();
            // Every server answers only once all three are connected.
            let _ = wait.lock().unwrap().recv_timeout(Duration::from_secs(5));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nx")
                .unwrap();
        });
        servers.push(server);
        let (exchange, events) = start(&lane, get(&url), Flow::Continue);
        receivers.push((exchange, events, seen));
    }
    for (_, _, seen) in &receivers {
        seen.recv_timeout(Duration::from_secs(5))
            .expect("all three connect");
    }
    for _ in 0..3 {
        go.send(()).unwrap();
    }
    for (_, events, _) in &receivers {
        assert!(until_end(events)
            .iter()
            .any(|e| matches!(e, Event::End(Ok(())))));
    }
    for server in servers {
        server.join().unwrap();
    }
}

#[test]
fn the_lane_keeps_no_cookies_and_no_cache_and_takes_its_host_limit() {
    let lane = DarwinLane::new(32);
    let (mut cookies, mut cache, mut per_host) = (-1, -1, -1);
    // SAFETY: the lane is live; the out pointers are this frame's.
    unsafe { ibex2_darwin_lane_state(lane.lane(), &mut cookies, &mut cache, &mut per_host) };
    assert_eq!((cookies, cache, per_host), (0, 0, 32));
}

#[test]
fn the_sink_is_freed_once_after_its_end_and_metrics() {
    let (url, server) = server(0, |mut stream, _| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nx")
            .unwrap();
    });
    let lane = DarwinLane::new(0);
    let drops = Arc::new(AtomicUsize::new(0));
    let (events, received) = mpsc::channel();
    let sink = Counted {
        recorder: Recorder {
            events,
            flow: Flow::Continue,
        },
        _dropped: Dropped(drops.clone()),
    };
    let exchange = lane.start(get(&url), Box::new(sink)).unwrap();
    exchange.activate();
    until_end(&received);
    // Past the second metrics get, whether or not they came.
    thread::sleep(Duration::from_millis(1200));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    server.join().unwrap();
}

#[test]
fn a_cancel_before_activation_sends_nothing_and_still_ends() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let lane = DarwinLane::new(0);
    let drops = Arc::new(AtomicUsize::new(0));
    let (events, received) = mpsc::channel();
    let sink = Counted {
        recorder: Recorder {
            events,
            flow: Flow::Continue,
        },
        _dropped: Dropped(drops.clone()),
    };
    let exchange = lane.start(get(&url), Box::new(sink)).unwrap();
    exchange.cancel();
    // Cancellation/retirement itself must deliver end, without activation.
    let seen = until_end(&received);
    assert!(
        seen.iter().any(|e| matches!(e, Event::End(Err(_)))),
        "{seen:?}"
    );
    assert!(listener.accept().is_err(), "no connection was made");
    // Past the second metrics get: the sink went once.
    thread::sleep(Duration::from_millis(1200));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn a_lane_dropped_before_activation_retires_without_activation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let lane = DarwinLane::new(0);
    let drops = Arc::new(AtomicUsize::new(0));
    let (events, received) = mpsc::channel();
    let sink = Counted {
        recorder: Recorder {
            events,
            flow: Flow::Continue,
        },
        _dropped: Dropped(drops.clone()),
    };
    let _exchange = lane.start(get(&url), Box::new(sink)).unwrap();
    drop(lane);
    // Cancellation/retirement itself must deliver end, without activation.
    let seen = until_end(&received);
    let ends = seen.iter().filter(|e| matches!(e, Event::End(_))).count();
    assert_eq!(ends, 1, "{seen:?}");
    assert!(
        !seen.iter().any(|e| matches!(e, Event::Metrics(_))),
        "{seen:?}"
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(listener.accept().is_err(), "no connection was made");
}

#[test]
fn queued_time_counts_from_activation_not_from_the_tasks_creation() {
    let (url, server) = server(0, |mut stream, _| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nx")
            .unwrap();
    });
    let lane = DarwinLane::new(0);
    let (events, received) = mpsc::channel();
    let exchange = lane
        .start(
            get(&url),
            Box::new(Recorder {
                events,
                flow: Flow::Continue,
            }),
        )
        .unwrap();
    thread::sleep(Duration::from_millis(400));
    exchange.activate();
    let seen = until_end(&received);
    let queued = seen.iter().find_map(|e| match e {
        Event::Metrics(m) => m.queued_ms,
        _ => None,
    });
    assert!(queued.is_some_and(|q| q < 300.0), "{seen:?}");
    server.join().unwrap();
}

#[test]
fn a_malformed_url_ends_once_after_activation_and_lets_its_body_go() {
    let lane = DarwinLane::new(0);
    let (drops, released) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let flag = released.clone();
    let (events, received) = mpsc::channel();
    let sink = Counted {
        recorder: Recorder {
            events,
            flow: Flow::Continue,
        },
        _dropped: Dropped(drops.clone()),
    };
    let mut request = get("http://[::1");
    request.method = "POST".into();
    request.body = Some(Upload::new(
        b"body".to_vec(),
        Some(Box::new(move || {
            flag.fetch_add(1, Ordering::SeqCst);
        })),
    ));
    let exchange = lane.start(request, Box::new(sink)).unwrap();
    assert!(received.recv_timeout(Duration::from_millis(300)).is_err());
    exchange.activate();
    let seen = until_end(&received);
    assert_eq!(
        seen.iter()
            .filter(|e| matches!(e, Event::End(Err(_))))
            .count(),
        1,
        "{seen:?}"
    );
    drop(exchange);
    for _ in 0..200 {
        if drops.load(Ordering::SeqCst) == 1 && released.load(Ordering::SeqCst) == 1 {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        (
            drops.load(Ordering::SeqCst),
            released.load(Ordering::SeqCst)
        ),
        (1, 1)
    );
}

#[test]
fn one_session_reuses_a_keep_alive_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        for _ in 0..2 {
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok",
                )
                .unwrap();
        }
    });
    let lane = DarwinLane::new(1);
    for reused in [false, true] {
        let (_, rx) = start(&lane, get(&url), Flow::Continue);
        let events = until_end(&rx);
        assert!(events
            .iter()
            .any(|event| matches!(event, Event::End(Ok(())))));
        let metrics = events
            .iter()
            .find_map(|event| match event {
                Event::Metrics(metrics) => Some(metrics),
                _ => None,
            })
            .expect("connection metrics");
        assert_eq!(metrics.reused, Some(reused));
    }
    worker.join().unwrap();
}

#[test]
fn a_native_sink_panic_is_contained_and_still_retires() {
    struct Panics(mpsc::Sender<Result<(), HostError>>, Arc<AtomicUsize>);
    impl Sink for Panics {
        fn head(&mut self, _: Head) -> Flow {
            panic!("head callback panic")
        }
        fn data(&mut self, _: &[u8]) -> Flow {
            panic!("data after panic")
        }
        fn end(&mut self, result: Result<(), HostError>) {
            self.0.send(result).unwrap();
        }
    }
    impl Drop for Panics {
        fn drop(&mut self) {
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (url, worker) = server(0, |mut stream, _| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nx")
            .unwrap();
    });
    let lane = DarwinLane::new(1);
    let (tx, rx) = mpsc::channel();
    let drops = Arc::new(AtomicUsize::new(0));
    let exchange = lane
        .start(get(&url), Box::new(Panics(tx, drops.clone())))
        .unwrap();
    exchange.activate();
    let error = rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("panicked"));
    for _ in 0..100 {
        if drops.load(Ordering::SeqCst) == 1 {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    worker.join().unwrap();
}

#[test]
fn the_native_attempt_timeout_retires_stalled_headers_and_body() {
    for body in [false, true] {
        let (url, worker) = server(0, move |mut stream, _| {
            if body {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nx")
                    .unwrap();
            }
            assert_eq!(stream.read(&mut [0]).unwrap(), 0);
        });
        let lane = DarwinLane::new(1);
        let mut request = get(&url);
        request.timeout = Some(Duration::from_millis(100));
        let (_, rx) = start(&lane, request, Flow::Continue);
        let events = until_end(&rx);
        assert!(events.iter().any(|e| matches!(e, Event::End(Err(_)))));
        worker.join().unwrap();
    }
}

#[test]
fn a_large_upload_replays_after_a_307_and_releases_with_handles_alive() {
    use crate::grant::{Grant, GrantSet};
    use crate::stdlib::abort::AbortSignal;
    use crate::stdlib::fetch::{origin_of, Request};
    use crate::stdlib::fetch_async::{Collected, FetchEvents, FetchTask, HeaderCaps};
    const LENGTH: usize = (256 << 10) + 7;
    let check = |request: Vec<u8>| {
        assert!(request.starts_with(b"POST "));
        assert_eq!(&request[request.len() - LENGTH..], vec![b'z'; LENGTH]);
    };
    let (target, second) = server(LENGTH, move |mut stream, request| {
        check(request);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .unwrap();
    });
    let redirect = target.clone();
    let (url, first) = server(LENGTH, move |mut stream, request| {
        check(request);
        stream.write_all(format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {redirect}\r\nContent-Length: 0\r\n\r\n").as_bytes()).unwrap();
    });
    struct Done(mpsc::Sender<Result<Collected, HostError>>, AtomicUsize);
    impl FetchEvents for Done {
        fn outcome(&self, result: Result<Collected, HostError>) {
            self.0.send(result).unwrap();
        }
        fn hop_ended(&self) {
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (tx, rx) = mpsc::channel();
    let events = Arc::new(Done(tx, AtomicUsize::new(0)));
    let released = Arc::new(AtomicUsize::new(0));
    let notice = released.clone();
    let upload = Upload::new(
        vec![b'z'; LENGTH],
        Some(Box::new(move || {
            notice.fetch_add(1, Ordering::SeqCst);
        })),
    );
    let grants = GrantSet::none()
        .with(Grant::Fetch(origin_of(&url).unwrap()))
        .with(Grant::Fetch(origin_of(&target).unwrap()));
    let lane = Arc::new(DarwinLane::new(1));
    let mut request = Request::get(&url);
    request.method = "POST".into();
    let task = FetchTask::start(
        lane.clone(),
        Arc::new(grants),
        request,
        Some(upload),
        AbortSignal::default(),
        HeaderCaps {
            bytes: 65536,
            entries: 128,
        },
        events.clone(),
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap()
            .body,
        b"ok"
    );
    assert_eq!(events.1.load(Ordering::SeqCst), 2);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    // Neither retained task control nor the reusable session owns upload bytes.
    drop((task, lane));
    first.join().unwrap();
    second.join().unwrap();
}

#[test]
fn cancelling_a_blocked_upload_releases_its_buffer_at_physical_end() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let (resume, wait) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut header = Vec::new();
        let mut byte = [0];
        while !header.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
        }
        tx.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut chunk = [0; 65536];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
    let released = Arc::new(AtomicUsize::new(0));
    let notice = released.clone();
    let lane = DarwinLane::new(1);
    let mut request = get(&url);
    request.method = "POST".into();
    request.body = Some(Upload::new(
        vec![0; 8 << 20],
        Some(Box::new(move || {
            notice.fetch_add(1, Ordering::SeqCst);
        })),
    ));
    let (exchange, events) = start(&lane, request, Flow::Continue);
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    exchange.cancel();
    let observed = until_end(&events);
    assert!(observed
        .iter()
        .any(|event| matches!(event, Event::End(Err(_)))));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    resume.send(()).unwrap();
    worker.join().unwrap();
}
