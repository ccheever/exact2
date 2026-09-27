//! Both geometric edges: an async start must settle before end can read its answer.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, CollectionFeedback, DataError, DataSource, Outcome, Request, Runner, Store,
};

const SOURCE: &str = r#"shape Window
  rows: list<number>
  earlier: number
  later: number
component App
  state cursor = 1
  resource history = history(cursor) as shape Window
  action start writes cursor
    cursor = history.earlier
  action end writes cursor
    cursor = history.later
  view
    list virtualized=true height=320 reachstart=start reachend=end
      each x in history.rows key=x
        text `${x}` height=1
"#;
struct AsyncRows {
    value: Value,
    later: bool,
    continuation: bool,
}
impl DataSource for AsyncRows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(self.value.clone())
    }
    fn answer(&mut self, _: &mut Store, _: &str, args: &[Value]) -> Result<Answer, DataError> {
        if self.later {
            Ok(Answer::Later(Request::continuation(
                args[0].as_number().unwrap() as u64 + 100,
            )))
        } else {
            Ok(Answer::Now(self.value.clone()))
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        if std::mem::take(&mut self.continuation) {
            Ok(Answer::Later(Request::continuation(777)))
        } else {
            Ok(Answer::Now(self.value.clone()))
        }
    }
}
fn boot_source(source: &str) -> Runner<AsyncRows> {
    let value = Value::record(vec![
        Value::list(vec![Value::Number(1.), Value::Number(2.)]),
        Value::Number(0.),
        Value::Number(2.),
    ]);
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        AsyncRows {
            value,
            later: false,
            continuation: false,
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.data().later = true;
    r
}
fn boot() -> Runner<AsyncRows> {
    boot_source(SOURCE)
}
fn feedback(r: &mut Runner<AsyncRows>) {
    let c = r.collections().remove(0);
    let result = r
        .collection_feedback(CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset: 0.,
            port_cross: 640.,
            port_main: 320.,
            cross: 640.,
            measurements: vec![],
            focus_view: None,
            interaction_view: None,
        })
        .unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
}
#[test]
fn second_edge_preserves_the_first_request_until_its_unchanged_answer_settles() {
    let mut r = boot();
    feedback(&mut r);
    assert_eq!(
        r.slot("cursor"),
        Some(&Value::Number(0.)),
        "end superseded the start cursor"
    );
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1, "only start may enqueue");
    assert_eq!(requests[0].request.continuation, Some(100));
    for _ in 0..3 {
        feedback(&mut r);
    }
    assert!(
        r.take_requests().is_empty(),
        "kept-answer feedback must not dispatch end"
    );
    let before = r.collections().remove(0);
    r.fulfill(requests[0].ticket, Outcome::Storage(vec![]))
        .unwrap();
    let after = r.collections().remove(0);
    assert_ne!(
        before.rows[0].epoch, after.rows[0].epoch,
        "unchanged geometry needs one follow-up after settlement"
    );
    feedback(&mut r);
    assert_eq!(r.slot("cursor"), Some(&Value::Number(2.)));
    assert_eq!(r.take_requests().len(), 1);
}

#[test]
fn deferred_end_waits_through_continuations_and_skips_a_settled_edge_outside_geometry() {
    let mut r = boot();
    feedback(&mut r);
    let first = r.take_requests().remove(0);
    r.data().continuation = true;
    r.fulfill(first.ticket, Outcome::Storage(vec![])).unwrap();
    let continuation = r.take_requests().remove(0);
    assert_ne!(continuation.ticket, first.ticket);
    feedback(&mut r);
    assert_eq!(r.slot("cursor"), Some(&Value::Number(0.)));
    assert!(r.take_requests().is_empty());
    r.data().value = Value::record(vec![
        Value::list((0..200).map(|i| Value::Number(i as f64)).collect()),
        Value::Number(0.),
        Value::Number(199.),
    ]);
    r.fulfill(continuation.ticket, Outcome::Storage(vec![]))
        .unwrap();
    feedback(&mut r);
    assert_eq!(r.slot("cursor"), Some(&Value::Number(0.)));
    assert!(
        r.take_requests().is_empty(),
        "the new end is outside the geometric window"
    );
    assert!(!r.journal().any(|line| line.contains("reachend view")));
}

#[test]
fn a_request_without_a_slot_change_still_defers_end() {
    let source = SOURCE.replace("cursor = history.earlier", "refresh history");
    let mut r = boot_source(&source);
    feedback(&mut r);
    assert_eq!(r.slot("cursor"), Some(&Value::Number(1.)));
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request.continuation, Some(101));
    feedback(&mut r);
    assert!(r.take_requests().is_empty());
    r.fulfill(requests[0].ticket, Outcome::Storage(vec![]))
        .unwrap();
    feedback(&mut r);
    assert_eq!(r.slot("cursor"), Some(&Value::Number(2.)));
    assert_eq!(r.take_requests().len(), 1);
}

#[test]
fn pure_noop_start_can_dispatch_async_end_in_the_same_call() {
    let mut r = boot_source(&SOURCE.replace("cursor = history.earlier", "cursor = cursor"));
    feedback(&mut r);
    assert_eq!(r.slot("cursor"), Some(&Value::Number(2.)));
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request.continuation, Some(102));
}
