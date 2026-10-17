//! A resource's two-way socket, the runner half (LLP 1016.000 D7): the
//! operations a mutation, or the socket's own mapper answering a frame,
//! stages reach the host only from a commit that stood, addressed to the
//! stream ticket of the resource that owns the socket while it is open.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Message, Outcome, Request, Runner, SocketOp, SocketOrigin,
    SocketSend, Store, Target,
};

const SRC: &str = r#"
component App
  state id = "a"
  resource feed = feed(id) as shape string
  resource side = feed("side") as shape string
  resource gate = gate(feed) as shape string
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
/// answers `what`; `misshapen()` answers outside its shape. `gate(feed)`
/// refuses the commit whose feed reads `message refuse`. The mapper (`parse`)
/// stages a reply to each frame: `subscribe` on `open`, `Ack N` for
/// `frame N`, and one for each frame that then fails.
struct Echo;

impl DataSource for Echo {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture answers through answer_for")
    }

    fn grants(&self) -> &str {
        "net.fetch https://echo.test\nnet.websocket wss://echo.test\nnet.websocket ws:"
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
            ("gate", _) if args[0].as_str() == Some("message refuse") => {
                Err(DataError::Unavailable("deliberate commit refusal".into()))
            }
            ("gate", _) => Ok(Answer::Now(args[0].clone())),
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
        target: Target,
        store: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Target::Resource(i) = target else {
            unreachable!("only resources stream")
        };
        let text = |t: &str| SocketOp::Text(t.into());
        match &outcome {
            Outcome::Message(m) if m.event == "open" => store.stage_socket(i, text("subscribe")),
            Outcome::Message(m) if m.data == "other" => store.stage_socket(1, text("stray")),
            Outcome::Message(m) => {
                let ack = m.data.strip_prefix("frame ").unwrap_or(&m.data);
                store.stage_socket(i, text(&format!("Ack {ack}")));
            }
            _ => store.stage_socket(i, text("after the end")),
        }
        match outcome {
            Outcome::Message(m) if m.data == "throw" => {
                Err(DataError::Unavailable("the mapper threw".into()))
            }
            Outcome::Message(m) if m.data == "misshapen" => Ok(Answer::Now(Value::Number(1.))),
            Outcome::Message(m) if m.data == "reopen" => Ok(Answer::stream(Request {
                socket: true,
                ..Request::get("ws://echo.test/again")
            })),
            Outcome::Message(m) => {
                let event = if m.event.is_empty() {
                    "message"
                } else {
                    &m.event
                };
                Ok(Answer::Now(Value::str(&format!("{event} {}", m.data))))
            }
            Outcome::Closed { code, reason, .. } => {
                Ok(Answer::Now(Value::str(&format!("close {code} {reason}"))))
            }
            other => Ok(Answer::Now(Value::str(&other.summary()))),
        }
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
    assert_eq!(out.len(), 2);
    assert!(
        out[0].request.socket,
        "the host is told the socket is two-way"
    );
    (r, out[0].ticket)
}

/// The socket opens; what its mapper sent on `open` is set aside.
fn opened(r: &mut Runner<Echo>, ticket: u64) {
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(open)).unwrap();
    r.take_socket_sends();
}

fn frame(r: &mut Runner<Echo>, ticket: u64, data: &str) {
    let m = Message {
        data: data.into(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(m)).unwrap();
}

/// Each batch taken: who staged it, and its text frames.
fn batches(sends: &[SocketSend]) -> Vec<(SocketOrigin, Vec<String>)> {
    let text = |op: &SocketOp| match op {
        SocketOp::Text(t) => t.clone(),
        other => format!("{other:?}"),
    };
    (sends.iter())
        .map(|s| {
            (
                s.origin.clone(),
                s.ops.iter().map(|(_, _, op)| text(op)).collect(),
            )
        })
        .collect()
}

fn mapper(frames: &[&str]) -> (SocketOrigin, Vec<String>) {
    let frames = frames.iter().map(|f| f.to_string()).collect();
    (SocketOrigin::Mapper("feed".into()), frames)
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
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    assert!(journal(&r).contains("send said failed: InvalidStateError: the socket is not open"));
    r.fulfill(ticket, Outcome::Message(open)).unwrap();
    assert_eq!(r.resource("feed"), Some(&Value::str("open ")));
    assert_eq!(
        batches(&r.take_socket_sends()),
        [mapper(&["subscribe"])],
        "the mapper subscribes on open"
    );
    said(&mut r, "hi");
    said(&mut r, "again");
    let sends = r.take_socket_sends();
    let ops: Vec<_> = sends
        .iter()
        .map(|s| (s.origin.clone(), s.ops.clone()))
        .collect();
    let text = |t: &str| vec![(ticket, "feed".to_string(), SocketOp::Text(t.into()))];
    let said = || SocketOrigin::Mutation("said".into());
    assert_eq!(ops, [(said(), text("hi")), (said(), text("again"))]);
    // The host sent them: the counts and journal lines follow.
    for batch in &sends {
        r.socket_published(batch, Ok(vec![(ticket, 1, 3)]));
    }
    let state = exact_runner::agent::state(&r);
    assert!(
        state.contains(r#""readyState":1,"bufferedAmount":3,"sent":2"#),
        "{state}"
    );
    // The line that admitted the socket: `ws:`, since `wss://echo.test` is
    // another scheme (LLP 1016.000 D7, scheme-wide grants).
    assert!(state.contains(r#""grant":"net.websocket ws:""#), "{state}");
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

/// The mapper's replies to each frame go out once that frame's commit
/// stands, one batch per frame in arrival order, ahead of a later
/// mutation's frames; frames that arrive before the host takes any each
/// keep their own.
#[test]
fn a_mapper_s_replies_go_out_after_each_frame_s_commit_in_arrival_order() {
    let (mut r, ticket) = boot();
    opened(&mut r, ticket);
    for n in 1..=3 {
        frame(&mut r, ticket, &format!("frame {n}"));
    }
    said(&mut r, "later");
    let sends = r.take_socket_sends();
    let said = (
        SocketOrigin::Mutation("said".into()),
        vec!["later".to_string()],
    );
    assert_eq!(
        batches(&sends),
        [
            mapper(&["Ack 1"]),
            mapper(&["Ack 2"]),
            mapper(&["Ack 3"]),
            said
        ]
    );
    assert!(sends
        .iter()
        .all(|s| s.ops.iter().all(|(t, ..)| *t == ticket)));
    assert_eq!(r.resource("feed"), Some(&Value::str("message frame 3")));
    // The host refused a batch at publication: the journal names the mapper.
    r.socket_published(
        &sends[0],
        Err("QuotaExceededError: socket buffer exceeds 1 MiB".into()),
    );
    assert!(journal(&r).contains(
        "mapper feed failed: QuotaExceededError: socket buffer exceeds 1 MiB; socket frames were refused"
    ));
}

/// A frame whose mapper fails, whose snapshot is outside its shape, whose
/// commit is refused or whose answer reopens the stream sends nothing it
/// staged; the frames before it keep theirs, and the journal says the
/// frames were refused.
#[test]
fn a_frame_that_fails_or_is_refused_sends_nothing_it_staged() {
    for (data, why) in [
        ("throw", "the mapper threw"),
        ("misshapen", "shape"),
        ("refuse", "deliberate commit refusal"),
        ("other", "a mapper sends only on its own socket"),
        ("reopen", "its answer reopened the stream"),
    ] {
        let (mut r, ticket) = boot();
        opened(&mut r, ticket);
        frame(&mut r, ticket, "frame 1");
        frame(&mut r, ticket, data);
        assert_eq!(
            batches(&r.take_socket_sends()),
            [mapper(&["Ack 1"])],
            "{data}"
        );
        let journal = journal(&r);
        assert!(journal.contains(why), "{data}: {journal}");
        assert!(
            journal.contains("mapper feed failed: ")
                && journal.contains("; socket frames were refused"),
            "{data}: {journal}"
        );
    }
}

/// The end of the stream is not a frame the mapper answers: what it stages
/// then is never sent.
#[test]
fn a_mapper_sends_nothing_at_the_end() {
    let (mut r, ticket) = boot();
    opened(&mut r, ticket);
    let closed = Outcome::Closed {
        code: 4001,
        reason: "finished".into(),
        was_clean: true,
    };
    r.fulfill(ticket, closed).unwrap();
    assert_eq!(r.resource("feed"), Some(&Value::str("close 4001 finished")));
    assert!(r.take_socket_sends().is_empty());
}
