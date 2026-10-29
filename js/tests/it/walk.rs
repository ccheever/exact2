//! `scheduler.yield()` (LLP 1141.001 D10): a turn over the budget fails and
//! names the repair, and the same walk yielding every few thousand items goes
//! on in new turns, each with a budget of its own, asked again with no work
//! and no clock (`Dispatch::Again`): inline, on a worker, and through the
//! runner. The budget is the default 100 ms, as an app has it, and turns are
//! timed on a ticking clock (`Module::set_turn_step`): what the budget allows
//! depends on the step, not on how the machine schedules the test.

#![cfg(exact_js_engine)]

use exact_js::{Module, Placement};
use exact_kernel::{Kernel, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, Dispatch, Event, Reply, Runner, Store, Work};
use std::collections::HashMap;
use std::time::Duration;

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/walk.hbc"));
const APP: &str = "test.walk";
/// Items to walk, in one turn or in `YIELDS` + 1.
const N: u32 = 12_000_000;
/// Yields per walk.
const YIELDS: u32 = 300;
/// What each turn takes on the ticking clock: within the budget, which the
/// turns of a yielding walk are far over together.
const TURN: Duration = Duration::from_millis(60);

const SRC: &str = r#"
component App
  state n = 0
  state m = 0
  state every = 40000
  resource total = walk(n, every) as shape string
  resource a = sharedA(m, every) as shape string
  resource b = sharedB(m, every) as shape string
  resource atLoad = atLoad() as shape string
  resource postTask = postTask() as shape string
  action start
    n = 12000000
  action restart
    n = 12000001
  action share
    m = 12000000
  view
    column
      button press=start aria-label="Start" testId="start"
        text "Start"
      button press=restart aria-label="Restart" testId="restart"
        text "Restart"
      button press=share aria-label="Share" testId="share"
        text "Share"
      text total testId="total"
      text a testId="a"
      text b testId="b"
"#;

fn plan() -> Plan {
    contract::compile(SRC).expect("the fixture's Contract compiles")
}

fn module(turn: Duration) -> Module {
    let mut m = Module::new(HBC.to_vec(), APP, "");
    m.set_turn_step(turn);
    m.bind(&plan());
    m.load().expect("the fixture loads");
    m
}

fn args(n: u32, every: u32) -> [Value; 2] {
    [Value::Number(n.into()), Value::Number(every.into())]
}

/// What the walk answers: its checksum and how many times it yielded.
fn expected(n: u32, yields: u32) -> String {
    let sum = (0..u64::from(n)).fold(0, |sum, i| (sum * 31 + i) % 1_000_003);
    format!("{sum}/{yields}")
}

/// Walk inline, asking again at each yield as a host settles a re-ask: the
/// answer and the number of turns it took.
fn walk(m: &mut Module, n: u32, every: u32) -> (Result<Value, DataError>, u32) {
    let mut s = Store::new("", []);
    let args = args(n, every);
    let mut answer = m.answer(&mut s, "walk", &args);
    let mut turns = 1;
    loop {
        match answer {
            Ok(Answer::Later(request)) => {
                let token = request.continuation.expect("a yield is a continuation");
                assert!(
                    matches!(m.dispatch(token, &s), Dispatch::Again),
                    "a yield is asked again, with no work"
                );
                answer = m.parse(&mut s, "walk", &args, Dispatch::again_outcome());
                turns += 1;
            }
            Ok(Answer::Now(v)) => return (Ok(v), turns),
            Err(e) => return (Err(e), turns),
        }
    }
}

#[test]
fn a_turn_over_the_budget_fails_and_names_scheduler_yield() {
    let mut m = module(Duration::from_millis(150));
    let (answer, turns) = walk(&mut m, N, 0);
    let Err(DataError::Overrun(message)) = answer else {
        panic!("one turn over all of it is over the budget: {answer:?}");
    };
    assert!(
        message.contains("over the 100 ms budget for one turn")
            && message.contains("`await scheduler.yield()`"),
        "{message}"
    );
    assert_eq!(turns, 1);
}

#[test]
fn the_same_walk_yielding_goes_on_in_new_turns_each_with_its_own_budget() {
    let mut m = module(TURN);
    let (answer, turns) = walk(&mut m, N, N / YIELDS);
    assert_eq!(answer, Ok(Value::str(&expected(N, YIELDS))));
    assert_eq!(turns, YIELDS + 1);
    assert_eq!(m.in_flight(), 0);
}

#[test]
fn a_yield_outside_an_answer_goes_on_and_post_task_refuses() {
    let mut m = module(TURN);
    let mut s = Store::new("", []);
    let Ok(Answer::Now(at_load)) = m.answer(&mut s, "atLoad", &[]) else {
        panic!("answered now");
    };
    assert_eq!(at_load, Value::str("resolved"));
    let Ok(Answer::Now(refused)) = m.answer(&mut s, "postTask", &[]) else {
        panic!("answered now");
    };
    let refused = refused.as_str().unwrap_or("");
    assert!(refused.contains("await scheduler.yield()"), "{refused}");
}

#[test]
fn a_worker_resumes_its_yields_on_its_owner() {
    let mut unplaced = Module::new(HBC.to_vec(), APP, "");
    unplaced.set_turn_step(TURN);
    unplaced.bind(&plan());
    let mut placed = unplaced.placed(Placement::Worker);
    placed.activate().unwrap();
    let mut s = Store::new("", []);
    let args = args(N, N / YIELDS);
    let token = match placed.answer(&mut s, "walk", &args).unwrap() {
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
    let outcome = rx
        .recv_timeout(Duration::from_secs(60))
        .expect("the owner walks to the end");
    let answer = placed.parse(&mut s, "walk", &args, outcome);
    assert!(
        matches!(&answer, Ok(Answer::Now(v)) if *v == Value::str(&expected(N, YIELDS))),
        "{answer:?}"
    );
}

fn runner() -> Runner<Module> {
    let mut baking = Module::loaded(HBC.to_vec(), APP, "").unwrap();
    baking.set_turn_step(TURN);
    let baked = contract::bake(plan(), baking)
        .expect("bake asks the walks of nothing, which answer at once");
    let mut module = Module::loaded(HBC.to_vec(), APP, "").unwrap();
    module.set_turn_step(TURN);
    Runner::boot(
        baked,
        module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn press(r: &mut Runner<Module>, test_id: &str) {
    let k = r.kernel();
    let view = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}

fn text_of(r: &Runner<Module>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id(test_id)[0])?;
    node.props.str(PropId::Text).map(str::to_string)
}

/// Settle what the runner asks as a host does: a yield, or a waiting answer
/// woken, is asked again with no work; one held waits for its release. At
/// most `limit` rounds; the rounds it took.
fn settle(r: &mut Runner<Module>, limit: u32) -> u32 {
    let mut tickets = HashMap::new();
    for round in 0..limit {
        let mut again = Vec::new();
        for a in r.take_requests() {
            let token = a.request.continuation.expect("a yield is a continuation");
            tickets.insert(token, a.ticket);
            match r.dispatch_work(token) {
                Dispatch::Again => again.push(a.ticket),
                Dispatch::Held => {}
                // Let go after it was asked: the runner holds it no more.
                Dispatch::Missing => {}
                _ => panic!("a yield runs no work"),
            }
        }
        for (token, dispatch) in r.release_work() {
            assert!(matches!(dispatch, Dispatch::Again));
            again.push(tickets[&token]);
        }
        if again.is_empty() {
            return round;
        }
        for ticket in again {
            r.fulfill(ticket, Dispatch::again_outcome()).unwrap();
        }
    }
    limit
}

#[test]
fn the_runner_asks_a_yielded_answer_again_until_it_answers() {
    let mut r = runner();
    press(&mut r, "start");
    assert_eq!(settle(&mut r, 1000), YIELDS, "one round per yield");
    assert_eq!(text_of(&r, "total"), Some(expected(N, YIELDS)));
    assert!(!r.has_pending());
}

#[test]
fn an_answer_awaiting_another_answers_yielding_walk_waits_for_it() {
    let mut r = runner();
    press(&mut r, "share");
    settle(&mut r, 10_000);
    assert_eq!(text_of(&r, "a"), Some(expected(N, YIELDS)));
    assert_eq!(text_of(&r, "b"), Some(expected(N, YIELDS)));
    assert!(!r.has_pending());
    assert_eq!(r.data().in_flight(), 0);
}

#[test]
fn a_yielding_answer_let_go_rejects_its_yield() {
    let mut r = runner();
    press(&mut r, "start");
    assert_eq!(settle(&mut r, 3), 3);
    // New arguments: the walk in flight is let go, and the new one answers.
    press(&mut r, "restart");
    settle(&mut r, 1000);
    assert_eq!(text_of(&r, "total"), Some(expected(N + 1, YIELDS)));
    assert!(!r.has_pending());
    let journal: Vec<&str> = r.journal().collect();
    assert!(
        journal
            .iter()
            .any(|l| l.contains(&format!("walk {N}: AbortError"))),
        "{journal:#?}"
    );
}
