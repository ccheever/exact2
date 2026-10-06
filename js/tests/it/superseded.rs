//! A send let go between storage steps still finishes its write
//! (splitter rough 7). Kept out of `storage.rs` for the line cap.
#![cfg(exact_js_engine)]
use super::storage::{Root, APP, GRANTS, HBC};
use exact_js::Module;
use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{DataSource, Runner};

/// A second send to a mutation whose first is between storage steps:
/// the first's reply is dropped, as the web drops it, but its write still
/// lands. A browser runs the first answer's promise chain to its end; the
/// native host must too, or two quick taps lose a write.
#[test]
fn a_superseded_send_between_storage_steps_still_writes() {
    let root = Root::new();
    let plan = contract::compile(
        r#"
shape Result
  text: string
component App
  mutation saved as shape Result
  action save(value: string)
    send saved = work("serial", value)
  view
    column
      match saved
        case some(r)
          text r.text testId="saved"
        case none
          text "none" testId="saved"
"#,
    )
    .unwrap();
    let mut m = Module::new(HBC.to_vec(), APP, GRANTS);
    m.set_budget_ms(f64::INFINITY);
    m.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("tmp"),
    )
    .unwrap();
    m.bind(&plan);
    m.activate().unwrap();
    let mut runner =
        Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let mut held = std::collections::HashMap::new();
    let mut work: std::collections::VecDeque<(u64, exact_runner::Work)> = Default::default();
    // The bridge after every commit: dispatch what was asked, run what was released.
    let emit = |runner: &mut Runner<Module>,
                held: &mut std::collections::HashMap<u64, u64>,
                work: &mut std::collections::VecDeque<(u64, exact_runner::Work)>| {
        for request in runner.take_requests() {
            let token = request.request.continuation.expect("storage continuation");
            match runner.dispatch_work(token) {
                exact_runner::Dispatch::Run(w) => work.push_back((request.ticket, w)),
                exact_runner::Dispatch::Held => {
                    held.insert(token, request.ticket);
                }
                _ => panic!("native storage work runs or is held"),
            }
        }
        for (token, dispatch) in runner.release_work() {
            let exact_runner::Dispatch::Run(w) = dispatch else {
                panic!("released work runs")
            };
            if let Some(ticket) = held.remove(&token) {
                work.push_back((ticket, w));
            }
        }
    };
    let step = |runner: &mut Runner<Module>,
                held: &mut std::collections::HashMap<u64, u64>,
                work: &mut std::collections::VecDeque<(u64, exact_runner::Work)>|
     -> bool {
        let Some((ticket, w)) = work.pop_front() else {
            return false;
        };
        let exact_runner::Work::Now(w) = w else {
            panic!("native work runs now")
        };
        let outcome = std::thread::spawn(w).join().unwrap();
        runner.fulfill(ticket, outcome).unwrap();
        emit(runner, held, work);
        true
    };
    let saved = |runner: &Runner<Module>| {
        let key = runner.kernel().find_by_test_id("saved")[0];
        let node = runner.kernel().node_by_key(key).unwrap();
        node.props.str(PropId::Text).unwrap_or_default().to_owned()
    };
    // The first save is let go at each of its storage steps in turn.
    for at in 0..5 {
        runner
            .act("save", vec![Value::str(&format!("a{at}"))])
            .unwrap();
        emit(&mut runner, &mut held, &mut work);
        for _ in 0..at {
            step(&mut runner, &mut held, &mut work);
        }
        runner
            .act("save", vec![Value::str(&format!("b{at}"))])
            .unwrap();
        emit(&mut runner, &mut held, &mut work);
        for _ in 0..400 {
            if !step(&mut runner, &mut held, &mut work) {
                break;
            }
        }
        assert!(!runner.has_pending(), "both sends settled");
        // Every row both sends wrote is there: two per round.
        assert_eq!(
            saved(&runner),
            format!("b{at}:{}", 2 * at + 2),
            "let go at step {at}"
        );
    }
}
