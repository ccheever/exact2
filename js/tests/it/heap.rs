//! The executor collects between calls once the module has allocated enough
//! since the last full collection (js/src/shim.cc, `collect_if_due`), so an
//! object nothing reaches is finalized by the next call.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_plan::Plan;
use exact_plan::Value;
use exact_runner::{Answer, DataSource, Outcome, Response, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/heap.hbc"));
const GRANTS: &str = "net.fetch https://heap.test";

const SRC: &str = r#"
shape Big
  text: string
component App
  resource atInit = atInit() as shape string
  resource large = large() as shape string
  resource collections = collections() as shape string
  resource heldLarge = heldLarge() as shape Big
  resource heldLargeState = heldLargeState() as shape string
  resource big = big() as shape string
  resource inCall = inCall() as shape string
  resource overlap = overlap(0) as shape string
  resource overlapCollected = overlapCollected() as shape string
  resource overlapCleared = overlapCleared(0) as shape string
  view
    column
      text atInit
"#;

fn module() -> Module {
    let plan: Plan = contract::compile(SRC).expect("the fixture's Contract compiles");
    let mut m = Module::new(HBC.to_vec(), "test.heap", GRANTS);
    m.set_budget_ms(f64::INFINITY);
    m.bind(&plan);
    m.load().expect("the fixture loads");
    m
}

fn state(m: &mut Module, s: &mut Store, source: &str) -> String {
    match m.answer(s, source, &[]) {
        Ok(Answer::Now(v)) if v.as_str().is_some() => v.as_str().unwrap_or("").to_string(),
        other => panic!("`{source}`: expected a string, got {other:?}"),
    }
}

#[test]
fn what_module_initialization_left_is_collected_after_the_first_call() {
    let mut m = module();
    let mut s = Store::new(GRANTS, []);
    // Hardening's collection precedes initialization; the first call's
    // check finds the megabyte initialization allocated since.
    assert_eq!(state(&mut m, &mut s, "atInit"), "held");
    assert_eq!(state(&mut m, &mut s, "atInit"), "cleared");
}

#[test]
fn a_call_that_collected_within_itself_is_still_followed_by_a_full_collection() {
    let mut m = module();
    let mut s = Store::new(GRANTS, []);
    state(&mut m, &mut s, "atInit");
    // The object outlives the young collections within the call, so it is
    // old generation, which only a full collection reclaims.
    assert_eq!(state(&mut m, &mut s, "big"), "held");
    assert_eq!(state(&mut m, &mut s, "inCall"), "cleared");
}

/// A large string made and sent in one answer, as a frame's is, is left to
/// the allocation-driven collections: a frame does not collect each time.
#[test]
fn a_large_reply_each_frame_does_not_collect_each_frame() {
    let mut m = module();
    let mut s = Store::new(GRANTS, []);
    state(&mut m, &mut s, "atInit");
    for _ in 0..20 {
        assert!(state(&mut m, &mut s, "large").len() > 65536);
    }
    let collected: u32 = state(&mut m, &mut s, "collections").parse().unwrap();
    assert!(
        collected <= 10,
        "{collected} of 20 frames followed a collection"
    );
}

/// A large string an answer held past a collection is collected after the
/// reply that sends it, though nothing else would make a collection due.
#[test]
fn a_large_reply_held_past_a_collection_is_collected_after_it() {
    let mut m = module();
    let mut s = Store::new(GRANTS, []);
    state(&mut m, &mut s, "atInit");
    assert!(matches!(
        m.answer(&mut s, "heldLarge", &[]),
        Ok(Answer::Now(_))
    ));
    // The registry reports at the drain that ends the call after the
    // collection; with no collection, the second call still finds it held.
    state(&mut m, &mut s, "heldLargeState");
    assert_eq!(state(&mut m, &mut s, "heldLargeState"), "cleared");
}

/// Two answers in flight at once, each replying with a large string after
/// a collection began while it waited: the collection a reply owes waits
/// until no answer awaits one, so replies alternating with new answers do
/// not collect each time (each collected the next one's value had marked),
/// and the last reply makes it.
#[test]
fn overlapping_large_replies_collect_once_none_is_pending() {
    let mut m = module();
    let mut s = Store::new(GRANTS, []);
    state(&mut m, &mut s, "atInit");
    let ok = || {
        Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: b"ok".to_vec(),
        })
    };
    let n = |i: u32| [Value::Number(f64::from(i))];
    assert!(matches!(
        m.answer(&mut s, "overlap", &n(0)),
        Ok(Answer::Later(_))
    ));
    for i in 1..=20 {
        assert!(matches!(
            m.answer(&mut s, "overlap", &n(i)),
            Ok(Answer::Later(_))
        ));
        assert!(matches!(
            m.parse(&mut s, "overlap", &n(i - 1), ok()),
            Ok(Answer::Now(_))
        ));
    }
    let collected: u32 = state(&mut m, &mut s, "overlapCollected").parse().unwrap();
    assert!(
        collected <= 2,
        "{collected} of 20 overlapping replies followed a collection"
    );
    assert!(matches!(
        m.parse(&mut s, "overlap", &n(20), ok()),
        Ok(Answer::Now(_))
    ));
    // The registry reports at the drain that ends the call after the
    // collection; with none, the second call still finds the sentinel held.
    let cleared = |m: &mut Module, s: &mut Store| match m.answer(s, "overlapCleared", &n(20)) {
        Ok(Answer::Now(v)) => v.as_str().unwrap_or("").to_string(),
        other => panic!("expected a string, got {other:?}"),
    };
    cleared(&mut m, &mut s);
    assert_eq!(cleared(&mut m, &mut s), "cleared");
}
