//! LLP 1016 end to end through the compiler: a `mutation` slot filled by
//! `send`, a request the host runs and brings back through `fulfill`,
//! `pending(x)` in the view, `refresh` on a resource, and what bake refuses.

use exact_kernel::{Kernel, PropId};
use exact_runner::{
    Answer, DataError, DataSource, Event, Outcome, Request, Response, Runner, RunnerError, Value,
};

const SRC: &str = r#"
shape Session
  ok: bool
  username: string
  error: string

shape Balance
  bricks: number

component App
  state who = ""
  state password = ""
  mutation session as shape Session
  derive token = match session { case some(s) => s.username, case none => "" }
  resource balance = balance(token) as shape Balance
  derive busy = pending(session)

  action setWho(v) writes who
    who = v
  action setPassword(v) writes password
    password = v
  action submit writes session
    send session = login(who, password)
  action logout writes session
    send session = logout(token)
    session = none
  action paid writes who
    refresh balance

  view
    column testId="app"
      input value=who change=setWho testId="who"
      input value=password change=setPassword testId="password"
      button press=submit aria-label="Log in" testId="login"
        text "Log in"
      button press=logout aria-label="Log out" testId="logout"
        text "Log out"
      button press=paid aria-label="Paid" testId="paid"
        text "Paid"
      when busy
        text "Logging in…" testId="busy"
      match session
        case some(s)
          when s.ok
            text `Signed in as ${s.username}` testId="signed-in"
          else
            text s.error testId="error"
        case none
          text "Signed out" testId="signed-out"
      text `${balance.bricks} bricks` testId="bricks"
"#;

/// A Castle that answers later: every `login` and `logout` is a request,
/// `balance` answers now and counts how often it was asked.
#[derive(Default)]
struct Castle {
    balance_asks: usize,
    later: bool,
}

fn session(ok: bool, username: &str, error: &str) -> Value {
    Value::record(vec![
        Value::Bool(ok),
        Value::str(username),
        Value::str(error),
    ])
}

impl DataSource for Castle {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "balance" => {
                self.balance_asks += 1;
                let n = if args[0].as_str() == Some("") {
                    0.0
                } else {
                    42.0
                };
                Ok(Value::record(vec![Value::Number(n)]))
            }
            "login" | "logout" => Err(DataError::Unavailable(format!("{source} answers later"))),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "login" | "logout" if self.later => Ok(Answer::Later(
                Request::post_json(
                    "https://api.castle.test/graphql",
                    &format!("{{\"op\":\"{source}\"}}"),
                )
                .header("x-args", &args.len().to_string()),
            )),
            _ => self.query(source, args).map(Answer::Now),
        }
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Value, DataError> {
        match (source, outcome) {
            ("login", Outcome::Response(r)) if r.status == 200 => {
                Ok(session(true, args[0].as_str().unwrap_or(""), ""))
            }
            ("login", Outcome::Response(r)) => {
                Ok(session(false, "", &format!("HTTP {}", r.status)))
            }
            ("login", Outcome::Failed { message, .. }) => Ok(session(false, "", &message)),
            ("logout", _) => Ok(session(false, "", "")),
            (other, _) => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn text_of(r: &Runner<Castle>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    k.node_by_key(key)?
        .props
        .str(PropId::Text)
        .map(str::to_string)
}
fn has(r: &Runner<Castle>, test_id: &str) -> bool {
    !r.kernel().find_by_test_id(test_id).is_empty()
}
fn view_of(r: &Runner<Castle>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}
fn ok(status: u16) -> Outcome {
    Outcome::Response(Response {
        status,
        headers: vec![],
        body: b"{}".to_vec(),
    })
}

fn boot() -> Runner<Castle> {
    let plan = contract::compile(SRC).unwrap();
    let baked = contract::bake(plan, Castle::default()).unwrap();
    Runner::boot(
        baked,
        Castle {
            later: true,
            ..Castle::default()
        },
        Kernel::with_monospace(),
    )
    .unwrap()
}

#[test]
fn a_mutation_is_none_at_boot_and_bake_never_sends() {
    let plan = contract::compile(SRC).unwrap();
    assert_eq!(plan.mutations.len(), 1);
    // The mutation's slot is a slot like any other, `none` until sent.
    assert!(plan.slots.iter().any(|s| plan.str(s.name) == "session"));
    let baked = contract::bake(plan, Castle::default()).unwrap();
    assert!(
        baked.resources[0].initial.len > 0,
        "balance is compiled data"
    );
    let r = Runner::boot(
        baked,
        Castle {
            later: true,
            ..Castle::default()
        },
        Kernel::with_monospace(),
    )
    .unwrap();
    assert!(has(&r, "signed-out") && !has(&r, "busy"));
    assert_eq!(r.slot("session"), Some(&Value::Option(None)));
    assert!(r.pending().is_empty());
}

#[test]
fn send_asks_the_host_and_fulfill_fills_the_slot() {
    let mut r = boot();
    r.dispatch(view_of(&r, "who"), Event::Change("ada".into()))
        .unwrap();
    r.dispatch(view_of(&r, "password"), Event::Change("pw".into()))
        .unwrap();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    // The request left with the send's arguments; the view says busy; the
    // slot is still none.
    let reqs = r.take_requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].target, "session");
    assert_eq!(reqs[0].request.method, "POST");
    assert!(reqs[0]
        .request
        .headers
        .contains(&("x-args".into(), "2".into())));
    assert_eq!(r.pending(), vec![("session".to_string(), reqs[0].ticket)]);
    assert!(has(&r, "busy") && has(&r, "signed-out"));
    // The reply: the source parses it, the slot fills, the view follows,
    // and the resource that reads the token re-requests by itself.
    let asks = r.data().balance_asks;
    let receipt = r.fulfill(reqs[0].ticket, ok(200)).unwrap();
    assert!(receipt.is_some());
    assert!(!has(&r, "busy"));
    assert_eq!(
        text_of(&r, "signed-in").as_deref(),
        Some("Signed in as ada")
    );
    assert_eq!(text_of(&r, "bricks").as_deref(), Some("42 bricks"));
    assert_eq!(
        r.data().balance_asks,
        asks + 1,
        "balance(token) followed the login"
    );
    assert!(r.pending().is_empty());
    // A late or unknown reply is dropped, not an error.
    assert_eq!(r.fulfill(reqs[0].ticket, ok(200)).unwrap(), None);
    // A failure is data the source shapes.
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let t = r.take_requests()[0].ticket;
    r.fulfill(
        t,
        Outcome::Failed {
            kind: exact_runner::FailureKind::Network,
            message: "no route".into(),
        },
    )
    .unwrap();
    assert_eq!(text_of(&r, "error").as_deref(), Some("no route"));
}

#[test]
fn the_newest_send_wins_and_an_assignment_forgets() {
    let mut r = boot();
    r.dispatch(view_of(&r, "who"), Event::Change("ada".into()))
        .unwrap();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let first = r.take_requests()[0].ticket;
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let second = r.take_requests()[0].ticket;
    assert_ne!(first, second);
    assert_eq!(r.pending().len(), 1, "one request in flight per mutation");
    // The first reply is dropped; the second lands.
    assert_eq!(r.fulfill(first, ok(200)).unwrap(), None);
    assert!(has(&r, "signed-out"));
    r.fulfill(second, ok(200)).unwrap();
    assert!(has(&r, "signed-in"));
    // Logout: the POST goes, the slot drops now, the reply is forgotten.
    r.dispatch(view_of(&r, "logout"), Event::Press).unwrap();
    let reqs = r.take_requests();
    assert_eq!(reqs.len(), 1, "the logout request went out");
    assert!(has(&r, "signed-out") && !has(&r, "busy"));
    assert!(r.pending().is_empty(), "the assignment forgot the ticket");
    assert_eq!(r.fulfill(reqs[0].ticket, ok(200)).unwrap(), None);
    assert!(r.journal().any(|l| l.contains("forget request")));
}

#[test]
fn refresh_re_requests_a_resource_whose_arguments_did_not_change() {
    let mut r = boot();
    let asks = r.data().balance_asks;
    r.dispatch(view_of(&r, "paid"), Event::Press).unwrap();
    assert_eq!(r.data().balance_asks, asks + 1);
    // Without `refresh`, the same arguments are not asked again.
    r.dispatch(view_of(&r, "who"), Event::Change("x".into()))
        .unwrap();
    assert_eq!(r.data().balance_asks, asks + 1);
}

#[test]
fn a_reload_carries_the_slot_and_never_resends() {
    let mut r = boot();
    r.dispatch(view_of(&r, "who"), Event::Change("ada".into()))
        .unwrap();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let t = r.take_requests()[0].ticket;
    r.fulfill(t, ok(200)).unwrap();
    let carried = r.carry();
    let plan = contract::compile(SRC).unwrap();
    let mut again = Runner::boot_carrying(
        plan,
        Castle {
            later: true,
            ..Castle::default()
        },
        Kernel::with_monospace(),
        &carried,
    )
    .unwrap();
    assert!(has(&again, "signed-in"));
    assert!(again.take_requests().is_empty(), "nothing was re-sent");
    assert!(again.pending().is_empty());
}

#[test]
fn bake_refuses_a_resource_that_answers_later_at_boot() {
    let src = SRC.replace(
        "resource balance = balance(token) as shape Balance",
        "resource balance = later(token) as shape Balance",
    );
    struct Remote;
    impl DataSource for Remote {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::Unavailable(source.into()))
        }
        fn answer(
            &mut self,
            _: &mut exact_runner::Store,
            _: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            Ok(Answer::Later(Request::get(
                "https://api.castle.test/balance",
            )))
        }
    }
    let plan = contract::compile(&src).unwrap();
    let err = contract::bake(plan, Remote).unwrap_err();
    assert!(
        matches!(err, contract::BakeError::Runner(RunnerError::Data { ref resource, .. }) if resource == "balance"),
        "{err:?}"
    );
}

#[test]
fn the_language_refuses_what_it_should() {
    let refuse = |edit: &str, with: &str, id: &str| {
        let e = contract::compile(&SRC.replace(edit, with)).unwrap_err();
        assert!(format!("{e}").contains(id), "{e}");
    };
    // A send needs its mutation in `writes`.
    refuse(
        "action submit writes session",
        "action submit",
        "analyze-write-not-declared",
    );
    // `send` only to a mutation; `refresh` only a resource.
    refuse(
        "send session = login(who, password)",
        "send who = login(who, password)",
        "type-send-not-mutation",
    );
    refuse(
        "refresh balance",
        "refresh session",
        "type-refresh-not-resource",
    );
    // `pending` names a resource or a mutation.
    refuse("pending(session)", "pending(who)", "type-pending-argument");
}

/// A derive that matches a mutation into its record, and derives after it
/// that read that record's fields, type in whatever order they are written
/// (LLP 1018 §4's `current`): the fixpoint waits for `?` to fill.
#[test]
fn a_derive_over_a_matched_record_types_in_any_order() {
    let src = "shape Session\n  ok: bool\n  username: string\n\ncomponent App\n  derive signedIn = current.ok\n  derive who = current.username\n  resource remembered = remember() as shape Session\n  mutation session as shape Session\n  derive current = match session { case some(s) => s, case none => remembered }\n  action go writes session\n    send session = login()\n  view\n    text `${who} ${signedIn}` press=go\n";
    let plan = contract::compile(src).unwrap();
    assert_eq!(plan.derives.len(), 3);
    // And a field that never types is still refused, by name.
    let bad = src.replace("current.ok", "current.nope");
    let err = contract::compile(&bad).unwrap_err();
    assert_eq!(err.id, "type-unknown-field");
}
