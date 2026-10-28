//! A resource's local listener, the runner half (LLP 1133.005): the request
//! carries its options, it is pending until `listening` lands, a listener
//! waiting for its address is pending but quiet (D2), its standing reaches
//! `state.streams` and the journal and no mapper, and its mapper's sends are
//! addressed to a connection (D6).
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::listen::{ConnectionView, Listen, ListenView, LISTEN_STATUS};
use exact_runner::{
    Answer, DataError, DataSource, Message, Outcome, Request, Runner, SocketOp, Store, Target,
};

const SRC: &str = r#"
component App
  resource control = control() as shape string
  view
    text control testId="control"
"#;

/// `control()` listens on `unix:/tmp/exact-listen-test/a.sock`; its mapper
/// answers each line `ok <line>` on the line's own connection and ends it.
struct Listener;

impl DataSource for Listener {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture answers through answer_for")
    }

    fn grants(&self) -> &str {
        "net.listen unix:/tmp/exact-listen-test"
    }

    fn answer_for(
        &mut self,
        _: Target,
        _: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        assert_eq!(source, "control");
        let options = Listen {
            takeover: true,
            first_line_timeout_ms: Some(5000),
            ..Listen::default()
        };
        let address = "unix:/tmp/exact-listen-test/a.sock";
        Ok(Answer::stream(Request::listen(address, options)))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        unreachable!("the fixture parses through parse_for")
    }

    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Target::Resource(i) = target else {
            unreachable!("only resources listen")
        };
        let Outcome::Message(m) = outcome else {
            return Ok(Answer::Now(Value::str("ended")));
        };
        assert_ne!(
            m.event, LISTEN_STATUS,
            "no mapper sees a listener's standing"
        );
        if m.event == "message" {
            let connection = m.id.parse().unwrap();
            let text = format!("ok {}", m.data);
            store.stage_socket(i, SocketOp::Line { connection, text });
            store.stage_socket(i, SocketOp::End { connection });
        }
        Ok(Answer::Now(Value::str(&format!("{} {}", m.event, m.id))))
    }
}

fn event(event: &str, id: &str, data: &str) -> Outcome {
    Outcome::Message(Message {
        event: event.into(),
        id: id.into(),
        data: data.into(),
        ..Message::default()
    })
}

fn boot() -> (Runner<Listener>, u64) {
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(
        plan,
        Listener,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .expect("boots");
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    let request = &out[0].request;
    assert!(request.stream && request.socket);
    let listen = request
        .listen
        .as_deref()
        .expect("the host is told it listens");
    assert!(listen.takeover);
    assert_eq!(listen.first_line_timeout_ms, Some(5000));
    assert_eq!(request.listen_refusal(), None);
    (r, out[0].ticket)
}

/// Whether the journal has `line` (each is stamped with its time).
fn logged(r: &Runner<Listener>, line: &str) -> bool {
    r.journal().any(|l| l.ends_with(line))
}

fn streams(r: &Runner<Listener>) -> String {
    exact_runner::agent::state(r)
}

#[test]
fn a_waiting_listener_is_pending_and_quiet_and_shown() {
    let (mut r, ticket) = boot();
    r.set_listen_reader(std::sync::Arc::new(move |t| {
        (t == ticket).then(|| ListenView {
            address: "unix:/tmp/exact-listen-test/a.sock".into(),
            connections: vec![ConnectionView {
                id: 3,
                received: 1,
                sent: 0,
            }],
        })
    }));
    assert!(r.in_flight().iter().any(|(_, t)| *t == ticket));
    let at = "unix:/tmp/exact-listen-test/a.sock";
    r.fulfill(ticket, event(LISTEN_STATUS, at, "waiting"))
        .unwrap();
    assert!(
        !r.in_flight().iter().any(|(_, t)| *t == ticket),
        "no clock waits on a listener waiting for its address"
    );
    let shown = streams(&r);
    assert!(
        shown.contains(&format!("\"listen\":\"waiting for {at}\"")),
        "{shown}"
    );
    assert!(
        shown.contains("\"grant\":\"net.listen unix:/tmp/exact-listen-test\""),
        "{shown}"
    );
    assert!(logged(&r, &format!("listen control {at}: waiting for it")));
    r.fulfill(ticket, event("listening", "0", at)).unwrap();
    let shown = streams(&r);
    assert!(shown.contains("\"listen\":\"listening\""), "{shown}");
    assert!(
        shown.contains("\"connections\":[{\"id\":3,\"received\":1,\"sent\":0}]"),
        "{shown}"
    );
    assert!(logged(&r, &format!("listen control {at} bound")));
    r.fulfill(ticket, event(LISTEN_STATUS, at, "displaced"))
        .unwrap();
    assert!(streams(&r).contains("\"listen\":\"displaced\""));
    r.fulfill(ticket, event(LISTEN_STATUS, at, "holding"))
        .unwrap();
    assert!(streams(&r).contains("\"listen\":\"listening\""));
    assert!(logged(&r, &format!("listen control {at}: taken over")));
}

#[test]
fn the_mapper_answers_its_connection_by_id() {
    let (mut r, ticket) = boot();
    r.fulfill(
        ticket,
        event("listening", "0", "unix:/tmp/exact-listen-test/a.sock"),
    )
    .unwrap();
    r.fulfill(ticket, event("connection", "3", "")).unwrap();
    assert!(logged(&r, "listen control: connection 3 opened"));
    assert!(r.take_socket_sends().is_empty());
    r.fulfill(ticket, event("message", "3", "{\"cwd\":\"/x\"}"))
        .unwrap();
    let sends = r.take_socket_sends();
    assert_eq!(sends.len(), 1);
    let ops: Vec<_> = sends[0]
        .ops
        .iter()
        .map(|(t, _, op)| (*t, op.clone()))
        .collect();
    assert_eq!(
        ops,
        vec![
            (
                ticket,
                SocketOp::Line {
                    connection: 3,
                    text: "ok {\"cwd\":\"/x\"}".into()
                }
            ),
            (ticket, SocketOp::End { connection: 3 }),
        ]
    );
    r.socket_published(&sends[0], Ok(vec![(ticket, 1, 0)]));
    assert!(logged(&r, "listen send: 16 bytes (control, connection 3)"));
    assert!(logged(&r, "listen end (control, connection 3)"));
    assert!(
        !r.journal().any(|l| l.contains("cwd")),
        "a line's text is never journaled"
    );
    r.fulfill(ticket, event("end", "3", "app")).unwrap();
    assert!(logged(&r, "listen control: connection 3 ended (app)"));
}

#[test]
fn net_listen_lines_are_the_runners_own_and_checked() {
    use exact_runner::grants;
    let spec = "net.fetch https://x.test\nnet.listen unix:\nnet.listen unix:/tmp/t3";
    assert!(grants::parse(spec).is_ok());
    assert_eq!(exact_runner::io_grants(spec), "net.fetch https://x.test");
    let bad = grants::parse("net.listen unix:relative\nnet.listen tcp://127.0.0.1:1").unwrap_err();
    assert_eq!(bad.len(), 2, "{bad:?}");
    assert!(bad[0].starts_with("line 1: "));
    let refused = Request {
        stream: false,
        ..Request::listen("unix:/tmp/a.sock", Listen::default())
    };
    assert!(refused.listen_refusal().is_some(), "a listener is a stream");
    let mut relative = Request::listen("unix:a.sock", Listen::default());
    relative.stream = true;
    assert!(relative.listen_refusal().unwrap().contains("absolute"));
}
