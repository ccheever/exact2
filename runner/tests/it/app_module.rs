//! A Rust source reaches the session's app module (LLP 1067.000 Q9): it asks
//! with `Request::native`, the runner hands the call to the handler the host
//! installed in its own slot (the source has none), and an announced topic
//! asks the watching resource again.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, Outcome, Reply, Request, Response, Runner, Store, Work,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const SOURCE: &str = r##"shape Status
  n: number
component App
  resource status = status() as shape Status
  view
    text `status ${status.n}`
"##;

struct Source;

impl DataSource for Source {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        if source != "status" {
            return Err(DataError::UnknownSource(source.into()));
        }
        store.observe_topic("status");
        Ok(Answer::Later(Request::native(
            br#"{"op":"status"}"#.to_vec(),
        )))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Outcome::Response(Response {
            status: 200, body, ..
        }) = outcome
        else {
            return Err(DataError::Unavailable(format!("{outcome:?}")));
        };
        // The module answers `{"n":<count>}`.
        let text = String::from_utf8(body).unwrap();
        let n: f64 = text
            .trim_start_matches("{\"n\":")
            .trim_end_matches('}')
            .parse()
            .unwrap();
        Ok(Answer::Now(Value::record(vec![Value::Number(n)])))
    }
}

/// Run every native request the runner handed out, as a host's worker does.
fn run_native(r: &mut Runner<Source>) -> usize {
    let mut ran = 0;
    for out in r.take_requests() {
        assert!(out.request.is_native(), "{:?}", out.request);
        let Dispatch::Run(Work::Later(work)) = r.native_work(&out.request) else {
            panic!("a native call is later work")
        };
        let (tx, rx) = std::sync::mpsc::channel();
        work(Reply::new(move |o| tx.send(o).unwrap()));
        r.fulfill(out.ticket, rx.recv().unwrap()).unwrap();
        ran += 1;
    }
    ran
}

#[test]
fn a_rust_source_reaches_the_app_module_the_host_installed() {
    let plan = contract::compile(SOURCE).unwrap();
    let mut r = Runner::boot(
        plan,
        Source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            assert_eq!(body, br#"{"op":"status"}"#);
            let n = count.fetch_add(1, Ordering::SeqCst) + 1;
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: format!("{{\"n\":{n}}}").into_bytes(),
            }));
        })));
    r.listen(Arc::new(|| {}));
    r.data_ready().unwrap();
    assert_eq!(run_native(&mut r), 1);
    assert_eq!(
        r.resource("status"),
        Some(&Value::record(vec![Value::Number(1.0)]))
    );

    // The module announces; the watching resource asks it again.
    r.native_slot().changed("status");
    r.apply_announced();
    assert_eq!(run_native(&mut r), 1);
    assert_eq!(
        r.resource("status"),
        Some(&Value::record(vec![Value::Number(2.0)]))
    );
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}

/// Each native request the runner handed out, run by the module now and
/// its reply kept for the test to deliver: a call that takes a while.
fn start_native(r: &mut Runner<Source>) -> Vec<(u64, Outcome)> {
    let mut out = Vec::new();
    for req in r.take_requests() {
        let Dispatch::Run(Work::Later(work)) = r.native_work(&req.request) else {
            panic!("a native call is later work")
        };
        let (tx, rx) = std::sync::mpsc::channel();
        work(Reply::new(move |o| tx.send(o).unwrap()));
        out.push((req.ticket, rx.recv().unwrap()));
    }
    out
}

fn status(n: f64) -> Option<Value> {
    Some(Value::record(vec![Value::Number(n)]))
}

/// LLP 1016.002 D4 (issue #109): a topic announced while the watching
/// resource's request is in flight does not forget that request. Its reply
/// lands and shows, then the resource is asked again once, however many
/// announcements came meanwhile; `pending` holds until that ask lands.
#[test]
fn an_announcement_while_a_call_is_in_flight_lets_its_reply_land_then_asks_once_more() {
    let plan = contract::compile(SOURCE).unwrap();
    let mut r = Runner::boot(
        plan,
        Source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    r.native_slot()
        .host(Some(Arc::new(move |_: Vec<u8>, reply: Reply| {
            let n = count.fetch_add(1, Ordering::SeqCst) + 1;
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: format!("{{\"n\":{n}}}").into_bytes(),
            }));
        })));
    r.listen(Arc::new(|| {}));
    r.data_ready().unwrap();
    let mut first = start_native(&mut r);
    assert_eq!(first.len(), 1);
    let (ticket, reply) = first.remove(0);

    // The topic changes three times while the call is out: nothing new is
    // asked, and the request in flight is still wanted.
    for _ in 0..3 {
        r.native_slot().changed("status");
        r.apply_announced();
        assert!(r.take_requests().is_empty(), "no new call while one is out");
        assert!(r.holds(ticket), "the request in flight is not forgotten");
    }
    assert_eq!(r.in_flight(), vec![("status".to_string(), ticket)]);

    // Its reply lands and shows; the same commit asks once more.
    r.fulfill(ticket, reply).unwrap();
    assert_eq!(r.resource("status"), status(1.0).as_ref());
    let mut second = start_native(&mut r);
    assert_eq!(second.len(), 1, "one more ask for all three announcements");
    let (next, reply) = second.remove(0);
    assert_ne!(next, ticket);
    assert_eq!(r.in_flight(), vec![("status".to_string(), next)]);

    // The newer reply replaces it, and nothing more is asked.
    r.fulfill(next, reply).unwrap();
    assert_eq!(r.resource("status"), status(2.0).as_ref());
    assert!(r.in_flight().is_empty());
    assert!(r.take_requests().is_empty());
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    let journal: Vec<&str> = r.journal().collect();
    assert!(
        journal.iter().any(|l| l.ends_with(&format!(
            "changed status: request {ticket} (status) lands first, then it is asked again"
        ))),
        "{journal:#?}"
    );
    assert!(
        !journal.iter().any(|l| l.contains("forget request")),
        "{journal:#?}"
    );
}

/// A reply that fails after the topic changed is still asked again: the
/// failure is of an answer from before the change.
#[test]
fn a_failed_reply_after_an_announcement_is_asked_again() {
    let plan = contract::compile(SOURCE).unwrap();
    let mut r = Runner::boot(
        plan,
        Source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    r.native_slot()
        .host(Some(Arc::new(move |_: Vec<u8>, reply: Reply| {
            let n = count.fetch_add(1, Ordering::SeqCst) + 1;
            reply.send(Outcome::Response(Response {
                status: if n == 1 { 500 } else { 200 },
                headers: Vec::new(),
                body: format!("{{\"n\":{n}}}").into_bytes(),
            }));
        })));
    r.listen(Arc::new(|| {}));
    r.data_ready().unwrap();
    let (ticket, reply) = start_native(&mut r).remove(0);
    r.native_slot().changed("status");
    r.apply_announced();
    assert!(r.take_requests().is_empty());
    let _ = r.fulfill(ticket, reply);
    let mut again = start_native(&mut r);
    assert_eq!(again.len(), 1, "the failed ask is asked once more");
    let (next, reply) = again.remove(0);
    r.fulfill(next, reply).unwrap();
    assert_eq!(r.resource("status"), status(2.0).as_ref());
    assert!(r.in_flight().is_empty());
}
