//! The executor collects between calls once the module has allocated enough
//! since the last full collection (js/src/shim.cc, `collect_if_due`), so an
//! object nothing reaches is finalized by the next call.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_plan::Plan;
use exact_runner::{Answer, DataSource, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/heap.hbc"));

const SRC: &str = r#"
component App
  resource atInit = atInit() as shape string
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
