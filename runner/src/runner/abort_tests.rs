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

/// [`Held`], keeping each let-go POST as module work (LLP 1146 D2) and
/// hearing what each kept one's reply was.
#[derive(Default)]
struct Kept {
    held: Held,
    landed: Arc<Mutex<Vec<(u64, Outcome)>>>,
}

impl DataSource for Kept {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.held.query(source, args)
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.held.answer(store, source, args)
    }
    fn take_aborted(&mut self) -> Vec<u64> {
        self.held.take_aborted()
    }
    fn keep_sent(&mut self, _: &Store, let_go: &[InFlight<'_>], _: bool) -> Vec<Option<u64>> {
        (0..let_go.len()).map(|i| Some(100 + i as u64)).collect()
    }
    fn module_landed(
        &mut self,
        _: &mut Store,
        token: u64,
        outcome: Outcome,
    ) -> Result<(), DataError> {
        self.landed.lock().unwrap().push((token, outcome));
        Ok(())
    }
}

/// The runner, the door's aborts, what kept work landed with, and the button.
type KeptRig = (
    Runner<Kept>,
    Arc<Mutex<Vec<u64>>>,
    Arc<Mutex<Vec<(u64, Outcome)>>>,
    ViewId,
);

fn kept_runner() -> KeptRig {
    let source = Kept::default();
    let (aborted, landed) = (source.held.aborted.clone(), source.landed.clone());
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let r = Runner::boot(
        plan,
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let hold = k.node_by_key(k.find_by_test_id("hold")[0]).unwrap().id;
    (r, aborted, landed, hold)
}

/// A let-go POST kept as module work, then aborted by the app: cancelled,
/// and answered aborted, so the work settles (its continuation hears it)
/// rather than waiting for a reply the cancel dropped.
#[test]
fn an_aborted_write_kept_as_module_work_settles_aborted() {
    let (mut r, aborted, landed, hold) = kept_runner();
    r.dispatch(hold, Event::Press).unwrap();
    let first = r.take_requests();
    r.dispatch(hold, Event::Press).unwrap();
    r.take_requests();
    assert!(
        r.is_module_work(first[0].ticket),
        "the replaced POST is kept"
    );
    aborted.lock().unwrap().push(1);
    assert!(r.collected_aborts());
    let (ticket, outcome) = r.take_request_refusal(false).expect("an answer is owed");
    assert_eq!(ticket, first[0].ticket);
    assert_eq!(r.take_explicit_aborts(), [first[0].ticket]);
    r.fulfill(ticket, outcome).unwrap();
    assert!(
        !r.is_module_work(first[0].ticket),
        "the module work settled"
    );
    let landed = landed.lock().unwrap();
    assert!(
        matches!(
            landed.as_slice(),
            [(
                100,
                Outcome::Failed {
                    kind: crate::FailureKind::Aborted,
                    ..
                }
            )]
        ),
        "{landed:?}"
    );
}

/// A kept write's abort still names it however many requests are let go
/// after it: module work is named for as long as it runs, past the retired
/// records' cap.
#[test]
fn a_kept_write_s_abort_names_it_past_the_retired_cap() {
    let (mut r, aborted, _, hold) = kept_runner();
    r.dispatch(hold, Event::Press).unwrap();
    let first = r.take_requests();
    for _ in 0..300 {
        r.dispatch(hold, Event::Press).unwrap();
        r.take_requests();
    }
    aborted.lock().unwrap().push(1);
    r.collect_aborts();
    let _ = r.take_request_refusal(false);
    assert!(r.take_explicit_aborts().contains(&first[0].ticket));
}
