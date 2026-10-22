//! A child process in a TypeScript module, through the runner (LLP
//! 1133.007 D2, D3, D13): the process request, the incarnation's handle,
//! staged writes, ends and kills addressed to it, the origin each carries,
//! and a one-shot `execFile`.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Message, Outcome, ProcessOp, Response, Runner, SocketOp, SocketOrigin, TurnOrigin,
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
    assert_eq!(sends[0].turn, TurnOrigin::Person);
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

#[test]
fn a_turn_that_is_not_a_persons_carries_its_origin() {
    let (mut r, ticket) = boot();
    spawn(&mut r, ticket, 1);
    r.take_socket_sends();
    r.set_turn_origin(TurnOrigin::Tool);
    assert_eq!(poke(&mut r, "kill"), "true");
    let sends = r.take_socket_sends();
    assert_eq!(sends[0].turn, TurnOrigin::Tool, "the host refuses it (D13)");
    // A child's mapper keeps the origin of the turn that started it.
    r.fulfill(ticket, event("exit", serde_json::json!({ "code": 0 })))
        .unwrap();
    r.fulfill(
        ticket,
        event("close", serde_json::json!({ "code": 0, "drained": true })),
    )
    .unwrap();
    spawn(&mut r, ticket, 2);
    assert_eq!(r.take_socket_sends()[0].turn, TurnOrigin::Person);
    // A spawn a tool's commit asks for is stamped for the host to refuse.
    r.act("switch", vec![]).unwrap();
    let out = r.take_requests();
    assert_eq!(out[0].request.process_origin(), TurnOrigin::Tool);
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
