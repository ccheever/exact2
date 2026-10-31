//! A data-module turn is held to its budget in its own time, the calling
//! thread's CPU, with a wall backstop of ten budgets (src/turn.rs): a turn
//! that waits without working is within 100 ms however long the machine
//! keeps it off the core, up to the backstop; one that works past 100 ms
//! is refused as before. (Under load a wall-clock budget refused a turn of
//! 60 ms of work that read 247 ms.)
#![cfg(exact_js_engine)]
use super::storage::{args, call, Root, APP, GRANTS, HBC};
use exact_js::{Module, NativeModule};
use exact_kernel::Kernel;
use exact_runner::{DataError, DataSource, Runner, Store};
use std::time::Duration;

/// A native module whose synchronous call waits `WAIT` without working, as
/// a hosted call that hops to another thread does.
struct Waits<const MS: u64>;
impl<const MS: u64> NativeModule for Waits<MS> {
    fn configure_storage(&mut self, _: exact_js::NativeStorage) -> Result<(), String> {
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
    let mut m = module(&root, |_| Box::new(Waits::<150>));
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    // 150 ms of wall, almost none of it the module's: a wall-clock budget refused it. (Only a
    // machine that kept the thread off the core another 850 ms meets the backstop: then the
    // refusal is the backstop's, which this test lets by, never the budget's.)
    let a = args("native-direct", "waited");
    match m.answer(&mut s, "work", &a) {
        Ok(answer) => assert_eq!(super::storage::finish(&mut m, &mut s, &a, answer), "waited"),
        Err(DataError::Overrun(e)) if e.contains("a turn may take") => return,
        Err(e) => panic!("a waiting turn was refused: {e:?}"),
    }
    assert_eq!(m.overruns(), 0);
    // Placed on main, where the UI waits on its thread: the hold is said, not
    // refused (theme2's 15:57 ruling); the runner's journal: below.
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
        Err(DataError::Overrun(e)) => {
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
        Err(DataError::Overrun(e)) => assert!(
            e.contains("over the 100 ms budget for one turn") && e.contains("scheduler.yield()"),
            "{e}"
        ),
        other => panic!("expected the budget's refusal, got {other:?}"),
    }
    assert_eq!(m.overruns(), 1);
}

#[test]
fn a_turn_that_held_the_ui_thread_is_said_in_the_runners_journal() {
    let root = Root::new();
    let plan = contract::compile(
        r#"
shape Result
  text: string
component App
  resource r = work("native-direct", "held") as shape Result else work("placeholder", "")
  view
    text r.text testId="r"
"#,
    )
    .unwrap();
    let mut m = Module::new(HBC.to_vec(), APP, GRANTS).with_native(|_| Box::new(Waits::<150>));
    m.set_budget_ms(100.0);
    m.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("tmp"),
    )
    .unwrap();
    m.bind(&plan);
    m.activate().unwrap();
    // (a machine that kept the turn off the core past the backstop refuses it, and boot fails with
    // the backstop's words: let by, as in the waiting test; anything else fails)
    let runner = match Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/") {
        Ok(runner) => runner,
        Err(e) if format!("{e:?}").contains("a turn may take") => return,
        Err(e) => panic!("boot: {e:?}"),
    };
    let journal: Vec<String> = runner.journal().map(str::to_owned).collect();
    // The thread and both clocks, from the ring.
    assert!(
        journal
            .iter()
            .any(|l| l.contains("a data turn held the UI thread")
                && l.contains("ms of its own")
                && l.contains("`work`")),
        "{journal:?}"
    );
}
