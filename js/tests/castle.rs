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

  action setWho(v) writes who
    who = v
  action setPassword(v) writes password
    password = v
  action submit writes session
    send session = login(who, password)
  action profile writes session
    send session = profile()
  action logout writes session
    send session = logout()
  action stuck writes probe
    send probe = stuck()
  action refused writes probe
    send probe = refused()
  action refusedLater writes probe
    send probe = refusedLater()
  action parallel writes probe
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
    let mut m = Module::loaded(HBC.to_vec(), APP, GRANTS).expect("the fixture loads");
    m.bind(&plan());
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
        Module::new(HBC.to_vec(), APP, GRANTS),
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
        Module::new(HBC.to_vec(), APP, GRANTS),
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
        Module::new(HBC.to_vec(), APP, GRANTS),
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
