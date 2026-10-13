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
    fn start(&self, request: HopRequest, sink: Box<dyn Sink>) -> Exchange {
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
        Exchange::new(
            move || on.lock().unwrap().push("activate"),
            move || off.lock().unwrap().push("cancel"),
        )
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
    held: Mutex<Option<Vec<Box<dyn FnOnce() + Send>>>>,
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
    // Aborted while the transport was starting it: activated, so its sink
    // ends, then cancelled.
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
    fn start(&self, _request: HopRequest, sink: Box<dyn Sink>) -> Exchange {
        *self.sink.lock().unwrap() = Some(sink);
        let held = self.sink.clone();
        Exchange::new(
            || {},
            move || {
                let sink = held.lock().unwrap().take();
                if let Some(mut sink) = sink {
                    sink.end(cancel_error());
                }
            },
        )
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
