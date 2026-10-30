//! `openAuthSession` (LLP 1069.006) through the runner, as every host's arm
//! asks it: the grants refuse (403), Linux has no browser (501), the agent
//! holds the request by its own ticket and answers it with a checked
//! callback URL or `cancel`, a second target's sheet is 409, and a
//! superseded session is forgotten for the host to cancel. Loopback
//! sessions (LLP 1133.005 D9) wait side by side, one per address, and take
//! only a callback with exactly one `code` or `error`.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::auth::{self, Arm, Browser};
use exact_runner::{agent, Answer, DataError, DataSource, Event, Outcome, Runner, Store};

const GRANTS: &str =
    "auth.session https://as.test\nauth.callback app.test:/cb\nauth.callback http://127.0.0.1/cb\nauth.callback http://[::1]/cb";

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

const APP: &str = "shape R\n  text: string\ncomponent App\n  state cb = \"app.test:/cb\"\n  mutation signed as shape R\n  mutation other as shape R\n  mutation third as shape R\n  action go\n    send signed = signIn(cb)\n  action bad\n    send signed = signIn(\"app.test:/elsewhere\")\n  action two\n    send other = other(cb)\n  action loopA\n    send signed = signIn(\"http://127.0.0.1:4401/cb\")\n  action loopB\n    send other = other(\"http://127.0.0.1:4402/cb\")\n  action loopSame\n    send other = other(\"http://127.0.0.1:4401/cb\")\n  action loopSix\n    send third = signIn(\"http://[::1]:4401/cb\")\n  view\n    column\n      button press=go testId=\"go\"\n        text \"go\"\n      button press=bad testId=\"bad\"\n        text \"bad\"\n      button press=two testId=\"two\"\n        text \"two\"\n      button press=loopA testId=\"loop-a\"\n        text \"loop a\"\n      button press=loopB testId=\"loop-b\"\n        text \"loop b\"\n      button press=loopSame testId=\"loop-same\"\n        text \"loop same\"\n      button press=loopSix testId=\"loop-six\"\n        text \"loop six\"\n      match signed\n        case some(s)\n          text s.text testId=\"out\"\n        case none\n          text \"\" testId=\"out\"\n";

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
        auth::arm(&mut r, &out, false, Browser::None, None),
        Arm::Settled
    );
    assert_eq!(deliver(&mut r), "501 no system browser on this host");
    // Under the agent a host with no browser holds it too (LLP 1146 D4).
    let out = press(&mut r, "go");
    assert_eq!(
        auth::arm(&mut r, &out, true, Browser::None, None),
        Arm::Held
    );
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
fn one_sheet_per_window_and_supersession_forgets_the_old_one() {
    let mut r = boot();
    let first = press(&mut r, "go");
    assert!(matches!(
        auth::arm(&mut r, &first, false, Browser::Native, None),
        Arm::Present(_)
    ));
    // Another target's sheet while this one is open: 409.
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

/// The settled answer's ticket and status, taken without the view.
fn settled(r: &mut Runner<Signer>) -> (u64, u16) {
    let (ticket, outcome) = r.take_request_refusal(true).expect("a settled answer");
    let status = exact_data::auth::reply(&outcome).map_or_else(|(s, _)| s, |_| 200);
    r.fulfill(ticket, outcome).unwrap();
    (ticket, status)
}

/// Loopback sign-ins wait at once, each on its own host and port, and
/// complete independently; a callback with the right `state` and no `code`
/// or `error` is refused and the hold stays; another on an address in use
/// is 502; a sheet beside a loopback sign-in opens (LLP 1133.005 D9).
#[test]
fn loopback_sessions_wait_side_by_side_and_take_only_a_code_or_an_error() {
    let mut r = boot();
    let a = press(&mut r, "loop-a");
    assert_eq!(
        auth::arm(&mut r, &a, true, Browser::Native, None),
        Arm::Held
    );
    let b = press(&mut r, "loop-b");
    assert_eq!(
        auth::arm(&mut r, &b, true, Browser::Native, None),
        Arm::Held
    );
    let typed = |r: &mut Runner<Signer>, t: u64, url: &str| {
        let json = format!(r#"{{"op":"type","ticket":{t},"text":"{url}"}}"#);
        agent::answer(r, &json).unwrap().0
    };
    // `[::1]:4401` binds beside `127.0.0.1:4401`, so it waits too.
    let six = press(&mut r, "loop-six");
    assert_eq!(
        auth::arm(&mut r, &six, true, Browser::Native, None),
        Arm::Held
    );
    let reply = typed(&mut r, six.ticket, "http://[::1]:4401/cb?code=c&state=s1");
    assert!(reply.contains("substituted"), "{reply}");
    assert_eq!(settled(&mut r), (six.ticket, 200));
    let stray = typed(&mut r, a.ticket, "http://127.0.0.1:4401/cb?state=s1");
    assert!(stray.contains("exactly one code or error"), "{stray}");
    let both = typed(
        &mut r,
        a.ticket,
        "http://127.0.0.1:4401/cb?code=c&error=e&state=s1",
    );
    assert!(both.contains("exactly one code or error"), "{both}");
    assert!(r.take_request_refusal(true).is_none(), "the hold stays");
    let reply = typed(&mut r, b.ticket, "http://127.0.0.1:4402/cb?code=c&state=s1");
    assert!(reply.contains("substituted"), "{reply}");
    assert_eq!(settled(&mut r), (b.ticket, 200));
    let denied = "http://127.0.0.1:4401/cb?error=access_denied&state=s1";
    assert!(typed(&mut r, a.ticket, denied).contains("substituted"));
    assert_eq!(settled(&mut r), (a.ticket, 200));

    // A host's receiver checks the same: its completion without a code is 502.
    let a = press(&mut r, "loop-a");
    assert!(matches!(
        auth::arm(&mut r, &a, false, Browser::Native, None),
        Arm::Present(_)
    ));
    let same = press(&mut r, "loop-same");
    assert_eq!(
        auth::arm(&mut r, &same, false, Browser::Native, None),
        Arm::Settled
    );
    assert_eq!(settled(&mut r), (same.ticket, 502));
    let sheet = press(&mut r, "two");
    assert!(matches!(
        auth::arm(&mut r, &sheet, false, Browser::Native, None),
        Arm::Present(_)
    ));
    auth::complete(&mut r, a.ticket, Ok("http://127.0.0.1:4401/cb?state=s1"));
    assert_eq!(settled(&mut r), (a.ticket, 502));
    // The web listens to nothing: a loopback callback other than the page's is 501.
    let a = press(&mut r, "loop-a");
    let page = Some("http://127.0.0.1:8080");
    assert_eq!(
        auth::arm(&mut r, &a, false, Browser::Web, page),
        Arm::Settled
    );
    assert_eq!(settled(&mut r), (a.ticket, 501));
}

/// `authorize` and `authorize cancel` parse as test steps and reach the driver
/// as its `authorize` operation; anything else after the word is refused
/// (LLP 1146 D4).
#[test]
fn authorize_steps_parse_and_reach_the_driver() {
    let tests = contract::tests("test \"sign in\"\n  authorize\n  authorize cancel\n").unwrap();
    let json = contract::tests_json(&tests);
    for want in [
        r#"{"op":"authorize","cancel":false"#,
        r#"{"op":"authorize","cancel":true"#,
    ] {
        assert!(json.contains(want), "{want}\n{json}");
    }
    let refused = contract::tests("test \"sign in\"\n  authorize now\n").unwrap_err();
    assert!(
        format!("{refused:?}").contains("takes nothing or `cancel`"),
        "{refused:?}"
    );
}
