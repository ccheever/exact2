//! An answer's own timers (LLP 1027.000 D1 as amended 2026-10-28) through
//! the Hermes executor and the runner: due on the runner's clock, kept back
//! as no I/O until an advance reaches them, cleared with their answer, and
//! fired on a clock that jumps at bake.
#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::{Plan, Value};
use exact_runner::{DataSource, FailureKind, Outcome, RequestOut, Runner};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/timers.hbc"));
const APP: &str = "test.timers";
const GRANTS: &str = "net.fetch https://fixture.exact.test\n";

const SRC: &str = r#"
component App
  state n = 0
  resource spun = spin(n) as shape string
  mutation result as shape string
  mutation waited as shape string
  action sleep(label: string, ms: number)
    send result = slept(label, ms)
  action order
    send result = order()
  action cleared
    send result = cleared()
  action abort(ms: number)
    send result = abort(ms)
  action fetchTimeout
    send result = fetchTimeout()
  action interval
    send result = interval()
  action atInit
    send result = atInit()
  action unawaited
    send result = unawaited()
  action readLog
    send result = log()
  action wait(n: string)
    send waited = letGo(n)
  action post(n: string, how: string)
    send waited = postTimeout(n, how)
  action postAlso(n: string, how: string)
    send result = postTimeout(n, how)
  action starter
    send waited = starter()
  action racer
    send result = racer()
  action raceFetch
    send result = raceFetch()
  action abortAt
    send result = abortAt()
  action tickThenFetch
    send result = tickThenFetch()
  action chain
    send result = chain()
  action respin(v: number)
    n = v
  view
    text "timers"
"#;

fn plan() -> Plan {
    contract::compile(SRC).expect("timers fixture compiles")
}

fn module() -> Module {
    let mut module = Module::loaded(HBC.to_vec(), APP, GRANTS).expect("timers fixture loads");
    module.set_budget_ms(f64::INFINITY);
    module.bind(&plan());
    module
}

fn runner() -> Runner<Module> {
    Runner::boot(
        plan(),
        module(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// A host's work after a commit or an advance: each continuation dispatched
/// and settled now, or parked until released; the fetches handed out are
/// returned, unanswered.
fn pump<D: DataSource>(r: &mut Runner<D>) -> Vec<RequestOut> {
    PARKED.with(|parked| {
        let mut parked = parked.borrow_mut();
        let mut fetches = Vec::new();
        loop {
            let mut work = Vec::new();
            for request in r.take_requests() {
                let Some(token) = request.request.continuation else {
                    fetches.push(request);
                    continue;
                };
                match crate::runnable(r.dispatch_work(token)) {
                    exact_runner::Dispatch::Held => {
                        parked.insert(token, request.ticket);
                    }
                    dispatch => work.push((request.ticket, dispatch)),
                }
            }
            for (token, dispatch) in r.release_work() {
                if let Some(ticket) = parked.remove(&token) {
                    work.push((ticket, crate::runnable(dispatch)));
                }
            }
            if work.is_empty() {
                return fetches;
            }
            for (ticket, dispatch) in work {
                let exact_runner::Dispatch::Run(exact_runner::Work::Now(run)) = dispatch else {
                    panic!("a continuation runs now");
                };
                r.fulfill(ticket, run()).unwrap();
            }
        }
    })
}

thread_local! {
    /// The continuations a test's host holds, by token: its ticket.
    static PARKED: std::cell::RefCell<std::collections::HashMap<u64, u64>> = Default::default();
}

fn ok() -> Outcome {
    Outcome::Response(exact_runner::Response {
        status: 200,
        headers: vec![],
        body: vec![],
    })
}

/// The agent's clock jump: advance, stopping where a timer's continuation
/// goes out, and land it before going on (`Runner::advance_until_request`).
/// The clock never goes back.
fn seek<D: DataSource>(r: &mut Runner<D>, to: f64) {
    loop {
        let before = r.now_ms();
        let advanced = r.advance_until_request(to);
        assert!(advanced.error.is_none(), "{:?}", advanced.error);
        assert!(
            r.now_ms() >= before,
            "the clock went back from {before} to {}",
            r.now_ms()
        );
        pump(r);
        assert!(!r.data_timer_landing(), "a released timer has landed");
        if !advanced.stopped_on_request {
            return;
        }
    }
}

fn result<D: DataSource>(r: &Runner<D>, slot: &str) -> Value {
    r.slot(slot).cloned().unwrap_or(Value::NONE)
}

fn said(text: &str) -> Value {
    Value::some(Value::str(text))
}

#[test]
fn an_answer_sleeps_until_the_runner_clock_reaches_its_timer() {
    let mut r = runner();
    r.act("sleep", vec![Value::str("a"), Value::Number(450.)])
        .unwrap();
    assert!(pump(&mut r).is_empty());
    assert_eq!(result(&r, "result"), Value::NONE);
    // No I/O: no clock waits on it, and the host's next deadline is it.
    assert!(!r.has_pending() && r.in_flight().is_empty());
    assert_eq!(r.data_timer_due_ms(), Some(450.));
    assert_eq!(r.timer_due_ms(), Some(450.));
    seek(&mut r, 449.);
    assert_eq!(result(&r, "result"), Value::NONE);
    seek(&mut r, 450.);
    assert_eq!(result(&r, "result"), said("a after 450"));
    assert_eq!(r.data_timer_due_ms(), None);
}

#[test]
fn timers_run_in_due_order_with_their_microtasks_between() {
    let mut r = runner();
    r.act("order", vec![]).unwrap();
    pump(&mut r);
    seek(&mut r, 349.);
    assert_eq!(
        result(&r, "result"),
        Value::NONE,
        "the last timer is armed at 300"
    );
    seek(&mut r, 350.);
    assert_eq!(
        result(&r, "result"),
        said("t100,m100,t200,m200,t300,m300,then")
    );
}

#[test]
fn a_cleared_timer_never_runs() {
    let mut r = runner();
    r.act("cleared", vec![]).unwrap();
    pump(&mut r);
    seek(&mut r, 100.);
    assert_eq!(result(&r, "result"), said("none"));
}

#[test]
fn abort_signal_timeout_aborts_on_the_runner_clock() {
    let mut r = runner();
    r.act("abort", vec![Value::Number(200.)]).unwrap();
    pump(&mut r);
    seek(&mut r, 199.);
    assert_eq!(result(&r, "result"), Value::NONE);
    seek(&mut r, 200.);
    assert_eq!(result(&r, "result"), said("TimeoutError"));
}

/// A fetch waits as one wait; the timeout signal it carries is its
/// deadline on the host, and the signal aborts when the host says so.
#[test]
fn a_fetch_carries_its_timeout_signal_as_its_deadline() {
    let mut r = runner();
    r.act("fetchTimeout", vec![]).unwrap();
    let fetches = pump(&mut r);
    assert_eq!(fetches.len(), 1);
    assert_eq!(fetches[0].request.timeout_ms, Some(250));
    let timed_out = Outcome::Failed {
        kind: FailureKind::Timeout,
        message: "the request timed out after 250 ms".into(),
    };
    r.fulfill(fetches[0].ticket, timed_out).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("TimeoutError"));
}

#[test]
fn a_let_go_answer_never_runs_its_timer() {
    let mut r = runner();
    r.act("wait", vec![Value::str("1")]).unwrap();
    pump(&mut r);
    r.act("wait", vec![Value::str("2")]).unwrap();
    pump(&mut r);
    seek(&mut r, 500.);
    assert_eq!(result(&r, "waited"), said("done 2"));
    r.act("readLog", vec![]).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("armed 1,armed 2,ran 2"));
}

/// A POST its answer let go runs to its end (LLP 1146 D2), and the
/// `AbortSignal.timeout` it carries, dropped with that answer's other timers,
/// still aborts at the deadline the host gave the request, as a browser's
/// fires: carried as it is, through `AbortSignal.any`, or beside an
/// `exactTimeout` as long. A later answer carrying the same signal has its
/// deadline too. One longer than a request waits is not the request's
/// deadline: the hour is, and the signal does not abort when it passes.
#[test]
fn an_adopted_fetch_s_timeout_still_aborts_its_signal() {
    let timed_out = || Outcome::Failed {
        kind: FailureKind::Timeout,
        message: "the request timed out after 250 ms".into(),
    };
    let post = |r: &mut Runner<Module>, action: &str, n: &str, how: &str| {
        r.act(action, vec![Value::str(n), Value::str(how)]).unwrap();
        let mut fetches = pump(r);
        assert_eq!(fetches.len(), 1, "{how}");
        fetches.remove(0)
    };
    let log = |r: &mut Runner<Module>| {
        r.act("readLog", vec![]).unwrap();
        pump(r);
        result(r, "result")
    };
    for how in ["direct", "any", "equal"] {
        let mut r = runner();
        let first = post(&mut r, "post", "1", how);
        assert_eq!(first.request.timeout_ms, Some(250), "{how}");
        let second = post(&mut r, "post", "2", how);
        assert!(r.holds(first.ticket), "{how}: the let-go POST runs on");
        r.fulfill(first.ticket, timed_out()).unwrap();
        pump(&mut r);
        r.fulfill(second.ticket, ok()).unwrap();
        pump(&mut r);
        assert_eq!(log(&mut r), said("1 TimeoutError true,posted 2"), "{how}");
    }
    let mut r = runner();
    let first = post(&mut r, "post", "1", "long");
    assert_eq!(first.request.timeout_ms, Some(3_600_000));
    let second = post(&mut r, "post", "2", "direct");
    r.fulfill(first.ticket, timed_out()).unwrap();
    pump(&mut r);
    r.fulfill(second.ticket, ok()).unwrap();
    pump(&mut r);
    assert_eq!(log(&mut r), said("1 FetchError false,posted 2"));
    let mut r = runner();
    let first = post(&mut r, "post", "1", "shared");
    let second = post(&mut r, "post", "2", "direct");
    let third = post(&mut r, "postAlso", "3", "shared");
    assert_eq!(third.request.timeout_ms, Some(250), "the first's signal");
    for (fetch, outcome) in [(first, ok()), (second, ok()), (third, timed_out())] {
        r.fulfill(fetch.ticket, outcome).unwrap();
        pump(&mut r);
    }
    assert_eq!(result(&r, "result"), said("done 3"));
    assert_eq!(log(&mut r), said("posted 1,posted 2,3 TimeoutError true"));
}

#[test]
fn a_timer_left_when_its_answer_gives_its_value_never_runs() {
    let mut r = runner();
    r.act("unawaited", vec![]).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("given"));
    assert_eq!(r.data_timer_due_ms(), None);
    seek(&mut r, 100.);
    r.act("readLog", vec![]).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), Value::some(Value::str("")));
}

#[test]
fn outside_an_answer_and_every_other_timer_refuse() {
    let mut r = runner();
    r.act("atInit", vec![]).unwrap();
    pump(&mut r);
    let Value::Option(Some(at_init)) = result(&r, "result") else {
        panic!("answered");
    };
    assert!(
        at_init
            .as_str()
            .unwrap()
            .starts_with("setTimeout() is unavailable here"),
        "{at_init:?}"
    );
    r.act("interval", vec![]).unwrap();
    pump(&mut r);
    let Value::Option(Some(interval)) = result(&r, "result") else {
        panic!("answered");
    };
    assert!(
        interval
            .as_str()
            .unwrap()
            .starts_with("setInterval() is unavailable in data sources"),
        "{interval:?}"
    );
}

/// The bake has no clock: each timer fires at once, on a clock that moves
/// to it, so an answer that waits still compiles, in the same order.
#[test]
fn at_bake_timers_fire_on_a_clock_that_jumps() {
    let mut baking = Module::inspect(HBC.to_vec()).unwrap();
    baking.bind(&plan());
    assert_eq!(
        baking
            .query("slept", &[Value::str("b"), Value::Number(450.)])
            .unwrap(),
        Value::str("b after 450")
    );
    assert_eq!(
        baking.query("order", &[]).unwrap(),
        Value::str("t100,m100,t200,m200,t300,m300,then")
    );
}

/// An answer racing another answer's fetch against its own long timer
/// natively waits for whichever comes first, as the web does: the fetch's
/// reply asks it again, and its timer is due all the while.
#[test]
fn an_answer_racing_another_answers_fetch_wakes_on_the_fetch() {
    let mut r = runner();
    r.act("starter", vec![]).unwrap();
    let fetches = pump(&mut r);
    r.act("racer", vec![]).unwrap();
    pump(&mut r);
    assert_eq!(r.data_timer_due_ms(), Some(10_000.));
    let waits = r.data_timer_waits();
    assert!(
        waits
            .iter()
            .any(|(name, _, due)| name == "result" && *due == 10_000.),
        "{waits:?}"
    );
    seek(&mut r, 200.);
    r.fulfill(fetches[0].ticket, ok()).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("shared"));
    assert_eq!(r.data_timer_due_ms(), None);
}

/// A timer raced against its answer's own fetch wins when it comes due
/// first, as on the web (host/web-js/answer-timers.test.mjs): the fetch runs
/// on as module work, and its late reply runs what chained on it.
#[test]
fn a_timer_raced_against_its_own_fetch_wins_when_due_first() {
    let mut r = runner();
    r.act("raceFetch", vec![]).unwrap();
    let fetches = pump(&mut r);
    assert_eq!(
        fetches[0].request.wake_ms,
        Some(100),
        "it carries its timer's time"
    );
    assert_eq!(r.data_timer_due_ms(), Some(100.));
    seek(&mut r, 99.);
    assert_eq!(result(&r, "result"), Value::NONE);
    seek(&mut r, 100.);
    assert_eq!(result(&r, "result"), said("slept"));
    assert!(!r
        .journal()
        .any(|l| l.contains("an answer awaits a fetch while a timer of its own is armed")));
    assert!(r.holds(fetches[0].ticket), "the fetch runs on");
    r.fulfill(fetches[0].ticket, ok()).unwrap();
    pump(&mut r);
    assert!(!r.holds(fetches[0].ticket));
    assert_eq!(result(&r, "result"), said("slept"));
    r.act("readLog", vec![]).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("fetch landed"));
}

/// A Rust child for the mixed composer that answers nothing.
struct Quiet;
impl DataSource for Quiet {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn app_id(&self) -> &str {
        APP
    }
    fn grants(&self) -> &str {
        ""
    }
}

/// The same race behind what a native host stacks over the module: app
/// storage, the mixed composer and the module on the main thread.
#[test]
fn a_timer_wins_its_race_behind_the_composers() {
    let sources = ["raceFetch", "log", "spin"];
    let mixed = exact_data::Mixed::new(
        module().placed(exact_runner::Placement::Main),
        exact_data::Placed::new(Quiet, exact_runner::Placement::Main),
        &sources,
        &[],
    )
    .unwrap();
    let mut source = exact_data_host::Storage::new(mixed);
    source.bind(&plan());
    source.activate().unwrap();
    let mut r = Runner::boot(
        plan(),
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("raceFetch", vec![]).unwrap();
    let fetches = pump(&mut r);
    seek(&mut r, 100.);
    assert_eq!(result(&r, "result"), said("slept"));
    r.fulfill(fetches[0].ticket, ok()).unwrap();
    pump(&mut r);
    r.act("readLog", vec![]).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("fetch landed"));
}

/// A fetch whose reply comes before its answer's timer wins, and the timer
/// is due no more.
#[test]
fn a_fetch_that_lands_before_its_answer_s_timer_wins() {
    let mut r = runner();
    r.act("raceFetch", vec![]).unwrap();
    let fetches = pump(&mut r);
    seek(&mut r, 50.);
    r.fulfill(fetches[0].ticket, ok()).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("fetched"));
    assert_eq!(r.data_timer_due_ms(), None);
    seek(&mut r, 200.);
    assert_eq!(result(&r, "result"), said("fetched"));
}

/// A timer that aborts the fetch its answer awaits ends it on time: the
/// host cancels the request, and the rejection reaches the answer.
#[test]
fn a_timer_aborts_the_fetch_its_answer_awaits_on_time() {
    let mut r = runner();
    r.act("abortAt", vec![]).unwrap();
    let fetches = pump(&mut r);
    seek(&mut r, 100.);
    assert_eq!(
        result(&r, "result"),
        Value::NONE,
        "aborted at the host's pump"
    );
    let (ticket, outcome) = r.take_request_refusal(false).expect("answered aborted");
    assert_eq!(ticket, fetches[0].ticket);
    r.fulfill(ticket, outcome).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("AbortError"));
    assert_eq!(
        r.take_explicit_aborts(),
        [fetches[0].ticket],
        "the host cancels it"
    );
}

/// A timer that settles nothing runs on time, and its answer goes on
/// waiting for the fetch, now module work.
#[test]
fn a_timer_that_settles_nothing_leaves_its_answer_waiting_for_the_fetch() {
    let mut r = runner();
    r.act("tickThenFetch", vec![]).unwrap();
    let fetches = pump(&mut r);
    seek(&mut r, 50.);
    assert_eq!(result(&r, "result"), Value::NONE);
    assert!(r.has_pending(), "the fetch is still out");
    r.fulfill(fetches[0].ticket, ok()).unwrap();
    pump(&mut r);
    assert_eq!(result(&r, "result"), said("fetched after ticked"));
}

/// Chained sleeps under one jump: each continuation lands before the jump
/// goes on, so each timer is due from the one before it (100, 300, 700).
#[test]
fn chained_sleeps_under_one_jump_are_due_one_after_another() {
    let mut r = runner();
    r.act("chain", vec![]).unwrap();
    pump(&mut r);
    seek(&mut r, 699.);
    assert_eq!(result(&r, "result"), Value::NONE);
    assert_eq!(r.data_timer_due_ms(), Some(700.));
    seek(&mut r, 700.);
    assert_eq!(result(&r, "result"), said("chained"));
}

/// A re-ask due after an answer's timer waits for it: the clock never goes
/// back to the timer from the re-ask's later time.
#[test]
fn a_reask_due_after_an_answers_timer_waits_for_it() {
    let mut m = module();
    m.set_budget_ms(25.);
    let mut r = Runner::boot(plan(), m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    r.act("sleep", vec![Value::str("a"), Value::Number(450.)])
        .unwrap();
    pump(&mut r);
    seek(&mut r, 250.);
    r.act("respin", vec![Value::Number(1.)]).unwrap();
    pump(&mut r);
    assert_eq!(
        r.reask_due_ms(),
        Some(500.),
        "refused over its budget, asked again at 500"
    );
    seek(&mut r, 1000.);
    assert_eq!(result(&r, "result"), said("a after 450"));
}

#[test]
fn takeover_releases_an_overdue_answer_timer_without_rewinding() {
    use std::{cell::Cell, rc::Rc};
    let mut r = runner();
    let wall = Rc::new(Cell::new(0.0));
    let sample = wall.clone();
    r.set_live_clock(Some(Box::new(move || sample.get())));
    r.act("sleep", vec![Value::str("takeover"), Value::Number(50.)])
        .unwrap();
    pump(&mut r);
    assert_eq!(r.data_timer_due_ms(), Some(50.));
    wall.set(100.);
    r.set_live_clock(None);
    assert_eq!(r.now_ms(), 100.);
    seek(&mut r, 100.);
    assert_eq!(r.now_ms(), 100.);
    assert_ne!(result(&r, "result"), Value::NONE);
}
