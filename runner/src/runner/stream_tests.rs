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
}

impl DataSource for Scripted {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        Ok(match (source, args.first()) {
            ("busy", Some(v)) => Answer::Now(v.clone()),
            (_, Some(Value::Number(n))) if *n == 0. => Answer::Now(Value::Number(-1.)),
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
    Runner::boot(
        plan(),
        Scripted::default(),
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
                coalesced: 0
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
            coalesced: 7
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
