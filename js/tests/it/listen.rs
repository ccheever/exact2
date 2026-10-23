//! A local listener in a TypeScript module, through the runner (LLP
//! 1133.005 D1, D6): `exactListen`'s request, each event as the mapper sees
//! it, each connection's handle, and the replies its mapper and a mutation
//! send on it.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{HttpScheduling, Message, Outcome, Runner, SocketOp, SocketOrigin};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/listen.hbc"));

const SRC: &str = r#"
component App
  state running = true
  resource control = control(running) as shape string
  resource detect = detect() as shape string
  mutation replied as shape string
  mutation listened as shape string
  action reply(id: number, text: string)
    send replied = reply(id, text)
  action listenNow
    send listened = listenNow()
  action stop
    running = false
  view
    text control testId="control"
"#;

fn boot() -> (Runner<Module>, u64) {
    let mut m = Module::new(
        HBC.to_vec(),
        "test.listen",
        "net.listen unix:/tmp/exact-listen-js\n",
    );
    m.load().expect("the fixture loads");
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    let request = &out[0].request;
    assert!(request.stream && request.is_listen());
    assert_eq!(request.url, "unix:/tmp/exact-listen-js/app.sock");
    let listen = request.listen.as_deref().expect("its options");
    assert!(listen.takeover);
    assert_eq!(listen.first_line_timeout_ms, 5000);
    assert_eq!(listen.max_line_bytes, 65_536);
    assert_eq!(
        request.http,
        HttpScheduling::Independent {
            max_response_bytes: 4 << 20
        }
    );
    (r, out[0].ticket)
}

fn event(r: &mut Runner<Module>, ticket: u64, event: &str, id: &str, data: &str) -> String {
    let m = Message {
        event: event.into(),
        id: id.into(),
        data: data.into(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(m)).unwrap();
    shown(r, "control")
}

fn shown(r: &Runner<Module>, name: &str) -> String {
    r.resource(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// The batches the host would send now: who staged each, and its operations.
fn sends(r: &mut Runner<Module>) -> Vec<(SocketOrigin, Vec<SocketOp>)> {
    (r.take_socket_sends().into_iter())
        .map(|s| (s.origin, s.ops.into_iter().map(|(_, _, op)| op).collect()))
        .collect()
}

fn line(connection: u64, text: &str) -> SocketOp {
    SocketOp::Line {
        connection,
        text: text.into(),
    }
}

fn mapper(ops: Vec<SocketOp>) -> (SocketOrigin, Vec<SocketOp>) {
    (SocketOrigin::Mapper("control".into()), ops)
}

fn reply(r: &mut Runner<Module>, id: u64, text: &str) -> String {
    r.act("reply", vec![Value::Number(id as f64), Value::str(text)])
        .unwrap();
    match r.slot("replied") {
        Some(Value::Option(Some(v))) => v.as_str().unwrap_or("").to_string(),
        _ => String::new(),
    }
}

fn journal(r: &Runner<Module>) -> String {
    r.journal().collect::<Vec<_>>().join("\n")
}

#[test]
fn the_mapper_sees_each_event_and_answers_its_own_connection() {
    let (mut r, ticket) = boot();
    let bound = r#"{"address":"unix:/tmp/exact-listen-js/app.sock","port":0,"tookOver":false}"#;
    assert_eq!(
        event(&mut r, ticket, "listening", "", bound),
        "listening unix:/tmp/exact-listen-js/app.sock 0"
    );
    assert_eq!(
        event(&mut r, ticket, "connection", "1", ""),
        "connection 1 1"
    );
    assert_eq!(sends(&mut r), [mapper(vec![line(1, "hello")])]);
    assert_eq!(
        event(&mut r, ticket, "message", "1", "bad"),
        "message 1 bad"
    );
    assert_eq!(
        sends(&mut r),
        [mapper(vec![
            line(1, "refused"),
            SocketOp::End { connection: 1 }
        ])]
    );
    // An ended connection's handle refuses, as a socket's does on `close`.
    assert_eq!(
        event(&mut r, ticket, "end", "1", "app"),
        "end 1 app 3 (Error: socket.send is only available in a mutation's synchronous source, its socket's own mapper or an answer in flight)"
    );
}

#[test]
fn a_mutation_answers_a_line_later_on_an_open_connection() {
    let (mut r, ticket) = boot();
    event(&mut r, ticket, "listening", "", "{}");
    event(&mut r, ticket, "connection", "1", "");
    sends(&mut r);
    event(&mut r, ticket, "message", "1", "open the project");
    assert!(sends(&mut r).is_empty(), "the mapper queued it");
    assert_eq!(reply(&mut r, 1, "{\"ok\":true}"), "sent");
    assert_eq!(
        sends(&mut r),
        [(
            SocketOrigin::Mutation("replied".into()),
            vec![line(1, "{\"ok\":true}")]
        )]
    );
    assert_eq!(reply(&mut r, 1, "end"), "sent");
    assert_eq!(
        sends(&mut r),
        [(
            SocketOrigin::Mutation("replied".into()),
            vec![SocketOp::End { connection: 1 }]
        )]
    );
    event(&mut r, ticket, "end", "1", "app");
    assert_eq!(reply(&mut r, 1, "late"), "no connection 1");
}

#[test]
fn what_a_connection_s_handle_refuses() {
    let (mut r, ticket) = boot();
    event(&mut r, ticket, "listening", "", "{}");
    event(&mut r, ticket, "connection", "1", "");
    event(&mut r, ticket, "connection", "2", "");
    sends(&mut r);
    assert_eq!(
        event(&mut r, ticket, "message", "2", "two"),
        "message 2 two (SyntaxError: connection.send takes one line: the peer would read a \\n as two)"
    );
    assert!(sends(&mut r).is_empty());
    let other = event(&mut r, ticket, "message", "2", "other");
    assert!(
        other.contains("is only available in a mutation's synchronous source, its socket's own mapper or an answer in flight"),
        "a mapper sends only on its own connection: {other}"
    );
    assert_eq!(
        event(&mut r, ticket, "message", "2", "after end"),
        "message 2 after end (InvalidStateError: the socket is not open)"
    );
    assert_eq!(
        sends(&mut r),
        [mapper(vec![SocketOp::End { connection: 2 }])]
    );
    r.act("listenNow", vec![]).unwrap();
    let listened = format!("{:?}", r.slot("listened"));
    assert!(
        listened.contains("InvalidStateError: exactListen() answers a resource"),
        "{listened}"
    );
}

#[test]
fn the_listener_s_end_closes_every_handle_and_names_its_kind() {
    let (mut r, ticket) = boot();
    event(&mut r, ticket, "listening", "", "{}");
    event(&mut r, ticket, "connection", "1", "");
    sends(&mut r);
    let failed = Outcome::Failed {
        kind: exact_runner::FailureKind::Network,
        message: "AddressInUse: unix:/tmp/exact-listen-js/app.sock is in use".into(),
    };
    r.fulfill(ticket, failed).unwrap();
    assert_eq!(
        shown(&r, "control"),
        "error AddressInUse: unix:/tmp/exact-listen-js/app.sock is in use"
    );
    assert_eq!(
        reply(&mut r, 1, "late"),
        "(InvalidStateError: the socket is not open)"
    );
    assert!(!journal(&r).contains("listen send"));
}

#[test]
fn the_global_is_there_on_a_target_with_a_listener() {
    let (r, _) = boot();
    let expected = cfg!(any(target_os = "macos", target_os = "linux"));
    assert_eq!(shown(&r, "detect"), expected.to_string());
}
