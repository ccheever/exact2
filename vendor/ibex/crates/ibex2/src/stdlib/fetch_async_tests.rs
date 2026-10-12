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
    cancelled: Arc<AtomicUsize>,
}

#[derive(Default)]
struct Scripted {
    started: Mutex<Vec<Started>>,
}

impl AsyncTransport for Scripted {
    fn start(&self, request: HopRequest, sink: Box<dyn Sink>) -> Exchange {
        let cancelled = Arc::new(AtomicUsize::new(0));
        let flag = cancelled.clone();
        self.started.lock().unwrap().push(Started {
            request,
            sink,
            cancelled,
        });
        Exchange::new(move || {
            flag.fetch_add(1, Ordering::SeqCst);
        })
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
    metrics: Mutex<Vec<HopMetrics>>,
}

impl FetchEvents for Recorded {
    fn outcome(&self, result: Result<Collected, HostError>) {
        self.outcomes.lock().unwrap().push(result);
    }
    fn hop_ended(&self) {
        self.ended.fetch_add(1, Ordering::SeqCst);
    }
    fn metrics(&self, metrics: HopMetrics) {
        self.metrics.lock().unwrap().push(metrics);
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
        events.metrics.lock().unwrap()[0].protocol.as_deref(),
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
    assert_eq!(hop.cancelled.load(Ordering::SeqCst), 1);
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
