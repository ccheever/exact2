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
