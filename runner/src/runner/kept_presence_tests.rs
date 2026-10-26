//! A presence secret in a session (LLP 1133.010 D5, D12): a launch that
//! begins locked shows no kept answer and holds no bytes, a locked write
//! goes to the backend and not the host's plain store, `get` fails the
//! answer that reads a locked name, and `unlock` answers on the store's own
//! path behind the agent's ticket, never for a tool.
use super::*;
use crate::presence::{Backend, MemoryVault, Occupancy, Vault};
use crate::{Answer, Dispatch, HoldAnswer, Outcome, Reply, Request, Store, Work};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{builder::PlanBuilder, Plan, TypeKind, Value};
use std::sync::{Arc, Mutex};

const GRANTS: &str = "secret.keep token user-presence optional\ndevice.local-auth purpose.lock";

#[derive(Default)]
struct Source {
    ready: bool,
}

impl DataSource for Source {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(&mut self, store: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        let who = store.get("token").map_or("signed out", |_| "signed in");
        Ok(Answer::Now(Value::str(who)))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        unreachable!("answers now")
    }

    fn ready(&self) -> bool {
        self.ready
    }

    fn grants(&self) -> &str {
        GRANTS
    }
}

fn plan() -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let mut a = exact_plan::asm::Asm::new();
    a.number(0.);
    let a = b.code(a);
    let session = b.resource(
        "session",
        "session",
        &[a],
        string,
        Some(&Value::str("signed out")),
    );
    b.set_resource_reader(session, true);
    b.set_resource_initial_args(session, &[Value::Number(0.)]);
    b.set_resource_placeholder_value(session, &Value::str("…"));
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

fn launch(ready: bool, snapshot: Vec<(String, Vec<u8>)>) -> Runner<Source> {
    Runner::boot_stored(
        plan(),
        Source { ready },
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap()
}

/// What a host's boot hands the runner: its plain store as `vault` holds
/// it, reconciled, with the occupancy and whether it keeps locked secrets.
fn reconciled(vault: &MemoryVault) -> (Backend, Vec<(String, Vec<u8>)>) {
    let lines = vec![(
        "token".to_owned(),
        crate::grants::parse(GRANTS)
            .unwrap()
            .presence()
            .next()
            .map(|(_, p)| p),
    )];
    let backend = Backend::new(Box::new(vault.clone()), lines);
    let mut snapshot: Vec<(String, Vec<u8>)> = vault.memory().plain.into_iter().collect();
    snapshot.push((
        kept::kept_name("session"),
        kept::encode(&[Value::Number(0.)], &Value::str("signed in, kept")),
    ));
    snapshot.extend(backend.snapshot_entries(true));
    (backend, snapshot)
}

fn holds_no(r: &Runner<Source>, secret: &str) {
    let carried = r.carry();
    let bytes = secret.as_bytes();
    let found = |v: &[u8]| v.windows(bytes.len()).any(|w| w == bytes);
    assert!(
        !carried.store.iter().any(|(_, v)| found(v)),
        "carried: {:?}",
        carried.store
    );
    assert!(!r.store.snapshot().iter().any(|(_, v)| found(v)));
    assert!(!r.store_names().iter().any(|n| n.contains(secret)));
    assert!(
        !r.journal().any(|l| l.contains(secret)),
        "the journal names no value"
    );
}

#[test]
fn a_launch_that_begins_locked_shows_no_kept_answer_and_holds_no_bytes() {
    let vault = MemoryVault::default();
    vault.clone().put_plain("token", Some(b"alice")).unwrap();
    // Unlocked, the kept answer shows before the source is ready.
    let (backend, snapshot) = reconciled(&vault);
    let mut open = launch(false, snapshot);
    open.install_presence(backend, None);
    assert_eq!(
        open.resource("session"),
        Some(&Value::str("signed in, kept"))
    );
    assert!(open.store_names().contains(&"token: plain".to_owned()));

    // Locked: the placeholder, and no bytes anywhere a value goes.
    let mut backend = Backend::new(
        Box::new(vault.clone()),
        vec![(
            "token".into(),
            crate::grants::parse(GRANTS)
                .unwrap()
                .presence()
                .next()
                .map(|(_, p)| p),
        )],
    );
    backend.enqueue("token", crate::presence::Op::Protect, Some(1));
    backend.pump(&crate::presence::Env {
        can_lock: true,
        moves: true,
        now_ms: 0.,
        faults: crate::presence::Faults::NONE,
    });
    assert_eq!(backend.occupancy("token"), Some(Occupancy::Locked));
    let (backend, snapshot) = reconciled(&vault);
    let mut r = launch(false, snapshot);
    r.install_presence(backend, None);
    assert_eq!(r.resource("session"), Some(&Value::str("…")));
    assert!(r.journal().any(|l| l.contains("kept answers not shown")));
    assert!(r
        .store_names()
        .contains(&"token: locked, user-presence".to_owned()));
    holds_no(&r, "alice");
}

#[test]
fn a_locked_write_goes_to_the_backend_and_a_get_of_it_fails_the_answer() {
    let vault = MemoryVault::default();
    let (backend, snapshot) = reconciled(&vault);
    let mut r = launch(true, snapshot);
    r.install_presence(backend, None);
    // An optional name starts plain: its write is the backend's, which keeps it plain.
    r.store.set("token", "alice").unwrap();
    assert!(
        r.take_store_writes().is_empty(),
        "no host write of a presence name"
    );
    assert_eq!(
        vault.memory().plain.get("token").map(Vec::as_slice),
        Some(&b"alice"[..])
    );
    let protect = Request::native(br#"{"presence":"protect","name":"token"}"#.to_vec());
    let outcome = run(&mut r, &protect);
    assert_eq!(body(&outcome), r#"{}"#);
    r.adopt_presence();
    assert!(r.store.locked("token"));
    // The rotated token is kept locked, and never in the store.
    r.store.set("token", "bob").unwrap();
    assert!(r.take_store_writes().is_empty());
    assert!(vault.memory().plain.is_empty(), "{:?}", vault.memory());
    holds_no(&r, "bob");
    let mut store = std::mem::take(&mut r.store);
    let answer = r.data.answer(&mut store, "session", &[]);
    r.store = store;
    let refused = r.refuse_locked(answer).unwrap_err();
    assert!(
        refused.to_string().contains("InvalidAccessError"),
        "{refused}"
    );
}

#[test]
fn unlock_answers_behind_the_agent_s_ticket_and_never_for_a_tool() {
    let vault = MemoryVault::default();
    let (backend, snapshot) = reconciled(&vault);
    let mut r = launch(true, snapshot);
    r.install_presence(backend, None);
    r.store.set("token", "alice").unwrap();
    r.take_store_writes();
    run(
        &mut r,
        &Request::native(br#"{"presence":"protect","name":"token"}"#.to_vec()),
    );
    r.adopt_presence();
    let unlock = Request::native(
        br#"{"presence":"unlock","name":"token","reason":"reason.unlock"}"#.to_vec(),
    );
    // A tool's call fails before anything is asked (D12).
    let refused = r.as_origin(super::modules::CallOrigin::Tool, |r| run(r, &unlock));
    assert!(
        body(&refused).contains("person-required"),
        "{}",
        body(&refused)
    );
    assert!(r.device_holds().is_empty());
    // The app's: a `presence` ticket, whose `allow` answers the value on this
    // call's reply alone.
    let (slot, reply) = pending();
    let Dispatch::Run(Work::Later(work)) = r.native_work(&unlock) else {
        panic!("a held call")
    };
    work(reply);
    let hold = r.device_holds()[0].clone();
    assert_eq!(hold.capability, "presence");
    assert!(
        hold.args.contains("\"reason\":\"reason.unlock\""),
        "{}",
        hold.args
    );
    assert!(slot.lock().unwrap().is_none());
    r.answer_hold(hold.ticket, &HoldAnswer::Choice("allow".into()))
        .unwrap();
    let outcome = slot.lock().unwrap().take().expect("answered");
    assert_eq!(body(&outcome), r#"{"value":"alice"}"#);
    holds_no(&r, "alice");
    assert!(r.store.locked("token"), "an unlock never removes the lock");
}

fn pending() -> (Arc<Mutex<Option<Outcome>>>, Reply) {
    let slot = Arc::new(Mutex::new(None));
    let to = slot.clone();
    (slot, Reply::new(move |o| *to.lock().unwrap() = Some(o)))
}

fn run(r: &mut Runner<Source>, request: &Request) -> Outcome {
    match r.native_work(request) {
        Dispatch::Run(Work::Now(work)) => work(),
        Dispatch::Run(Work::Later(work)) => {
            let (slot, reply) = pending();
            work(reply);
            let outcome = slot.lock().unwrap().take();
            outcome.expect("answered at once")
        }
        _ => panic!("a presence call is the runner's"),
    }
}

fn body(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Response(r) => String::from_utf8_lossy(&r.body).into_owned(),
        other => panic!("{other:?}"),
    }
}

fn unavailable_presence(
    occupancy: Occupancy,
) -> (
    Runner<Source>,
    MemoryVault,
    Arc<Mutex<Option<&'static str>>>,
) {
    let mut memory = MemoryVault::default();
    let policy = crate::grants::parse(GRANTS)
        .unwrap()
        .presence()
        .next()
        .unwrap()
        .1;
    match occupancy {
        Occupancy::Plain => memory.put_plain("token", Some(b"alice")).unwrap(),
        Occupancy::Locked => {
            memory.add_locked("token", 1, &policy, b"alice").unwrap();
            memory
                .put_record("token", Some(b"locked user-presence optional"))
                .unwrap();
        }
        _ => unreachable!(),
    }
    let failure = Arc::new(Mutex::new(Some("generations")));
    let vault = crate::presence::tests::UnavailableVault {
        memory: memory.clone(),
        failure: failure.clone(),
    };
    let backend = Backend::new(Box::new(vault), vec![("token".into(), Some(policy))]);
    let mut plan = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 0);
    plan.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let mut runner = Runner::boot_stored(
        plan.finish().unwrap(),
        Source { ready: true },
        Kernel::with_monospace(),
        backend.snapshot_entries(true),
        Default::default(),
        "/",
    )
    .unwrap();
    runner.install_presence(backend, None);
    assert_eq!(runner.store.occupancy("token"), Some(Occupancy::Pending));
    (runner, memory, failure)
}

#[test]
fn presence_retry_deadline_finishes_an_idle_unlock_without_another_request() {
    for repeated_failure in [false, true] {
        let (mut r, vault, failure) = unavailable_presence(Occupancy::Locked);
        assert!(!r.has_timers(), "no authored timer can rescue this request");
        let unlock = Request::native(br#"{"presence":"unlock","name":"token"}"#.to_vec());
        let (slot, reply) = pending();
        let Dispatch::Run(Work::Later(work)) = r.native_work(&unlock) else {
            panic!("the unlock waits for reconciliation")
        };
        work(reply);
        assert!(r.device_holds().is_empty());
        let mut due = r
            .timer_due_ms()
            .expect("the pending probe must wake its host");
        assert_eq!(due, 1000.0);
        r.advance(due - 1.0).unwrap();
        assert!(r.device_holds().is_empty());
        if repeated_failure {
            r.advance(due).unwrap();
            assert!(r.device_holds().is_empty());
            let next = r
                .timer_due_ms()
                .expect("a failed retry must schedule another");
            assert!(next > due);
            due = next;
        }
        *failure.lock().unwrap() = None;
        r.advance(due).unwrap();
        assert_eq!(r.store.occupancy("token"), Some(Occupancy::Locked));
        assert_eq!(r.timer_due_ms(), None);
        assert_eq!(r.device_holds().len(), 1);
        let hold = r.device_holds()[0].clone();
        assert!(slot.lock().unwrap().is_none());
        r.answer_hold(hold.ticket, &HoldAnswer::Choice("allow".into()))
            .unwrap();
        assert_eq!(
            body(&slot.lock().unwrap().take().unwrap()),
            r#"{"value":"alice"}"#
        );
        assert_eq!(
            vault.memory().locked.values().collect::<Vec<_>>(),
            [&b"alice".to_vec()]
        );
        holds_no(&r, "alice");
    }
}

#[test]
fn presence_retry_deadline_publishes_recovered_occupancy_without_a_call() {
    for occupancy in [Occupancy::Plain, Occupancy::Locked] {
        let (mut r, _, failure) = unavailable_presence(occupancy);
        let due = r
            .timer_due_ms()
            .expect("reconciliation must wake an idle host");
        *failure.lock().unwrap() = None;
        let receipts = r.advance(due).unwrap();
        assert_eq!(receipts.len(), 1, "the recovered occupancy is committed");
        assert_eq!(r.store.occupancy("token"), Some(occupancy));
        assert_eq!(r.timer_due_ms(), None);
        assert!(
            r.device_holds().is_empty(),
            "reconciliation never authenticates"
        );
        if occupancy == Occupancy::Plain {
            assert_eq!(r.store.get("token"), Some("alice"));
        } else {
            holds_no(&r, "alice");
        }
    }
}

#[test]
fn presence_retry_deadline_retries_a_failed_forget_without_another_request() {
    let (mut r, vault, failure) = unavailable_presence(Occupancy::Locked);
    *failure.lock().unwrap() = None;
    r.advance(r.timer_due_ms().unwrap()).unwrap();
    *failure.lock().unwrap() = Some("delete");
    r.store.forget("token").unwrap();
    assert!(r.take_store_writes().is_empty());
    let due = r
        .timer_due_ms()
        .expect("a failed forget must wake its host");
    assert!(due > r.now_ms());
    r.advance(due).unwrap();
    assert!(!vault.memory().locked.is_empty());
    assert!(!vault.memory().records.is_empty());
    let next = r.timer_due_ms().expect("the queued forget retains a retry");
    assert!(
        next > due,
        "failed deletion must not cause an overdue wake loop"
    );
    *failure.lock().unwrap() = None;
    r.advance(next).unwrap();
    assert!(vault.memory().locked.is_empty());
    assert!(vault.memory().records.is_empty());
    assert_eq!(r.store.occupancy("token"), Some(Occupancy::Absent));
    assert_eq!(r.timer_due_ms(), None);
    assert!(r.device_holds().is_empty());
}

#[test]
fn presence_retry_deadline_waits_for_an_authenticated_read_before_replacing_its_item() {
    let (mut r, vault, failure) = unavailable_presence(Occupancy::Locked);
    *failure.lock().unwrap() = None;
    r.advance(r.timer_due_ms().unwrap()).unwrap();
    *failure.lock().unwrap() = Some("add");
    r.store.set("token", "bob").unwrap();
    assert!(r.take_store_writes().is_empty());
    let due = r
        .timer_due_ms()
        .expect("the refused write must wake its host");
    *failure.lock().unwrap() = None;
    let unlock = Request::native(br#"{"presence":"unlock","name":"token"}"#.to_vec());
    let (slot, reply) = pending();
    let Dispatch::Run(Work::Later(work)) = r.native_work(&unlock) else {
        panic!("the check waits for the person")
    };
    work(reply);
    let hold = r.device_holds()[0].clone();
    assert_eq!(
        r.timer_due_ms(),
        None,
        "a pending check suspends the write retry"
    );
    r.advance(due).unwrap();
    assert_eq!(
        vault.memory().locked.values().collect::<Vec<_>>(),
        [&b"alice".to_vec()]
    );
    assert!(slot.lock().unwrap().is_none());
    r.answer_hold(hold.ticket, &HoldAnswer::Choice("allow".into()))
        .unwrap();
    assert_eq!(
        body(&slot.lock().unwrap().take().unwrap()),
        r#"{"value":"alice"}"#
    );
    assert_eq!(
        r.timer_due_ms(),
        Some(due),
        "the finished read releases the retry"
    );
    r.advance(due).unwrap();
    assert_eq!(
        vault.memory().locked.values().collect::<Vec<_>>(),
        [&b"bob".to_vec()]
    );
    assert_eq!(r.timer_due_ms(), None);
    holds_no(&r, "bob");
}

#[test]
fn presence_retry_deadline_preserves_the_virtual_request_stop_when_recovery_asks_data() {
    struct RequestOnRecovery;
    impl DataSource for RequestOnRecovery {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!("the fixture implements answer")
        }
        fn answer(&mut self, store: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            Ok(if store.locked("token") {
                Answer::Now(Value::str("locked"))
            } else {
                Answer::Later(Request::get("https://example.test/recovered"))
            })
        }
        fn ready(&self) -> bool {
            true
        }
        fn grants(&self) -> &str {
            "secret.keep token user-presence optional\ndevice.local-auth purpose.lock\nnet.fetch https://example.test"
        }
    }
    let mut memory = MemoryVault::default();
    memory.put_plain("token", Some(b"alice")).unwrap();
    let failure = Arc::new(Mutex::new(Some("generations")));
    let vault = crate::presence::tests::UnavailableVault {
        memory,
        failure: failure.clone(),
    };
    let policy = crate::grants::parse(GRANTS)
        .unwrap()
        .presence()
        .next()
        .unwrap()
        .1;
    let backend = Backend::new(Box::new(vault), vec![("token".into(), Some(policy))]);
    let mut r = Runner::boot_stored(
        plan(),
        RequestOnRecovery,
        Kernel::with_monospace(),
        backend.snapshot_entries(true),
        Default::default(),
        "/",
    )
    .unwrap();
    r.install_presence(backend, None);
    assert!(r.take_requests().is_empty());
    let due = r.timer_due_ms().unwrap();
    *failure.lock().unwrap() = None;
    let advanced = r.advance_until_request(due);
    assert!(advanced.error.is_none(), "{:?}", advanced.error);
    assert!(advanced.stopped_on_request);
    assert_eq!(advanced.now_ms, due);
    assert_eq!(advanced.receipts.len(), 1);
    assert_eq!(r.take_requests().len(), 1);
    assert_eq!(r.store.occupancy("token"), Some(Occupancy::Plain));
    assert_eq!(r.timer_due_ms(), None);
}
