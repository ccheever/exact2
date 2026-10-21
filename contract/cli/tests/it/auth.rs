//! `openAuthSession` (LLP 1069.006) through the runner, as every host's arm
//! asks it: the grants refuse (403), Linux has no browser (501), the agent
//! holds the request by its own ticket and answers it with a checked
//! callback URL or `cancel`, a second target's session is 409, and a
//! superseded session is forgotten for the host to cancel.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::auth::{self, Arm, Browser};
use exact_runner::{agent, Answer, DataError, DataSource, Event, Outcome, Runner, Store};

const GRANTS: &str = "auth.session https://as.test\nauth.callback app.test:/cb";

struct Signer;

impl DataSource for Signer {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let callback = args
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or("app.test:/cb");
        let url = if source == "other" {
            "https://as.test/b"
        } else {
            "https://as.test/a?client_id=c&request_uri=r&login_hint=h"
        };
        Ok(Answer::Later(exact_data::auth::open_auth_session(
            url, callback, "s1", false,
        )))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let text = match exact_data::auth::reply(&outcome) {
            Ok(url) => format!("200 {}", url.len()),
            Err((status, message)) => format!("{status} {message}"),
        };
        Ok(Answer::Now(Value::record(vec![Value::str(&text)])))
    }
    fn grants(&self) -> &str {
        GRANTS
    }
}

const APP: &str = "shape R\n  text: string\ncomponent App\n  state cb = \"app.test:/cb\"\n  mutation signed as shape R\n  mutation other as shape R\n  action go\n    send signed = signIn(cb)\n  action bad\n    send signed = signIn(\"app.test:/elsewhere\")\n  action two\n    send other = other(cb)\n  view\n    column\n      button press=go testId=\"go\"\n        text \"go\"\n      button press=bad testId=\"bad\"\n        text \"bad\"\n      button press=two testId=\"two\"\n        text \"two\"\n      match signed\n        case some(s)\n          text s.text testId=\"out\"\n        case none\n          text \"\" testId=\"out\"\n";

fn boot() -> Runner<Signer> {
    Runner::boot(
        contract::compile(APP).unwrap(),
        Signer,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn press(r: &mut Runner<Signer>, id: &str) -> exact_runner::RequestOut {
    let key = r.kernel().find_by_test_id(id)[0];
    let node = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(node, Event::Press).unwrap();
    r.take_requests()
        .into_iter()
        .find(|o| o.request.is_auth())
        .expect("an auth request")
}

fn deliver(r: &mut Runner<Signer>) -> String {
    let (ticket, outcome) = r.take_request_refusal(true).expect("a settled answer");
    r.fulfill(ticket, outcome).unwrap();
    let key = r.kernel().find_by_test_id("out")[0];
    r.kernel()
        .node_by_key(key)
        .unwrap()
        .props
        .str(exact_kernel::PropId::Text)
        .unwrap_or("")
        .to_string()
}

#[test]
fn grants_and_hosts_refuse_before_anything_opens() {
    let mut r = boot();
    let out = press(&mut r, "bad");
    assert_eq!(
        auth::arm(&mut r, &out, false, Browser::Native, None),
        Arm::Settled
    );
    assert_eq!(
        deliver(&mut r),
        "403 outside the app's grants (auth.callback app.test:/elsewhere)"
    );
    let out = press(&mut r, "go");
    assert_eq!(
        auth::arm(&mut r, &out, true, Browser::None, None),
        Arm::Settled
    );
    assert_eq!(deliver(&mut r), "501 no system browser on this host");
}

#[test]
fn the_agent_answers_a_hold_by_its_ticket_checked_as_a_host_checks() {
    let mut r = boot();
    let out = press(&mut r, "go");
    assert_eq!(
        auth::arm(&mut r, &out, true, Browser::Native, None),
        Arm::Held
    );
    assert!(!r.has_pending(), "a hold is not I/O: no clock waits on it");
    let state = agent::handle(&r, r#"{"op":"state"}"#);
    assert!(state.contains(r#""capability":"auth","args":{"origin":"https://as.test","path":"/a","params":["client_id","request_uri","login_hint"],"client_id":"c","request_uri":"r","callback":"app.test:/cb","state":"s1""#), "{state}");
    assert!(!state.contains("\"h\""), "no other parameter's value");
    let t = out.ticket;
    let wrong =
        format!(r#"{{"op":"type","ticket":{t},"text":"app.test:/cb?code=secret&state=s2"}}"#);
    let (reply, _) = agent::answer(&mut r, &wrong).unwrap();
    assert!(
        reply.contains("state is not the request's") && !reply.contains("secret"),
        "{reply}"
    );
    let right = format!(
        r#"{{"op":"type","ticket":{t},"text":"app.test:/cb?code=secret&state=s1&iss=https%3A%2F%2Fas.test"}}"#
    );
    let (reply, _) = agent::answer(&mut r, &right).unwrap();
    assert!(
        reply.contains(r#""delivery":"substituted""#) && !reply.contains("secret"),
        "{reply}"
    );
    assert_eq!(deliver(&mut r), "200 59");
    assert!(r.journal().all(|l| !l.contains("secret")));
    // Cancel is 499.
    let out = press(&mut r, "go");
    auth::arm(&mut r, &out, true, Browser::Native, None);
    agent::answer(
        &mut r,
        &format!(
            r#"{{"op":"tap","ticket":{},"choice":"cancel"}}"#,
            out.ticket
        ),
    )
    .unwrap();
    assert_eq!(deliver(&mut r), "499 cancelled");
}

#[test]
fn one_session_per_window_and_supersession_forgets_the_old_one() {
    let mut r = boot();
    let first = press(&mut r, "go");
    assert!(matches!(
        auth::arm(&mut r, &first, false, Browser::Native, None),
        Arm::Present(_)
    ));
    // Another target's session while this one is open: 409.
    let other = press(&mut r, "two");
    assert_eq!(
        auth::arm(&mut r, &other, false, Browser::Native, None),
        Arm::Settled
    );
    let (ticket, outcome) = r.take_request_refusal(true).unwrap();
    assert_eq!(
        (ticket, exact_data::auth::reply(&outcome).unwrap_err().0),
        (other.ticket, 409)
    );
    // A second press of the same target supersedes: the host cancels the first.
    let second = press(&mut r, "go");
    assert_eq!(auth::forgotten(&mut r), vec![first.ticket]);
    assert!(matches!(
        auth::arm(&mut r, &second, false, Browser::Native, None),
        Arm::Present(_)
    ));
    // A late completion of the first is dropped; the second's is checked.
    auth::complete(&mut r, first.ticket, Ok("app.test:/cb?state=s1"));
    auth::complete(&mut r, second.ticket, Ok("app.test:/cb?code=c&state=s1"));
    assert_eq!(deliver(&mut r), "200 28");
}
