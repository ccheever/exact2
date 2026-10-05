//! LLP 1092 D1–D6 end to end: a `queue` mutation's sends wait their turn,
//! one request in flight, each asked in a `next` commit of its own after
//! the reply before it and its `then`.

use exact_kernel::Kernel;
use exact_runner::{
    Answer, DataError, DataSource, FailureKind, Outcome, Request, Response, Runner, RunnerError,
    Value,
};

const APP: &str = r#"
shape Ack
  op: string
  n: number

component App
  state log = ""
  state label = "x"
  state ticks = 0
  state trigger = 0
  resource probe = check() as shape number
  resource big = big(trigger) as shape list<Ack>
  mutation wrote as shape Ack queue refreshes probe then afterWrote
  mutation rec as shape Ack queue
  derive last = match wrote { case some(a) => a.op, case none => "-" }
  derive recd = match rec { case some(a) => a.op, case none => "-" }
  action pair
    send wrote = save("a")
    send wrote = save("b")
  action one(op: string)
    send wrote = save(op)
  action tap
    send wrote = save(label)
    label = "y"
  action clear
    wrote = none
  action afterWrote
    log = `${log} ${last}`
    if last == "boom"
      send rec = broken("then")
  action record(op: string)
    send rec = save(op)
  action recordBroken
    send rec = save("p")
    send rec = broken("z")
    send rec = save("after")
  action multi
    send wrote = two("m")
    send wrote = save("n")
  action nothing
    label = label
  action bump
    ticks = ticks + 1
  action tick
    log = `${log} t:${recd}`
  action poison
    trigger = 1
  view
    column
      text log testId="log"
      text pending(wrote) ? "busy" : "idle" testId="busy"
      each a in big key=a.op
        text a.op + a.op
"#;

/// A desk that answers `save` now or later, counting every ask with its
/// arguments (what the data module sees, D5).
#[derive(Default)]
struct Desk {
    later: bool,
    fail_parse: bool,
    fail_check: bool,
    asks: Vec<String>,
    checks: usize,
}

fn ack(op: &str, n: usize) -> Value {
    Value::record(vec![Value::str(op), Value::Number(n as f64)])
}

impl DataSource for Desk {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "check" => {
                self.checks += 1;
                if self.fail_check {
                    return Err(DataError::Unavailable("check refused".into()));
                }
                Ok(Value::Number(self.checks as f64))
            }
            "big" if args[0].as_number() == Some(0.0) => Ok(Value::list(vec![])),
            // Doubled by the view, past the runner's longest string: a trap
            // while the tree changes poisons the runner.
            "big" => Ok(Value::list(vec![ack(
                &"x".repeat((exact_runner::vm::MAX_STRING / 2) + 1),
                0,
            )])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let op = args
            .first()
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if matches!(source, "save" | "two" | "broken") {
            self.asks.push(format!("{source}:{op}"));
        }
        match source {
            "broken" => Err(DataError::Unavailable("broken refuses".into())),
            "two" => Ok(Answer::Later(Request::post_json(
                "https://desk.test/step1",
                &op,
            ))),
            "save" if self.later => Ok(Answer::Later(Request::post_json(
                "https://desk.test/save",
                &op,
            ))),
            "save" => Ok(Answer::Now(ack(&op, self.asks.len()))),
            _ => self.query(source, args).map(Answer::Now),
        }
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if self.fail_parse {
            return Err(DataError::Unavailable("parse refused".into()));
        }
        let op = args[0].as_str().unwrap_or("");
        match outcome {
            // A storage step and its continuation: one more round.
            Outcome::Response(r) if source == "two" && r.body == b"step1" => Ok(Answer::Later(
                Request::post_json("https://desk.test/step2", op),
            )),
            Outcome::Response(_) => Ok(Answer::Now(ack(op, self.asks.len()))),
            Outcome::Failed { message, .. } => Ok(Answer::Now(ack(&message, 0))),
            _ => Err(DataError::Unavailable("unexpected".into())),
        }
    }
}

fn boot_with(src: &str, desk: Desk) -> Runner<Desk> {
    let baked = contract::bake(contract::compile(src).unwrap(), Desk::default()).unwrap();
    Runner::boot(
        baked,
        desk,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn boot(later: bool) -> Runner<Desk> {
    boot_with(
        APP,
        Desk {
            later,
            ..Desk::default()
        },
    )
}

fn reply(body: &str) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: body.as_bytes().to_vec(),
    })
}

fn slot(r: &Runner<Desk>, name: &str) -> String {
    r.slot(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn busy(r: &Runner<Desk>) -> bool {
    let k = r.kernel();
    let key = k.find_by_test_id("busy")[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .str(exact_kernel::PropId::Text)
        == Some("busy")
}

fn tickets(r: &mut Runner<Desk>) -> Vec<u64> {
    r.take_requests().iter().map(|q| q.ticket).collect()
}

#[test]
fn two_sends_in_one_action_answered_now_run_in_order_each_with_its_then() {
    let mut r = boot(false);
    r.act("pair", vec![]).unwrap();
    // The first is asked in the action; the second waits for its reply's
    // `then`, which reads the slot as that reply set it.
    assert_eq!(r.data().asks, ["save:a"]);
    assert_eq!(r.queued(), [("wrote".to_string(), 1)]);
    assert!(busy(&r), "a waiting send is pending");
    assert_eq!(r.timer_due_ms(), Some(0.0));
    let commits = r.advance(0.0).unwrap();
    assert_eq!(commits.len(), 3, "then a, next b, then b");
    assert_eq!(r.data().asks, ["save:a", "save:b"]);
    assert_eq!(slot(&r, "log"), " a b");
    assert!(!busy(&r));
    assert_eq!(r.timer_due_ms(), None);
    assert!(r.journal().any(|l| l.ends_with("wrote next (0 waiting)")));
}

#[test]
fn two_sends_answered_later_ask_one_at_a_time_with_send_time_arguments() {
    let mut r = boot(true);
    // `tap` sends the label it was tapped with, then changes it.
    r.act("tap", vec![]).unwrap();
    r.act("tap", vec![]).unwrap();
    let first = tickets(&mut r);
    assert_eq!(first.len(), 1, "one request in flight");
    assert_eq!(r.data().asks, ["save:x"]);
    assert_eq!(r.queued(), [("wrote".to_string(), 1)]);
    assert_eq!(
        r.timer_due_ms(),
        None,
        "nothing is due while one is in flight"
    );
    r.fulfill(first[0], reply("ok")).unwrap();
    assert!(!r.holds(first[0]));
    // Its `then` first, then the waiting send, asked with `y`.
    r.advance(0.0).unwrap();
    assert_eq!(slot(&r, "log"), " x");
    assert_eq!(r.data().asks, ["save:x", "save:y"]);
    let second = tickets(&mut r);
    assert_eq!(second.len(), 1);
    assert!(busy(&r));
    r.fulfill(second[0], reply("ok")).unwrap();
    r.advance(0.0).unwrap();
    assert_eq!(slot(&r, "log"), " x y");
    assert!(!busy(&r));
}

#[test]
fn a_send_while_one_is_in_flight_waits_and_a_queue_without_then_is_due_alone() {
    let mut r = boot(true);
    r.act("record", vec![Value::str("p")]).unwrap();
    let first = tickets(&mut r);
    r.act("record", vec![Value::str("q")]).unwrap();
    assert!(tickets(&mut r).is_empty(), "q waits");
    r.fulfill(first[0], reply("ok")).unwrap();
    // No `then`: the reply alone makes the head due, on the host's clock.
    assert_eq!(r.timer_due_ms(), Some(0.0));
    r.advance(0.0).unwrap();
    assert_eq!(tickets(&mut r).len(), 1);
    assert_eq!(r.data().asks, ["save:p", "save:q"]);
}

#[test]
fn a_next_refused_by_its_own_ask_is_dropped_and_its_successor_goes_before_a_timer() {
    let src = APP.replace(
        "  view\n",
        "  task beat mount\n    every(1000, tick)\n  view\n",
    );
    let mut r = boot_with(&src, Desk::default());
    r.act("recordBroken", vec![]).unwrap();
    assert_eq!(r.queued(), [("rec".to_string(), 2)]);
    // `broken` is asked at its turn and refuses: the advance stops there.
    assert!(r.advance(0.0).is_err());
    assert!(r
        .journal()
        .any(|l| l.contains("rec queued send refused: Data")));
    assert_eq!(r.queued(), [("rec".to_string(), 1)]);
    assert_eq!(r.timer_due_ms(), Some(0.0), "the successor is due");
    r.advance(1000.0).unwrap();
    assert_eq!(r.data().asks, ["save:p", "broken:z", "save:after"]);
    assert_eq!(slot(&r, "log"), " t:after", "asked before the timer");
}

#[test]
fn a_next_refused_by_settlement_stalls_until_a_commit_changes_state() {
    let mut r = boot(false);
    r.act("pair", vec![]).unwrap();
    r.data().fail_check = true;
    // The `then` stands; the `next` re-reads `probe`, whose source refuses.
    assert!(r.advance(0.0).is_err());
    assert_eq!(slot(&r, "log"), " a");
    assert!(r
        .journal()
        .any(|l| l.contains("wrote next refused (Data") && l.ends_with("waits for a change")));
    assert_eq!(r.queued(), [("wrote".to_string(), 1)], "the head is kept");
    assert_eq!(r.timer_due_ms(), None, "a stall wakes no host");
    assert!(busy(&r));
    // A commit that changes nothing leaves it stalled.
    let checks = r.data().checks;
    r.act("nothing", vec![]).unwrap();
    assert_eq!(r.timer_due_ms(), None);
    assert!(r.advance(10.0).unwrap().is_empty());
    assert_eq!(r.data().checks, checks, "not retried");
    // One that changes a slot asks it again.
    r.data().fail_check = false;
    r.act("bump", vec![]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(10.0));
    r.advance(10.0).unwrap();
    assert_eq!(slot(&r, "log"), " a b");
    assert!(r.queued().is_empty());
}

#[test]
fn frame_tasks_fire_every_frame_while_a_next_is_stalled() {
    for (task, retried) in [("nothing", false), ("bump", true)] {
        let src = APP.replace(
            "  view\n",
            &format!("  task f mount\n    every(frame, {task})\n  view\n"),
        );
        let mut r = boot_with(&src, Desk::default());
        r.present_frames(true);
        r.act("pair", vec![]).unwrap();
        r.data().fail_check = true;
        for k in 1..=5 {
            let a = r.frame(k as f64 * 16.0);
            // Each frame's own frame task fires, whatever its prelude met.
            assert!(!a.receipts.is_empty(), "frame {k} ({task}): {:?}", a.error);
        }
        let checks = r.data().checks;
        r.frame(100.0);
        // A frame task that changes nothing never clears the stall; one
        // that changes a slot lets the `next` try again on the next wake.
        assert_eq!(r.data().checks > checks, retried, "{task}");
        assert_eq!(r.queued(), [("wrote".to_string(), 1)]);
    }
}

#[test]
fn a_refused_then_does_not_stall_its_queue() {
    let mut r = boot(false);
    r.act("one", vec![Value::str("boom")]).unwrap();
    r.act("one", vec![Value::str("after")]).unwrap();
    // `afterWrote` refuses for `boom` (it sends `broken`): that is the
    // `then`'s refusal, not the queue's, and the head is due at once.
    assert!(r.advance(0.0).is_err());
    assert_eq!(r.timer_due_ms(), Some(0.0));
    r.advance(0.0).unwrap();
    assert_eq!(r.data().asks, ["save:boom", "broken:then", "save:after"]);
    assert_eq!(slot(&r, "log"), " after");
}

#[test]
fn an_assignment_forgets_nothing_and_a_later_reply_overwrites_it() {
    let mut r = boot(true);
    r.act("one", vec![Value::str("a")]).unwrap();
    let t = tickets(&mut r);
    r.act("clear", vec![]).unwrap();
    assert_eq!(r.derive("last"), Some(&Value::str("-")));
    assert!(r.holds(t[0]), "the reply in flight still lands");
    assert!(busy(&r));
    r.fulfill(t[0], reply("ok")).unwrap();
    assert_eq!(r.derive("last"), Some(&Value::str("a")));
}

#[test]
fn a_failure_that_brings_no_answer_frees_the_queue() {
    let mut r = boot(true);
    r.act("pair", vec![]).unwrap();
    let t = tickets(&mut r);
    r.data().fail_parse = true;
    let _ = r.fulfill(t[0], reply("ok"));
    r.data().fail_parse = false;
    assert!(r
        .journal()
        .any(|l| l.contains("failed and is no longer pending: it ends unsent")));
    assert_eq!(slot(&r, "log"), "", "no `then` for no answer");
    assert_eq!(r.timer_due_ms(), Some(0.0));
    r.advance(0.0).unwrap();
    assert_eq!(r.data().asks, ["save:a", "save:b"]);
    let t = tickets(&mut r);
    r.fulfill(t[0], reply("ok")).unwrap();
    r.advance(0.0).unwrap();
    assert_eq!(slot(&r, "log"), " b");
}

/// A storage step and its continuation finish before the next send is
/// asked: a further `Later` round is not a reply. Before LLP 1092 the
/// second send forgot the first's ticket, and the save stopped between
/// its steps.
#[test]
fn a_two_step_send_completes_behind_a_second_send() {
    let mut r = boot(true);
    r.act("multi", vec![]).unwrap();
    let step1 = tickets(&mut r);
    assert_eq!(step1.len(), 1);
    r.act("one", vec![Value::str("o")]).unwrap();
    assert!(r.holds(step1[0]), "nothing forgets the continuation");
    r.fulfill(step1[0], reply("step1")).unwrap();
    let step2 = tickets(&mut r);
    assert_eq!(step2.len(), 1, "one more round, same send");
    assert_eq!(r.timer_due_ms(), None, "not a reply: no `then`, no `next`");
    assert_eq!(r.data().asks, ["two:m"]);
    r.fulfill(step2[0], reply("done")).unwrap();
    r.advance(0.0).unwrap();
    assert_eq!(slot(&r, "log"), " m");
    assert_eq!(r.data().asks, ["two:m", "save:n"]);
    let t = tickets(&mut r);
    r.fulfill(t[0], reply("ok")).unwrap();
    r.advance(0.0).unwrap();
    let t = tickets(&mut r);
    r.fulfill(t[0], reply("ok")).unwrap();
    r.advance(0.0).unwrap();
    assert_eq!(slot(&r, "log"), " m n o");
}

#[test]
fn the_sixty_fifth_waiter_refuses_its_action() {
    let mut r = boot(true);
    r.act("one", vec![Value::str("first")]).unwrap();
    for i in 0..exact_runner::QUEUE_BOUND {
        r.act("one", vec![Value::str(&i.to_string())]).unwrap();
    }
    assert!(matches!(
        r.act("one", vec![Value::str("over")]),
        Err(RunnerError::QueueFull { mutation }) if mutation == "wrote"
    ));
    assert_eq!(r.queued(), [("wrote".to_string(), 64)]);
    assert_eq!(r.data().asks.len(), 1);
}

#[test]
fn poison_and_reload_forget_waiting_sends_without_asking() {
    let mut r = boot(true);
    r.act("one", vec![Value::str("a")]).unwrap();
    r.act("one", vec![Value::str("b")]).unwrap();
    r.act("one", vec![Value::str("c")]).unwrap();
    let carried = r.carry();
    // `poison` poisons the tree: the queue is forgotten with it.
    assert!(r.act("poison", vec![]).is_err());
    assert!(r.is_poisoned());
    assert!(r
        .journal()
        .any(|l| l.ends_with(" forgot 2 waiting sends (wrote)")));
    assert!(r.queued().is_empty());
    assert_eq!(r.timer_due_ms(), None);
    assert_eq!(r.data().asks, ["save:a"]);
    // A reload carries the slot, never the queue, and says so once.
    let again = Runner::boot_carrying(
        contract::compile(APP).unwrap(),
        Desk {
            later: true,
            ..Desk::default()
        },
        Kernel::with_monospace(),
        &carried,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(again
        .journal()
        .any(|l| l.ends_with(" forgot 2 waiting sends (wrote)")));
    assert!(again.queued().is_empty());
    assert!(again.pending().is_empty());
}

#[test]
fn queue_parses_in_its_position_and_lowers_to_the_plan() {
    let plan = contract::compile(APP).unwrap();
    assert!(plan.mutations.iter().all(|m| m.queue));
    // Only after the shape: `queue` is a keyword nowhere else.
    let e = contract::compile(&APP.replace(
        "as shape Ack queue refreshes probe then afterWrote",
        "as shape Ack refreshes probe queue then afterWrote",
    ))
    .unwrap_err();
    assert!(e.id.starts_with("syntax-"), "{e}");
    let named = APP
        .replace("state label = \"x\"", "state queue = \"x\"")
        .replace("${label}", "${queue}")
        .replace("save(label)", "save(queue)")
        .replace("label = \"y\"", "queue = \"y\"")
        .replace("label = label", "queue = queue");
    contract::compile(&named).unwrap();
}

/// LLP 1092 D6: two sends to a queue mutation compile; to any other, the
/// refusal names `queue` — unless the action also assigns the slot, a
/// reply it means to drop.
#[test]
fn send_twice_is_accepted_for_a_queue_and_names_queue_otherwise() {
    let app = |queue: &str, extra: &str| {
        format!("shape Ack\n  op: string\ncomponent App\n  state open = true\n  mutation edited as shape Ack{queue} then afterEdit\n  action afterEdit\n    open = false\n  action commitEdit(k: string)\n    send edited = saveCard(\"a\")\n    send edited = setImage(\"b\")\n{extra}  view\n    button \"s\" press=commitEdit(\"1\")\n")
    };
    contract::compile(&app(" queue", "")).unwrap();
    contract::compile(&app(" queue", "    edited = none\n")).unwrap();
    let e = contract::compile(&app("", "")).unwrap_err();
    assert_eq!(
        (e.id.as_str(), e.message.as_str()),
        (
            "analyze-send-twice",
            "`commitEdit` sends `edited` twice; only the last send's reply reaches `then afterEdit` (LLP 1016 D5). Send once, use a mutation per request, or declare `mutation edited … queue` to run both in order"
        )
    );
    let e = contract::compile(&app("", "    edited = none\n")).unwrap_err();
    assert_eq!(
        e.message,
        "`commitEdit` sends `edited` twice; only the last send's reply reaches `then afterEdit` (LLP 1016 D5). Send once, or use a mutation per request"
    );
}

#[test]
fn a_shaped_failure_is_an_answer_and_runs_then() {
    let mut r = boot(true);
    r.act("pair", vec![]).unwrap();
    let t = tickets(&mut r);
    r.fulfill(
        t[0],
        Outcome::Failed {
            kind: FailureKind::Network,
            message: "offline".into(),
        },
    )
    .unwrap();
    r.advance(0.0).unwrap();
    assert_eq!(slot(&r, "log"), " offline");
    assert_eq!(r.data().asks, ["save:a", "save:b"]);
}

/// LLP 1092 D3, D8: a `next` whose answer makes a task's key no key is
/// refused by the gate step, not by its ask: the head is kept and the
/// queue stalls until a standing commit changes the state.
#[test]
fn a_next_refused_by_a_task_key_keeps_its_head_and_stalls() {
    let src = APP.replace(
        "  view\n",
        "  action twoRec\n    send rec = save(\"ok\")\n    send rec = save(\"nan\")\n  task watch key=recd == \"nan\" ? 0 / 0 : 1\n    after(100000, bump)\n  view\n",
    );
    let mut r = boot_with(&src, Desk::default());
    r.act("twoRec", vec![]).unwrap();
    assert!(matches!(
        r.advance(0.0),
        Err(RunnerError::TaskKey { task }) if task == "watch"
    ));
    assert!(r
        .journal()
        .any(|l| l.contains("rec next refused (TaskKey") && l.ends_with("waits for a change")));
    assert_eq!(r.queued(), [("rec".to_string(), 1)]);
    assert_eq!(r.derive("recd"), Some(&Value::str("ok")));
    assert_eq!(r.timer_due_ms(), Some(100_000.0), "only the task is due");
    // A change asks it again; it is refused the same way and waits again.
    r.act("bump", vec![]).unwrap();
    assert!(r.advance(0.0).is_err());
    assert_eq!(r.data().asks, ["save:ok", "save:nan", "save:nan"]);
    assert_eq!(r.queued(), [("rec".to_string(), 1)]);
}

/// LLP 1092 D8: the gate step is on every commit's settlement path — a
/// reply's (`fulfill_inner`) and a commit made again after a failed reply
/// (`commit_again`) — and a key that is no key refuses that commit.
#[test]
fn a_task_key_refuses_a_reply_and_a_commit_made_again() {
    // A reply whose answer makes the key no key: refused, the slot as it was.
    let src = APP.replace(
        "  view\n",
        "  task watch key=recd == \"bad\" ? 0 / 0 : 1\n    after(100000, bump)\n  view\n",
    );
    let mut r = boot_with(
        &src,
        Desk {
            later: true,
            ..Desk::default()
        },
    );
    r.act("record", vec![Value::str("bad")]).unwrap();
    let t = tickets(&mut r);
    assert!(matches!(
        r.fulfill(t[0], reply("ok")),
        Err(RunnerError::TaskKey { task }) if task == "watch"
    ));
    assert_eq!(r.derive("recd"), Some(&Value::str("-")));
    assert_eq!(r.timer_due_ms(), Some(100_000.0), "the timer as it was");
    // A failed reply's release commits again; that commit's key is no key
    // once nothing is pending.
    let src = APP.replace("  state ticks = 0\n", "  state ticks = 0\n  state armed = false\n").replace(
        "  view\n",
        "  action arm\n    armed = true\n    send rec = save(\"x\")\n  task watch key=armed and not pending(rec) ? 0 / 0 : 1\n    after(100000, bump)\n  view\n",
    );
    let mut r = boot_with(
        &src,
        Desk {
            later: true,
            ..Desk::default()
        },
    );
    r.act("arm", vec![]).unwrap();
    let t = tickets(&mut r);
    r.data().fail_parse = true;
    assert!(matches!(
        r.fulfill(t[0], reply("ok")),
        Err(RunnerError::TaskKey { .. })
    ));
    assert!(r
        .journal()
        .any(|l| l.contains("a failed request (0 asked again) refused: TaskKey")));
}

/// A source whose `save` answers now and whose `items` answers later once
/// booted: the shape of a module that writes at once and lists by fetch.
#[derive(Default)]
struct Shelf {
    lists: usize,
}

impl DataSource for Shelf {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "items" => Ok(Value::Number(0.0)),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "save" => Ok(Answer::Now(ack(args[0].as_str().unwrap_or(""), 0))),
            "items" => {
                self.lists += 1;
                Ok(Answer::Later(Request::post_json(
                    "https://desk.test/items",
                    "",
                )))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

/// b6 review A1: a `next` whose send answers at once has landed, so what
/// its mutation refreshes is forced — asked again even with unchanged
/// arguments — as an action's send answered at once forces it. A re-read
/// would drop the later answer and nothing would ever ask again.
#[test]
fn a_next_answered_at_once_forces_what_it_refreshes() {
    let src = "shape Ack\n  op: string\n  n: number\ncomponent App\n  resource items = items() as shape number\n  mutation save as shape Ack queue refreshes items\n  action pair\n    send save = save(\"a\")\n    send save = save(\"b\")\n  view\n    text `${items}`\n";
    let baked = contract::bake(contract::compile(src).unwrap(), Shelf::default()).unwrap();
    let mut r = Runner::boot(
        baked,
        Shelf::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let items = |r: &mut Runner<Shelf>| {
        r.take_requests()
            .iter()
            .filter(|q| q.target == "items")
            .count()
    };
    items(&mut r);
    r.act("pair", vec![]).unwrap();
    assert_eq!(items(&mut r), 1, "the first send's landing asks items");
    r.advance(1.0).unwrap();
    assert_eq!(
        items(&mut r),
        1,
        "the queued send's landing asks items again"
    );
}

/// A module its host loads after first pixel: not ready until `loaded`.
struct Late {
    loaded: std::rc::Rc<std::cell::Cell<bool>>,
}

impl DataSource for Late {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "readDoc" => Ok(Answer::Now(ack(args[0].as_str().unwrap_or(""), 0))),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn ready(&self) -> bool {
        self.loaded.get()
    }
}

/// b6 review A2: `data_ready`'s commit, which sends what waited for the
/// module, runs the gate step as every commit does (LLP 1092 D8): a gate
/// its answer opens arms the task, and a key that is no key refuses it.
#[test]
fn data_ready_s_sends_open_a_gate_and_a_bad_key_refuses_them() {
    let src = |task: &str| {
        format!("shape Ack\n  op: string\n  n: number\ncomponent App\n  state ticks = 0\n  mutation loaded as shape Ack\n  action open\n    send loaded = readDoc(\"doc\")\n  action tick\n    ticks = ticks + 1\n{task}  view\n    text \"x\"\n")
    };
    let boot = |src: &str| {
        let loaded = std::rc::Rc::new(std::cell::Cell::new(false));
        let r = Runner::boot(
            contract::compile(src).unwrap(),
            Late {
                loaded: loaded.clone(),
            },
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        (r, loaded)
    };
    let (mut r, loaded) = boot(&src(
        "  task poll when loaded != none\n    every(1000, tick)\n",
    ));
    r.act("open", vec![]).unwrap();
    assert_eq!(
        r.timer_due_ms(),
        None,
        "the gate is shut while the send waits"
    );
    loaded.set(true);
    r.data_ready().unwrap();
    assert_eq!(r.timer_due_ms(), Some(1000.0), "its answer opens the gate");
    let (mut r, loaded) = boot(&src(
        "  task poll key=(loaded != none ? 0 / 0 : 1)\n    every(1000, tick)\n",
    ));
    r.act("open", vec![]).unwrap();
    loaded.set(true);
    assert!(matches!(
        r.data_ready(),
        Err(RunnerError::TaskKey { task }) if task == "poll"
    ));
    assert_eq!(r.slot("loaded"), Some(&Value::NONE), "the commit as it was");
}
