//! Another thread stops a running call (LLP 1048.000 D10): the bytecode's
//! async break checks let Hermes stop a synchronous loop, and the call is
//! refused while the module goes on answering.

#![cfg(exact_js_engine)]

use exact_js::{Module, Placement};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, Dispatch, Reply, Store, Work};
use std::time::{Duration, Instant};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/spin.hbc"));
const APP: &str = "test.spin";

/// Also the render test's app: `words` answers at once, except for the name
/// "spin", for which it never returns on its own.
pub const SRC: &str = r#"
routes nav
  tab home "/" render=request
    page "/page/:name" render=request

component App
  resource words = words(params(nav, "name")) as shape list<string>
  view
    column
      each w in words key = w
        text w
"#;

fn plan() -> Plan {
    contract::compile(SRC).expect("the fixture's Contract compiles")
}

fn unloaded() -> Module {
    let mut m = Module::new(HBC.to_vec(), APP, "");
    // Functional fixtures carry no wall-clock budget (castle.rs says why).
    m.set_budget_ms(f64::INFINITY);
    m.bind(&plan());
    m
}

fn module() -> Module {
    let mut m = unloaded();
    m.load().expect("the fixture loads");
    m
}

fn named(name: &str) -> [Value; 1] {
    [Value::list(vec![Value::str(name)])]
}

fn words(answer: Result<Answer, DataError>) -> Vec<String> {
    match answer {
        Ok(Answer::Now(Value::List(items))) => items
            .iter()
            .map(|w| w.as_str().unwrap_or("").to_string())
            .collect(),
        other => panic!("expected words, got {other:?}"),
    }
}

fn interrupted(result: &Result<Answer, DataError>, source: &str) -> bool {
    matches!(result, Err(DataError::Unavailable(m)) if *m == format!("`{source}` was interrupted"))
}

#[test]
fn a_spinning_call_is_stopped_from_another_thread_and_the_module_answers_again() {
    let mut m = module();
    let mut s = Store::new("", []);
    let stop = m.interrupt().expect("a module can be interrupted");
    let started = Instant::now();
    let timer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        stop.trigger();
    });
    let spun = m.answer(&mut s, "words", &named("spin"));
    timer.join().unwrap();
    assert!(interrupted(&spun, "words"), "{spun:?}");
    assert!(started.elapsed() >= Duration::from_millis(100));
    // The interrupt was spent on that call: the module answers again.
    assert_eq!(
        words(m.answer(&mut s, "words", &named("calm"))),
        ["hello", "calm"]
    );
    assert_eq!(m.in_flight(), 0);
}

#[test]
fn an_interrupt_with_nothing_running_stops_the_next_call_only() {
    let mut m = module();
    let mut s = Store::new("", []);
    m.interrupt().unwrap().trigger();
    let stopped = m.answer(&mut s, "words", &named("calm"));
    assert!(interrupted(&stopped, "words"), "{stopped:?}");
    assert_eq!(
        words(m.answer(&mut s, "words", &named("calm"))),
        ["hello", "calm"]
    );
}

#[test]
fn an_interrupt_before_the_module_loads_stops_its_initialization() {
    let mut m = unloaded();
    m.interrupt().unwrap().trigger();
    assert_eq!(
        m.load().unwrap_err(),
        "exact-js: the module was interrupted while it loaded"
    );
    assert!(!m.is_loaded());
    m.load().expect("the next load runs");
    assert_eq!(
        words(m.answer(&mut Store::new("", []), "words", &named("calm"))),
        ["hello", "calm"]
    );
}

#[test]
fn a_worker_instance_is_stopped_through_its_template() {
    let mut placed = unloaded().placed(Placement::Worker);
    placed.activate().unwrap();
    let stop = placed.interrupt().expect("the template's interrupt");
    let mut s = Store::new("", []);
    let token = match placed.answer(&mut s, "words", &named("spin")).unwrap() {
        Answer::Later(request) => request.continuation.expect("a turn"),
        Answer::Now(v) => panic!("a worker answers later, not {v:?}"),
    };
    let Dispatch::Run(Work::Later(work)) = placed.dispatch(token, &s) else {
        panic!("a turn for the owner");
    };
    let (tx, rx) = std::sync::mpsc::channel();
    work(Reply::new(move |outcome| {
        let _ = tx.send(outcome);
    }));
    std::thread::sleep(Duration::from_millis(100));
    stop.trigger();
    let outcome = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("the owner's turn ends when interrupted");
    let refused = placed.parse(&mut s, "words", &named("spin"), outcome);
    assert!(interrupted(&refused, "words"), "{refused:?}");
}
