//! A data-module turn is held to its budget in its own time, the calling
//! thread's CPU, with a wall backstop of ten budgets (src/turn.rs): a turn
//! that waits without working is within 100 ms however long the machine
//! keeps it off the core, up to the backstop; one that works past 100 ms
//! is refused as before. (Under load a wall-clock budget refused a turn of
//! 60 ms of work that read 247 ms.)
#![cfg(exact_js_engine)]
use super::storage::{args, call, Root, GRANTS};
use exact_js::{Module, NativeModule};
use exact_runner::{DataError, DataSource, Store};
use std::path::PathBuf;
use std::time::Duration;

/// A native module whose synchronous call waits `WAIT` without working, as
/// a hosted call that hops to another thread does.
struct Waits<const MS: u64>;
impl<const MS: u64> NativeModule for Waits<MS> {
    fn configure_storage(&mut self, _: PathBuf, _: PathBuf, _: PathBuf) -> Result<(), String> {
        Ok(())
    }
    fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        std::thread::sleep(Duration::from_millis(MS));
        Ok(serde_json::json!({ "text": request["value"] }))
    }
}

fn module(root: &Root, native: fn(&str) -> Box<dyn NativeModule>) -> Module {
    let mut m = root.module().with_native(native);
    m.set_budget_ms(100.0);
    m.activate().unwrap();
    m
}

#[test]
fn a_turn_that_waits_without_working_is_within_its_budget() {
    let root = Root::new();
    let mut m = module(&root, |_| Box::new(Waits::<250>));
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    // 250 ms of wall, almost none of it the module's: a wall-clock budget refused it.
    assert_eq!(call(&mut m, &mut s, "native-direct", "waited"), "waited");
    assert_eq!(m.overruns(), 0);
    // Placed on main, where the UI waits on its thread: the hold is said in
    // the journal, not refused (theme2's 15:57 ruling).
    let logs = m.take_logs();
    assert!(
        logs.iter()
            .any(|l| l.starts_with("a data turn held the UI thread") && l.contains("`work`")),
        "{logs:?}"
    );
}

#[test]
fn a_turn_that_waits_past_the_backstop_is_refused() {
    let root = Root::new();
    let mut m = module(&root, |_| Box::new(Waits::<1100>));
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    match m.answer(&mut s, "work", &args("native-direct", "too long")) {
        Err(DataError::Unavailable(e)) => {
            assert!(e.contains("over the 1000 ms a turn may take"), "{e}")
        }
        other => panic!("expected the backstop's refusal, got {other:?}"),
    }
    assert_eq!(m.overruns(), 1);
}

#[test]
fn a_turn_that_works_past_its_budget_is_refused() {
    let root = Root::new();
    let mut m = module(&root, |_| Box::new(Waits::<0>));
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    // A short loop is within it.
    assert_eq!(call(&mut m, &mut s, "busy", "1000"), "496491"); // (7 × 499500 mod 1000003)
                                                                // Enough iterations for well over 100 ms of the module's own time on any machine.
    match m.answer(&mut s, "work", &args("busy", "30000000")) {
        Err(DataError::Unavailable(e)) => assert!(
            e.contains("over the 100 ms budget for one turn") && e.contains("scheduler.yield()"),
            "{e}"
        ),
        other => panic!("expected the budget's refusal, got {other:?}"),
    }
    assert_eq!(m.overruns(), 1);
}
