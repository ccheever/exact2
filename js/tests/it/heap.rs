//! The executor collects between calls once the module has allocated enough
//! since the last full collection (js/src/shim.cc, `collect_if_due`), so an
//! object nothing reaches is finalized by the next call.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_plan::Plan;
use exact_runner::{Answer, DataSource, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/heap.hbc"));

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
  view
    column
      text atInit
"#;

fn module() -> Module {
    let plan: Plan = contract::compile(SRC).expect("the fixture's Contract compiles");
    let mut m = Module::new(HBC.to_vec(), "test.heap", "");
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
    let mut s = Store::new("", []);
    // Hardening's collection precedes initialization; the first call's
    // check finds the megabyte initialization allocated since.
    assert_eq!(state(&mut m, &mut s, "atInit"), "held");
    assert_eq!(state(&mut m, &mut s, "atInit"), "cleared");
}

#[test]
fn a_call_that_collected_within_itself_is_still_followed_by_a_full_collection() {
    let mut m = module();
    let mut s = Store::new("", []);
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
    let mut s = Store::new("", []);
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
    let mut s = Store::new("", []);
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
