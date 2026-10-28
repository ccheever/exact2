//! An app's abort that the runner hears after a newer send replaced its
//! request, as the web does (the replacing answer's turn runs after the
//! swap): no answer is owed, but the host is still told to cancel it.
use super::*;
use crate::{Event, Store};
use exact_kernel::Kernel;
use std::sync::{Arc, Mutex};

/// `hold()` asks a POST, fetch tickets 1, 2, …; `aborted` is what the
/// module's door said since the last take.
#[derive(Default)]
struct Held {
    fetches: u64,
    aborted: Arc<Mutex<Vec<u64>>>,
}

impl DataSource for Held {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        self.fetches += 1;
        let mut request = Request::post_json("https://example.test/held", "{}");
        request.fetch = Some(self.fetches);
        Ok(Answer::Later(request))
    }
    fn take_aborted(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.aborted.lock().unwrap())
    }
}

const SRC: &str = r#"component App
  mutation m as shape bool
  action hold
    send m = hold()
  view
    column
      button press=hold testId="hold"
        text "Hold"
"#;

#[test]
fn an_abort_heard_after_its_request_was_replaced_still_cancels_it() {
    let source = Held::default();
    let aborted = source.aborted.clone();
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(
        plan,
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let hold = k.node_by_key(k.find_by_test_id("hold")[0]).unwrap().id;
    r.dispatch(hold, Event::Press).unwrap();
    let first = r.take_requests();
    assert_eq!(first.len(), 1);
    // A newer send replaces the held POST, which is let run, before the
    // replacing answer's turn aborts it.
    r.dispatch(hold, Event::Press).unwrap();
    let second = r.take_requests();
    assert_eq!(second.len(), 1);
    aborted.lock().unwrap().push(1);
    assert!(r.collected_aborts(), "a pump is owed for the cancel");
    // Whether an answer is owed is the pump's to say, from what is pending
    // then (a refused commit may have put the request back).
    assert!(r.has_module_aborts());
    assert!(r.take_explicit_aborts().is_empty());
    assert!(r.take_request_refusal(false).is_none(), "no answer is owed");
    assert_eq!(r.take_explicit_aborts(), [first[0].ticket]);
    assert!(!r.collected_aborts());
    assert!(!r.has_request_refusals(false));
    // Heard again later, it names nothing.
    aborted.lock().unwrap().push(1);
    r.collect_aborts();
    assert!(r.take_explicit_aborts().is_empty());
}

#[test]
fn an_abort_of_a_request_two_sends_back_still_cancels_it_until_its_reply() {
    let source = Held::default();
    let aborted = source.aborted.clone();
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(
        plan,
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let hold = k.node_by_key(k.find_by_test_id("hold")[0]).unwrap().id;
    let mut sent = Vec::new();
    for _ in 0..3 {
        r.dispatch(hold, Event::Press).unwrap();
        sent.extend(r.take_requests().into_iter().map(|r| r.ticket));
    }
    assert_eq!(sent.len(), 3);
    // The app still holds the first one's controller, two sends back.
    aborted.lock().unwrap().push(1);
    assert!(r.collected_aborts());
    assert!(r.take_request_refusal(false).is_none(), "no answer is owed");
    assert_eq!(r.take_explicit_aborts(), [sent[0]]);
    // The second one's reply comes: it has ended, and its abort names nothing.
    assert!(r
        .fulfill(
            sent[1],
            Outcome::Failed {
                kind: crate::FailureKind::Network,
                message: "x".into()
            }
        )
        .unwrap()
        .is_none());
    aborted.lock().unwrap().push(2);
    assert!(!r.collected_aborts());
    assert!(r.take_explicit_aborts().is_empty());
}

#[test]
fn a_request_a_refused_commit_retires_again_is_recorded_once() {
    let source = Held::default();
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(
        plan,
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let hold = k.node_by_key(k.find_by_test_id("hold")[0]).unwrap().id;
    r.dispatch(hold, Event::Press).unwrap();
    assert_eq!(r.take_requests().len(), 1);
    // A refused commit retires the request, then puts it back, again and again.
    let held = r.pending[0].clone();
    for _ in 0..300 {
        r.note_retired(&held);
    }
    assert_eq!(r.retired_fetches, [(1, held.ticket)]);
}
