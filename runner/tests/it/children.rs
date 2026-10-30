//! A child a person's input or the app's own code started, against a tool's
//! commit (LLP 1133.007 D13): whatever the tool's commit asks of its
//! resource, the child keeps running under its ticket; the arguments the
//! tool asked stay refused after the child ends, or when no child ran, until
//! the person's own commit changes them, and a refused commit puts them
//! back; a tool's write to the child is dropped alone, its call ending
//! `person-required` (LLP 1111 D4a); and what a wake's commit leaves for
//! `data_ready` keeps the wake's origin (LLP 1133.002 D12).
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, CallOrigin, DataError, DataSource, Message, Outcome, ProcessOp, Request, Runner,
    SocketOp, Store, Target,
};
use std::cell::Cell;
use std::rc::Rc;

const SRC: &str = r#"
component App
  state id = "a"
  state then = "spawn"
  resource server = server(id, then) as shape string
  resource feed = feed() as shape string
  resource bad = bad(id) as shape string
  mutation poked as shape string
  mutation ran as shape string
  tool pokeChild writes additive closed "Write to the child and the feed." = poke
  action switch
    id = "b"
  action back
    id = "c"
  action broke
    id = "x"
  action next(what: string)
    then = what
  action reload
    refresh server
  action poke
    send poked = poke()
  action runIt
    send ran = run()
  view
    column
      text server testId="server"
      text feed testId="feed"
      text bad testId="bad"
"#;

/// `server(id, then)` reads the store, then spawns `path:t3 <id>` when
/// `then` is `spawn`, and otherwise answers with what `then` names: `now`
/// at once, or an `execFile`, a fetch or a storage read. `feed()` opens a
/// WebSocket, whose `write` frame writes the store. `bad(id)` answers what
/// its shape refuses for `x`. `poke()` writes to the child and sends a
/// frame on the feed; `run()` runs a program once.
struct Src {
    ready: Rc<Cell<bool>>,
}

fn spawn(arg: &str) -> Answer {
    let body = serde_json::json!({ "kind": "spawn", "program": "path:t3", "args": [arg] });
    Answer::Later(Request::process(
        body.to_string().into_bytes(),
        true,
        1 << 20,
    ))
}

impl DataSource for Src {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("answers through answer_for")
    }

    fn grants(&self) -> &str {
        "process.spawn path:t3\nnet.fetch https://x.test\nnet.websocket wss://x.test\nsecret.keep token"
    }

    fn ready(&self) -> bool {
        self.ready.get()
    }

    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let a = |i: usize| {
            args.get(i)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        // `server` reads the store: a write asks it again when it failed.
        if source == "server" {
            let _ = store.get("token");
        }
        Ok(match source {
            "server" => match a(1).as_str() {
                "spawn" => spawn(&a(0)),
                "now" => Answer::Now(Value::str("off")),
                "execFile" => {
                    let body = br#"{"kind":"execFile","program":"path:t3"}"#.to_vec();
                    Answer::Later(Request::process(body, false, 4096))
                }
                "fetch" => Answer::Later(Request::get("https://x.test/status")),
                _ => Answer::Later(Request::storage(Vec::new())),
            },
            "feed" => Answer::stream(Request {
                socket: true,
                ..Request::get("wss://x.test/feed")
            }),
            "bad" if a(0) == "x" => Answer::Now(Value::Number(1.0)),
            "bad" => Answer::Now(Value::str("ok")),
            "run" => {
                let body = br#"{"kind":"execFile","program":"path:t3","args":["run"]}"#;
                Answer::Later(Request::process(body.to_vec(), false, 4096))
            }
            "poke" => {
                assert!(matches!(target, Target::Mutation(_)));
                let write = ProcessOp::Write {
                    data: b"hello\n".to_vec(),
                    drain: false,
                };
                store.stage_socket(
                    0,
                    SocketOp::Process {
                        incarnation: 1,
                        op: write,
                    },
                );
                store.stage_socket(1, SocketOp::Text("hi".into()));
                Answer::Now(Value::str("poked"))
            }
            other => return Err(DataError::UnknownSource(other.into())),
        })
    }

    fn parse(
        &mut self,
        store: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if matches!(&outcome, Outcome::Message(m) if m.event == "write") {
            store.set("token", "t2").unwrap();
        }
        let text = match outcome {
            Outcome::Message(m) => m.event,
            Outcome::Closed { .. } => "closed".into(),
            other => other.summary(),
        };
        Ok(Answer::Now(Value::str(&text)))
    }
}

fn boot() -> Runner<Src> {
    boot_ready(true).0
}

/// A runner whose source answers once `ready` is set, and `data_ready`.
fn boot_ready(ready: bool) -> (Runner<Src>, Rc<Cell<bool>>) {
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let flag = Rc::new(Cell::new(ready));
    let src = Src {
        ready: flag.clone(),
    };
    let r = Runner::boot(plan, src, Kernel::with_monospace(), Default::default(), "/").unwrap();
    (r, flag)
}

/// What goes to the host now: each process request's first argument and
/// origin, and whether anything else went.
fn spawns(r: &mut Runner<Src>) -> Vec<(u64, String, CallOrigin)> {
    (r.take_requests().into_iter())
        .filter(|o| o.request.is_process())
        .map(|o| {
            let body: serde_json::Value = serde_json::from_slice(&o.request.body).unwrap();
            let arg = body["args"][0].as_str().unwrap_or("").to_string();
            (o.ticket, arg, o.request.reach.origin)
        })
        .collect()
}

fn opened(r: &mut Runner<Src>, ticket: u64, event: &str) {
    let data =
        serde_json::json!({ "pid": 41, "restarts": 0, "incarnation": 1, "ran": "script:s.json" });
    let m = Message {
        event: event.into(),
        data: data.to_string(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(m)).unwrap();
}

/// A booted runner whose child for `a` runs, as the app started it.
fn running() -> (Runner<Src>, u64) {
    let mut r = boot();
    let out = r.take_requests();
    let child = out
        .iter()
        .find(|o| o.target == "server")
        .expect("the child");
    assert_eq!(child.request.reach.origin, CallOrigin::App);
    let (ticket, feed) = (
        child.ticket,
        out.iter().find(|o| o.target == "feed").unwrap().ticket,
    );
    opened(&mut r, ticket, "spawn");
    opened(&mut r, feed, "open");
    (r, ticket)
}

fn journal(r: &Runner<Src>) -> String {
    r.journal().collect::<Vec<_>>().join("\n")
}

#[test]
fn a_tools_arguments_stay_refused_after_the_kept_child_ends() {
    let (mut r, ticket) = running();
    r.act_from(CallOrigin::Tool, "switch", vec![]).unwrap();
    assert!(spawns(&mut r).is_empty(), "kept: {}", journal(&r));
    assert!(r.holds(ticket));
    // The kept child ends for good: its close lands as the app's.
    let close = serde_json::json!({ "code": 0, "signal": null, "drained": true });
    let closed = Outcome::Closed {
        code: 1000,
        reason: close.to_string(),
        was_clean: true,
    };
    r.fulfill(ticket, closed).unwrap();
    assert!(spawns(&mut r).is_empty(), "the tool's `b`: {}", journal(&r));
    // A person's unrelated commit and a refresh leave them refused too.
    r.act("next", vec![Value::str("spawn")]).unwrap();
    assert!(spawns(&mut r).is_empty(), "{}", journal(&r));
    // The person's own commit that changes them starts theirs.
    r.act("back", vec![]).unwrap();
    let seen: Vec<(String, CallOrigin)> = (spawns(&mut r).into_iter())
        .map(|(_, a, o)| (a, o))
        .collect();
    assert_eq!(
        seen,
        [("c".to_string(), CallOrigin::App)],
        "{}",
        journal(&r)
    );
}

#[test]
fn a_tools_commit_lets_no_child_go_whatever_the_resource_would_answer() {
    for then in ["now", "execFile", "fetch", "storage"] {
        let (mut r, ticket) = running();
        r.act_from(CallOrigin::Tool, "next", vec![Value::str(then)])
            .unwrap();
        assert!(r.holds(ticket), "{then}: {}", journal(&r));
        assert!(r.take_requests().is_empty(), "{then}: nothing new is asked");
        assert!(
            journal(&r).contains("kept running, its re-ask refused, person-required (tool origin)"),
            "{then}: {}",
            journal(&r)
        );
        // A person's commit to the same arguments leaves them refused; one
        // that changes them lets the child go.
        r.act("next", vec![Value::str(then)]).unwrap();
        assert!(r.holds(ticket), "{then}: {}", journal(&r));
        r.act("back", vec![]).unwrap();
        assert!(!r.holds(ticket), "{then}: {}", journal(&r));
    }
}

#[test]
fn a_tools_write_is_dropped_alone_and_its_call_ends_person_required() {
    let (mut r, _) = running();
    let called = r
        .call_agent("call", r#"{"tool":"pokeChild","args":{}}"#)
        .expect("the agent's call")
        .unwrap();
    let id: serde_json::Value = serde_json::from_str(&called).unwrap();
    r.advance(0.0).unwrap();
    // The feed's frame goes; the child's write does not.
    let sends = r.take_socket_sends();
    let ops: Vec<&SocketOp> = sends
        .iter()
        .flat_map(|s| s.ops.iter().map(|(_, _, op)| op))
        .collect();
    assert!(
        matches!(ops.as_slice(), [SocketOp::Text(data)] if data == "hi"),
        "{ops:?}: {}",
        journal(&r)
    );
    assert!(
        journal(&r).contains("person-required (tool origin): child.write (LLP 1133.007 D13); dropped, the batch's other operations go"),
        "{}",
        journal(&r)
    );
    let answer = r
        .call_agent(
            "called",
            &serde_json::json!({ "id": id["call"] }).to_string(),
        )
        .expect("the call")
        .unwrap();
    assert!(answer.contains("person-required"), "{answer}");
}

/// What went to the host, as first argument and origin.
fn started(r: &mut Runner<Src>) -> Vec<(String, CallOrigin)> {
    (spawns(r).into_iter()).map(|(_, a, o)| (a, o)).collect()
}

/// A refusal the runner made is handed back as a host's answer would be.
fn settle_refusals(r: &mut Runner<Src>) {
    while let Some((ticket, outcome)) = r.take_request_refusal(true) {
        let _ = r.fulfill(ticket, outcome);
    }
}

#[test]
fn a_refused_start_stays_refused_with_no_child_running() {
    let mut r = boot();
    r.act("next", vec![Value::str("now")]).unwrap();
    r.take_requests();
    // No child runs. A tool asks for one: refused before the host.
    r.act_from(CallOrigin::Tool, "next", vec![Value::str("spawn")])
        .unwrap();
    assert!(spawns(&mut r).is_empty(), "{}", journal(&r));
    settle_refusals(&mut r);
    // A person's refresh asks the tool's arguments again: still refused.
    r.act("reload", vec![]).unwrap();
    assert!(spawns(&mut r).is_empty(), "{}", journal(&r));
    // The person's own commit that changes them starts theirs.
    r.act("back", vec![]).unwrap();
    let theirs = [("c".to_string(), CallOrigin::App)];
    assert_eq!(started(&mut r), theirs, "{}", journal(&r));
}

#[test]
fn a_store_write_asks_no_refused_start_again() {
    let mut r = boot();
    let out = r.take_requests();
    let feed = out.iter().find(|o| o.target == "feed").unwrap().ticket;
    opened(&mut r, feed, "open");
    r.act("next", vec![Value::str("now")]).unwrap();
    r.take_requests();
    r.act_from(CallOrigin::Tool, "next", vec![Value::str("spawn")])
        .unwrap();
    settle_refusals(&mut r);
    assert!(spawns(&mut r).is_empty(), "{}", journal(&r));
    // The person's own feed writes the store: the failed reader is asked
    // again, at the refused arguments, with no input from the person.
    opened(&mut r, feed, "write");
    assert!(spawns(&mut r).is_empty(), "{}", journal(&r));
}

#[test]
fn a_refused_persons_commit_puts_the_refused_arguments_back() {
    let (mut r, ticket) = running();
    r.act_from(CallOrigin::Tool, "switch", vec![]).unwrap();
    assert!(r.holds(ticket));
    // The person's commit to `x` changes the arguments and is refused
    // (`bad`'s shape): the tool's `b` stays refused.
    assert!(r.act("broke", vec![]).is_err());
    // The child's next frame lands as the app's: the child stays, and the
    // tool's arguments start nothing.
    opened(&mut r, ticket, "stdout");
    assert!(spawns(&mut r).is_empty(), "{}", journal(&r));
    assert!(r.holds(ticket), "{}", journal(&r));
}

#[test]
fn a_wakes_commit_before_data_ready_starts_nothing_there() {
    let (mut r, ready) = boot_ready(false);
    assert!(spawns(&mut r).is_empty());
    // Before the source can answer, a wake's commit gives the child's
    // resource new arguments and makes a send that would run a program.
    r.act_from(CallOrigin::Wake, "switch", vec![]).unwrap();
    r.act_from(CallOrigin::Wake, "runIt", vec![]).unwrap();
    assert!(spawns(&mut r).is_empty());
    ready.set(true);
    r.data_ready().unwrap();
    // Each is asked under the wake's origin there, and refused.
    assert!(spawns(&mut r).is_empty(), "{}", journal(&r));
    for kind in ["spawn", "execFile"] {
        let line = format!("process refused, person-required (wake origin): {kind}");
        assert!(journal(&r).contains(&line), "{line}: {}", journal(&r));
    }
}
