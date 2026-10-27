//! A resource a turn over its budget refused is asked again (theme2's 18:06
//! ruling; runner/src/runner/overrun.rs), over every hop a real app uses: the
//! module in process (`Main`), on its own owner (`Worker`, the reply's
//! envelope), and behind the mixed composer (its envelope). The fixture's
//! `busy-once` runs over 100 ms of its own time the first time and answers at
//! once after. Driven on the runner's own clock, never by wall sleeps.
#![cfg(exact_js_engine)]
use super::storage::{Root, APP, GRANTS, HBC};
use exact_js::{Module, Placement};
use exact_kernel::Kernel;
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource, Dispatch, Reply, Runner, Store, Work};

/// The resource, with a placeholder when `stand_in` (a worker's answers come later, and so would a
/// placeholder's: it has none).
fn plan_with(stand_in: bool) -> Plan {
    let placeholder = if stand_in {
        r#" else work("placeholder", "")"#
    } else {
        ""
    };
    contract::compile(&format!(
        "shape Result\n  text: string\ncomponent App\n  resource r = work(\"busy-once\", \"x\") as shape Result{placeholder}\n  view\n    text r.text testId=\"r\"\n"
    ))
    .unwrap()
}

fn plan() -> Plan {
    plan_with(true)
}

fn module(root: &Root) -> Module {
    let mut m = Module::new(HBC.to_vec(), APP, GRANTS);
    m.set_budget_ms(100.0);
    m.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("tmp"),
    )
    .unwrap();
    m
}

/// Every request the runner hands out run through its source's dispatch, the
/// reply fulfilled, until none is left; what each reply that crossed a turn's
/// envelope decodes to on the runner's side, in order.
fn drive<D: DataSource>(r: &mut Runner<D>) -> Vec<String> {
    let mut decoded = Vec::new();
    for _ in 0..20 {
        let requests = r.take_requests();
        if requests.is_empty() {
            return decoded;
        }
        for request in requests {
            let token = request.request.continuation.expect("a turn's continuation");
            let outcome = match r.dispatch_work(token) {
                Dispatch::Run(Work::Now(work)) => std::thread::spawn(work).join().unwrap(),
                Dispatch::Run(Work::Later(work)) => {
                    let (tx, rx) = std::sync::mpsc::channel();
                    work(Reply::new(move |outcome| {
                        let _ = tx.send(outcome);
                    }));
                    rx.recv_timeout(std::time::Duration::from_secs(60))
                        .expect("the turn replied")
                }
                _ => panic!("a turn to run"),
            };
            if exact_data::envelope::is_turn(&outcome) {
                let mut store = Store::new(GRANTS, []);
                let reply =
                    exact_data::envelope::apply(outcome.clone(), &mut store, &mut Vec::new());
                decoded.push(match reply {
                    Err(DataError::Overrun(_)) => "Overrun".to_string(),
                    Err(e) => format!("{e:?}"),
                    Ok(_) => "an answer".to_string(),
                });
            }
            r.fulfill(request.ticket, outcome).unwrap();
        }
    }
    decoded
}

fn text<D: DataSource>(r: &Runner<D>) -> String {
    match r.resource("r") {
        Some(Value::Record(fields)) => fields[0].as_str().unwrap_or("").to_string(),
        other => format!("{other:?}"),
    }
}

/// Booted, its first answer over budget, then asked again at 250 ms. Over an
/// envelope (`crosses`), the refusal decodes as `Overrun` on the far side.
fn asks_again<D: DataSource>(mut r: Runner<D>, hop: &str, crosses: bool) {
    let decoded = drive(&mut r);
    let want: &[&str] = if crosses { &["Overrun"] } else { &[] };
    assert_eq!(decoded, want, "{hop}: what crossed the envelope");
    let journal: Vec<String> = r.journal().map(str::to_owned).collect();
    assert!(
        journal
            .iter()
            .any(|l| l.contains("resource r:") && l.contains("asked again in 250 ms (1 of 2)")),
        "{hop}: {journal:?}"
    );
    assert!(
        r.failed_resources().is_empty(),
        "{hop}: no failure shows while it waits"
    );
    assert!(
        r.has_pending() || text(&r) != "answered",
        "{hop}: it waits as an ask in flight"
    );
    let t0 = r.now_ms();
    r.advance(t0 + 250.).unwrap();
    let decoded = drive(&mut r);
    let want: &[&str] = if crosses { &["an answer"] } else { &[] };
    assert_eq!(decoded, want, "{hop}: the re-ask's reply");
    assert_eq!(text(&r), "answered", "{hop}: asked again, and answered");
    assert!(r.failed_resources().is_empty(), "{hop}");
    assert_eq!(r.timer_due_ms(), None, "{hop}: nothing left to ask");
}

#[test]
fn an_overrun_is_asked_again_in_process() {
    let root = Root::new();
    let plan = plan();
    let mut m = module(&root);
    m.bind(&plan);
    m.activate().unwrap();
    let r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    asks_again(r, "in process", false);
}

#[test]
fn an_overrun_is_asked_again_from_a_worker() {
    let root = Root::new();
    let plan = plan_with(false);
    let mut m = module(&root).placed(Placement::Worker);
    m.bind(&plan);
    m.activate().unwrap();
    let r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    asks_again(r, "worker", true);
}

/// A Rust child for the mixed composer: it answers nothing here, but keeps a
/// secret the module keeps too, on a worker, so the two are one ordered set and
/// the module's `Main` turns cross the composer's envelope.
struct Quiet;
impl DataSource for Quiet {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn app_id(&self) -> &str {
        APP
    }
    fn grants(&self) -> &str {
        "secret.keep session\n"
    }
}

#[test]
fn an_overrun_is_asked_again_behind_the_mixed_composer() {
    let root = Root::new();
    // (the set's placement is a worker's: a worker's answers come later, and so would a
    // placeholder's)
    let plan = plan_with(false);
    let quiet = exact_data::Placed::new(Quiet, Placement::Worker);
    let mut mixed =
        exact_data::Mixed::new(module(&root).placed(Placement::Main), quiet, &["work"], &[])
            .unwrap();
    mixed.bind(&plan);
    mixed.activate().unwrap();
    let r = Runner::boot(
        plan,
        mixed,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    asks_again(r, "mixed", true);
}

/// A mutation's turn over its budget is never asked again (LLP 1041 D2: its
/// earlier turns may have had effects): its send fails, and nothing more runs.
#[test]
fn a_mutations_overrun_is_not_asked_again() {
    let root = Root::new();
    let plan = super::storage::plan();
    let mut m = module(&root);
    m.bind(&plan);
    m.activate().unwrap();
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let _ = r.act("invoke", vec![Value::str("busy"), Value::str("30000000")]);
    drive(&mut r);
    let journal: Vec<String> = r.journal().map(str::to_owned).collect();
    assert!(
        !journal.iter().any(|l| l.contains("asked again in")),
        "{journal:?}"
    );
    let t0 = r.now_ms();
    r.advance(t0 + 5_000.).unwrap();
    assert!(r.take_requests().is_empty(), "nothing asked again");
    assert_eq!(r.timer_due_ms(), None);
    assert!(!r.journal().any(|l| l.contains("asked again in")));
}
