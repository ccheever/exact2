//! A child a person's input or the app's own code started, against a tool's
//! commit (LLP 1133.007 D13): whatever the tool's commit asks of its
//! resource, the child keeps running under its ticket; the arguments the
//! tool asked stay refused after the child ends, until the person's own
//! commit changes them; and a tool's write to the child is dropped alone,
//! its call ending `person-required` (LLP 1111 D4a).
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, CallOrigin, DataError, DataSource, Message, Outcome, ProcessOp, Request, Runner,
    SocketOp, Store, Target,
};

const SRC: &str = r#"
component App
  state id = "a"
  state then = "spawn"
  resource server = server(id, then) as shape string
  resource feed = feed() as shape string
  mutation poked as shape string
  tool pokeChild writes additive closed "Write to the child and the feed." = poke
  action switch
    id = "b"
  action back
    id = "c"
  action next(what: string)
    then = what
  action poke
    send poked = poke()
  view
    column
      text server testId="server"
      text feed testId="feed"
"#;

/// `server(id, then)` spawns `path:t3 <id>` when `then` is `spawn`, and
/// otherwise answers with what `then` names: `now` at once, or an
/// `execFile`, a fetch or a storage read. `feed()` opens a WebSocket.
/// `poke()` writes to the child and sends a frame on the feed.
struct Src;

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
        "process.spawn path:t3\nnet.fetch https://x.test\nnet.websocket wss://x.test"
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
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let text = match outcome {
            Outcome::Message(m) => m.event,
            Outcome::Closed { .. } => "closed".into(),
            other => other.summary(),
        };
        Ok(Answer::Now(Value::str(&text)))
    }
}

fn boot() -> Runner<Src> {
    let plan = contract::compile(SRC).expect("the fixture compiles");
    Runner::boot(plan, Src, Kernel::with_monospace(), Default::default(), "/").unwrap()
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
