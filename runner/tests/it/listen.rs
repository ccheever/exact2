//! A local listener, the runner half (LLP 1133.005): its record for
//! `state.streams` and the journal (D12), the host's notes that commit
//! nothing (D2), and replies addressed to its connections (D6), from its
//! mapper and from a mutation, only while the connection is open.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Message, Outcome, Request, Runner, SocketOp, SocketOrigin,
    SocketSend, Store, Target,
};

const SRC: &str = r#"
component App
  resource control = control("unix:/tmp/exact-listen-it/app.sock") as shape string
  mutation replied as shape string
  action reply(id: number, text: string)
    send replied = reply(id, text)
  view
    text control testId="control"
"#;

/// `control(address)` listens. Its mapper answers `<event> <id> <data>`,
/// greets each connection with `hello`, and answers a line `bye` with
/// `later` and the connection's end. `reply(id, text)` sends `text` on
/// connection `id`. Each mapper turn keeps the listener snapshot (`seen`).
#[derive(Default)]
struct Control {
    seen: Vec<Vec<(usize, u64, u64)>>,
}

impl DataSource for Control {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture answers through answer_for")
    }

    fn grants(&self) -> &str {
        "net.listen unix:/tmp/exact-listen-it"
    }

    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match (source, target) {
            ("control", Target::Resource(_)) => Ok(Answer::stream(Request::listen(
                args[0].as_str().unwrap_or(""),
            ))),
            ("reply", Target::Mutation(_)) => {
                let connection = args[0].as_number().unwrap_or(0.) as u64;
                let text = args[1].as_str().unwrap_or("").to_string();
                store.stage_socket(
                    0,
                    SocketOp::Line {
                        connection,
                        text: text.clone(),
                    },
                );
                Ok(Answer::Now(Value::str(&text)))
            }
            _ => Err(DataError::UnknownSource(source.into())),
        }
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
        _: Target,
        store: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.seen.push(store.listen_buffered().to_vec());
        let Outcome::Message(m) = outcome else {
            return Ok(Answer::Now(Value::str(&outcome.summary())));
        };
        let connection = m.id.parse::<u64>().unwrap_or(0);
        if m.event == "connection" {
            store.stage_socket(
                0,
                SocketOp::Line {
                    connection,
                    text: "hello".into(),
                },
            );
        }
        if m.event == "message" && m.data == "bye" {
            store.stage_socket(
                0,
                SocketOp::Line {
                    connection,
                    text: "later".into(),
                },
            );
            store.stage_socket(0, SocketOp::End { connection });
        }
        Ok(Answer::Now(Value::str(&format!(
            "{} {} {}",
            m.event, m.id, m.data
        ))))
    }
}

fn boot() -> (Runner<Control>, u64) {
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(
        plan,
        Control::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .expect("boots");
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    assert!(out[0].request.is_listen() && out[0].request.stream);
    assert_eq!(out[0].request.url, "unix:/tmp/exact-listen-it/app.sock");
    (r, out[0].ticket)
}

fn event(r: &mut Runner<Control>, ticket: u64, event: &str, id: &str, data: &str) {
    let m = Message {
        event: event.into(),
        id: id.into(),
        data: data.into(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(m)).unwrap();
}

fn listening(r: &mut Runner<Control>, ticket: u64) {
    let data = r#"{"address":"unix:/tmp/exact-listen-it/app.sock","port":0,"tookOver":false}"#;
    event(r, ticket, "listening", "", data);
}

/// Each batch: who staged it, and its operations as (connection, text).
fn batches(sends: &[SocketSend]) -> Vec<(SocketOrigin, Vec<(u64, String)>)> {
    (sends.iter())
        .map(|s| {
            let ops = s.ops.iter().map(|(_, _, op)| match op {
                SocketOp::Line { connection, text } => (*connection, text.clone()),
                SocketOp::End { connection } => (*connection, "<end>".into()),
                other => (0, format!("{other:?}")),
            });
            (s.origin.clone(), ops.collect())
        })
        .collect()
}

fn journal(r: &Runner<Control>) -> String {
    r.journal().collect::<Vec<_>>().join("\n")
}

#[test]
fn a_waiting_listener_is_pending_but_quiescent_until_it_binds() {
    let (mut r, ticket) = boot();
    assert!(r.has_pending(), "binding is in flight");
    event(&mut r, ticket, "exact:waiting", "", "");
    assert!(
        !r.has_pending(),
        "no clock waits on a listener waiting for its address"
    );
    assert_eq!(
        r.resource("control"),
        Some(&Value::str("")),
        "a note commits nothing"
    );
    let state = exact_runner::agent::state(&r);
    assert!(
        state.contains(r#""address":"unix:/tmp/exact-listen-it/app.sock","listener":"waiting""#),
        "{state}"
    );
    assert!(
        state.contains(r#""grant":"net.listen unix:/tmp/exact-listen-it""#),
        "{state}"
    );
    assert!(journal(&r).contains("listen control: waiting for unix:/tmp/exact-listen-it/app.sock"));
    listening(&mut r, ticket);
    assert_eq!(r.resource("control"), Some(&Value::str("listening  {\"address\":\"unix:/tmp/exact-listen-it/app.sock\",\"port\":0,\"tookOver\":false}")));
    assert!(!r.has_pending());
    let state = exact_runner::agent::state(&r);
    assert!(
        state.contains(r#""listener":"listening","connections":[]"#),
        "{state}"
    );
    assert!(journal(&r).contains("listen control: unix:/tmp/exact-listen-it/app.sock bound"));
}

#[test]
fn replies_reach_open_connections_only() {
    let (mut r, ticket) = boot();
    listening(&mut r, ticket);
    event(&mut r, ticket, "connection", "1", "");
    let sends = r.take_socket_sends();
    assert_eq!(
        batches(&sends),
        [(
            SocketOrigin::Mapper("control".into()),
            vec![(1, "hello".into())]
        )]
    );
    assert_eq!(
        sends[0].ops[0].0, ticket,
        "addressed to the listener's stream"
    );
    r.socket_published(&sends[0], Ok(vec![(ticket, 1, 6)]));
    event(&mut r, ticket, "message", "1", "ask");
    r.act("reply", vec![Value::Number(1.), Value::str("answer")])
        .unwrap();
    let sends = r.take_socket_sends();
    assert_eq!(
        batches(&sends),
        [(
            SocketOrigin::Mutation("replied".into()),
            vec![(1, "answer".into())]
        )]
    );
    r.socket_published(&sends[0], Ok(vec![(ticket, 1, 7)]));
    let state = exact_runner::agent::state(&r);
    assert!(
        state.contains(r#""connections":[{"id":1,"received":1,"sent":2}]"#),
        "{state}"
    );
    let log = journal(&r);
    assert!(log.contains("listen control: connection 1 opened"), "{log}");
    assert!(
        log.contains("listen send: 6 bytes (control, connection 1)"),
        "{log}"
    );
    // The mapper answers `bye` and ends the connection; once its end lands,
    // a send on it is refused.
    event(&mut r, ticket, "message", "1", "bye");
    assert_eq!(
        batches(&r.take_socket_sends()),
        [(
            SocketOrigin::Mapper("control".into()),
            vec![(1, "later".into()), (1, "<end>".into())]
        )]
    );
    event(&mut r, ticket, "end", "1", "app");
    assert!(journal(&r).contains("listen control: connection 1 ended (app)"));
    r.act("reply", vec![Value::Number(1.), Value::str("late")])
        .unwrap();
    assert!(r.take_socket_sends().is_empty());
    assert!(journal(&r).contains("send replied failed: InvalidStateError: the socket is not open"));
    // Nor on a connection that never opened.
    r.act("reply", vec![Value::Number(9.), Value::str("stray")])
        .unwrap();
    assert!(r.take_socket_sends().is_empty());
}

#[test]
fn each_turn_reads_its_connections_unwritten_bytes() {
    let (mut r, ticket) = boot();
    r.set_listen_reader(std::sync::Arc::new(move |t, c| {
        (t == ticket && c == 1).then_some(40)
    }));
    listening(&mut r, ticket);
    event(&mut r, ticket, "connection", "1", "");
    event(&mut r, ticket, "message", "1", "ask");
    // The host has not taken the mapper's greeting yet: it counts too.
    assert_eq!(r.data().seen.last().unwrap(), &vec![(0, 1, 40 + 5)]);
}

#[test]
fn a_net_listen_line_names_unix_addresses_only() {
    use exact_runner::grants;
    assert!(grants::parse("net.listen unix:\nnet.listen unix:/tmp/x").is_ok());
    for bad in [
        "net.listen tcp://127.0.0.1:0",
        "net.listen unix:relative",
        "net.listen",
        "net.listen unix: unix:/a",
    ] {
        assert!(grants::parse(bad).is_err(), "{bad}");
    }
    assert_eq!(
        exact_runner::io_grants("net.listen unix:\nnet.fetch https://x.test"),
        "net.fetch https://x.test"
    );
    let spec = "net.listen unix:/tmp/a\nnet.listen unix:/var/b/";
    assert_eq!(
        grants::admitting_listen(spec, None, "unix:/tmp/a/x.sock"),
        Some("net.listen unix:/tmp/a")
    );
    assert_eq!(
        grants::admitting_listen(spec, None, "unix:/tmp/ab/x.sock"),
        None
    );
    assert_eq!(
        grants::admitting_listen(spec, None, "unix:/var/b/x.sock"),
        Some("net.listen unix:/var/b/")
    );
}
