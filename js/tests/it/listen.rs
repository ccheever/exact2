//! A local listener in a TypeScript module, through the runner (LLP
//! 1133.005): `exactListen`'s request and options, its events with the
//! connection handle beside them, the mapper's replies on its own
//! connection, a mutation's on a handle it kept, and what refuses.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Message, Outcome, Runner, SocketOp, SocketOrigin};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/listen.hbc"));

const SRC: &str = r#"
component App
  resource control = control() as shape string
  resource present = present() as shape string
  mutation replied as shape string
  mutation listened as shape string
  action reply(id: string, text: string)
    send replied = reply(id, text)
  action listen
    send listened = listenFromMutation()
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
    assert_eq!(out.len(), 1, "present() answers now");
    let request = &out[0].request;
    assert!(request.stream && request.socket);
    assert_eq!(request.url, "unix:/tmp/exact-listen-js/a.sock");
    let listen = request.listen.as_deref().expect("a listener");
    assert!(listen.takeover);
    assert_eq!(listen.first_line_timeout_ms, Some(5000));
    assert_eq!(listen.max_line_bytes, 65_536);
    assert_eq!(listen.max_connections, 16);
    (r, out[0].ticket)
}

fn shown(r: &Runner<Module>, name: &str) -> String {
    r.resource(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// A mutation's answer.
fn slot(r: &Runner<Module>, name: &str) -> String {
    match r.slot(name) {
        Some(Value::Option(Some(v))) => v.as_str().unwrap_or("").to_string(),
        _ => String::new(),
    }
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

fn sends(r: &mut Runner<Module>) -> Vec<(SocketOrigin, Vec<SocketOp>)> {
    (r.take_socket_sends().into_iter())
        .map(|s| (s.origin, s.ops.into_iter().map(|(_, _, op)| op).collect()))
        .collect()
}

#[test]
fn the_global_is_defined_on_a_desktop_host() {
    let (r, _) = boot();
    let here = cfg!(any(target_os = "macos", target_os = "linux"));
    assert_eq!(shown(&r, "present"), if here { "yes" } else { "no" });
}

#[test]
fn events_reach_the_mapper_with_their_connection() {
    let (mut r, ticket) = boot();
    let at = "unix:/tmp/exact-listen-js/a.sock";
    assert_eq!(
        event(&mut r, ticket, "listening", "0", at),
        format!("listening {at} 0 -")
    );
    assert_eq!(
        event(&mut r, ticket, "connection", "1", ""),
        "connection 1  1"
    );
    assert_eq!(
        event(&mut r, ticket, "message", "1", "{}"),
        "message 1 {} 1"
    );
    assert!(sends(&mut r).is_empty());
    // A bad line is answered at once, on its own connection.
    assert_eq!(
        event(&mut r, ticket, "message", "1", "bad"),
        "message 1 bad 1"
    );
    let mapper = SocketOrigin::Mapper("control".into());
    let line = |text: &str| SocketOp::Line {
        connection: 1,
        text: text.into(),
    };
    assert_eq!(
        sends(&mut r),
        vec![(
            mapper,
            vec![line("refused"), SocketOp::End { connection: 1 }]
        )]
    );
    // A line with a newline would be two lines to the peer.
    assert_eq!(
        event(&mut r, ticket, "message", "1", "newline"),
        "message 1 SyntaxError"
    );
    // The mapper sends only on the connection whose line it answers.
    assert_eq!(
        event(&mut r, ticket, "connection", "2", ""),
        "connection 2  1"
    );
    assert_eq!(
        event(&mut r, ticket, "message", "2", "other"),
        "message 2 InvalidStateError"
    );
    assert!(sends(&mut r).is_empty());
}

#[test]
fn a_mutation_answers_a_kept_connection_and_not_an_ended_one() {
    let (mut r, ticket) = boot();
    event(
        &mut r,
        ticket,
        "listening",
        "0",
        "unix:/tmp/exact-listen-js/a.sock",
    );
    event(&mut r, ticket, "connection", "1", "");
    event(&mut r, ticket, "message", "1", "{}");
    r.act("reply", vec![Value::str("1"), Value::str("{\"ok\":true}")])
        .unwrap();
    assert_eq!(slot(&r, "replied"), "sent");
    let sent = sends(&mut r);
    assert_eq!(
        sent,
        vec![(
            SocketOrigin::Mutation("replied".into()),
            vec![
                SocketOp::Line {
                    connection: 1,
                    text: "{\"ok\":true}".into()
                },
                SocketOp::End { connection: 1 }
            ]
        )]
    );
    assert_eq!(event(&mut r, ticket, "end", "1", "app"), "end 1 app 3");
    r.act("reply", vec![Value::str("ended"), Value::str("x")])
        .unwrap();
    assert_eq!(slot(&r, "replied"), "InvalidStateError");
    assert!(sends(&mut r).is_empty());
}

#[test]
fn a_mutation_cannot_listen() {
    let (mut r, _) = boot();
    r.act("listen", vec![]).unwrap();
    assert_eq!(slot(&r, "listened"), "InvalidStateError");
    assert!(r.take_requests().is_empty());
}
