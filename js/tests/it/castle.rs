//! `fetch` over the host's ticket path, the store as the seam's, and an
//! answer that awaits two fetches in a row (LLP 1027 D1a) — through the
//! executor alone, and through a compiled Contract and the runner.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::{Kernel, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{
    Answer, DataError, DataSource, Event, FailureKind, Outcome, Response, Runner, Store,
};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/castle.hbc"));
const APP: &str = "xyz.castle.test";
const GRANTS: &str = "net.fetch https://api.castle.xyz\nsecret.keep castle.session\n";

const SRC: &str = r#"
shape Session
  ok: bool
  username: string
  error: string

component App
  state who = ""
  state password = ""
  resource remembered = remember() as shape Session
  mutation session as shape Session
  mutation probe as shape Session
  derive busy = pending(session)

  action setWho(v)
    who = v
  action setPassword(v)
    password = v
  action submit
    send session = login(who, password)
  action profile
    send session = profile()
  action logout
    send session = logout()
  action stuck
    send probe = stuck()
  action refused
    send probe = refused()
  action refusedLater
    send probe = refusedLater()
  action parallel
    send probe = parallel()

  view
    column testId="app"
      input value=who change=setWho testId="who"
      input value=password change=setPassword testId="password"
      button press=submit aria-label="Log in" testId="login"
        text "Log in"
      button press=profile aria-label="Profile" testId="profile"
        text "Profile"
      button press=logout aria-label="Log out" testId="logout"
        text "Log out"
      text remembered.username testId="remembered"
      when busy
        text "busy" testId="busy"
      match session
        case some(s)
          text s.username testId="session"
          text s.error testId="error"
        case none
          text "signed out" testId="signed-out"
"#;

fn plan() -> Plan {
    contract::compile(SRC).expect("the fixture's Contract compiles")
}

fn module() -> Module {
    let mut m = unloaded();
    m.load().expect("the fixture loads");
    m.bind(&plan());
    m
}

fn unloaded() -> Module {
    let mut m = Module::new(HBC.to_vec(), APP, GRANTS);
    // Functional fixtures carry no wall-clock budget; it is not a stable
    // gate on a shared test machine. Budget tests set their own.
    m.set_budget_ms(f64::INFINITY);
    m
}

fn store() -> Store {
    Store::new(GRANTS, Vec::<(String, String)>::new())
}

fn response(status: u16, body: &str) -> Outcome {
    Outcome::Response(Response {
        status,
        headers: vec![("content-type".into(), "application/json".into())],
        body: body.as_bytes().to_vec(),
    })
}

fn session(v: &Value) -> (bool, String, String) {
    match v {
        Value::Record(f) if f.len() == 3 => (
            matches!(f[0], Value::Bool(true)),
            f[1].as_str().unwrap_or("").to_string(),
            f[2].as_str().unwrap_or("").to_string(),
        ),
        other => panic!("not a session: {other:?}"),
    }
}

fn now(a: Answer) -> Value {
    match a {
        Answer::Now(v) => v,
        Answer::Later(r) => panic!("expected a value, got a request for {}", r.url),
    }
}

fn later(a: Answer) -> exact_runner::Request {
    match a {
        Answer::Later(r) => r,
        Answer::Now(v) => panic!("expected a request, got {v:?}"),
    }
}

const LOGIN_OK: &str = r#"{"data":{"loginV2":{"token":"t0k","username":"ada"}}}"#;

#[test]
fn independent_fetch_is_explicit_bounded_and_keeps_each_invocation() {
    let mut m = module();
    m.bind(&contract::compile("component App\n  resource result = scheduled(\"query\", 524288, true) as shape string\n  view\n    text result\n").unwrap());
    let mut s = store();
    let older = [
        Value::str("older"),
        Value::Number(524288.0),
        Value::Bool(true),
    ];
    let newer = [
        Value::str("newer"),
        Value::Number(262144.0),
        Value::Bool(true),
    ];
    for (args, ceiling) in [(&older, 524288), (&newer, 262144)] {
        let request = later(m.answer(&mut s, "scheduled", args).unwrap());
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.http,
            exact_runner::HttpScheduling::Independent {
                max_response_bytes: ceiling,
            }
        );
    }
    // The submitted search retains the ordered lane while suggestions overlap.
    let ordered = [
        Value::str("submitted"),
        Value::Number(0.0),
        Value::Bool(false),
    ];
    let request = later(m.answer(&mut s, "scheduled", &ordered).unwrap());
    assert_eq!(request.http, exact_runner::HttpScheduling::Ordered);
    for args in [&ordered[..], &newer[..], &older[..]] {
        let expected = args[0].as_str().unwrap();
        assert_eq!(
            now(m
                .parse(&mut s, "scheduled", args, response(200, expected))
                .unwrap()),
            Value::str(expected)
        );
    }
    for bad in [
        Value::Number(0.0),
        Value::Number(-1.0),
        Value::Number(1.5),
        Value::Number(67108865.0),
    ] {
        let error = m
            .answer(
                &mut s,
                "scheduled",
                &[Value::str("invalid"), bad, Value::Bool(true)],
            )
            .unwrap_err();
        assert!(
            matches!(&error, DataError::Unavailable(message) if message.contains("maxResponseBytes")),
            "{error:?}"
        );
        assert_eq!(m.in_flight(), 0);
    }
}

#[test]
fn a_fetch_deadline_reaches_the_request_and_its_timeout_rejects_with_its_kind() {
    let mut m = module();
    m.bind(&contract::compile("component App\n  resource result = timed(250) as shape string\n  view\n    text result\n").unwrap());
    let mut s = store();
    let args = [Value::Number(250.0)];
    let request = later(m.answer(&mut s, "timed", &args).unwrap());
    assert_eq!(request.timeout_ms, Some(250));
    let timed_out = Outcome::Failed {
        kind: exact_runner::FailureKind::Timeout,
        message: "the request timed out after 250 ms".into(),
    };
    assert_eq!(
        now(m.parse(&mut s, "timed", &args, timed_out).unwrap()),
        Value::str("failed: Timeout: the request timed out after 250 ms")
    );
    // A deadline out of range is the fetch's TypeError, before any request.
    for bad in [0.0, -1.0, 1.5, 3600001.0] {
        let refused = now(m.answer(&mut s, "timed", &[Value::Number(bad)]).unwrap());
        assert!(
            refused.as_str().is_some_and(|m| m.contains(
                "exactTimeout must be an integer number of milliseconds from 1 to 3600000"
            )),
            "{refused:?}"
        );
        assert_eq!(m.in_flight(), 0);
    }
}

fn event(id: &str, data: &str, coalesced: u32) -> Outcome {
    Outcome::Message(exact_runner::Message {
        event: String::new(),
        id: id.into(),
        data: data.into(),
        coalesced,
    })
}

/// LLP 1016.000, TypeScript: `fetch(url, { exactStream })` answers a
/// stream; each event, and the end, is mapped now by the source's function,
/// which reads the store like any answer.
#[test]
fn a_stream_answer_maps_each_event_and_its_end() {
    let mut m = module();
    m.bind(&contract::compile("component App\n  resource feed = events(\"0\") as shape string\n  view\n    text feed\n").unwrap());
    let mut s = store();
    let args = [Value::str("0")];
    let request = later(m.answer(&mut s, "events", &args).unwrap());
    assert!(request.stream, "the host is told it streams");
    assert_eq!(request.url, "https://api.castle.xyz/events?since=0");
    assert_eq!(
        request.http,
        exact_runner::HttpScheduling::Independent {
            max_response_bytes: 1 << 20
        }
    );
    assert_eq!(m.in_flight(), 1);
    assert_eq!(
        now(m
            .parse(&mut s, "events", &args, event("1", "a", 0))
            .unwrap()),
        Value::str("message 1:a:0:-")
    );
    s.set("castle.session", "seen").unwrap();
    assert_eq!(
        now(m
            .parse(&mut s, "events", &args, event("4", "d", 2))
            .unwrap()),
        Value::str("message 4:d:2:seen")
    );
    let ended = Outcome::Failed {
        kind: FailureKind::Network,
        message: "the event stream ended".into(),
    };
    assert_eq!(
        now(m.parse(&mut s, "events", &args, ended).unwrap()),
        Value::str("ended: the event stream ended")
    );
    assert_eq!(m.in_flight(), 0, "the end lets the call go");
    assert!(m
        .parse(&mut s, "events", &args, event("5", "e", 0))
        .is_err());
}

/// LLP 1069.004 slice 3: the spelling of a socket is its URL. A text frame
/// is a `message` event; the far side's close is the end.
#[test]
fn a_socket_answer_is_a_stream_to_a_wss_url() {
    let mut m = module();
    m.bind(&contract::compile("component App\n  resource feed = socket(\"7\") as shape string\n  view\n    text feed\n").unwrap());
    let mut s = store();
    let args = [Value::str("7")];
    let request = later(m.answer(&mut s, "socket", &args).unwrap());
    assert!(request.stream);
    assert_eq!(request.url, "wss://jetstream.castle.xyz/subscribe?cursor=7");
    assert_eq!(
        now(m
            .parse(
                &mut s,
                "socket",
                &args,
                event("", "{\"kind\":\"commit\"}", 0)
            )
            .unwrap()),
        Value::str("message:{\"kind\":\"commit\"}")
    );
    let closed = Outcome::Failed {
        kind: FailureKind::Network,
        message: "the socket closed (1000)".into(),
    };
    assert_eq!(
        now(m.parse(&mut s, "socket", &args, closed).unwrap()),
        Value::str("closed: the socket closed (1000)")
    );
}

/// Through the runner: three events commit three times, the stream stays
/// held, and a newer argument forgets the stream's call in the module.
#[test]
fn a_stream_answer_through_the_runner() {
    let src = "component App\n  state since = \"0\"\n  resource feed = events(since) as shape string\n  action next\n    since = \"9\"\n  view\n    text feed testId=\"feed\"\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        plan,
        module(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    assert!(out[0].request.stream);
    let ticket = out[0].ticket;
    for n in 1..=3 {
        let receipt = r.fulfill(ticket, event(&n.to_string(), "x", 0)).unwrap();
        assert!(receipt.is_some());
        assert_eq!(
            r.resource("feed"),
            Some(&Value::str(&format!("message {n}:x:0:-")))
        );
        assert!(r.holds(ticket) && !r.has_pending());
    }
    r.act("next", vec![]).unwrap();
    assert!(!r.holds(ticket));
    assert_eq!(
        r.data().in_flight(),
        1,
        "only the newer stream's call is left"
    );
}

#[test]
fn parallel_fetches_keep_every_request_and_binary_response() {
    let mut m = module();
    let mut s = store();
    assert_eq!(
        later(m.answer(&mut s, "parallel", &[]).unwrap()).url,
        "https://api.castle.xyz/a"
    );
    let binary = Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: vec![0, 255],
    });
    assert_eq!(
        later(m.parse(&mut s, "parallel", &[], binary).unwrap()).url,
        "https://api.castle.xyz/b"
    );
    assert_eq!(
        session(&now(m
            .parse(&mut s, "parallel", &[], response(200, "ok"))
            .unwrap()))
        .2,
        "0,255/ok"
    );
}

#[test]
fn login_describes_a_request_and_the_reply_becomes_the_session_and_a_store_write() {
    let mut m = module();
    let mut s = store();
    let args = [Value::str("ada"), Value::str("pw")];
    // The form's own refusal needs no round trip.
    let v = now(m
        .answer(&mut s, "login", &[Value::str(""), Value::str("pw")])
        .unwrap());
    assert_eq!(session(&v).2, "Enter a username and a password");
    // A real login: the module describes; the host will run it.
    let req = later(m.answer(&mut s, "login", &args).unwrap());
    assert_eq!(req.method, "POST");
    assert_eq!(req.url, "https://api.castle.xyz/graphql");
    assert!(req
        .headers
        .contains(&("content-type".into(), "application/json".into())));
    assert!(String::from_utf8_lossy(&req.body).contains("\"who\":\"ada\""));
    assert_eq!(m.in_flight(), 1);
    // The reply: the continuation after `await` runs, writes the store, and
    // the value takes the declared shape.
    let v = now(m
        .parse(&mut s, "login", &args, response(200, LOGIN_OK))
        .unwrap());
    assert_eq!(session(&v), (true, "ada".into(), "".into()));
    assert_eq!(m.in_flight(), 0);
    assert!(s.get("castle.session").unwrap().contains("t0k"));
    // `remember` reads what login kept — a counted read.
    let before = s.reads();
    let v = now(m.answer(&mut s, "remember", &[]).unwrap());
    assert_eq!(session(&v), (true, "ada".into(), "".into()));
    assert!(s.reads() > before, "the store read is counted");
    // `logout` forgets it.
    let v = now(m.answer(&mut s, "logout", &[]).unwrap());
    assert!(!session(&v).0);
    assert!(s.get("castle.session").is_none());
}

#[test]
fn a_failed_fetch_is_a_fetch_error_the_module_turns_into_its_own_words() {
    let mut m = module();
    let mut s = store();
    let args = [Value::str("ada"), Value::str("pw")];
    later(m.answer(&mut s, "login", &args).unwrap());
    let v = now(m
        .parse(
            &mut s,
            "login",
            &args,
            Outcome::Failed {
                kind: FailureKind::Refused,
                message: "not granted".into(),
            },
        )
        .unwrap());
    assert_eq!(session(&v).2, "Castle is not a host this app may reach");
    later(m.answer(&mut s, "login", &args).unwrap());
    let v = now(m
        .parse(&mut s, "login", &args, response(500, "<html>"))
        .unwrap());
    assert_eq!(session(&v).2, "Castle answered HTTP 500 without JSON");
}

/// Hermes watches a fetch's signal through Ibex's abort hooks, which an
/// app's listener cannot stop, and leaves no listener behind (review r4a
/// 7, 8; the browser realm's listener is js/web/tests/browser.rs's).
#[test]
fn a_reused_signal_gains_no_listener_per_fetch() {
    let mut m = module();
    m.bind(&contract::compile(&SHARED.replace("thread(story)", "reused()")).unwrap());
    let mut s = store();
    for _ in 0..2 {
        later(m.answer(&mut s, "reused", &[]).unwrap());
        let v = now(m.parse(&mut s, "reused", &[], response(200, "ok")).unwrap());
        assert_eq!(session(&v).2, "0/0");
    }
}

#[test]
fn an_answer_may_await_two_fetches_in_a_row() {
    let mut m = module();
    let mut s = store();
    s.set("castle.session", r#"{"token":"t0k","username":"ada"}"#)
        .unwrap();
    let me = later(m.answer(&mut s, "profile", &[]).unwrap());
    assert_eq!(me.url, "https://api.castle.xyz/me");
    assert!(me.headers.contains(&("x-auth-token".into(), "t0k".into())));
    // The first reply makes the second request: `parse` hands back a request.
    let p = later(
        m.parse(
            &mut s,
            "profile",
            &[],
            response(200, r#"{"username":"ada"}"#),
        )
        .unwrap(),
    );
    assert_eq!(p.url, "https://api.castle.xyz/profile/ada");
    assert_eq!(p.method, "GET");
    assert_eq!(m.in_flight(), 1);
    let v = now(m
        .parse(&mut s, "profile", &[], response(200, "hello"))
        .unwrap());
    assert_eq!(session(&v), (true, "ada".into(), "hello".into()));
    assert_eq!(m.in_flight(), 0);
}

#[test]
fn refusals_thrown_before_and_after_a_fetch_and_an_answer_pending_on_nothing() {
    let mut m = module();
    let mut s = store();
    assert!(matches!(
        m.answer(&mut s, "refused", &[]),
        Err(DataError::Unavailable(ref e)) if e == "refused on purpose"
    ));
    later(m.answer(&mut s, "refusedLater", &[]).unwrap());
    assert!(matches!(
        m.parse(&mut s, "refusedLater", &[], response(200, "{}")),
        Err(DataError::Unavailable(ref e)) if e == "after the fetch"
    ));
    assert!(matches!(
        m.answer(&mut s, "stuck", &[]),
        Err(DataError::Unavailable(ref e)) if e.contains("pending on nothing")
    ));
    // A reply for nothing in flight, and a fetching source at bake.
    assert!(matches!(
        m.parse(&mut s, "login", &[Value::str("x"), Value::str("y")], response(200, "{}")),
        Err(DataError::Unavailable(ref e)) if e.contains("not in flight")
    ));
    assert!(matches!(
        m.query("login", &[Value::str("ada"), Value::str("pw")]),
        Err(DataError::Unavailable(ref e)) if e.contains("no host to run it")
    ));
    assert_eq!(m.in_flight(), 0);
    // The bake's path has no store: reads are empty, writes refused.
    let v = m.query("remember", &[]).unwrap();
    assert!(!session(&v).0);
    assert!(
        matches!(m.query("logout", &[]), Err(DataError::Unavailable(ref e)) if e.contains("no store at bake"))
    );
}

// --- through the runner ---------------------------------------------------

fn text_of(r: &Runner<Module>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    k.node_by_key(key)?
        .props
        .str(PropId::Text)
        .map(str::to_string)
}
fn has(r: &Runner<Module>, test_id: &str) -> bool {
    !r.kernel().find_by_test_id(test_id).is_empty()
}
fn view_of(r: &Runner<Module>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

#[test]
fn the_runner_drives_a_typescript_login_and_a_two_request_profile() {
    let baked = contract::bake(plan(), Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap())
        .expect("bake runs the module for the first frame");
    let mut r = Runner::boot(
        baked,
        Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(has(&r, "signed-out"));
    assert_eq!(text_of(&r, "remembered").as_deref(), Some(""));

    r.dispatch(view_of(&r, "who"), Event::Change("ada".into()))
        .unwrap();
    r.dispatch(view_of(&r, "password"), Event::Change("pw".into()))
        .unwrap();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let reqs = r.take_requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].target, "session");
    assert_eq!(reqs[0].request.url, "https://api.castle.xyz/graphql");
    assert!(has(&r, "busy"));
    r.fulfill(reqs[0].ticket, response(200, LOGIN_OK)).unwrap();
    assert_eq!(text_of(&r, "session").as_deref(), Some("ada"));
    assert!(!has(&r, "busy"));
    // login's store write re-answered the store-reading resource.
    assert_eq!(text_of(&r, "remembered").as_deref(), Some("ada"));
    let logs = r.data().take_logs();
    assert!(logs.is_empty(), "{logs:?}");

    // Two requests in a row on one send.
    r.dispatch(view_of(&r, "profile"), Event::Press).unwrap();
    let me = r.take_requests();
    assert_eq!(me.len(), 1);
    assert_eq!(me[0].request.url, "https://api.castle.xyz/me");
    let receipt = r
        .fulfill(me[0].ticket, response(200, r#"{"username":"ada"}"#))
        .unwrap();
    assert!(
        receipt.is_some(),
        "an empty commit carries the next request out"
    );
    assert!(r.has_pending(), "the reply asked for one more request");
    assert!(has(&r, "busy"));
    let p = r.take_requests();
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].request.url, "https://api.castle.xyz/profile/ada");
    r.fulfill(p[0].ticket, response(200, "hello")).unwrap();
    assert_eq!(text_of(&r, "error").as_deref(), Some("hello"));
    assert!(!r.has_pending());

    r.dispatch(view_of(&r, "logout"), Event::Press).unwrap();
    assert!(r.take_requests().is_empty(), "logout answers now");
    assert_eq!(text_of(&r, "remembered").as_deref(), Some(""));
}

/// Two targets that ask one source with equal arguments, both in flight.
const TWINS: &str = r#"
shape Session
  ok: bool
  username: string
  error: string

component App
  mutation first as shape Session
  mutation second as shape Session

  action both
    send first = profile()
    send second = profile()

  view
    column
      button press=both aria-label="Both" testId="both"
        text "Both"
      match first
        case some(s)
          text s.username testId="first-user"
          text s.error testId="first-text"
        case none
          text "none" testId="first-none"
      match second
        case some(s)
          text s.username testId="second-user"
          text s.error testId="second-text"
        case none
          text "none" testId="second-none"
"#;

fn targets(requests: &[exact_runner::RequestOut]) -> Vec<(&str, &str)> {
    requests
        .iter()
        .map(|q| (q.target.as_str(), q.request.url.as_str()))
        .collect()
}

#[test]
fn two_targets_asking_one_source_with_equal_arguments_each_settle_with_their_own_replies() {
    let mut r = Runner::boot_stored(
        contract::compile(TWINS).expect("the fixture's Contract compiles"),
        Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap(),
        Kernel::with_monospace(),
        vec![(
            "castle.session".into(),
            r#"{"token":"t0k","username":"ada"}"#.into(),
        )],
        Default::default(),
        "/",
    )
    .unwrap();
    r.dispatch(view_of(&r, "both"), Event::Press).unwrap();
    let me = r.take_requests();
    assert_eq!(
        targets(&me),
        [
            ("first", "https://api.castle.xyz/me"),
            ("second", "https://api.castle.xyz/me"),
        ]
    );
    assert_eq!(r.data().in_flight(), 2, "both calls are parked");
    // Each `/me` reply resumes its own call, which asks for its own profile.
    r.fulfill(me[0].ticket, response(200, r#"{"username":"ada"}"#))
        .unwrap();
    r.fulfill(me[1].ticket, response(200, r#"{"username":"bob"}"#))
        .unwrap();
    let profiles = r.take_requests();
    assert_eq!(
        targets(&profiles),
        [
            ("first", "https://api.castle.xyz/profile/ada"),
            ("second", "https://api.castle.xyz/profile/bob"),
        ]
    );
    // The replies, the other way round.
    r.fulfill(profiles[1].ticket, response(200, "bob's"))
        .unwrap();
    r.fulfill(profiles[0].ticket, response(200, "ada's"))
        .unwrap();
    assert_eq!(text_of(&r, "first-user").as_deref(), Some("ada"));
    assert_eq!(text_of(&r, "first-text").as_deref(), Some("ada's"));
    assert_eq!(text_of(&r, "second-user").as_deref(), Some("bob"));
    assert_eq!(text_of(&r, "second-text").as_deref(), Some("bob's"));
    assert!(!r.has_pending());
    assert_eq!(r.data().in_flight(), 0);
}

/// Two resources whose answers share one fetch (hn-reader F7).
const SHARED: &str = r#"
shape Session
  ok: bool
  username: string
  error: string

component App
  state story = ""
  resource detail = item(story) as shape Session
  resource comments = thread(story) as shape Session
  action open
    story = "8863"
  view
    column
      button press=open testId="open"
        text "Open"
      text detail.error testId="detail"
      text comments.error testId="comments"
"#;

/// An answer that awaits a promise another answer's fetch settles waits for
/// it, as in a browser, rather than refusing the press that asked
/// (LLP 1027.003.000 §13, the module-wide rule; hn-reader F7).
#[test]
fn an_answer_awaiting_another_answers_fetch_waits_for_it_and_both_settle() {
    use exact_runner::{Dispatch, Work};
    let plan = contract::compile(SHARED).expect("the fixture's Contract compiles");
    let baked = contract::bake(plan, Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap()).unwrap();
    let mut r = Runner::boot(
        baked,
        Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.dispatch(view_of(&r, "open"), Event::Press).unwrap();
    let asked = r.take_requests();
    assert_eq!(asked.len(), 2, "the fetch, and the answer that waits on it");
    let (fetch, waiting): (Vec<_>, Vec<_>) =
        asked.iter().partition(|a| a.request.continuation.is_none());
    assert_eq!(fetch[0].request.url, "https://api.castle.xyz/item/8863");
    let token = waiting[0].request.continuation.unwrap();
    assert!(
        matches!(r.dispatch_work(token), Dispatch::Held),
        "it waits while the fetch it shares is in flight"
    );
    assert!(r.release_work().is_empty(), "nothing has landed yet");
    r.fulfill(fetch[0].ticket, response(200, "the story"))
        .unwrap();
    let released = r.release_work();
    assert_eq!(released.len(), 1, "the delivery wakes the waiting answer");
    let (woken, Dispatch::Run(Work::Now(work))) = released.into_iter().next().unwrap() else {
        panic!("a waiting answer is asked again at once");
    };
    assert_eq!(woken, token);
    r.fulfill(waiting[0].ticket, work()).unwrap();
    assert_eq!(text_of(&r, "detail").as_deref(), Some("the story"));
    assert_eq!(text_of(&r, "comments").as_deref(), Some("the story"));
    assert!(!r.has_pending());
    assert_eq!(r.data().in_flight(), 0);
}

/// An answer that awaits another's fetch and then fetches on its own: its
/// second request runs, though its continuation ran as the other answer,
/// which had already settled (review r4a, finding 1).
#[test]
fn a_fetch_made_after_awaiting_another_answers_fetch_is_not_left_behind() {
    use exact_runner::{Dispatch, Work};
    let plan = contract::compile(&SHARED.replace("thread(story)", "followup(story)"))
        .expect("the fixture's Contract compiles");
    let baked = contract::bake(plan, Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap()).unwrap();
    let mut r = Runner::boot(
        baked,
        Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.dispatch(view_of(&r, "open"), Event::Press).unwrap();
    let asked = r.take_requests();
    let (fetch, waiting): (Vec<_>, Vec<_>) =
        asked.iter().partition(|a| a.request.continuation.is_none());
    let token = waiting[0].request.continuation.unwrap();
    assert!(matches!(r.dispatch_work(token), Dispatch::Held));
    r.fulfill(fetch[0].ticket, response(200, "the story"))
        .unwrap();
    assert_eq!(text_of(&r, "detail").as_deref(), Some("the story"));
    let (_, Dispatch::Run(Work::Now(work))) = r.release_work().into_iter().next().unwrap() else {
        panic!("the waiting answer is asked again");
    };
    r.fulfill(waiting[0].ticket, work()).unwrap();
    let more = r.take_requests();
    assert_eq!(more.len(), 1, "its own second fetch is handed out");
    assert_eq!(more[0].request.url, "https://api.castle.xyz/comments/8863");
    r.fulfill(more[0].ticket, response(200, "talk")).unwrap();
    assert_eq!(text_of(&r, "comments").as_deref(), Some("the story + talk"));
    assert!(!r.has_pending());
    assert_eq!(r.data().in_flight(), 0);
}

/// An answer that fetches without awaiting and finishes while another answer
/// awaits its own fetch: its request is still handed out, not left in the
/// module (Grok's batch 2 review, runtime). A ticket made while the answer
/// was still in flight is its own, whoever else is pending.
#[test]
fn an_unawaited_fetch_is_sent_while_another_answer_is_in_flight() {
    let src = SHARED
        .replace(
            "  resource comments = thread(story) as shape Session\n",
            "  mutation saved as shape Session\n  action save\n    send saved = saveQuietly(\"8863\")\n",
        )
        .replace(
            "      text comments.error testId=\"comments\"\n",
            "      button press=save testId=\"save\"\n        text \"Save\"\n      match saved\n        case some(s)\n          text s.error testId=\"saved\"\n        case none\n          text \"\"\n",
        );
    let plan = contract::compile(&src).expect("the fixture's Contract compiles");
    let baked = contract::bake(plan, Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap()).unwrap();
    let mut r = Runner::boot(
        baked,
        Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.dispatch(view_of(&r, "open"), Event::Press).unwrap();
    let item = r.take_requests();
    assert_eq!(item.len(), 1);
    assert_eq!(item[0].request.url, "https://api.castle.xyz/item/8863");
    r.dispatch(view_of(&r, "save"), Event::Press).unwrap();
    let saved = r.take_requests();
    assert_eq!(
        saved.len(),
        1,
        "the unawaited POST is handed out: {saved:?}"
    );
    assert_eq!(saved[0].request.url, "https://api.castle.xyz/save/8863");
    assert_eq!(saved[0].request.method, "POST");
    r.fulfill(saved[0].ticket, response(200, "")).unwrap();
    assert_eq!(text_of(&r, "saved").as_deref(), Some("saved"));
    r.fulfill(item[0].ticket, response(200, "the story"))
        .unwrap();
    assert_eq!(text_of(&r, "detail").as_deref(), Some("the story"));
    assert!(!r.has_pending());
    assert_eq!(r.data().in_flight(), 0);
}

// --- the kept answer (LLP 1027 D4, as ruled 2026-09-03) --------------------

#[test]
fn a_store_reading_resource_boots_from_its_kept_answer_and_is_asked_again_once_the_engine_is_up() {
    let baked =
        contract::bake(plan(), Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap()).expect("bake");
    // The bake compiled the empty-store answer for `remembered` and marked it.
    let i = baked
        .resources
        .iter()
        .position(|r| baked.str(r.name) == "remembered")
        .unwrap();
    assert!(baked.resources[i].reader, "bake found it reading the store");
    assert!(
        baked.resources[i].initial.len > 0,
        "the empty-store answer is compiled"
    );

    // A fresh install: the engine is not loaded at boot; the first frame is
    // the placeholder; nothing happens until the host says the engine is up.
    let mut r = Runner::boot(
        baked.clone(),
        unloaded(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&r, "remembered").as_deref(), Some(""));
    assert!(
        r.data_ready().unwrap().is_none(),
        "not ready: nothing to do"
    );
    r.data().load().unwrap();
    assert!(
        r.data_ready().unwrap().is_some(),
        "the placeholder is asked again"
    );
    assert_eq!(text_of(&r, "remembered").as_deref(), Some(""));
    assert!(r.data_ready().unwrap().is_none(), "asked once");

    // Live replacement loads beside the old client. It must not disable
    // keeping fresh answers merely because its engine is already ready.
    let mut r = Runner::boot_carrying(
        baked.clone(),
        Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap(),
        Kernel::with_monospace(),
        &r.carry(),
        Default::default(),
        "/",
    )
    .unwrap();

    // Log in: the fresh answer to `remembered` is kept beside the session,
    // and the app cannot see it.
    r.dispatch(view_of(&r, "who"), Event::Change("ada".into()))
        .unwrap();
    r.dispatch(view_of(&r, "password"), Event::Change("pw".into()))
        .unwrap();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let reqs = r.take_requests();
    r.fulfill(reqs[0].ticket, response(200, LOGIN_OK)).unwrap();
    assert_eq!(text_of(&r, "remembered").as_deref(), Some("ada"));
    let writes = r.take_store_writes();
    assert!(
        writes.iter().any(|w| w.name == "castle.session"),
        "{writes:?}"
    );
    assert!(
        writes.iter().any(|w| w.name == "exact.kept.remembered"),
        "{writes:?}"
    );
    assert!(r.store().get("exact.kept.remembered").is_none());
    assert!(r.store().names().contains(&"exact.kept.remembered"));

    // The next launch, engine not yet loaded: the first frame is yesterday's
    // answer, and it stands once the engine confirms it.
    let snapshot = r.store().snapshot();
    let mut next = Runner::boot_stored(
        baked.clone(),
        unloaded(),
        Kernel::with_monospace(),
        snapshot.clone(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&next, "remembered").as_deref(), Some("ada"));
    next.data().load().unwrap();
    assert!(next.data_ready().unwrap().is_some());
    assert_eq!(text_of(&next, "remembered").as_deref(), Some("ada"));

    // The keychain was cleared underneath: the kept answer shows for a
    // frame, then the engine corrects it.
    let cleared: Vec<(String, String)> = snapshot
        .into_iter()
        .filter(|(n, _)| n != "castle.session")
        .collect();
    let mut stale = Runner::boot_stored(
        baked,
        unloaded(),
        Kernel::with_monospace(),
        cleared,
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&stale, "remembered").as_deref(), Some("ada"));
    stale.data().load().unwrap();
    stale.data_ready().unwrap();
    assert_eq!(text_of(&stale, "remembered").as_deref(), Some(""));
}

#[test]
fn a_resumed_capture_over_budget_retires_the_call_and_keeps_later_answers_clean() {
    let mut m = module();
    let mut s = store();
    let args = [Value::str("ada"), Value::str("pw")];
    later(m.answer(&mut s, "login", &args).unwrap());
    let body = serde_json::json!({"data":{"loginV2":{"token":"t0k","username":"a".repeat(65536)}}})
        .to_string();
    m.set_budget_ms(0.0);
    let result = m.parse(&mut s, "login", &args, response(200, &body));
    assert!(
        matches!(result, Err(DataError::Unavailable(ref message)) if message.contains("over the 0 ms budget"))
    );
    assert_eq!(m.overruns(), 1);
    assert_eq!(m.in_flight(), 0);
    m.set_budget_ms(f64::INFINITY);
    assert!(!session(&now(m.answer(&mut s, "logout", &[]).unwrap())).0);
    assert!(!session(&now(m.answer(&mut s, "remember", &[]).unwrap())).0);
}

/// A worker-placed answer (LLP 1027.002) whose reply body is large: every
/// stage crosses to the owner thread, the body with it (Crew port, F4/F6).
#[test]
fn a_worker_placed_answer_takes_a_large_reply_as_it_takes_a_small_one() {
    use exact_js::Placement;
    use exact_runner::{Dispatch, Reply, Work};
    fn run(placed: &mut exact_js::Placed<Module>, s: &mut Store, answer: Answer) -> Answer {
        let token = later(answer).continuation.expect("a turn for the owner");
        let Dispatch::Run(Work::Later(work)) = placed.dispatch(token, s) else {
            panic!("a turn for the owner");
        };
        let (tx, rx) = std::sync::mpsc::channel();
        work(Reply::new(move |outcome| {
            let _ = tx.send(outcome);
        }));
        let outcome = rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("the owner's turn replied");
        placed.parse(s, "profile", &[], outcome).unwrap()
    }
    for size in [24, 200 * 1024] {
        let mut placed = module().placed(Placement::Worker);
        placed.activate().unwrap();
        let mut s = store();
        s.set("castle.session", r#"{"token":"t0k","username":"ada"}"#)
            .unwrap();
        let first = placed.answer(&mut s, "profile", &[]).unwrap();
        let me = later(run(&mut placed, &mut s, first));
        assert_eq!(me.url, "https://api.castle.xyz/me");
        let resumed = placed
            .parse(
                &mut s,
                "profile",
                &[],
                response(200, r#"{"username":"ada"}"#),
            )
            .unwrap();
        let p = later(run(&mut placed, &mut s, resumed));
        assert_eq!(p.url, "https://api.castle.xyz/profile/ada");
        let body = format!("\"{}\"", "x".repeat(size - 2));
        let resumed = placed
            .parse(&mut s, "profile", &[], response(200, &body))
            .unwrap();
        let v = now(run(&mut placed, &mut s, resumed));
        let (ok, username, text) = session(&v);
        assert!(ok && username == "ada", "{size}");
        assert_eq!(text.len(), size, "{size}");
    }
}
