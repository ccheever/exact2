//! A child process in a TypeScript module, through the runner (LLP
//! 1133.007 D2, D3, D13): the process request, the incarnation's handle,
//! staged writes, ends and kills addressed to it, their admission by the
//! commit's origin, and a one-shot `execFile`.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    CallOrigin, FailureKind, Message, Outcome, ProcessOp, Response, Runner, SocketOp, SocketOrigin,
};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/process.hbc"));

const SRC: &str = r#"
component App
  state id = "a"
  resource server = server(id) as shape string
  mutation poked as shape string
  action poke(what: string)
    send poked = poke(what)
  action switch
    id = "b"
  action other
    id = "c"
  action reload
    refresh server
  view
    text server testId="server"
"#;

fn boot() -> (Runner<Module>, u64) {
    let mut m = Module::new(HBC.to_vec(), "test.process", "process.spawn path:t3\n");
    m.load().expect("the fixture loads");
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    let request = &out[0].request;
    assert!(request.is_process() && request.stream && request.socket);
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["kind"], "spawn");
    assert_eq!(body["program"], "path:t3");
    assert_eq!(
        body["args"],
        serde_json::json!(["--bootstrap-fd", "0", "a"])
    );
    assert_eq!(body["restart"]["when"], "always");
    (r, out[0].ticket)
}

fn event(name: &str, data: serde_json::Value) -> Outcome {
    Outcome::Message(Message {
        event: name.into(),
        data: data.to_string(),
        ..Message::default()
    })
}

fn shown(r: &Runner<Module>) -> String {
    r.resource("server")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn poke(r: &mut Runner<Module>, what: &str) -> String {
    r.act("poke", vec![Value::str(what)]).unwrap();
    match r.slot("poked") {
        Some(Value::Option(Some(v))) => v.as_str().unwrap_or("").to_string(),
        _ => String::new(),
    }
}

fn spawn(r: &mut Runner<Module>, ticket: u64, n: u32) {
    let data = serde_json::json!({ "pid": 40 + n, "restarts": n - 1, "incarnation": n, "ran": "script:s.json" });
    r.fulfill(ticket, event("spawn", data)).unwrap();
}

#[test]
fn each_incarnations_handle_stages_its_own_writes() {
    let (mut r, ticket) = boot();
    spawn(&mut r, ticket, 1);
    assert_eq!(shown(&r), "spawn 41 0");
    // The mapper's write and end, addressed to incarnation 1, for this commit.
    let sends = r.take_socket_sends();
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].origin, SocketOrigin::Mapper("server".into()));
    let ops: Vec<_> = sends[0]
        .ops
        .iter()
        .map(|(t, _, op)| (*t, op.clone()))
        .collect();
    assert_eq!(
        ops,
        [
            (
                ticket,
                SocketOp::Process {
                    incarnation: 1,
                    op: ProcessOp::Write(b"envelope a\n".to_vec())
                }
            ),
            (
                ticket,
                SocketOp::Process {
                    incarnation: 1,
                    op: ProcessOp::End
                }
            ),
        ]
    );
    r.fulfill(
        ticket,
        Outcome::Message(Message {
            event: "stdout".into(),
            data: "listening".into(),
            ..Message::default()
        }),
    )
    .unwrap();
    assert_eq!(shown(&r), "stdout listening");
    // Its stdin was ended by the mapper: a mutation's write refuses.
    assert!(
        poke(&mut r, "write").starts_with("InvalidStateError"),
        "{}",
        poke(&mut r, "write")
    );
    assert_eq!(poke(&mut r, "kill"), "true");
    let kill = r.take_socket_sends();
    assert_eq!(
        kill.last().unwrap().ops[0].2,
        SocketOp::Process {
            incarnation: 1,
            op: ProcessOp::Kill("SIGINT".into())
        }
    );
    assert!(poke(&mut r, "bad signal").starts_with("TypeError"));
    // Its exit and close: the handle is stale from the close on.
    r.fulfill(
        ticket,
        event(
            "exit",
            serde_json::json!({ "code": null, "signal": "SIGINT" }),
        ),
    )
    .unwrap();
    assert_eq!(shown(&r), "exit null SIGINT");
    r.fulfill(
        ticket,
        event(
            "close",
            serde_json::json!({ "code": null, "signal": "SIGINT", "drained": true }),
        ),
    )
    .unwrap();
    assert_eq!(shown(&r), "close null true (InvalidStateError)");
    assert!(poke(&mut r, "kill").contains("incarnation has ended"));
    // The restart's incarnation has a handle of its own.
    spawn(&mut r, ticket, 2);
    assert_eq!(shown(&r), "spawn 42 1");
    let again = r.take_socket_sends();
    assert!(matches!(
        again[0].ops[0].2,
        SocketOp::Process { incarnation: 2, .. }
    ));
    // The last close ends the stream.
    let close = serde_json::json!({ "code": 0, "signal": null, "drained": false });
    r.fulfill(
        ticket,
        Outcome::Closed {
            code: 1000,
            reason: close.to_string(),
            was_clean: false,
        },
    )
    .unwrap();
    assert_eq!(shown(&r), "close 0 false (InvalidStateError)");
    assert!(!r.holds(ticket));
    let logs = r.journal().collect::<Vec<_>>().join("\n");
    assert!(
        logs.contains(&format!("process {ticket} (server): spawn path:t3")),
        "{logs}"
    );
    assert!(
        !logs.contains("--bootstrap-fd"),
        "arguments are never journalled: {logs}"
    );
}

/// The refusal the runner holds for the request it kept from the host.
fn refused(r: &mut Runner<Module>) -> String {
    match r.take_request_refusal(true) {
        Some((_, Outcome::Failed { kind, message })) => {
            assert_eq!(kind, FailureKind::Refused);
            message
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_tool_or_a_wake_directs_no_child() {
    let (mut r, ticket) = boot();
    spawn(&mut r, ticket, 1);
    r.take_socket_sends();
    for origin in [CallOrigin::Tool, CallOrigin::Wake] {
        r.act_from(origin, "poke", vec![Value::str("kill")])
            .unwrap();
        assert!(r.take_socket_sends().is_empty(), "refused (D13)");
    }
    let journal: Vec<&str> = r.journal().collect();
    assert!(
        (journal.iter()).any(|l| l.contains("person-required (wake origin): child.kill")),
        "{journal:?}"
    );
    // A person's kill goes.
    assert_eq!(poke(&mut r, "kill"), "true");
    assert_eq!(r.take_socket_sends().len(), 1);
    // A tool's commit that changes the child's arguments asks for a new
    // spawn, which is refused once; the running child is kept, not let go.
    r.act_from(CallOrigin::Tool, "switch", vec![]).unwrap();
    assert!(r.take_requests().is_empty());
    assert!(r.take_request_refusal(true).is_none());
    assert!(
        r.pending().iter().any(|(_, t)| *t == ticket),
        "{:?}",
        r.pending()
    );
    // The tool's arguments are not inherited: a person's commit that
    // leaves them, or a reload, starts nothing, and nothing is said again.
    r.act("switch", vec![]).unwrap();
    r.act("reload", vec![]).unwrap();
    assert!(r.take_requests().is_empty());
    assert!(
        r.pending().iter().any(|(_, t)| *t == ticket),
        "{:?}",
        r.pending()
    );
    let kept = (r.journal())
        .filter(|l| l.contains("kept running, its replacement refused, person-required"))
        .count();
    assert_eq!(kept, 1);
    // A person's other arguments replace it.
    r.act("other", vec![]).unwrap();
    assert_eq!(r.take_requests().len(), 1);
}

#[test]
fn an_answer_asks_with_the_origin_of_the_commit_that_asked() {
    let mut m = Module::new(HBC.to_vec(), "test.process", "process.spawn path:t3\n");
    m.load().unwrap();
    let src = "component App\n  mutation asked as shape string\n  action ask\n    send asked = twice()\n  view\n    text \"x\"\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    r.take_requests();
    let reply = |out: &str| {
        let body = serde_json::json!({ "stdout": out, "stderr": "", "code": 0, "signal": null, "timedOut": false, "overflow": null });
        Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: body.to_string().into_bytes(),
        })
    };
    // A tool's ask starts nothing.
    r.act_from(CallOrigin::Tool, "ask", vec![]).unwrap();
    assert!(r.take_requests().is_empty());
    assert!(refused(&mut r).starts_with("person-required"));
    // A person's ask goes, and so does the second one-shot its answer asks.
    r.act("ask", vec![]).unwrap();
    let first = r.take_requests();
    assert_eq!(first[0].request.reach.origin, CallOrigin::App);
    r.fulfill(first[0].ticket, reply("1.0")).unwrap();
    let second = r.take_requests();
    assert_eq!(second.len(), 1, "{second:?}");
    assert_eq!(second[0].request.reach.origin, CallOrigin::App);
    r.fulfill(second[0].ticket, reply("usage")).unwrap();
    let asked = match r.slot("asked") {
        Some(Value::Option(Some(v))) => v.as_str().unwrap_or("").to_string(),
        other => format!("{other:?}"),
    };
    assert_eq!(asked, "1.0 usage");
}

#[test]
fn exec_file_answers_from_the_process_request_and_refused_options_reject() {
    let mut m = Module::new(HBC.to_vec(), "test.process", "process.spawn path:t3\n");
    m.load().unwrap();
    let src = "component App\n  resource v = version() as shape string\n  resource s = status() as shape string\n  resource bad = refused() as shape string\n  view\n    text v\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    assert_eq!(
        r.resource("s").and_then(Value::as_str),
        Some(r#"{"available":true,"state":"granted"}"#)
    );
    let out = r.take_requests();
    let exec = out.iter().find(|o| o.target == "v").unwrap();
    let body: serde_json::Value = serde_json::from_slice(&exec.request.body).unwrap();
    assert_eq!(
        (body["kind"].as_str(), body["timeout"].as_u64()),
        (Some("execFile"), Some(1500))
    );
    assert!(!exec.request.stream);
    let reply = serde_json::json!({ "stdout": "1.2.0\n", "stderr": "", "code": 0, "signal": null, "timedOut": false, "overflow": null });
    r.fulfill(
        exec.ticket,
        Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: reply.to_string().into_bytes(),
        }),
    )
    .unwrap();
    assert_eq!(r.resource("v").and_then(Value::as_str), Some("0 1.2.0"));
    let bad = r
        .resource("bad")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    assert!(bad.contains("`shell` is refused"), "{bad}");
}

#[test]
fn an_exec_file_its_signal_aborts_resumes_rejected_and_is_cancelled() {
    let mut m = Module::new(HBC.to_vec(), "test.process", "process.spawn path:t3\n");
    m.load().unwrap();
    let src = "component App\n  resource held = held() as shape string\n  mutation halted as shape string\n  action halt\n    send halted = halt()\n  view\n    text held\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let out = r.take_requests();
    let exec = out.iter().find(|o| o.target == "held").unwrap();
    assert!(exec.request.is_process() && !exec.request.stream);
    // Another answer's turn aborts it: answered aborted at the host's next
    // pump, and handed to the host to cancel, which ends the child (D3).
    r.act("halt", vec![]).unwrap();
    let (ticket, outcome) = r.take_request_refusal(false).expect("the aborted answer");
    assert_eq!(ticket, exec.ticket);
    r.fulfill(ticket, outcome).unwrap();
    assert_eq!(
        r.resource("held").and_then(Value::as_str),
        Some("AbortError")
    );
    assert_eq!(r.take_explicit_aborts(), [exec.ticket]);
}
