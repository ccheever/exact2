//! `FetchTask` over a scripted transport: each started exchange waits in a
//! queue, and the test delivers its events by hand, in order.
use super::*;
use crate::grant::{Grant, Origin};
use crate::stdlib::abort::AbortController;
use std::sync::atomic::{AtomicUsize, Ordering};

/// One exchange the task started: what it sent, its sink, and whether it
/// was cancelled.
struct Started {
    request: HopRequest,
    sink: Box<dyn Sink>,
    /// "activate" and "cancel", in the order the task called them.
    calls: Arc<Mutex<Vec<&'static str>>>,
}

#[derive(Default)]
struct Scripted {
    started: Mutex<Vec<Started>>,
    /// Run inside `start`, as a caller's re-entrant abort would.
    on_start: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl AsyncTransport for Scripted {
    fn start(&self, request: HopRequest, sink: Box<dyn Sink>) -> Result<Exchange, HostError> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (on, off) = (calls.clone(), calls.clone());
        self.started.lock().unwrap().push(Started {
            request,
            sink,
            calls,
        });
        if let Some(hook) = self.on_start.lock().unwrap().take() {
            hook();
        }
        Ok(Exchange::new(
            move || on.lock().unwrap().push("activate"),
            move || off.lock().unwrap().push("cancel"),
        ))
    }
}

impl Scripted {
    /// The `n`th exchange started, taken out to drive.
    fn take(&self, n: usize) -> Started {
        let mut started = self.started.lock().unwrap();
        assert!(
            n < started.len(),
            "only {} exchanges started",
            started.len()
        );
        started.remove(n)
    }

    fn count(&self) -> usize {
        self.started.lock().unwrap().len()
    }
}

#[derive(Default)]
struct Recorded {
    outcomes: Mutex<Vec<Result<Collected, HostError>>>,
    ended: AtomicUsize,
    metrics: Mutex<Vec<(String, HopMetrics)>>,
    /// Next hops held back, to start by hand.
    held: Mutex<Option<Vec<Job>>>,
    /// Run at a hop's end, as a caller's re-entrant abort would.
    on_end: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl FetchEvents for Recorded {
    fn outcome(&self, result: Result<Collected, HostError>) {
        self.outcomes.lock().unwrap().push(result);
    }
    fn hop_ended(&self) {
        self.ended.fetch_add(1, Ordering::SeqCst);
        if let Some(hook) = self.on_end.lock().unwrap().take() {
            hook();
        }
    }
    fn metrics(&self, url: &str, metrics: HopMetrics) {
        self.metrics
            .lock()
            .unwrap()
            .push((url.to_string(), metrics));
    }
    fn next_hop(&self, start: Box<dyn FnOnce() + Send>) {
        match self.held.lock().unwrap().as_mut() {
            Some(held) => held.push(start),
            None => start(),
        }
    }
}

impl Recorded {
    fn outcome(&self) -> Result<Collected, HostError> {
        let outcomes = self.outcomes.lock().unwrap();
        assert_eq!(outcomes.len(), 1, "one outcome, exactly");
        outcomes[0].clone()
    }
    fn none(&self) -> bool {
        self.outcomes.lock().unwrap().is_empty()
    }
}

const CAPS: HeaderCaps = HeaderCaps {
    bytes: 64 << 10,
    entries: 1024,
};

fn granted(hosts: &[&str]) -> Arc<GrantSet> {
    let mut grants = GrantSet::none();
    for host in hosts {
        grants = grants.with(Grant::Fetch(Origin::new("https", host, 443)));
    }
    Arc::new(grants)
}

fn start(
    request: Request,
    upload: Option<Arc<Upload>>,
    hosts: &[&str],
) -> (Arc<Scripted>, Arc<Recorded>, AbortController) {
    let transport = Arc::new(Scripted::default());
    let events = Arc::new(Recorded::default());
    let abort = AbortController::new();
    FetchTask::start(
        transport.clone(),
        granted(hosts),
        request,
        upload,
        abort.signal(),
        CAPS,
        events.clone(),
    );
    (transport, events, abort)
}

fn head(status: u16, headers: &[(&str, &str)], declared: Option<u64>) -> Head {
    let mut h = Headers::new();
    for (k, v) in headers {
        h.set_response(k, v);
    }
    Head {
        status,
        headers: h,
        declared_length: declared,
    }
}

fn cancel_error() -> Result<(), HostError> {
    Err(HostError::Failed("cancelled".into()))
}

#[test]
fn a_response_is_collected_and_reported_once_after_its_end() {
    let (transport, events, _abort) =
        start(Request::get("https://a.example/x"), None, &["a.example"]);
    let mut hop = transport.take(0);
    assert_eq!(hop.request.url, "https://a.example/x");
    assert_eq!(
        hop.sink.head(head(200, &[("x", "1")], Some(5))),
        Flow::Continue
    );
    assert_eq!(hop.sink.data(b"he"), Flow::Continue);
    assert_eq!(hop.sink.data(b"llo"), Flow::Continue);
    assert!(events.none(), "nothing is reported before the end");
    hop.sink.metrics(HopMetrics {
        protocol: Some("h2".into()),
        ..HopMetrics::default()
    });
    hop.sink.end(Ok(()));
    let done = events.outcome().unwrap();
    assert_eq!(
        (done.status, &done.body[..], done.redirected),
        (200, &b"hello"[..], false)
    );
    assert_eq!(events.ended.load(Ordering::SeqCst), 1);
    assert_eq!(
        events.metrics.lock().unwrap()[0].1.protocol.as_deref(),
        Some("h2")
    );
}

#[test]
fn a_declared_length_over_the_ceiling_is_refused_before_a_redirect_is_followed() {
    let mut request = Request::get("https://a.example/x");
    request.max_body = Some(10);
    let (transport, events, _abort) = start(request, None, &["a.example"]);
    let mut hop = transport.take(0);
    let flow = hop.sink.head(head(302, &[("location", "/y")], Some(11)));
    assert_eq!(flow, Flow::Stop);
    assert_eq!(events.outcome().unwrap_err(), over_limit(10));
    // The cancellation's end changes nothing, and no second hop starts.
    hop.sink.end(cancel_error());
    assert_eq!(events.outcome().unwrap_err(), over_limit(10));
    assert_eq!(transport.count(), 0);
    assert_eq!(events.ended.load(Ordering::SeqCst), 1);
}

#[test]
fn a_redirect_body_is_skipped_and_a_refused_next_hop_is_denied_redirect() {
    let (transport, events, _abort) =
        start(Request::get("https://a.example/x"), None, &["a.example"]);
    let mut hop = transport.take(0);
    let flow = hop
        .sink
        .head(head(302, &[("location", "https://b.example/y")], Some(100)));
    assert_eq!(flow, Flow::Skip);
    assert!(events.none());
    hop.sink.end(cancel_error());
    assert_eq!(
        events.outcome().unwrap_err(),
        HostError::DeniedRedirect {
            capability: "net.fetch",
            origin: "https://b.example".into()
        }
    );
    assert_eq!(transport.count(), 0, "a refused hop never starts");
}

#[test]
fn a_followed_redirect_starts_the_next_hop_after_the_old_one_ends() {
    let (transport, events, _abort) = start(
        Request::get("https://a.example/x"),
        None,
        &["a.example", "b.example"],
    );
    let mut first = transport.take(0);
    assert_eq!(
        first
            .sink
            .head(head(301, &[("location", "https://b.example/y")], None)),
        Flow::Skip
    );
    assert_eq!(
        transport.count(),
        0,
        "the next hop waits for the old one's end"
    );
    first.sink.end(cancel_error());
    let mut second = transport.take(0);
    assert_eq!(second.request.url, "https://b.example/y");
    // A late event from the first hop is ignored by hop identity.
    assert_eq!(first.sink.data(b"late"), Flow::Stop);
    assert_eq!(second.sink.head(head(200, &[], None)), Flow::Continue);
    second.sink.data(b"ok");
    second.sink.end(Ok(()));
    let done = events.outcome().unwrap();
    assert_eq!(
        (done.url.as_str(), done.redirected, &done.body[..]),
        ("https://b.example/y", true, &b"ok"[..])
    );
    assert_eq!(events.ended.load(Ordering::SeqCst), 2);
}

#[test]
fn a_body_over_the_ceiling_stops_with_over_limit_and_late_events_change_nothing() {
    let mut request = Request::get("https://a.example/x");
    request.max_body = Some(4);
    let (transport, events, _abort) = start(request, None, &["a.example"]);
    let mut hop = transport.take(0);
    hop.sink.head(head(200, &[], None));
    assert_eq!(hop.sink.data(b"abc"), Flow::Continue);
    assert_eq!(hop.sink.data(b"de"), Flow::Stop);
    assert_eq!(hop.sink.data(b"f"), Flow::Stop);
    hop.sink.end(Ok(()));
    assert_eq!(events.outcome().unwrap_err(), over_limit(4));
}

#[test]
fn oversized_headers_on_an_intermediate_hop_fail_the_fetch() {
    let (transport, events, _abort) =
        start(Request::get("https://a.example/x"), None, &["a.example"]);
    let mut hop = transport.take(0);
    let big = "v".repeat(64 << 10);
    let flow = hop
        .sink
        .head(head(302, &[("location", "/y"), ("x-big", &big)], None));
    assert_eq!(flow, Flow::Stop);
    assert_eq!(
        events.outcome().unwrap_err(),
        HostError::Failed("HTTP headers exceed limit".into())
    );
}

#[test]
fn an_abort_reports_at_once_and_cancels_the_exchange() {
    let (transport, events, abort) =
        start(Request::get("https://a.example/x"), None, &["a.example"]);
    abort.abort();
    assert_eq!(events.outcome().unwrap_err(), aborted());
    let mut hop = transport.take(0);
    assert_eq!(*hop.calls.lock().unwrap(), ["activate", "cancel"]);
    assert_eq!(hop.sink.head(head(200, &[], None)), Flow::Stop);
    hop.sink.end(cancel_error());
    assert_eq!(events.outcome().unwrap_err(), aborted());
    assert_eq!(events.ended.load(Ordering::SeqCst), 1);
}

#[test]
fn an_abort_before_the_start_starts_nothing() {
    let transport = Arc::new(Scripted::default());
    let events = Arc::new(Recorded::default());
    let abort = AbortController::new();
    abort.abort();
    FetchTask::start(
        transport.clone(),
        granted(&["a.example"]),
        Request::get("https://a.example/x"),
        None,
        abort.signal(),
        CAPS,
        events.clone(),
    );
    assert_eq!(events.outcome().unwrap_err(), aborted());
    assert_eq!(transport.count(), 0);
}

#[test]
fn one_upload_rides_a_307_and_a_303_drops_it_after_the_hop_that_sent_it() {
    let released = Arc::new(AtomicUsize::new(0));
    let flag = released.clone();
    let upload = Upload::new(
        b"body".to_vec(),
        Some(Box::new(move || {
            flag.fetch_add(1, Ordering::SeqCst);
        })),
    );
    let mut request = Request::get("https://a.example/x");
    request.method = "POST".into();
    let (transport, events, _abort) = start(request, Some(upload.clone()), &["a.example"]);
    drop(upload);
    let mut first = transport.take(0);
    let sent = first.request.body.clone().expect("the body is sent");
    assert_eq!(sent.bytes(), b"body");
    first.sink.head(head(307, &[("location", "/y")], None));
    first.sink.end(cancel_error());
    let mut second = transport.take(0);
    assert_eq!(second.request.method, "POST");
    assert!(
        Arc::ptr_eq(second.request.body.as_ref().unwrap(), &sent),
        "one buffer, not a copy"
    );
    second.sink.head(head(303, &[("location", "/z")], None));
    drop(first);
    second.sink.end(cancel_error());
    let mut third = transport.take(0);
    assert_eq!(third.request.method, "GET");
    assert!(third.request.body.is_none());
    assert_eq!(
        released.load(Ordering::SeqCst),
        0,
        "the platform's copy still holds it"
    );
    drop((sent, second));
    assert_eq!(
        released.load(Ordering::SeqCst),
        1,
        "released with the last holder"
    );
    third.sink.head(head(200, &[], None));
    third.sink.end(Ok(()));
    assert!(events.outcome().is_ok());
}

#[test]
fn a_cross_origin_hop_drops_credentials() {
    let mut request = Request::get("https://a.example/x");
    request.headers.set("authorization", "Bearer t");
    let (transport, _events, _abort) = start(request, None, &["a.example", "b.example"]);
    let mut first = transport.take(0);
    assert_eq!(first.request.headers.get("authorization"), Some("Bearer t"));
    first
        .sink
        .head(head(302, &[("location", "https://b.example/")], None));
    first.sink.end(cancel_error());
    let second = transport.take(0);
    assert_eq!(second.request.headers.get("authorization"), None);
}

#[test]
fn a_transport_error_without_a_head_is_the_outcome() {
    let (transport, events, _abort) =
        start(Request::get("https://a.example/x"), None, &["a.example"]);
    let mut hop = transport.take(0);
    hop.sink.end(Err(HostError::Failed(
        "TypeError: Failed to fetch — offline".into(),
    )));
    assert_eq!(
        events.outcome().unwrap_err(),
        HostError::Failed("TypeError: Failed to fetch — offline".into())
    );
}

fn start_with(
    request: Request,
    upload: Option<Arc<Upload>>,
    hosts: &[&str],
    transport: Arc<Scripted>,
    events: Arc<Recorded>,
    abort: &AbortController,
) {
    FetchTask::start(
        transport,
        granted(hosts),
        request,
        upload,
        abort.signal(),
        CAPS,
        events,
    );
}

#[test]
fn every_exchange_is_activated_and_an_unwanted_one_cancelled() {
    let (transport, _events, _abort) =
        start(Request::get("https://a.example/x"), None, &["a.example"]);
    let hop = transport.take(0);
    assert_eq!(*hop.calls.lock().unwrap(), ["activate"]);
    // Aborted while the transport was starting it: cancelled first, so its
    // activation sends nothing, and activated, so its sink still ends.
    let (transport, events, abort) = (
        Arc::new(Scripted::default()),
        Arc::new(Recorded::default()),
        AbortController::new(),
    );
    let inside = abort.clone();
    *transport.on_start.lock().unwrap() = Some(Box::new(move || inside.abort()));
    start_with(
        Request::get("https://a.example/x"),
        None,
        &["a.example"],
        transport.clone(),
        events.clone(),
        &abort,
    );
    assert_eq!(events.outcome().unwrap_err(), aborted());
    let hop = transport.take(0);
    // Cancelled first, so its activation sends nothing; activated, so its
    // sink still gets an end.
    assert_eq!(*hop.calls.lock().unwrap(), ["cancel", "activate"]);
}

#[test]
fn an_abort_while_the_next_hop_waits_starts_no_hop() {
    let (transport, events, abort) = (
        Arc::new(Scripted::default()),
        Arc::new(Recorded::default()),
        AbortController::new(),
    );
    *events.held.lock().unwrap() = Some(Vec::new());
    start_with(
        Request::get("https://a.example/x"),
        None,
        &["a.example"],
        transport.clone(),
        events.clone(),
        &abort,
    );
    let mut first = transport.take(0);
    first.sink.head(head(302, &[("location", "/y")], None));
    first.sink.end(cancel_error());
    abort.abort();
    assert_eq!(events.outcome().unwrap_err(), aborted());
    for next in events.held.lock().unwrap().take().unwrap() {
        next();
    }
    assert_eq!(transport.count(), 0);
}

#[test]
fn a_release_that_aborts_does_not_deadlock() {
    // A grant refusal drops the upload; its release aborts the same fetch.
    let abort = AbortController::new();
    let inside = abort.clone();
    let upload = Upload::new(b"body".to_vec(), Some(Box::new(move || inside.abort())));
    let (transport, events) = (Arc::new(Scripted::default()), Arc::new(Recorded::default()));
    let mut request = Request::get("https://denied.example/x");
    request.method = "POST".into();
    start_with(
        request,
        Some(upload),
        &["a.example"],
        transport,
        events.clone(),
        &abort,
    );
    assert!(matches!(events.outcome(), Err(HostError::Denied { .. })));
}

#[test]
fn an_abort_from_a_hop_end_does_not_replace_a_response() {
    let (transport, events, abort) = (
        Arc::new(Scripted::default()),
        Arc::new(Recorded::default()),
        AbortController::new(),
    );
    let inside = abort.clone();
    *events.on_end.lock().unwrap() = Some(Box::new(move || inside.abort()));
    start_with(
        Request::get("https://a.example/x"),
        None,
        &["a.example"],
        transport.clone(),
        events.clone(),
        &abort,
    );
    let mut hop = transport.take(0);
    hop.sink.head(head(200, &[], None));
    hop.sink.end(Ok(()));
    assert!(events.outcome().is_ok());
}

#[test]
fn head_204_and_304_declare_no_body_to_refuse() {
    for (method, status) in [("HEAD", 200), ("GET", 204), ("GET", 205), ("GET", 304)] {
        let mut request = Request::get("https://a.example/x");
        request.method = method.into();
        request.max_body = Some(1);
        let (transport, events, _abort) = start(request, None, &["a.example"]);
        let mut hop = transport.take(0);
        assert_eq!(
            hop.sink.head(head(status, &[], Some(1 << 20))),
            Flow::Continue
        );
        hop.sink.end(Ok(()));
        assert_eq!(events.outcome().unwrap().status, status);
    }
}

#[test]
fn the_header_count_cap_is_inclusive() {
    for (count, ok) in [(1024, true), (1025, false)] {
        let (transport, events, _abort) =
            start(Request::get("https://a.example/x"), None, &["a.example"]);
        let mut hop = transport.take(0);
        let names: Vec<String> = (0..count).map(|i| format!("x-{i}")).collect();
        let pairs: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "v")).collect();
        let flow = hop.sink.head(head(200, &pairs, None));
        assert_eq!(flow == Flow::Continue, ok, "{count}");
        hop.sink.end(Ok(()));
        assert_eq!(events.outcome().is_ok(), ok, "{count}");
    }
}

#[test]
fn manual_returns_the_redirect_and_error_refuses_it() {
    let mut request = Request::get("https://a.example/x");
    request.redirect = crate::stdlib::fetch::RedirectMode::Manual;
    let (transport, events, _abort) = start(request, None, &["a.example"]);
    let mut hop = transport.take(0);
    assert_eq!(
        hop.sink.head(head(302, &[("location", "/y")], None)),
        Flow::Continue
    );
    hop.sink.end(Ok(()));
    assert_eq!(events.outcome().unwrap().status, 302);

    let mut request = Request::get("https://a.example/x");
    request.redirect = crate::stdlib::fetch::RedirectMode::Error;
    let (transport, events, _abort) = start(request, None, &["a.example"]);
    let mut hop = transport.take(0);
    assert_eq!(
        hop.sink.head(head(302, &[("location", "/y")], None)),
        Flow::Stop
    );
    assert!(events.outcome().is_err());
}

#[test]
fn the_twenty_first_redirect_fails() {
    let (transport, events, _abort) =
        start(Request::get("https://a.example/x"), None, &["a.example"]);
    for n in 0..=crate::stdlib::fetch::MAX_REDIRECTS {
        let mut hop = transport.take(0);
        let flow = hop.sink.head(head(302, &[("location", "/again")], None));
        if n < crate::stdlib::fetch::MAX_REDIRECTS {
            assert_eq!(flow, Flow::Skip, "{n}");
            hop.sink.end(cancel_error());
        } else {
            assert_eq!(flow, Flow::Stop);
        }
    }
    assert!(events
        .outcome()
        .unwrap_err()
        .to_string()
        .contains("too many redirects"));
}

#[test]
fn late_metrics_name_the_hop_that_sent_them() {
    let (transport, events, _abort) = start(
        Request::get("https://a.example/x"),
        None,
        &["a.example", "b.example"],
    );
    let mut first = transport.take(0);
    first
        .sink
        .head(head(302, &[("location", "https://b.example/y")], None));
    first.sink.end(cancel_error());
    let mut second = transport.take(0);
    second.sink.head(head(200, &[], None));
    second.sink.end(Ok(()));
    first.sink.metrics(HopMetrics {
        protocol: Some("h2".into()),
        ..HopMetrics::default()
    });
    let metrics = events.metrics.lock().unwrap();
    assert_eq!(metrics[0].0, "https://a.example/x");
}

/// A transport whose cancel delivers the cancellation's end at once, on
/// the cancelling stack, as a platform may.
#[derive(Default)]
struct Reentrant {
    sink: Arc<Mutex<Option<Box<dyn Sink>>>>,
}

impl AsyncTransport for Reentrant {
    fn start(&self, _request: HopRequest, sink: Box<dyn Sink>) -> Result<Exchange, HostError> {
        *self.sink.lock().unwrap() = Some(sink);
        let held = self.sink.clone();
        Ok(Exchange::new(
            || {},
            move || {
                let sink = held.lock().unwrap().take();
                if let Some(mut sink) = sink {
                    sink.end(cancel_error());
                }
            },
        ))
    }
}

#[test]
fn a_cancellation_ending_on_the_abort_stack_stays_aborted() {
    let transport = Arc::new(Reentrant::default());
    let events = Arc::new(Recorded::default());
    let abort = AbortController::new();
    FetchTask::start(
        transport.clone(),
        granted(&["a.example"]),
        Request::get("https://a.example/x"),
        None,
        abort.signal(),
        CAPS,
        events.clone(),
    );
    // Reported before the cancellation's end, which comes on its stack.
    *events.on_end.lock().unwrap() = Some(Box::new({
        let events = events.clone();
        move || assert!(!events.none(), "the outcome came first")
    }));
    abort.abort();
    assert_eq!(events.outcome().unwrap_err(), aborted());
    assert_eq!(events.ended.load(Ordering::SeqCst), 1);
}

#[test]
fn a_post_301_or_302_drops_the_body_and_a_308_keeps_it() {
    for (status, keeps) in [(301, false), (302, false), (308, true)] {
        let mut request = Request::get("https://a.example/x");
        request.method = "POST".into();
        request.body = Some(b"body".to_vec());
        let (transport, _events, _abort) = start(request, None, &["a.example"]);
        let mut first = transport.take(0);
        first.sink.head(head(status, &[("location", "/y")], None));
        first.sink.end(cancel_error());
        let second = transport.take(0);
        assert_eq!(second.request.body.is_some(), keeps, "{status}");
        assert_eq!(second.request.method == "POST", keeps, "{status}");
    }
}

#[test]
fn a_cross_origin_hop_drops_cookie_and_proxy_authorization() {
    let mut request = Request::get("https://a.example/x");
    request.headers.set("cookie", "c=1");
    request.headers.set("proxy-authorization", "Basic x");
    let (transport, _events, _abort) = start(request, None, &["a.example", "b.example"]);
    let mut first = transport.take(0);
    assert_eq!(first.request.headers.get("cookie"), Some("c=1"));
    first
        .sink
        .head(head(302, &[("location", "https://b.example/")], None));
    first.sink.end(cancel_error());
    let second = transport.take(0);
    assert_eq!(second.request.headers.get("cookie"), None);
    assert_eq!(second.request.headers.get("proxy-authorization"), None);
}

#[test]
fn a_stopped_fetch_lets_its_upload_go_while_the_task_is_still_held() {
    let released = Arc::new(AtomicUsize::new(0));
    let flag = released.clone();
    let upload = Upload::new(
        b"body".to_vec(),
        Some(Box::new(move || {
            flag.fetch_add(1, Ordering::SeqCst);
        })),
    );
    let transport = Arc::new(Scripted::default());
    let events = Arc::new(Recorded::default());
    let abort = AbortController::new();
    let mut request = Request::get("https://a.example/x");
    request.method = "POST".into();
    request.max_body = Some(1);
    let task = FetchTask::start(
        transport.clone(),
        granted(&["a.example"]),
        request,
        Some(upload),
        abort.signal(),
        CAPS,
        events.clone(),
    );
    let mut hop = transport.take(0);
    hop.sink.head(head(200, &[], Some(2)));
    hop.sink.end(cancel_error());
    // The platform's copy (the hop's request) is the last holder.
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(hop);
    assert_eq!(released.load(Ordering::SeqCst), 1, "not held by the task");
    drop(task);
}

/// A blocking transport answering every request with `body`.
struct Answers(Vec<u8>);
impl crate::stdlib::fetch::Transport for Answers {
    fn open(
        &self,
        request: &Request,
        signal: &AbortSignal,
    ) -> Result<crate::stdlib::fetch::StreamingResponse, HostError> {
        let mut headers = Headers::new();
        headers.set_response("content-length", &self.0.len().to_string());
        Ok(crate::stdlib::fetch::Response {
            status: 200,
            status_text: "OK".into(),
            headers,
            body: self.0.clone(),
            url: request.url.clone(),
            redirected: false,
        }
        .into_stream(request.body_limit(), signal.clone()))
    }
}

fn adapter(body: &[u8], spawns: bool) -> BlockingAdapter {
    struct Refuse;
    impl WorkerService for Refuse {
        fn reserve(&self) -> Result<Box<dyn ReservedWorker>, HostError> {
            Err(HostError::Failed("HTTP worker capacity exhausted".into()))
        }
    }
    let workers: Arc<dyn WorkerService> = if spawns {
        Arc::new(BoundedWorkerPool::new(1, 2).unwrap())
    } else {
        Arc::new(Refuse)
    };
    BlockingAdapter::new(Arc::new(Answers(body.to_vec())), workers)
}

#[test]
fn a_blocking_transport_runs_as_an_asynchronous_one() {
    let events = Arc::new(Recorded::default());
    let abort = AbortController::new();
    FetchTask::start(
        Arc::new(adapter(b"hello", true)),
        granted(&["a.example"]),
        Request::get("https://a.example/x"),
        None,
        abort.signal(),
        CAPS,
        events.clone(),
    );
    for _ in 0..500 {
        if !events.none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let done = events.outcome().unwrap();
    assert_eq!((done.status, &done.body[..]), (200, &b"hello"[..]));
    assert_eq!(events.ended.load(Ordering::SeqCst), 1);
}

#[test]
fn an_adapter_refusal_settles_without_an_accepted_exchange() {
    let events = Arc::new(Recorded::default());
    let abort = AbortController::new();
    FetchTask::start(
        Arc::new(adapter(b"hello", false)),
        granted(&["a.example"]),
        Request::get("https://a.example/x"),
        None,
        abort.signal(),
        CAPS,
        events.clone(),
    );
    assert!(events
        .outcome()
        .unwrap_err()
        .to_string()
        .contains("capacity exhausted"));
    assert_eq!(events.ended.load(Ordering::SeqCst), 0);
}

#[test]
fn an_adapters_declared_length_over_the_ceiling_is_refused() {
    let events = Arc::new(Recorded::default());
    let abort = AbortController::new();
    let mut request = Request::get("https://a.example/x");
    request.max_body = Some(3);
    FetchTask::start(
        Arc::new(adapter(b"hello", true)),
        granted(&["a.example"]),
        request,
        None,
        abort.signal(),
        CAPS,
        events.clone(),
    );
    for _ in 0..500 {
        if !events.none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(events.outcome().unwrap_err(), over_limit(3));
}

#[test]
fn reserved_capacity_refuses_without_callback_and_cancel_retires_without_activation() {
    use std::sync::mpsc;
    struct Observer(mpsc::Sender<(std::thread::ThreadId, Result<(), HostError>)>);
    impl Sink for Observer {
        fn head(&mut self, _: Head) -> Flow {
            panic!("cancelled request reached transport")
        }
        fn data(&mut self, _: &[u8]) -> Flow {
            panic!("cancelled request delivered bytes")
        }
        fn end(&mut self, result: Result<(), HostError>) {
            self.0.send((std::thread::current().id(), result)).unwrap();
        }
    }
    let workers = Arc::new(BoundedWorkerPool::new(1, 1).unwrap());
    let transport = BlockingAdapter::new(Arc::new(Answers(vec![])), workers.clone());
    let request = || HopRequest {
        method: "GET".into(),
        url: "https://a.example/".into(),
        headers: Headers::new(),
        body: None,
        max_body: 16,
        timeout: None,
    };
    let (tx, rx) = mpsc::channel();
    let exchange = transport
        .start(request(), Box::new(Observer(tx.clone())))
        .unwrap();
    assert!(transport.start(request(), Box::new(Observer(tx))).is_err());
    assert!(rx.try_recv().is_err());
    let caller = std::thread::current().id();
    exchange.cancel(); // No activate call is needed for retirement.
    exchange.cancel();
    let (thread, result) = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    assert_ne!(thread, caller, "no inline cancellation callback");
    assert_eq!(result.unwrap_err(), aborted());
    assert!(rx.try_recv().is_err());
    assert!(workers.reserve().is_ok());
}

#[test]
fn worker_guards_live_through_pool_retirement_and_job_panics() {
    use std::sync::mpsc;
    use std::time::Duration;
    struct Guard(mpsc::Sender<std::thread::ThreadId>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.send(std::thread::current().id()).unwrap();
        }
    }
    let (retired_tx, retired_rx) = mpsc::channel();
    let pool = BoundedWorkerPool::with_worker_guards(vec![Guard(retired_tx)], 2).unwrap();
    let pending = pool.reserve().unwrap();
    let running = pool.reserve().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    running.dispatch(Box::new(move || {
        started_tx.send(std::thread::current().id()).unwrap();
        release_rx.recv().unwrap();
        panic!("a panicking job must not release the worker guard");
    }));
    let worker = started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(pool);
    assert!(
        retired_rx.try_recv().is_err(),
        "running worker still owns its guard"
    );
    release_tx.send(()).unwrap();
    let (drained_tx, drained_rx) = mpsc::channel();
    pending.dispatch(Box::new(move || {
        assert!(
            retired_rx.try_recv().is_err(),
            "guard outlives accepted jobs"
        );
        drained_tx.send(retired_rx).unwrap();
    }));
    let retired_rx = drained_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        retired_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        worker
    );
    assert_ne!(worker, std::thread::current().id());
}

#[test]
fn idle_worker_guards_retire_on_the_workers() {
    use std::sync::mpsc;
    use std::time::Duration;
    struct Guard(mpsc::Sender<std::thread::ThreadId>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.send(std::thread::current().id()).unwrap();
        }
    }
    let (tx, rx) = mpsc::channel();
    let pool =
        BoundedWorkerPool::with_worker_guards(vec![Guard(tx.clone()), Guard(tx)], 1).unwrap();
    let pending = pool.reserve().unwrap();
    drop(pool);
    assert!(
        rx.try_recv().is_err(),
        "reservation retains both idle workers"
    );
    drop(pending);
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let second = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_ne!(first, second);
    assert_ne!(first, std::thread::current().id());
    assert_ne!(second, std::thread::current().id());
}

#[test]
fn callback_can_overlap_activation_return_and_duplicate_end_is_ignored() {
    struct Overlap;
    impl AsyncTransport for Overlap {
        fn start(&self, _: HopRequest, mut sink: Box<dyn Sink>) -> Result<Exchange, HostError> {
            Ok(Exchange::new(
                move || {
                    // Transport activation waits for another context only in this
                    // deterministic fixture, proving no return handshake is assumed.
                    std::thread::spawn(move || {
                        sink.head(head(200, &[], Some(1)));
                        sink.data(b"x");
                        sink.end(Ok(()));
                        sink.end(Ok(()));
                    })
                    .join()
                    .unwrap();
                },
                || {},
            ))
        }
    }
    let events = Arc::new(Recorded::default());
    let task = FetchTask::start(
        Arc::new(Overlap),
        granted(&["a.example"]),
        Request::get("https://a.example/"),
        None,
        AbortSignal::default(),
        CAPS,
        events.clone(),
    );
    assert_eq!(events.outcome().unwrap().body, b"x");
    assert_eq!(events.ended.load(Ordering::SeqCst), 1);
    task.abort();
    assert!(events.outcome().is_ok());
}

#[test]
fn completed_sink_for_late_metrics_does_not_retain_task_or_transport() {
    let transport = Arc::new(Scripted::default());
    let weak = Arc::downgrade(&transport);
    let events = Arc::new(Recorded::default());
    let task = FetchTask::start(
        transport.clone(),
        granted(&["a.example"]),
        Request::get("https://a.example/"),
        None,
        AbortSignal::default(),
        CAPS,
        events.clone(),
    );
    let mut hop = transport.take(0);
    hop.sink.head(head(200, &[], None));
    hop.sink.end(Ok(()));
    drop(task);
    drop(transport);
    assert!(
        weak.upgrade().is_none(),
        "late metrics must not retain the fetch"
    );
    hop.sink.metrics(HopMetrics::default());
    assert_eq!(events.metrics.lock().unwrap().len(), 1);
}

#[test]
fn panicking_outcome_does_not_skip_cancellation_or_physical_retirement() {
    struct Panicking(AtomicUsize);
    impl FetchEvents for Panicking {
        fn outcome(&self, _: Result<Collected, HostError>) {
            panic!("consumer panic")
        }
        fn hop_ended(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let transport = Arc::new(Reentrant::default());
    let events = Arc::new(Panicking(AtomicUsize::new(0)));
    let task = FetchTask::start(
        transport,
        granted(&["a.example"]),
        Request::get("https://a.example/"),
        None,
        AbortSignal::default(),
        CAPS,
        events.clone(),
    );
    task.abort();
    assert_eq!(events.0.load(Ordering::SeqCst), 1);
}

#[test]
fn one_worker_and_one_pending_slot_can_follow_a_redirect() {
    struct Redirect(std::sync::atomic::AtomicUsize);
    impl crate::stdlib::fetch::Transport for Redirect {
        fn open(
            &self,
            request: &Request,
            signal: &AbortSignal,
        ) -> Result<crate::stdlib::fetch::StreamingResponse, HostError> {
            let first = self.0.fetch_add(1, Ordering::SeqCst) == 0;
            let mut headers = Headers::new();
            if first {
                headers.set("location", "/next");
            }
            Ok(crate::stdlib::fetch::Response {
                status: if first { 302 } else { 200 },
                status_text: String::new(),
                headers,
                body: b"ok".to_vec(),
                url: request.url.clone(),
                redirected: false,
            }
            .into_stream(request.body_limit(), signal.clone()))
        }
    }
    let adapter = BlockingAdapter::new(
        Arc::new(Redirect(AtomicUsize::new(0))),
        Arc::new(BoundedWorkerPool::new(1, 1).unwrap()),
    );
    let events = Arc::new(Recorded::default());
    FetchTask::start(
        Arc::new(adapter),
        granted(&["a.example"]),
        Request::get("https://a.example/"),
        None,
        AbortSignal::default(),
        CAPS,
        events.clone(),
    );
    for _ in 0..500 {
        if !events.none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(events.outcome().unwrap().body, b"ok");
    assert_eq!(events.ended.load(Ordering::SeqCst), 2);
}

#[test]
fn unlimited_sentinels_are_refused_before_transport_admission() {
    let mut request = Request::get("https://a.example/");
    request.max_body = Some(usize::MAX);
    let (transport, events, _) = start(request, None, &["a.example"]);
    assert!(matches!(
        events.outcome(),
        Err(HostError::InvalidArgument(_))
    ));
    assert_eq!(transport.count(), 0);
}

#[test]
fn blocking_and_async_share_the_redirect_and_grant_corpus() {
    use crate::stdlib::fetch::{fetch, RedirectMode, Response, StreamingResponse, Transport};
    struct Replay(Mutex<std::collections::VecDeque<Response>>);
    impl Transport for Replay {
        fn open(
            &self,
            request: &Request,
            signal: &AbortSignal,
        ) -> Result<StreamingResponse, HostError> {
            let mut response = self.0.lock().unwrap().pop_front().unwrap();
            response.url = request.url.clone();
            Ok(response.into_stream(request.body_limit(), signal.clone()))
        }
    }
    for (status, location, mode) in [
        (302, "/next", RedirectMode::Follow),
        (303, "/next", RedirectMode::Follow),
        (307, "/next", RedirectMode::Follow),
        (308, "/next", RedirectMode::Follow),
        (302, "https://denied.example/", RedirectMode::Follow),
        (302, "http://[", RedirectMode::Follow),
        (302, "/next", RedirectMode::Manual),
        (302, "/next", RedirectMode::Error),
    ] {
        let mut request = Request::get("https://a.example/");
        request.method = "POST".into();
        request.body = Some(b"upload".to_vec());
        request.redirect = mode;
        let mut redirect_headers = Headers::new();
        redirect_headers.set("location", location);
        let responses = vec![
            Response {
                status,
                status_text: String::new(),
                headers: redirect_headers,
                body: b"redirect".to_vec(),
                url: String::new(),
                redirected: false,
            },
            Response {
                status: 200,
                status_text: String::new(),
                headers: Headers::new(),
                body: b"ok".to_vec(),
                url: String::new(),
                redirected: false,
            },
        ];
        let blocking = fetch(
            &Replay(Mutex::new(responses.clone().into())),
            &granted(&["a.example"]),
            request.clone(),
        )
        .map(|r| (r.status, r.url, r.body, r.redirected));
        let (transport, events, _) = start(request, None, &["a.example"]);
        for response in responses {
            if transport.count() == 0 {
                break;
            }
            let mut exchange = transport.take(0);
            if exchange.sink.head(Head {
                status: response.status,
                headers: response.headers,
                declared_length: None,
            }) == Flow::Continue
            {
                exchange.sink.data(&response.body);
                exchange.sink.end(Ok(()));
            } else {
                exchange.sink.end(cancel_error());
            }
        }
        let asynchronous = events
            .outcome()
            .map(|r| (r.status, r.url, r.body, r.redirected));
        assert_eq!(
            blocking, asynchronous,
            "status {status}, location {location}, mode {mode:?}"
        );
    }
}
