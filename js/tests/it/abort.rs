//! A fetch the app aborts is cancelled, as a browser's is: the answer that
//! awaited it resumes at the host's next pump with the abort, its reply never
//! read, and the host is handed its ticket to cancel whatever its method. A
//! fetch aborted before its answer awaited it is withdrawn unsent. A host
//! whose door predates the abort leaves the request running, as before.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::{Kernel, PropId};
use exact_runner::{Event, FailureKind, Outcome, Response, Runner};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/castle.hbc"));
const APP: &str = "xyz.castle.test";
const GRANTS: &str = "net.fetch https://api.castle.xyz\nsecret.keep castle.session\n";

const SRC: &str = r#"
shape Session
  ok: bool
  username: string
  error: string

component App
  state id = ""
  mutation session as shape Session
  mutation stop as shape Session
  mutation other as shape Session

  action get(v)
    id = v
    send session = holding(v, "GET")
  action post
    id = "p"
    send session = holding("p", "POST")
  action pair
    send session = withdrawn()
  action fall
    id = "f"
    send session = fallingBack("f")
  action both
    id = "b"
    send session = holding("b", "POST")
    send stop = abortHeld("b")
  action replace
    send stop = abortHeld(id)
    send session = logout()
  action halt
    send stop = abortHeld(id)
  action bare
    send stop = abortBare(id)
  action ping(v)
    send other = holding(v, "GET")

  view
    column
      input value=id change=get testId="get"
      input value="" change=ping testId="ping"
      button press=post aria-label="Post" testId="post"
        text "Post"
      button press=pair aria-label="Pair" testId="pair"
        text "Pair"
      button press=fall aria-label="Fall back" testId="fall"
        text "Fall back"
      button press=both aria-label="Both" testId="both"
        text "Both"
      button press=replace aria-label="Replace" testId="replace"
        text "Replace"
      button press=halt aria-label="Abort" testId="halt"
        text "Abort"
      button press=bare aria-label="Abort bare" testId="bare"
        text "Abort bare"
      match session
        case some(s)
          text `${s.username}: ${s.error}` testId="session"
        case none
          text "none" testId="none"
"#;

fn boot(older_door: bool) -> Runner<Module> {
    let mut module = Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap();
    module.set_budget_ms(f64::INFINITY);
    if older_door {
        module.refuse_door_op(23);
    }
    Runner::boot(
        contract::compile(SRC).expect("the fixture's Contract compiles"),
        module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view_of(r: &Runner<Module>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn text_of(r: &Runner<Module>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    k.node_by_key(key)?
        .props
        .str(PropId::Text)
        .map(str::to_string)
}

fn ok(body: &str) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: body.as_bytes().to_vec(),
    })
}

/// Press `halt`: another answer's turn aborts the held fetch.
fn abort_from_another_answer(r: &mut Runner<Module>) {
    r.dispatch(view_of(r, "halt"), Event::Press).unwrap();
    // `stop` answers at once; it sends nothing.
    let stop: Vec<_> = r.take_requests();
    assert!(stop.is_empty(), "abortHeld fetches nothing");
}

/// The held request is answered aborted at the next pump, resumes its
/// answer with the rejection, and its ticket is handed to the host to cancel.
fn aborted_and_resumed(r: &mut Runner<Module>, ticket: u64, id: &str) {
    assert!(r.has_module_aborts(), "the abort waits for the host's pump");
    assert!(r.has_request_refusals(false), "a pump is owed");
    let (answered, outcome) = r.take_request_refusal(false).expect("the aborted answer");
    assert_eq!(answered, ticket);
    assert!(matches!(
        outcome,
        Outcome::Failed {
            kind: FailureKind::Aborted,
            ..
        }
    ));
    assert!(
        r.fulfill(ticket, outcome).unwrap().is_some(),
        "the answer resumes in a commit"
    );
    assert_eq!(
        text_of(r, "session").as_deref(),
        Some(format!("{id}: aborted: by the app").as_str())
    );
    assert_eq!(r.take_explicit_aborts(), [ticket], "the host cancels it");
    assert!(!r.has_module_aborts());
    assert_eq!(r.data().in_flight(), 0, "no answer waits on the wire");
    // The request's own reply, if the cancel lost the race, is never read.
    assert!(r.fulfill(ticket, ok("late")).unwrap().is_none());
    assert_eq!(
        text_of(r, "session").as_deref(),
        Some(format!("{id}: aborted: by the app").as_str())
    );
}

#[test]
fn an_aborted_get_resumes_its_answer_at_the_next_pump_and_is_cancelled() {
    let mut r = boot(false);
    r.dispatch(view_of(&r, "get"), Event::Change("a".into()))
        .unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request.method, "GET");
    assert_eq!(
        r.data().in_flight(),
        1,
        "its answer is parked on the request"
    );
    abort_from_another_answer(&mut r);
    aborted_and_resumed(&mut r, requests[0].ticket, "a");
}

#[test]
fn an_aborted_post_is_cancelled_too_whatever_its_method() {
    let mut r = boot(false);
    r.dispatch(view_of(&r, "post"), Event::Press).unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request.method, "POST");
    abort_from_another_answer(&mut r);
    aborted_and_resumed(&mut r, requests[0].ticket, "p");
}

#[test]
fn a_fetch_aborted_before_its_answer_awaits_it_is_withdrawn_unsent() {
    let mut r = boot(false);
    r.dispatch(view_of(&r, "pair"), Event::Press).unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1, "only the awaited fetch goes out");
    assert!(requests[0].request.url.ends_with("/first"));
    assert_eq!(r.data().unsent(), 0, "the aborted one is not left recorded");
    assert!(!r.has_module_aborts(), "nothing was handed out to cancel");
    r.fulfill(requests[0].ticket, ok("first")).unwrap();
    assert_eq!(text_of(&r, "session").as_deref(), Some("first: AbortError"));
    assert_eq!(r.data().unsent(), 0);
}

#[test]
fn on_a_door_without_the_abort_the_request_runs_and_its_answer_waits_for_the_reply() {
    let mut r = boot(true);
    r.dispatch(view_of(&r, "get"), Event::Change("old".into()))
        .unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    // The abort is harmless: the answer's turn is not refused, and nothing
    // is answered early or handed to the host to cancel.
    abort_from_another_answer(&mut r);
    assert!(!r.has_module_aborts());
    assert!(r.take_request_refusal(true).is_none());
    assert!(r.take_explicit_aborts().is_empty());
    assert_eq!(
        r.data().in_flight(),
        1,
        "the answer waits for the reply, as before"
    );
    // The reply resumes it; the rejection was already delivered.
    assert!(r.fulfill(requests[0].ticket, ok("late")).unwrap().is_some());
    assert_eq!(
        text_of(&r, "session").as_deref(),
        Some("old: aborted: by the app")
    );
    assert_eq!(r.data().in_flight(), 0);
}

#[test]
fn an_abort_answered_after_its_request_replied_leaves_the_answers_next_fetch_alone() {
    let mut r = boot(false);
    r.dispatch(view_of(&r, "fall"), Event::Press).unwrap();
    let held = r.take_requests();
    assert_eq!(held.len(), 1);
    abort_from_another_answer(&mut r);
    assert!(r.has_module_aborts(), "the abort waits for the host's pump");
    // The pump drains the real reply first: the answer resumes with the
    // rejection and falls back to a second fetch of the same target.
    assert!(r.fulfill(held[0].ticket, ok("late")).unwrap().is_some());
    let next = r.take_requests();
    assert_eq!(next.len(), 1);
    assert!(next[0].request.url.ends_with("/fallback/f"));
    // The abort named the first request, which has landed: no answer is
    // owed, only that request is let go, and the second goes on.
    assert!(r.take_request_refusal(true).is_none());
    assert_eq!(r.take_explicit_aborts(), [held[0].ticket]);
    assert!(!r.has_module_aborts());
    assert!(r.fulfill(next[0].ticket, ok("ok")).unwrap().is_some());
    assert_eq!(text_of(&r, "session").as_deref(), Some("f: fell back: ok"));
}

#[test]
fn a_fetch_aborted_in_the_action_that_asked_it_is_never_sent() {
    let mut r = boot(false);
    // One action asks a POST and, in its next send, aborts it, before the
    // runner has handed the request out.
    r.dispatch(view_of(&r, "both"), Event::Press).unwrap();
    assert!(
        r.take_requests().is_empty(),
        "the aborted POST is never handed out"
    );
    assert!(r.has_module_aborts());
    let (ticket, outcome) = r.take_request_refusal(false).expect("the aborted answer");
    assert!(matches!(
        outcome,
        Outcome::Failed {
            kind: FailureKind::Aborted,
            ..
        }
    ));
    assert!(r.fulfill(ticket, outcome).unwrap().is_some());
    assert_eq!(
        text_of(&r, "session").as_deref(),
        Some("b: aborted: by the app")
    );
    assert_eq!(
        r.take_explicit_aborts(),
        [ticket],
        "the host lets go of what it never got"
    );
}

#[test]
fn an_abort_whose_request_was_replaced_still_cancels_it() {
    let mut r = boot(false);
    r.dispatch(view_of(&r, "post"), Event::Press).unwrap();
    let held = r.take_requests();
    assert_eq!(held.len(), 1);
    // One action aborts the POST and then asks `session` again, answered at
    // once: the POST is no longer the runner's, but it is still cancelled.
    r.dispatch(view_of(&r, "replace"), Event::Press).unwrap();
    assert!(r.has_module_aborts());
    assert!(r.take_request_refusal(false).is_none(), "no answer is owed");
    assert_eq!(r.take_explicit_aborts(), [held[0].ticket]);
    assert!(r.fulfill(held[0].ticket, ok("late")).unwrap().is_none());
}

#[test]
fn a_source_that_aborts_its_last_fetch_and_fetches_again_cancels_the_last() {
    let mut r = boot(false);
    r.dispatch(view_of(&r, "post"), Event::Press).unwrap();
    let first = r.take_requests();
    assert_eq!(first.len(), 1);
    // The same call again, on the same key: in its own turn the source aborts
    // its last fetch and fetches again, replacing the last's parked answer
    // while its POST still runs.
    r.dispatch(view_of(&r, "post"), Event::Press).unwrap();
    let second = r.take_requests();
    assert_eq!(second.len(), 1);
    assert_ne!(second[0].ticket, first[0].ticket);
    // The last is still named by its abort and cancelled, no answer owed;
    // the new one goes on.
    r.collect_aborts();
    assert!(r.take_request_refusal(false).is_none(), "no answer is owed");
    assert_eq!(r.take_explicit_aborts(), [first[0].ticket]);
    assert!(r.fulfill(second[0].ticket, ok("second")).unwrap().is_some());
    assert_eq!(text_of(&r, "session").as_deref(), Some("p: fetched"));
}

#[test]
fn a_held_fetch_is_still_named_by_its_abort_after_many_other_fetches_were_let_go() {
    let mut r = boot(false);
    r.dispatch(view_of(&r, "post"), Event::Press).unwrap();
    let held = r.take_requests();
    assert_eq!(held.len(), 1);
    // Many other fetches park while the POST is held, each let go unanswered
    // when the next send replaces it.
    for i in 0..300 {
        r.dispatch(view_of(&r, "ping"), Event::Change(format!("o{i}").into()))
            .unwrap();
        assert_eq!(r.take_requests().len(), 1);
    }
    abort_from_another_answer(&mut r);
    assert!(r.has_module_aborts(), "the abort still names the POST");
    let (answered, outcome) = r.take_request_refusal(false).expect("the aborted answer");
    assert_eq!(answered, held[0].ticket);
    assert!(r.fulfill(answered, outcome).unwrap().is_some());
    assert_eq!(
        text_of(&r, "session").as_deref(),
        Some("p: aborted: by the app")
    );
    assert_eq!(r.take_explicit_aborts(), [held[0].ticket]);
}

/// A bare `abort()` rejects with its signal's reason, an `AbortError`, as
/// Fetch does (and as the JS target does, host/web-js/abort-reason.test.mjs);
/// a reason the app gave is the rejection itself (`holding`'s check).
#[test]
fn a_bare_abort_rejects_with_an_abort_error_that_is_its_signal_s_reason() {
    let mut r = boot(false);
    r.dispatch(view_of(&r, "get"), Event::Change("a".into()))
        .unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    r.dispatch(view_of(&r, "bare"), Event::Press).unwrap();
    assert!(r.take_requests().is_empty());
    let (answered, outcome) = r.take_request_refusal(false).expect("the aborted answer");
    assert_eq!(answered, requests[0].ticket);
    r.fulfill(answered, outcome).unwrap();
    assert_eq!(
        text_of(&r, "session").as_deref(),
        Some("a: aborted: AbortError")
    );
}
