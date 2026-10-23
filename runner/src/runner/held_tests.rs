//! Requests the server holds (LLP 1016.000 D9), the runner half: pending as
//! any request, but no clock waits on one; the work its reply starts does.
use super::*;
use crate::{Outcome, Response, Store};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, Plan, TypeKind};

/// `changes(wave)`: wave 0 answers now; 1 asks held; 2 asks the same, not
/// held (a long hold no one marked); a held reply whose body is `more` asks
/// again, not held, as an answer's work after its held fetch would.
/// `busy(pending(changes))` echoes.
struct Polls;

fn ask(wave: f64) -> Request {
    let request = Request::get(&format!("https://example.test/changes?wave={wave}"));
    if wave == 1. {
        request.held()
    } else {
        request
    }
}

impl DataSource for Polls {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        Ok(match (source, args.first()) {
            ("busy", Some(v)) => Answer::Now(v.clone()),
            (_, Some(Value::Number(n))) if *n == 0. => Answer::Now(Value::Number(-1.)),
            (_, Some(Value::Number(n))) => Answer::Later(ask(*n)),
            _ => return Err(DataError::BadArguments("changes(wave)".into())),
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
            Outcome::Response(r) if r.body == b"more" => Ok(Answer::Later(Request::get(
                "https://example.test/changes/detail",
            ))),
            Outcome::Response(r) => Ok(Answer::Now(Value::Number(r.body.len() as f64))),
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
    let changes = b.resource("changes", "changes", &[arg], number, None);
    let mut arg = Asm::new();
    arg.pending_resource(changes);
    let arg = b.code(arg);
    b.resource("busy", "busy", &[arg], boolean, None);
    let mut body = Asm::new();
    body.load_param(0).store_slot(wave);
    let body = b.code(body);
    b.action("wave", &[("value", number)], &[wave], body);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

fn boot() -> Runner<Polls> {
    Runner::boot(
        plan(),
        Polls,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn reply(body: &str) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: body.as_bytes().to_vec(),
    })
}

fn one_request(r: &mut Runner<Polls>) -> (u64, Request) {
    let mut out = r.take_requests();
    assert_eq!(out.len(), 1, "one request: {out:?}");
    let out = out.remove(0);
    (out.ticket, out.request)
}

#[test]
fn a_held_request_is_pending_but_no_clock_waits_on_it() {
    let mut r = boot();
    let (ticket, request) = one_request(&mut r);
    assert!(
        request.held && request.is_held(),
        "the host is told it is held"
    );
    assert_eq!(
        r.resource("busy"),
        Some(&Value::Bool(true)),
        "pending(changes) holds"
    );
    assert!(!r.has_pending(), "no clock waits on it");
    assert!(r.in_flight().is_empty(), "nor is it listed as waited on");
    assert_eq!(r.held_requests(), vec![("changes".to_string(), ticket)]);
    // It still lands, and commits.
    assert!(r.held_replies().is_empty());
    assert!(r.fulfill(ticket, reply("abc")).unwrap().is_some());
    assert_eq!(r.held_replies(), [("changes".to_string(), 1)]);
    assert_eq!(r.resource("changes"), Some(&Value::Number(3.)));
    assert_eq!(r.resource("busy"), Some(&Value::Bool(false)));
    assert!(r.held_requests().is_empty());
}

#[test]
fn the_work_a_held_reply_starts_is_waited_on() {
    let mut r = boot();
    let (ticket, _) = one_request(&mut r);
    r.fulfill(ticket, reply("more")).unwrap();
    let (next, request) = one_request(&mut r);
    assert!(!request.held, "the answer's next ask is its own");
    assert!(r.has_pending(), "a clock waits on it again");
    assert_eq!(r.in_flight(), vec![("changes".to_string(), next)]);
    assert!(r.held_requests().is_empty());
    r.fulfill(next, reply("done")).unwrap();
    assert!(!r.has_pending());
}

#[test]
fn an_unmarked_long_hold_is_still_waited_on() {
    let mut r = boot();
    let (first, _) = one_request(&mut r);
    r.fulfill(first, reply("x")).unwrap();
    r.act("wave", vec![Value::Number(2.)]).unwrap();
    let (ticket, request) = one_request(&mut r);
    assert!(!request.held);
    assert!(
        r.has_pending(),
        "calendar F10: what is in flight is waited on"
    );
    assert_eq!(r.in_flight(), vec![("changes".to_string(), ticket)]);
    assert!(r.held_requests().is_empty());
}

#[test]
fn a_send_to_its_target_supersedes_a_held_request() {
    let mut r = boot();
    let (held, _) = one_request(&mut r);
    r.act("wave", vec![Value::Number(2.)]).unwrap();
    let (next, _) = one_request(&mut r);
    assert!(!r.holds(held), "the held ask is let go");
    assert!(r.held_requests().is_empty());
    assert_eq!(r.in_flight(), vec![("changes".to_string(), next)]);
    assert_eq!(
        r.fulfill(held, reply("late")).unwrap(),
        None,
        "its late reply is dropped"
    );
    assert!(r.held_replies().is_empty(), "a reply let go is no reply");
}

#[test]
fn an_upload_is_never_held() {
    let mut upload = Request::get("https://example.test/up")
        .held()
        .body_from("app:/data/a");
    upload.method = "POST".into();
    assert!(upload.held && !upload.is_held());
}

#[test]
fn state_pending_lists_a_held_request_as_held() {
    let r = boot();
    let state = crate::agent::state(&r);
    assert!(
        state.contains("\"pending\":[{\"name\":\"changes\",\"ticket\":1,\"held\":true}"),
        "{state}"
    );
}

#[test]
fn the_held_op_answers_held_requests_and_their_replies_cheaply() {
    let mut r = boot();
    let (ticket, _) = one_request(&mut r);
    assert_eq!(
        crate::agent::handle(&r, r#"{"op":"held"}"#),
        format!("{{\"held\":[{{\"name\":\"changes\",\"ticket\":{ticket}}}],\"heldReplies\":{{}}}}")
    );
    r.fulfill(ticket, reply("x")).unwrap();
    assert_eq!(
        crate::agent::handle(&r, r#"{"op":"held"}"#),
        "{\"held\":[],\"heldReplies\":{\"changes\":1}}"
    );
}
