//! A resource's two-way socket, the runner half (LLP 1016.000 D7): a
//! mutation's staged operations reach the host only from a commit that
//! stood, addressed to the stream ticket of the resource that owns the
//! socket while it is open.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Message, Outcome, Request, Runner, SocketOp, Store, Target,
};

const SRC: &str = r#"
component App
  state id = "a"
  resource feed = feed(id) as shape string
  mutation said as shape string
  mutation other as shape string
  action say(what: string)
    send said = say(what)
  action both(what: string)
    send said = say(what)
    send other = misshapen()
  action switch
    id = "b"
  view
    text feed testId="feed"
"#;

/// `feed(id)` opens a socket; `say(what)` stages a text frame on it and
/// answers `what`; `misshapen()` answers outside its shape.
struct Echo;

impl DataSource for Echo {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture answers through answer_for")
    }

    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match (source, target) {
            ("feed", Target::Resource(_)) => {
                let id = args[0].as_str().unwrap_or("");
                let request = Request {
                    socket: true,
                    ..Request::get(&format!("ws://echo.test/{id}"))
                };
                Ok(Answer::stream(request))
            }
            ("say", Target::Mutation(_)) => {
                let what = args[0].as_str().unwrap_or("").to_string();
                store.stage_socket(0, SocketOp::Text(what.clone()));
                Ok(Answer::Now(Value::str(&what)))
            }
            ("misshapen", Target::Mutation(_)) => Ok(Answer::Now(Value::Number(1.))),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(Value::str(&match outcome {
            Outcome::Message(m) => format!("{} {}", m.event, m.data),
            Outcome::Closed { code, reason, .. } => format!("close {code} {reason}"),
            other => other.summary(),
        })))
    }
}

fn boot() -> (Runner<Echo>, u64) {
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(
        plan,
        Echo,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .expect("boots");
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    assert!(
        out[0].request.socket,
        "the host is told the socket is two-way"
    );
    (r, out[0].ticket)
}

fn opened(r: &mut Runner<Echo>, ticket: u64) {
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(open)).unwrap();
}

fn said(r: &mut Runner<Echo>, what: &str) {
    r.act("say", vec![Value::str(what)]).unwrap();
}

fn journal(r: &Runner<Echo>) -> String {
    r.journal().collect::<Vec<_>>().join("\n")
}

#[test]
fn a_send_before_open_is_refused_and_after_open_goes_out_once_the_commit_stands() {
    let (mut r, ticket) = boot();
    said(&mut r, "early");
    assert!(r.take_socket_sends().is_empty());
    assert!(journal(&r).contains("send said failed: InvalidStateError: the socket is not open"));
    opened(&mut r, ticket);
    assert_eq!(r.resource("feed"), Some(&Value::str("open ")));
    said(&mut r, "hi");
    said(&mut r, "again");
    let sends = r.take_socket_sends();
    let ops: Vec<_> = sends
        .iter()
        .map(|s| (s.mutation.as_str(), s.ops.clone()))
        .collect();
    let text = |t: &str| vec![(ticket, "feed".to_string(), SocketOp::Text(t.into()))];
    assert_eq!(ops, [("said", text("hi")), ("said", text("again"))]);
    // The host sent them: the counts and journal lines follow.
    for batch in &sends {
        r.socket_published(batch, Ok(vec![(ticket, 1, 3)]));
    }
    let state = exact_runner::agent::state(&r);
    assert!(
        state.contains(r#""readyState":1,"bufferedAmount":3,"sent":2"#),
        "{state}"
    );
    assert!(journal(&r).contains("socket send: 2 bytes (feed)"));
    r.socket_published(
        &sends[0],
        Err("QuotaExceededError: socket buffer exceeds 1 MiB".into()),
    );
    assert!(journal(&r).contains(
        "send said failed: QuotaExceededError: socket buffer exceeds 1 MiB; socket frames were refused"
    ));
}

#[test]
fn a_refused_commit_takes_its_frames_back() {
    let (mut r, ticket) = boot();
    opened(&mut r, ticket);
    assert!(r.act("both", vec![Value::str("lost")]).is_err());
    assert!(
        r.take_socket_sends().is_empty(),
        "nothing of a refused commit"
    );
    said(&mut r, "kept");
    assert_eq!(r.take_socket_sends().len(), 1);
}

#[test]
fn a_closed_or_released_socket_takes_no_more_frames() {
    let (mut r, ticket) = boot();
    opened(&mut r, ticket);
    let closed = Outcome::Closed {
        code: 4001,
        reason: "finished".into(),
        was_clean: true,
    };
    r.fulfill(ticket, closed).unwrap();
    assert_eq!(r.resource("feed"), Some(&Value::str("close 4001 finished")));
    assert!(!r.holds(ticket));
    said(&mut r, "late");
    assert!(r.take_socket_sends().is_empty());

    let (mut r, ticket) = boot();
    opened(&mut r, ticket);
    r.act("switch", vec![]).unwrap();
    assert!(!r.holds(ticket), "new arguments release the old socket");
    let newer = r.take_requests()[0].ticket;
    said(&mut r, "between");
    assert!(
        r.take_socket_sends().is_empty(),
        "the newer socket is not open"
    );
    opened(&mut r, newer);
    said(&mut r, "now");
    assert_eq!(r.take_socket_sends()[0].ops[0].0, newer);
}
