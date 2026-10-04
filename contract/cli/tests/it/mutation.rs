//! LLP 1016 end to end through the compiler: a `mutation` slot filled by
//! `send`, a request the host runs and brings back through `fulfill`,
//! `pending(x)` in the view, `refresh` on a resource, and what bake refuses.

use exact_kernel::{Kernel, PropId};
use exact_runner::{
    Answer, DataError, DataSource, Event, FailureKind, Outcome, Request, Response, Runner,
    RunnerError, Value,
};

const WITH_CONTEXT: &str = r#"component App
  state car = "train"
  state minute = 10
  state display = true
  derive sample = minute + 1
  resource status = status(car) with sample, display as shape string
  resource plain = status(car, sample, display) as shape string
  resource singleton = singleton() with sample as shape number
  action tick(value: number)
    minute = value
  action toggle
    display = not display
  action choose(value: string)
    car = value
  view
    text status
"#;

#[test]
fn resource_context_lowers_types_and_evaluates_in_full_argument_order() {
    use exact_plan::{Plan, TypeKind};
    #[derive(Default)]
    struct ContextData(Vec<(String, Vec<Value>)>);
    impl DataSource for ContextData {
        fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            self.0.push((source.into(), args.to_vec()));
            match (source, args) {
                ("status", [car, Value::Number(minute), Value::Bool(display)])
                    if car.as_str().is_some() =>
                {
                    Ok(Value::str(&format!(
                        "{}/{minute}/{display}",
                        car.as_str().unwrap()
                    )))
                }
                ("singleton", [Value::Number(minute)]) => Ok(Value::Number(*minute)),
                _ => Err(DataError::Unavailable("wrong argument order".into())),
            }
        }
    }
    let plan = contract::compile(WITH_CONTEXT).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    assert_eq!(
        plan.resources
            .iter()
            .map(|r| (r.args.len, r.context))
            .collect::<Vec<_>>(),
        [(3, 2), (3, 0), (1, 1)]
    );
    let signature = plan
        .sources
        .iter()
        .find(|r| plan.str(r.name) == "status")
        .unwrap();
    let params: Vec<_> = signature
        .params
        .iter()
        .map(|p| plan.source_param(p).ty)
        .collect();
    assert_eq!(
        params
            .iter()
            .map(|ty| plan.type_(*ty).kind)
            .collect::<Vec<_>>(),
        [TypeKind::String, TypeKind::Number, TypeKind::Bool]
    );
    let typescript = contract::typescript(&plan).unwrap();
    assert!(typescript.contains(&format!(
        "\"status\": {{ args: [T{}, T{}, T{}]; result:",
        params[0].0, params[1].0, params[2].0
    )));
    let mut r = Runner::boot(
        plan,
        ContextData::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let expected = |minute, display| {
        vec![
            Value::str("train"),
            Value::Number(minute),
            Value::Bool(display),
        ]
    };
    assert_eq!(
        r.data().0,
        [
            ("status".into(), expected(11.0, true)),
            ("status".into(), expected(11.0, true)),
            ("singleton".into(), vec![Value::Number(11.0)]),
        ]
    );
    r.act("tick", vec![Value::Number(10.0)]).unwrap();
    assert_eq!(r.data().0.len(), 3, "equal full arguments reuse the answer");
    r.act("tick", vec![Value::Number(20.0)]).unwrap();
    assert_eq!(
        &r.data().0[3..],
        [
            ("status".into(), expected(21.0, true)),
            ("status".into(), expected(21.0, true)),
            ("singleton".into(), vec![Value::Number(21.0)]),
        ]
    );
    r.act("toggle", vec![]).unwrap();
    assert_eq!(
        &r.data().0[6..],
        [
            ("status".into(), expected(21.0, false)),
            ("status".into(), expected(21.0, false)),
        ]
    );
    assert_eq!(
        r.resource_args("status"),
        Some(expected(21.0, false).as_slice())
    );
}

#[test]
fn resource_context_participates_in_source_signature_checks() {
    for replacement in ["with sample", "with sample, car"] {
        let error = contract::compile(&WITH_CONTEXT.replace("with sample, display", replacement))
            .unwrap_err();
        assert_eq!(error.id, "type-source-signature", "{error}");
    }
}

#[test]
fn resource_context_count_refuses_overflow() {
    let values = vec!["0"; usize::from(u16::MAX) + 1].join(",");
    let source = format!(
        "component App\n  resource result = status() with {values} as shape number\n  view\n    text result\n"
    );
    assert_eq!(
        contract::compile(&source).unwrap_err().id,
        "lower-resource-context"
    );
}

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

  action setWho(v)
    who = v
  action setPassword(v)
    password = v
  action submit
    send session = login(who, password)
  action logout
    send session = logout(token)
    session = none
  action paid
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
    fail_parse: bool,
    fail_empty_balance: bool,
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
                if self.fail_empty_balance && args[0].as_str() == Some("") {
                    return Err(DataError::Unavailable("empty balance refused".into()));
                }
                let n = if args[0].as_str() == Some("") {
                    0.0
                } else {
                    42.0
                };
                Ok(Value::record(vec![Value::Number(n)]))
            }
            "login" | "logout" => Err(DataError::Unavailable(format!("{source} answers later"))),
            "greet" => Ok(session(true, args[0].as_str().unwrap_or(""), "")),
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
    ) -> Result<Answer, DataError> {
        if self.fail_parse && source == "login" {
            return Err(DataError::Unavailable("parse refused".into()));
        }
        Ok(Answer::Now(match (source, outcome) {
            ("login", Outcome::Response(r)) if r.status == 200 => {
                session(true, args[0].as_str().unwrap_or(""), "")
            }
            ("login", Outcome::Response(r)) => session(false, "", &format!("HTTP {}", r.status)),
            ("login", Outcome::Failed { message, .. }) => session(false, "", &message),
            ("logout", _) => session(false, "", ""),
            (other, _) => return Err(DataError::UnknownSource(other.into())),
        }))
    }
}

fn text_of<D: DataSource>(r: &Runner<D>, test_id: &str) -> Option<String> {
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
        Default::default(),
        "/",
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
        Default::default(),
        "/",
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

/// No host delivers a ticket twice, so a reply the source fails to take is
/// let go: the mutation ends unsent and the view stops showing it pending.
#[test]
fn a_failed_fulfill_ends_the_mutation_unsent() {
    let mut r = boot();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let ticket = r.take_requests()[0].ticket;
    assert!(has(&r, "busy"));
    r.data().fail_parse = true;

    assert!(r.fulfill(ticket, ok(200)).unwrap().is_some());
    assert!(r.pending().is_empty());
    assert!(!has(&r, "busy"), "the view shows it no longer pending");
    assert!(r.journal().any(|l| l.contains("parse refused")));
    assert!(r
        .journal()
        .any(|l| l.contains("failed and is no longer pending: it ends unsent")));
    assert!(
        r.fulfill(ticket, ok(200)).unwrap().is_none(),
        "a released ticket commits nothing"
    );
}

#[test]
fn a_mutation_refused_admission_ends_unsent_and_is_never_retried() {
    let mut r = boot();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let ticket = r.take_requests()[0].ticket;
    assert!(has(&r, "busy"));
    r.data().fail_parse = true;
    r.refuse_request(ticket, "native executor admission limit reached", true);
    let (refused, outcome) = r.take_request_refusal(true).unwrap();
    assert_eq!(refused, ticket);
    assert!(r.fulfill(ticket, outcome).unwrap().is_some());
    assert!(r.pending().is_empty());
    assert!(!has(&r, "busy"), "the view shows it no longer pending");
    assert!(r.take_requests().is_empty(), "a write is never retried");
    assert!(r
        .journal()
        .any(|l| l.contains("was refused admission: it ends unsent")));
}

#[test]
fn a_refused_mutation_assignment_keeps_the_previous_ticket() {
    let mut r = boot();
    r.dispatch(view_of(&r, "who"), Event::Change("ada".into()))
        .unwrap();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let first = r.take_requests()[0].ticket;
    r.fulfill(first, ok(200)).unwrap();

    // A second login is in flight while the prior successful value remains
    // in the mutation slot. Logout tentatively assigns `none`, which changes
    // balance's arguments and makes settlement refuse.
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let wanted = r.take_requests()[0].ticket;
    r.data().fail_empty_balance = true;
    assert!(matches!(
        r.dispatch(view_of(&r, "logout"), Event::Press),
        Err(RunnerError::Data {
            error: DataError::Unavailable(ref message),
            ..
        }) if message == "empty balance refused"
    ));
    assert!(matches!(r.slot("session"), Some(Value::Option(Some(_)))));
    assert_eq!(r.pending(), vec![("session".to_string(), wanted)]);
    assert!(
        r.take_requests().is_empty(),
        "the refused logout sent nothing"
    );

    r.data().fail_empty_balance = false;
    assert!(r.fulfill(wanted, ok(200)).unwrap().is_some());
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
fn every_reply_is_journaled_with_what_came_back() {
    let mut r = boot();
    r.dispatch(view_of(&r, "who"), Event::Change("ada".into()))
        .unwrap();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let first = r.take_requests()[0].ticket;
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let second = r.take_requests()[0].ticket;
    assert_eq!(r.fulfill(first, ok(200)).unwrap(), None);
    // A failure the app shapes into a value is still on the record.
    let refused = Outcome::Failed {
        kind: FailureKind::Refused,
        message: "the app declares no grants".into(),
    };
    let _ = r.fulfill(second, refused);
    let journal: Vec<_> = r.journal().collect();
    let dropped = format!("reply {first} dropped: no such request in flight [HTTP 200, 2 bytes]");
    let failed = format!("fulfil {second} (session) [refused: the app declares no grants]");
    assert!(journal.iter().any(|l| l.contains(&dropped)), "{journal:#?}");
    assert!(journal.iter().any(|l| l.contains(&failed)), "{journal:#?}");
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
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(has(&again, "signed-in"));
    assert!(again.take_requests().is_empty(), "nothing was re-sent");
    assert!(again.pending().is_empty());
}

#[test]
fn a_record_that_answers_later_at_the_bake_is_not_compiled_and_shows_its_zero() {
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
    // @ref LLP 1054.000.002 D1/D4 — where the bake used to refuse: the
    // resource shows its zero, pending, and the zero is not its answer.
    let plan = contract::bake(contract::compile(&src).unwrap(), Remote).unwrap();
    let row = plan
        .resources
        .iter()
        .find(|r| plan.str(r.name) == "balance")
        .unwrap();
    assert_eq!(row.initial.len, 0);
    let mut r = Runner::boot(
        plan,
        Remote,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&r, "bricks").as_deref(), Some("0 bricks"));
    assert!(r.take_requests().iter().any(|q| q.target == "balance"));
}

#[test]
fn the_language_refuses_what_it_should() {
    let refuse = |edit: &str, with: &str, id: &str| {
        let e = contract::compile(&SRC.replace(edit, with)).unwrap_err();
        assert!(format!("{e}").contains(id), "{e}");
    };
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
    let src = "shape Session\n  ok: bool\n  username: string\n\ncomponent App\n  derive signedIn = current.ok\n  derive who = current.username\n  resource remembered = remember() as shape Session\n  mutation session as shape Session\n  derive current = match session { case some(s) => s, case none => remembered }\n  action go\n    send session = login()\n  view\n    text `${who} ${signedIn}` press=go\n";
    let plan = contract::compile(src).unwrap();
    assert_eq!(plan.derives.len(), 3);
    // And a field that never types is still refused, by name.
    let bad = src.replace("current.ok", "current.nope");
    let err = contract::compile(&bad).unwrap_err();
    assert_eq!(err.id, "type-unknown-field");
}

const POISON_SRC: &str = r#"
shape Item
  id: string

shape Effect
  ok: bool

component App
  state trigger = 0
  resource items = items(trigger) as shape list<Item>
  mutation effect as shape Effect

  action go
    send effect = effect()
    trigger = trigger + 1
    setScheme("dark")

  view
    column testId="root"
      each item in items key=item.id
        text item.id + item.id
"#;

struct PoisonSource;

impl DataSource for PoisonSource {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "items" if args[0].as_number() == Some(0.0) => {
                Ok(Value::list(vec![Value::record(vec![Value::str("one")])]))
            }
            // Doubled by the view, past the runner's longest string: a trap
            // while the tree changes, the way data can still poison it.
            "items" => Ok(Value::list(vec![Value::record(vec![Value::str(
                &"x".repeat((exact_runner::vm::MAX_STRING / 2) + 1),
            )])])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "effect" => {
                store.set("token", "not-committed")?;
                Ok(Answer::Later(Request::post_json(
                    "https://api.castle.test/effect",
                    "{}",
                )))
            }
            _ => self.query(source, args).map(Answer::Now),
        }
    }

    fn grants(&self) -> &'static str {
        "secret.keep token\n"
    }
}

#[test]
fn poison_leaks_no_effects_from_the_failed_commit() {
    let plan = contract::compile(POISON_SRC).unwrap();
    let baked = contract::bake(plan, PoisonSource).unwrap();
    let mut r = Runner::boot(
        baked,
        PoisonSource,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();

    assert!(matches!(
        r.act("go", vec![]),
        Err(RunnerError::Instance(
            exact_runner::instance::InstanceError::Trap(exact_runner::Trap::StringTooLong { .. })
        ))
    ));
    assert!(r.is_poisoned());
    assert!(r.take_commands().is_empty());
    assert!(r.take_requests().is_empty());
    assert!(r.pending().is_empty());
    assert!(r.take_store_writes().is_empty());
    assert!(r.store_names().is_empty());
}

const STORE_SRC: &str = r#"
shape Session
  username: string

shape Flag
  value: bool

component App
  state clear = false
  resource remembered = remember() as shape Session
  resource echoed = echo(remembered.username) as shape Session
  resource writer = writer(clear) as shape Flag
  mutation session as shape Session

  action submit
    send session = login()
  action logout
    send session = logout()
  action clearStore
    clear = true

  view
    column
      text remembered.username testId="remembered"
      text echoed.username testId="echoed"
"#;

#[derive(Default)]
struct StoreSource {
    remember_asks: usize,
    login_later: bool,
}

impl DataSource for StoreSource {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "remember" => {
                self.remember_asks += 1;
                Ok(Answer::Now(Value::record(vec![Value::str(
                    store.get("session").unwrap_or(""),
                )])))
            }
            "echo" => Ok(Answer::Now(Value::record(vec![args[0].clone()]))),
            "login" if self.login_later => Ok(Answer::Later(Request::post_json(
                "https://api.castle.test/login",
                "{}",
            ))),
            "logout" if self.login_later => {
                store.forget("session")?;
                Ok(Answer::Later(Request::post_json(
                    "https://api.castle.test/logout",
                    "{}",
                )))
            }
            "writer" if args[0].as_bool() == Some(false) => {
                Ok(Answer::Now(Value::record(vec![Value::Bool(false)])))
            }
            "writer" => {
                store.forget("session")?;
                Ok(Answer::Now(Value::record(vec![Value::Bool(true)])))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn parse(
        &mut self,
        store: &mut exact_runner::Store,
        source: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        if source != "login" {
            return Err(DataError::UnknownSource(source.into()));
        }
        store.set("session", "ada")?;
        Ok(Answer::Now(Value::record(vec![Value::str("ada")])))
    }

    fn grants(&self) -> &'static str {
        "secret.keep session\n"
    }
}

#[test]
fn a_parse_store_write_reanswers_store_reading_resources() {
    let plan = contract::compile(STORE_SRC).unwrap();
    let baked = contract::bake(plan, StoreSource::default()).unwrap();
    let mut r = Runner::boot_stored(
        baked,
        StoreSource {
            login_later: true,
            ..StoreSource::default()
        },
        Kernel::with_monospace(),
        vec![],
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.data().remember_asks, 1);
    assert_eq!(
        r.resource("remembered"),
        Some(&Value::record(vec![Value::str("")]))
    );

    r.act("submit", vec![]).unwrap();
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(ticket, ok(200)).unwrap();

    assert_eq!(r.data().remember_asks, 2);
    assert_eq!(
        r.resource("remembered"),
        Some(&Value::record(vec![Value::str("ada")]))
    );
    assert_eq!(
        r.take_store_writes(),
        vec![exact_runner::StoreWrite {
            name: "session".into(),
            value: Some("ada".into()),
        }]
    );
}

#[test]
fn a_mutation_answer_store_write_reanswers_store_reading_resources() {
    let plan = contract::compile(STORE_SRC).unwrap();
    let baked = contract::bake(plan, StoreSource::default()).unwrap();
    let mut r = Runner::boot_stored(
        baked,
        StoreSource {
            login_later: true,
            ..StoreSource::default()
        },
        Kernel::with_monospace(),
        vec![("session".into(), "ada".into())],
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(
        r.resource("remembered"),
        Some(&Value::record(vec![Value::str("ada")]))
    );

    r.act("logout", vec![]).unwrap();

    assert_eq!(r.data().remember_asks, 2);
    assert_eq!(
        r.resource("remembered"),
        Some(&Value::record(vec![Value::str("")]))
    );
    assert_eq!(r.take_requests().len(), 1);
    assert_eq!(r.take_store_writes()[0].value, None);
}

#[test]
fn a_resource_answer_store_write_reanswers_an_earlier_store_reader() {
    let plan = contract::compile(STORE_SRC).unwrap();
    let baked = contract::bake(plan, StoreSource::default()).unwrap();
    let mut r = Runner::boot_stored(
        baked,
        StoreSource {
            login_later: true,
            ..StoreSource::default()
        },
        Kernel::with_monospace(),
        vec![("session".into(), "ada".into())],
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(
        r.resource("remembered"),
        Some(&Value::record(vec![Value::str("ada")]))
    );

    r.act("clearStore", vec![]).unwrap();

    assert_eq!(r.data().remember_asks, 2);
    assert_eq!(
        r.resource("remembered"),
        Some(&Value::record(vec![Value::str("")]))
    );
    assert_eq!(r.take_store_writes()[0].value, None);
}

#[test]
fn a_baked_store_dependency_is_transitive_through_resource_arguments() {
    let plan = contract::compile(STORE_SRC).unwrap();
    let baked = contract::bake(plan, StoreSource::default()).unwrap();
    for name in ["remembered", "echoed"] {
        let row = baked
            .resources
            .iter()
            .find(|row| baked.str(row.name) == name)
            .unwrap();
        // LLP 1027 D4 (2026-09-03): a reader is compiled — the empty-store
        // answer, as the placeholder for a source not ready at boot — and
        // marked, so the device answers it whenever the source is ready.
        assert!(row.reader, "{name} reads the store");
        assert!(
            row.initial.len > 0,
            "{name} has its empty-store placeholder"
        );
    }

    let r = Runner::boot_stored(
        baked,
        StoreSource::default(),
        Kernel::with_monospace(),
        vec![("session".into(), "ada".into())],
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(
        r.resource("echoed"),
        Some(&Value::record(vec![Value::str("ada")]))
    );
}

/// LLP 1016.001: `then` runs a named action after each answer lands, as a
/// commit of its own that reads the answer from the mutation's slot.
const THEN: &str = r#"
shape Session
  ok: bool
  username: string
  error: string

component App
  state who = "ada"
  state greeted = ""
  state landings = 0
  mutation session as shape Session then signedIn
  action submit
    send session = login(who, "pw")
  action quick
    send session = greet(who)
  action signedIn
    landings = landings + 1
    match session
      case some(s)
        greeted = s.ok ? `hello ${s.username}` : s.error
      case none
        greeted = "?"
  view
    column testId="app"
      button press=submit aria-label="Log in" testId="login"
        text "Log in"
      button press=quick aria-label="Greet" testId="quick"
        text "Greet"
      text `${greeted}/${landings}` testId="greeted"
"#;

fn boot_then(later: bool) -> Runner<Castle> {
    let baked = contract::bake(contract::compile(THEN).unwrap(), Castle::default()).unwrap();
    Runner::boot(
        baked,
        Castle {
            later,
            ..Castle::default()
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn then_runs_after_a_later_answer_lands_as_its_own_commit() {
    let mut r = boot_then(true);
    assert_eq!(r.timer_due_ms(), None, "nothing is armed before an answer");
    r.advance(10.0).unwrap();
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let t = r.take_requests()[0].ticket;
    assert_eq!(r.timer_due_ms(), None, "a request is not an answer");
    r.fulfill(t, ok(200)).unwrap().unwrap();
    // The answer's commit shows the answer and nothing it caused.
    assert_eq!(text_of(&r, "greeted").as_deref(), Some("/0"));
    // It is due at once; the host's next advance runs it.
    assert_eq!(r.timer_due_ms(), Some(10.0));
    let commits = r.advance(10.0).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(text_of(&r, "greeted").as_deref(), Some("hello ada/1"));
    assert_eq!(r.timer_due_ms(), None, "spent until the next answer");
    assert!(r.advance(20.0).unwrap().is_empty());
    // A failure the source shapes is an answer too.
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let t = r.take_requests()[0].ticket;
    r.fulfill(
        t,
        Outcome::Failed {
            kind: FailureKind::Network,
            message: "no route".into(),
        },
    )
    .unwrap();
    r.advance(20.0).unwrap();
    assert_eq!(text_of(&r, "greeted").as_deref(), Some("no route/2"));
}

#[test]
fn then_runs_after_an_answer_in_the_sending_commit_and_not_after_a_failure() {
    // `greet` answers now: the slot fills in the send's own commit, and the
    // `then` follows as the next one.
    let mut r = boot_then(false);
    r.dispatch(view_of(&r, "quick"), Event::Press).unwrap();
    assert_eq!(text_of(&r, "greeted").as_deref(), Some("/0"));
    assert_eq!(r.timer_due_ms(), Some(0.0));
    assert_eq!(r.advance(0.0).unwrap().len(), 1);
    assert_eq!(text_of(&r, "greeted").as_deref(), Some("hello ada/1"));
    // A refused send (`login` cannot answer now here) arms nothing.
    assert!(r.dispatch(view_of(&r, "login"), Event::Press).is_err());
    assert_eq!(r.timer_due_ms(), None, "a refused commit arms nothing");
    // A reply the source cannot parse is let go, not an answer.
    let mut r = boot_then(true);
    r.data().fail_parse = true;
    r.dispatch(view_of(&r, "login"), Event::Press).unwrap();
    let t = r.take_requests()[0].ticket;
    let _ = r.fulfill(t, ok(200));
    assert_eq!(r.timer_due_ms(), None);
    assert_eq!(text_of(&r, "greeted").as_deref(), Some("/0"));
}

#[test]
fn then_names_an_action_that_takes_nothing() {
    let refuse = |edit: &str, with: &str, id: &str| {
        let e = contract::compile(&THEN.replace(edit, with)).unwrap_err();
        assert!(format!("{e}").contains(id), "{e}");
    };
    refuse("then signedIn", "then nobody", "analyze-unknown-action");
    refuse(
        "action signedIn\n",
        "action signedIn(x: number)\n",
        "analyze-handler-arity",
    );
}

#[test]
fn then_cannot_send_its_own_mutation_even_in_a_branch() {
    for body in [
        "    send session = greet(who)",
        "    if who == \"ada\"\n      send session = greet(who)",
        "    match session\n      case some(s)\n        send session = greet(s.username)\n      case none\n        greeted = \"?\"",
    ] {
        let src = THEN.replace("    landings = landings + 1", body);
        let error = contract::compile(&src).unwrap_err();
        assert_eq!(error.id, "analyze-then-self-send");
    }
    // Clearing the answer is allowed; only sending can arm the action again.
    let src = THEN.replace("    landings = landings + 1", "    session = none");
    contract::compile(&src).unwrap();
}

#[test]
fn two_answers_before_advance_currently_coalesce_into_one_then() {
    let mut r = boot_then(false);
    r.act("quick", vec![]).unwrap();
    r.act("quick", vec![]).unwrap();
    assert_eq!(r.advance(0.0).unwrap().len(), 1);
    assert_eq!(text_of(&r, "greeted").as_deref(), Some("hello ada/1"));
    assert_eq!(r.timer_due_ms(), None);
}

/// LLP 1085 D8 (flashcards F4): two sends to one mutation on one path are
/// refused — the second forgets the first's reply (LLP 1016 D5). Exclusive
/// arms pass, and so do sequential `if`s that test one unchanged name
/// against different literals; an arm's send and another in the common
/// suffix do not.
#[test]
fn two_sends_to_one_mutation_on_one_path_are_refused() {
    let app = |body: &str| {
        format!("shape Ack\n  op: string\ncomponent App\n  state open = true\n  state mode = \"a\"\n  mutation edited as shape Ack then afterEdit\n  action afterEdit\n    open = false\n  action commitEdit(k: string)\n{body}  view\n    button \"s\" press=commitEdit(\"1\")\n")
    };
    let message = "`commitEdit` sends `edited` twice; only the last send's reply reaches `then afterEdit` (LLP 1016 D5). Send once, or use a mutation per request";
    for body in [
        // Flashcards' `commitEdit`.
        "    send edited = saveCard(\"a\")\n    send edited = setImage(\"b\")\n",
        // An arm, then the common suffix.
        "    if k == \"1\"\n      send edited = setImage(\"b\")\n    send edited = saveCard(\"a\")\n",
        // A `match` arm, then the suffix.
        "    match some(k)\n      case some(x)\n        send edited = setImage(x)\n      case none\n        open = true\n    send edited = saveCard(\"a\")\n",
        // The tested name changes between the two tests.
        "    if mode == \"a\"\n      send edited = saveCard(\"a\")\n      mode = \"b\"\n    if mode == \"b\"\n      send edited = setImage(\"b\")\n",
        // Two tests that can both hold.
        "    if k == \"1\"\n      send edited = saveCard(\"a\")\n    if open\n      send edited = setImage(\"b\")\n",
    ] {
        let e = contract::compile(&app(body)).unwrap_err();
        assert_eq!(
            (e.id.as_str(), e.message.as_str()),
            ("analyze-send-twice", message),
            "{body}"
        );
        assert_eq!(e.related.len(), 1, "{body}");
    }
    for body in [
        "    if k == \"1\"\n      send edited = setImage(\"b\")\n    else\n      send edited = saveCard(\"a\")\n",
        "    match some(k)\n      case some(x)\n        send edited = setImage(x)\n      case none\n        send edited = saveCard(\"a\")\n",
        // A key handler's sequential, mutually exclusive tests.
        "    if k == \"1\" and open\n      send edited = saveCard(\"a\")\n    if k == \"2\" or k == \"3\"\n      send edited = setImage(\"b\")\n    mode = \"c\"\n",
        "    send edited = saveCard(\"a\")\n",
    ] {
        let plan = contract::compile(&app(body));
        assert!(plan.is_ok(), "{body}\n{plan:?}");
    }
}
