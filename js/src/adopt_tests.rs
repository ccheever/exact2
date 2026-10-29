//! An adopted fetch (LLP 1146 D2) on the module itself: only one an answer's
//! settle handed out is adopted; the app's abort of it waits for the module
//! turn, unless the host has no door 23; and it ends with its instance, so no
//! surviving realm awaits it.
use super::Module;
use exact_plan::Value;
use exact_runner::{Answer, DataSource, FailureKind, Outcome, Store, Target};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/outlive.hbc"));
const GRANTS: &str =
    "net.fetch https://outlive.test\nnet.fetch local-network development\nsecret.keep outlive.token\n";
const KEY: &str = "outlive.token";
const CONTRACT: &str = "shape Row
  text: string

component App
  resource twice = twice() as shape Row
  resource write = write(0, \"http://127.0.0.1:9/token\") as shape Row
  resource cancel = cancelAwait(2) as shape Row
  view
    column
      text twice.text
      text write.text
      text cancel.text
";

fn module() -> (Module, Store) {
    let mut m = Module::loaded(HBC.to_vec(), "test.outlive", GRANTS).expect("the fixture loads");
    m.set_budget_ms(f64::INFINITY);
    m.bind(&contract::compile(CONTRACT).expect("the fixture's Contract compiles"));
    (m, Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new()))
}

fn adopt(m: &mut Module, call: u64, ticket: u64) -> String {
    let engine = m.engine.as_mut().expect("loaded");
    (engine.call(
        "__exact_adopt",
        [&call.to_string(), &ticket.to_string(), ""],
    ))
    .expect("__exact_adopt answers")
}

/// `write`'s POST, handed out, adopted and its call forgotten as
/// `keep_sent_now` does; then `cancelAwait` aborts it. Its ticket.
fn adopted_then_aborted(m: &mut Module, s: &mut Store) -> u64 {
    let a = m.answer_for(
        Target::Resource(1),
        s,
        "write",
        &[Value::Number(0.0), "http://127.0.0.1:9/token".into()],
    );
    assert!(matches!(a, Ok(Answer::Later(_))), "{a:?}");
    let (_, parked) = m.parked.remove(0);
    assert_eq!(adopt(m, parked.call, parked.ticket), "adopted");
    m.forget_calls(vec![parked.call]);
    let _ = m.answer_for(Target::Resource(2), s, "cancelAwait", &[Value::Number(2.0)]);
    parked.ticket
}

#[test]
fn only_a_fetch_handed_out_is_adopted() {
    let (mut m, mut s) = module();
    let a = m.answer_for(Target::Resource(0), &mut s, "twice", &[]);
    assert!(matches!(a, Ok(Answer::Later(_))), "{a:?}");
    let (call, first) = (m.parked[0].1.call, m.parked[0].1.ticket);
    // The second POST, made before the first was awaited, is not handed out.
    assert_eq!(m.unsent(), 1);
    assert_eq!(
        adopt(&mut m, call, first + 1),
        "",
        "an unclaimed fetch is not adopted"
    );
    assert_eq!(adopt(&mut m, call, first), "adopted");
}

#[test]
fn an_aborted_adopted_fetch_rejects_in_its_module_turn() {
    let (mut m, mut s) = module();
    let ticket = adopted_then_aborted(&mut m, &mut s);
    // Cancelled on the host, not rejected in the aborting answer's turn.
    assert_eq!(m.take_aborted(), [ticket]);
    assert_eq!(s.get(KEY), None, "nothing ran in the aborting turn");
    assert!(
        m.host.module_outstanding > 0,
        "the adopted fetch is outstanding"
    );
    let aborted = Outcome::Failed {
        kind: FailureKind::Aborted,
        message: "the app aborted the fetch".into(),
    };
    m.module_landed_now(&mut s, ticket, aborted)
        .expect("the module turn runs");
    // The let-go call's catch ran in the module turn; what is still
    // outstanding is the aborting answer's own step, which waits for its turn.
    assert_eq!(s.get(KEY), Some("aborted AbortError"));
}

#[test]
fn without_door_23_an_aborted_adopted_fetch_rejects_at_once() {
    let (mut m, mut s) = module();
    m.refuse_door_op(23);
    let ticket = adopted_then_aborted(&mut m, &mut s);
    assert!(m.take_aborted().is_empty());
    assert_eq!(s.get(KEY), Some("aborted AbortError"));
    // Its late answer finds nothing to settle.
    let aborted = Outcome::Failed {
        kind: FailureKind::Aborted,
        message: "the app aborted the fetch".into(),
    };
    m.module_landed_now(&mut s, ticket, aborted)
        .expect("the module turn runs");
    assert_eq!(m.host.module_outstanding, 0);
}

/// Torn down while an aborted adopted fetch waits for its module turn, the
/// instance takes the fetch, its continuation and the answer awaiting it
/// along: a module loaded again tracks none of them, and the kept ticket's
/// late answer reaches no realm.
#[test]
fn an_adopted_fetch_ends_with_its_instance() {
    let (mut m, mut s) = module();
    let ticket = adopted_then_aborted(&mut m, &mut s);
    // Aborted, and waiting for its module turn, when the instance goes.
    assert_eq!(s.get(KEY), None);
    assert!(m.host.module_outstanding > 0);
    assert_eq!(m.host.aborted, [ticket]);
    assert!(m.calls_open().is_some_and(|n| n > 0));
    m.unload();
    assert_eq!(m.calls_open(), None);
    assert_eq!((m.host.module_outstanding, m.in_flight()), (0, 0));
    assert!(m.take_aborted().is_empty());
    let aborted = Outcome::Failed {
        kind: FailureKind::Aborted,
        message: "the app aborted the fetch".into(),
    };
    assert!(m.module_landed_now(&mut s, ticket, aborted).is_err());
    m.load().expect("the module loads again");
    assert_eq!(m.calls_open(), Some(0));
    assert_eq!(s.get(KEY), None);
}
