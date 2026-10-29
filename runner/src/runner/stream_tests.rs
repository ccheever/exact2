//! Answers that keep coming, the runner half (LLP 1016.000 D1, D2, D4, D5;
//! LLP 1069.004 slice 1): a scripted stream, no network.
use super::*;
use crate::{Message, Outcome, Response, Store};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, Plan, TypeKind};

/// `progress(wave)` streams for a wave, answers `-1` now for none, and
/// shapes a stream's end as `-2`; `busy(pending(progress))` echoes.
#[derive(Default)]
struct Scripted {
    bad_message: bool,
    reopen_on_gap: bool,
    /// Its stream is a long poll (LLP 1016.000 D8).
    long_poll: bool,
    /// … and asked of native work, which no host may run as one.
    native: bool,
    /// Its stream is an upload that tells its bytes sent (LLP 1016.000 D10).
    upload: bool,
    /// … whose failure it fails to take.
    failing_end: bool,
}

impl DataSource for Scripted {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        Ok(match (source, args.first()) {
            ("busy", Some(v)) => Answer::Now(v.clone()),
            (_, Some(Value::Number(n))) if *n == 0. => Answer::Now(Value::Number(-1.)),
            (_, Some(Value::Number(_))) if self.native => Answer::Later({
                let mut r = crate::Request::native(b"x".to_vec()).long_poll("since", "seq");
                r.stream = true;
                r
            }),
            (_, Some(Value::Number(n))) if self.upload => {
                // It watches a topic, which asks any other answer again.
                store.observe_topic("clock");
                let mut r = crate::Request::get(&format!("https://example.test/up?wave={n}"))
                    .upload_progress();
                r.method = "POST".into();
                Answer::stream(r)
            }
            (_, Some(Value::Number(n))) if self.long_poll => Answer::stream(
                crate::Request::get(&format!("https://example.test/changes?wave={n}"))
                    .long_poll("since", "seq"),
            ),
            (_, Some(Value::Number(n))) => Answer::stream(crate::Request::get(&format!(
                "https://example.test/events?wave={n}"
            ))),
            _ => return Err(DataError::BadArguments("progress(wave)".into())),
        })
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        match outcome {
            Outcome::Message(_) if self.bad_message => {
                Err(DataError::Unavailable("not a number".into()))
            }
            // A gap: reopen from the cursor (LLP 1016.000 D6).
            Outcome::Message(m) if self.reopen_on_gap && m.coalesced > 0 => Ok(Answer::stream(
                crate::Request::get("https://example.test/events").header("last-event-id", "1"),
            )),
            Outcome::Failed { .. } if self.failing_end => {
                Err(DataError::Unavailable("the upload failed".into()))
            }
            Outcome::Message(m) if m.sent().is_some() => {
                Ok(Answer::Now(Value::Number(m.sent().unwrap().0 as f64)))
            }
            Outcome::Message(m) => Ok(Answer::Now(Value::Number(
                m.data
                    .parse()
                    .map_err(|_| DataError::Unavailable("nan".into()))?,
            ))),
            _ => Ok(Answer::Now(Value::Number(-2.))),
        }
    }
}

fn plan() -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let boolean = b.primitive(TypeKind::Bool);
    let one = b.constant(&Value::Number(1.));
    let wave = b.slot("wave", number, one);
    let mut arg = Asm::new();
    arg.load_slot(wave);
    let arg = b.code(arg);
    let progress = b.resource("progress", "progress", &[arg], number, None);
    let mut arg = Asm::new();
    arg.pending_resource(progress);
    let arg = b.code(arg);
    b.resource("busy", "busy", &[arg], boolean, None);
    let mut body = Asm::new();
    body.load_param(0).store_slot(wave);
    let body = b.code(body);
    b.action("wave", &[("value", number)], &[wave], body);
    let mut body = Asm::new();
    body.refresh(progress);
    let body = b.code(body);
    b.action("refresh", &[], &[], body);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

fn boot() -> Runner<Scripted> {
    boot_with(Scripted::default())
}

fn boot_with(scripted: Scripted) -> Runner<Scripted> {
    Runner::boot(
        plan(),
        scripted,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn message(data: &str, coalesced: u32) -> Outcome {
    Outcome::Message(Message {
        data: data.into(),
        id: data.into(),
        coalesced,
        ..Message::default()
    })
}

fn value(r: &Runner<Scripted>, name: &str) -> Value {
    r.resource(name).cloned().expect("answered")
}

fn one_stream(r: &mut Runner<Scripted>) -> u64 {
    let out = r.take_requests();
    assert_eq!(out.len(), 1, "one request: {out:?}");
    assert!(out[0].request.stream, "the host is told it streams");
    out[0].ticket
}

#[test]
fn three_messages_then_a_close_commit_four_times() {
    let mut r = boot();
    let ticket = one_stream(&mut r);
    assert_eq!(
        value(&r, "busy"),
        Value::Bool(true),
        "pending until a message"
    );
    assert!(r.has_pending());
    for (i, n) in ["10", "20", "30"].iter().enumerate() {
        let receipt = r.fulfill(ticket, message(n, 0)).unwrap();
        assert!(receipt.is_some(), "message {i} commits");
        assert_eq!(value(&r, "progress"), Value::Number(n.parse().unwrap()));
        assert_eq!(
            value(&r, "busy"),
            Value::Bool(false),
            "pending ends at the first"
        );
        assert!(!r.has_pending(), "settle does not wait on an open stream");
        assert!(r.holds(ticket), "the ticket stays open");
        assert!(r.in_flight().is_empty());
    }
    assert_eq!(
        r.streams(),
        vec![(
            "progress".to_string(),
            ticket,
            StreamCount {
                messages: 3,
                coalesced: 0,
                socket: None,
            }
        )]
    );
    assert!(r.take_requests().is_empty(), "no message asks again");
    let closed = Outcome::Failed {
        kind: crate::FailureKind::Network,
        message: "the event stream ended".into(),
    };
    assert!(r.fulfill(ticket, closed).unwrap().is_some());
    assert_eq!(
        value(&r, "progress"),
        Value::Number(-2.),
        "the end is shaped"
    );
    assert!(!r.holds(ticket) && r.streams().is_empty());
    assert!(
        r.take_requests().is_empty(),
        "the host never reconnects by itself"
    );
}

#[test]
fn forgetting_mid_stream_lets_the_ticket_go_and_drops_late_messages() {
    let mut r = boot();
    let old = one_stream(&mut r);
    r.fulfill(old, message("5", 0)).unwrap();
    r.act("wave", vec![Value::Number(2.)]).unwrap();
    assert!(!r.holds(old), "the host aborts what the runner let go");
    let new = one_stream(&mut r);
    assert!(new != old);
    assert_eq!(
        value(&r, "busy"),
        Value::Bool(true),
        "the new stream is pending"
    );
    assert_eq!(r.fulfill(old, message("99", 0)).unwrap(), None, "dropped");
    assert_eq!(value(&r, "progress"), Value::Number(5.), "no commit after");
    assert!(r
        .journal()
        .any(|l| l.contains(&format!("reply {old} dropped"))));
    // Arguments that answer now let the open stream go too.
    r.fulfill(new, message("7", 0)).unwrap();
    r.act("wave", vec![Value::Number(0.)]).unwrap();
    assert!(!r.holds(new));
    assert_eq!(value(&r, "progress"), Value::Number(-1.));
    assert!(r.take_requests().is_empty() && r.streams().is_empty());
}

#[test]
fn refresh_closes_the_stream_and_opens_another() {
    let mut r = boot();
    let first = one_stream(&mut r);
    r.fulfill(first, message("1", 0)).unwrap();
    r.act("refresh", vec![]).unwrap();
    assert!(!r.holds(first));
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    assert!(out[0].forced && out[0].request.stream);
    assert_eq!(
        value(&r, "progress"),
        Value::Number(1.),
        "keeps its last value"
    );
}

#[test]
fn coalesced_messages_are_counted_and_the_newest_lands() {
    let mut r = boot();
    let ticket = one_stream(&mut r);
    r.fulfill(ticket, message("1", 0)).unwrap();
    r.fulfill(ticket, message("9", 7)).unwrap();
    assert_eq!(value(&r, "progress"), Value::Number(9.));
    assert_eq!(
        r.streams()[0].2,
        StreamCount {
            messages: 2,
            coalesced: 7,
            socket: None,
        }
    );
    let state = crate::agent::state(&r);
    assert!(
        state.contains(
            "\"streams\":[{\"name\":\"progress\",\"ticket\":1,\"messages\":2,\"coalesced\":7}]"
        ),
        "{state}"
    );
    assert!(state.contains("\"pending\":[]"), "{state}");
}

#[test]
fn a_long_poll_waiting_to_ask_again_shows_retrying() {
    let mut r = boot();
    let ticket = one_stream(&mut r);
    r.fulfill(ticket, message("1", 0)).unwrap();
    r.set_retry_reader(std::sync::Arc::new(move |t| {
        (t == ticket).then(|| crate::Retrying {
            attempt: 2,
            in_ms: 1900,
            error: "HTTP \"503\"".into(),
        })
    }));
    let state = crate::agent::state(&r);
    assert!(
        state.contains(
            "\"coalesced\":0,\"retrying\":{\"attempt\":2,\"inMs\":1900,\"error\":\"HTTP \\\"503\\\"\"}}]"
        ),
        "{state}"
    );
}

/// LLP 1016.000 D8 on D9: a long poll's first reply of any kind takes it out
/// of every clock's wait; from then it is held and named,
/// and each reply (no event, an event, its end) raises `heldReplies`.
#[test]
fn a_long_poll_s_replies_hold_it_and_each_counts() {
    let mut r = boot_with(Scripted {
        long_poll: true,
        ..Scripted::default()
    });
    let ticket = one_stream(&mut r);
    assert!(r.has_pending() && r.held_requests().is_empty());
    let reply = Outcome::Message(Message {
        event: crate::LONG_POLL_REPLY.into(),
        ..Message::default()
    });
    let before = r.resource("progress").cloned();
    assert!(
        r.fulfill(ticket, reply.clone()).unwrap().is_none(),
        "no commit"
    );
    assert_eq!(r.resource("progress").cloned(), before, "no mapper call");
    assert_eq!(
        value(&r, "busy"),
        Value::Bool(true),
        "pending(…) until its first event"
    );
    assert!(!r.has_pending(), "but waited on by no clock");
    assert_eq!(r.held_requests(), [("progress".to_string(), ticket)]);
    assert_eq!(r.held_replies(), [("progress".to_string(), 1)]);
    r.fulfill(ticket, message("5", 0)).unwrap();
    assert_eq!(value(&r, "progress"), Value::Number(5.));
    assert_eq!(r.held_replies(), [("progress".to_string(), 2)]);
    let state = crate::agent::state(&r);
    assert!(
        state.contains(&format!(
            "{{\"name\":\"progress\",\"ticket\":{ticket},\"held\":true}}"
        )),
        "{state}"
    );
    let end = Outcome::Failed {
        kind: crate::FailureKind::Network,
        message: "gone".into(),
    };
    r.fulfill(ticket, end).unwrap();
    assert_eq!(r.held_replies(), [("progress".to_string(), 3)]);
    assert!(r.held_requests().is_empty());
    // On a stream that is no long poll, the same event is an ordinary message's.
    let mut r = boot();
    let ticket = one_stream(&mut r);
    assert!(r.fulfill(ticket, reply).is_err() || r.held_replies().is_empty());
}

/// A long poll on native work is no HTTP request's: `long_poll_refusal` says
/// so, for the hosts' dispatch, which hands it to the executor to refuse.
#[test]
fn a_long_poll_of_other_host_work_is_one_every_host_refuses() {
    let mut r = boot_with(Scripted {
        native: true,
        ..Scripted::default()
    });
    let out = r.take_requests();
    assert!(out[0]
        .request
        .long_poll_refusal()
        .is_some_and(|w| w.contains("not other host work")));
}

#[test]
fn a_gap_reopens_the_stream_from_its_cursor() {
    let mut r = boot();
    r.data().reopen_on_gap = true;
    let first = one_stream(&mut r);
    r.fulfill(first, message("4", 0)).unwrap();
    assert!(r.fulfill(first, message("8", 3)).unwrap().is_some());
    assert!(!r.holds(first), "the gapped stream is let go");
    let again = r.take_requests();
    assert_eq!(again.len(), 1);
    assert!(again[0].request.stream);
    assert!(again[0]
        .request
        .headers
        .contains(&("last-event-id".into(), "1".into())));
    assert_eq!(
        value(&r, "progress"),
        Value::Number(4.),
        "keeps its last value"
    );
    // The reopened stream is in flight until its first message.
    assert!(r.has_pending());
    r.fulfill(again[0].ticket, message("9", 0)).unwrap();
    assert_eq!(value(&r, "progress"), Value::Number(9.));
}

#[test]
fn a_message_the_source_cannot_take_ends_the_stream() {
    let mut r = boot();
    let ticket = one_stream(&mut r);
    r.fulfill(ticket, message("3", 0)).unwrap();
    r.data().bad_message = true;
    assert!(r.fulfill(ticket, message("4", 0)).unwrap().is_some());
    assert!(!r.holds(ticket), "let go, and the host aborts it");
    assert_eq!(
        value(&r, "progress"),
        Value::Number(3.),
        "keeps its last value"
    );
    assert!(r.take_requests().is_empty());
}

#[test]
fn a_response_that_is_not_an_event_stream_ends_it() {
    // A stream's ticket stays open only for messages: the host delivers a
    // response it cannot read as events (a 404) whole, and it ends.
    let mut r = boot();
    let ticket = one_stream(&mut r);
    r.fulfill(
        ticket,
        Outcome::Response(Response {
            status: 404,
            headers: vec![],
            body: b"no such wave".to_vec(),
        }),
    )
    .unwrap();
    assert!(!r.holds(ticket), "a response that is not a stream ends it");
    assert_eq!(value(&r, "progress"), Value::Number(-2.));
}

/// An upload (LLP 1016.000 D10) is in flight until its reply, for
/// `pending(…)`, `state.pending` and every clock, while each count of its
/// bytes sent commits; the reply ends it.
#[test]
fn an_upload_is_pending_until_its_reply_while_its_progress_commits() {
    let mut r = boot_with(Scripted {
        upload: true,
        ..Scripted::default()
    });
    let out = r.take_requests();
    let ticket = out[0].ticket;
    assert!(out[0].request.stream && out[0].request.upload_progress);
    for loaded in [1024, 4096] {
        let sent = Outcome::Message(Message::upload_progress(loaded, 4096));
        assert!(r.fulfill(ticket, sent).unwrap().is_some(), "each commits");
        assert_eq!(value(&r, "progress"), Value::Number(loaded as f64));
        assert_eq!(
            value(&r, "busy"),
            Value::Bool(true),
            "pending until the reply"
        );
        assert!(r.has_pending(), "every clock waits for it");
        assert_eq!(r.in_flight(), [("progress".to_string(), ticket)]);
        assert!(r.holds(ticket));
    }
    let reply = Outcome::Response(Response {
        status: 201,
        headers: Vec::new(),
        body: b"{}".to_vec(),
    });
    assert!(r.fulfill(ticket, reply).unwrap().is_some());
    assert_eq!(
        value(&r, "progress"),
        Value::Number(-2.),
        "the reply is shaped"
    );
    assert_eq!(value(&r, "busy"), Value::Bool(false));
    assert!(!r.has_pending() && r.in_flight().is_empty() && !r.holds(ticket));
}

/// An upload is asked once for its arguments (LLP 1016.000 D10): neither a
/// topic it watches nor a refresh asks it again, in flight or after its
/// reply; new arguments do, letting the running one go.
#[test]
fn an_upload_is_asked_once_for_its_arguments() {
    let mut r = boot_with(Scripted {
        upload: true,
        ..Scripted::default()
    });
    let ticket = r.take_requests()[0].ticket;
    let sent = Outcome::Message(Message::upload_progress(1024, 4096));
    r.fulfill(ticket, sent).unwrap();
    r.changed("clock").unwrap();
    r.act("refresh", vec![]).unwrap();
    assert!(
        r.take_requests().is_empty(),
        "nothing asks it again in flight"
    );
    assert!(r.holds(ticket));
    let reply = Outcome::Response(Response {
        status: 201,
        headers: Vec::new(),
        body: b"{}".to_vec(),
    });
    r.fulfill(ticket, reply).unwrap();
    r.changed("clock").unwrap();
    r.act("refresh", vec![]).unwrap();
    assert!(r.take_requests().is_empty(), "nor after its reply");
    assert_eq!(
        value(&r, "progress"),
        Value::Number(-2.),
        "its answer stands"
    );
    // New arguments ask again; a running upload they replace is let go.
    r.act("wave", vec![Value::Number(2.)]).unwrap();
    let second = r.take_requests();
    assert_eq!(second.len(), 1);
    r.act("wave", vec![Value::Number(3.)]).unwrap();
    assert!(
        !r.holds(second[0].ticket),
        "the host aborts what the runner let go"
    );
    assert_eq!(r.take_requests().len(), 1);
}

/// … and so is one whose end its source failed to take: the failure stands
/// through a topic it watches, a refresh, and another reply's store write
/// (LLP 1146 §2.3); new arguments ask again.
#[test]
fn an_upload_that_failed_is_not_asked_again_at_its_arguments() {
    let mut r = boot_with(Scripted {
        upload: true,
        failing_end: true,
        ..Scripted::default()
    });
    let ticket = r.take_requests()[0].ticket;
    let gone = Outcome::Failed {
        kind: crate::FailureKind::Network,
        message: "gone".into(),
    };
    r.fulfill(ticket, gone).unwrap();
    assert!(!r.holds(ticket));
    assert!(r.upload_failed(0), "its failure is an upload's");
    r.changed("clock").unwrap();
    r.act("refresh", vec![]).unwrap();
    // As if its answer had read the store, and another reply's turn wrote it.
    r.store_readers[0] = true;
    r.ask_failed_readers(None);
    r.act("wave", vec![Value::Number(1.)]).unwrap();
    assert!(r.take_requests().is_empty(), "nothing asks it again");
    assert!(r.upload_failed(0), "the failure stands");
    r.act("wave", vec![Value::Number(2.)]).unwrap();
    assert_eq!(r.take_requests().len(), 1, "new arguments ask again");
    assert!(!r.upload_failed(0));
}
