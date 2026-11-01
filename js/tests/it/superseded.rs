//! A send let go between storage steps still finishes its write
//! (splitter rough 7), and its chain arms no timer. Kept out of
//! `storage.rs` for the line cap.
#![cfg(exact_js_engine)]
use super::storage::{Root, APP, GRANTS, HBC};
use exact_js::Module;
use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{DataSource, Dispatch, Runner, Work};
use std::collections::{HashMap, VecDeque};

/// A second send to a mutation whose first is between storage steps:
/// the first's reply is dropped, as the web drops it, but its write still
/// lands. A browser runs the first answer's promise chain to its end; the
/// native host must too, or two quick taps lose a write.
#[test]
fn a_superseded_send_between_storage_steps_still_writes() {
    let root = Root::new();
    let mut bridge = Bridge::boot(
        &root,
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
    );
    // The first save is let go at each of its storage steps in turn.
    for at in 0..5 {
        bridge.act("save", &format!("a{at}"));
        for _ in 0..at {
            bridge.step();
        }
        bridge.act("save", &format!("b{at}"));
        bridge.settle();
        // Every row both sends wrote is there: two per round.
        assert_eq!(
            bridge.text("saved"),
            format!("b{at}:{}", 2 * at + 2),
            "let go at step {at}"
        );
    }
}

/// A send let go during its write runs the rest of its chain once the write
/// lands, as a browser does, but arms no timer there: a timer belongs to an
/// answer, and a let-go one's never runs (LLP 1027.000 D1).
#[test]
fn a_superseded_send_s_chain_arms_no_timer() {
    let root = Root::new();
    let mut bridge = Bridge::boot(
        &root,
        r#"
shape Result
  text: string
component App
  mutation saved as shape Result
  mutation said as shape Result
  action save(value: string)
    send saved = work("write-then-sleep", value)
  action read
    send said = work("slept", "")
  view
    column
      match said
        case some(r)
          text r.text testId="said"
        case none
          text "none" testId="said"
"#,
    );
    bridge.act("save", "a");
    bridge.act("save", "b");
    bridge.settle();
    bridge.act("read", "");
    bridge.settle();
    let said = bridge.text("said");
    assert!(
        said.starts_with("a refused: setTimeout() is unavailable here")
            && said.ends_with("as long as its HTTP headers allow; b armed"),
        "{said}"
    );
}

/// The runner over the storage fixture, driven as a native bridge drives it
/// after every commit: what was asked is dispatched, what was released runs.
struct Bridge {
    runner: Runner<Module>,
    held: HashMap<u64, u64>,
    work: VecDeque<(u64, Work)>,
}

impl Bridge {
    fn boot(root: &Root, contract: &str) -> Bridge {
        let plan = contract::compile(contract).unwrap();
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
        let runner =
            Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
        Bridge {
            runner,
            held: HashMap::new(),
            work: VecDeque::new(),
        }
    }

    fn act(&mut self, action: &str, value: &str) {
        let args = if value.is_empty() {
            vec![]
        } else {
            vec![Value::str(value)]
        };
        self.runner.act(action, args).unwrap();
        self.emit();
    }

    fn emit(&mut self) {
        for request in self.runner.take_requests() {
            let token = request.request.continuation.expect("storage continuation");
            match crate::runnable(self.runner.dispatch_work(token)) {
                Dispatch::Run(w) => self.work.push_back((request.ticket, w)),
                Dispatch::Held => {
                    self.held.insert(token, request.ticket);
                }
                _ => panic!("native storage work runs or is held"),
            }
        }
        for (token, dispatch) in self.runner.release_work() {
            let Dispatch::Run(w) = crate::runnable(dispatch) else {
                panic!("released work runs")
            };
            if let Some(ticket) = self.held.remove(&token) {
                self.work.push_back((ticket, w));
            }
        }
    }

    /// Runs the next work out and lands it: false when there was none.
    fn step(&mut self) -> bool {
        let Some((ticket, w)) = self.work.pop_front() else {
            return false;
        };
        let Work::Now(w) = w else {
            panic!("native work runs now")
        };
        let outcome = std::thread::spawn(w).join().unwrap();
        self.runner.fulfill(ticket, outcome).unwrap();
        self.emit();
        true
    }

    fn settle(&mut self) {
        for _ in 0..400 {
            if !self.step() {
                break;
            }
        }
        assert!(!self.runner.has_pending(), "every send settled");
    }

    fn text(&self, id: &str) -> String {
        let key = self.runner.kernel().find_by_test_id(id)[0];
        let node = self.runner.kernel().node_by_key(key).unwrap();
        node.props.str(PropId::Text).unwrap_or_default().to_owned()
    }
}
