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
