//! A store reader's build-time answer at a store holding secrets (LLP 1027
//! D4, approved 2026-10-10): the bake computed it from an empty store, so
//! before the source is ready the declared placeholder shows instead.
use super::*;
use crate::{Answer, Outcome, Store};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{builder::PlanBuilder, Plan, TypeKind, Value};

#[derive(Default)]
struct Source {
    ready: bool,
}

impl DataSource for Source {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(&mut self, store: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        assert!(
            self.ready,
            "a deferred source must not run before readiness"
        );
        let who = store
            .get("token")
            .map_or("signed out".to_string(), |t| format!("signed in as {t}"));
        Ok(Answer::Now(Value::str(&who)))
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
        "secret.keep token"
    }
}

/// `session(0)`, a store reader whose build-time answer is "signed out",
/// with `else "…"` when `declared`.
fn plan(declared: bool) -> Plan {
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
    if declared {
        b.set_resource_placeholder_value(session, &Value::str("…"));
    }
    let number = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.));
    let gen = b.slot("gen", number, zero);
    let mut body = exact_plan::asm::Asm::new();
    body.load_param(0).store_slot(gen);
    let body = b.code(body);
    b.action("gen", &[("value", number)], &[gen], body);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

fn launch(declared: bool, snapshot: Vec<(String, Vec<u8>)>) -> Runner<Source> {
    Runner::boot_stored(
        plan(declared),
        Source::default(),
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap()
}

fn shows(r: &Runner<Source>, value: &str) {
    assert_eq!(r.resource("session"), Some(&Value::str(value)));
}

fn passed_over(r: &Runner<Source>) -> bool {
    r.journal().any(|l| {
        l.contains(
            "session shows its placeholder until its source answers: the store holds secrets",
        )
    })
}

fn kept(args: f64, value: &str) -> (String, Vec<u8>) {
    (
        kept::kept_name("session"),
        kept::encode(&[Value::Number(args)], &Value::str(value)),
    )
}

fn token() -> (String, Vec<u8>) {
    ("token".into(), b"alice".to_vec())
}

#[test]
fn a_store_holding_secrets_shows_the_placeholder_not_the_build_time_answer() {
    let mut r = launch(true, vec![token()]);
    shows(&r, "…");
    assert!(passed_over(&r));
    assert!(!r
        .journal()
        .any(|l| l.contains("shows its build-time answer")));
    r.data.ready = true;
    r.data_ready().unwrap();
    shows(&r, "signed in as alice");
}

#[test]
fn an_empty_store_still_shows_the_build_time_answer() {
    // Only the runner's own kept entries, for other arguments: no secret.
    for snapshot in [vec![], vec![kept(1., "signed in as bob")]] {
        let mut r = launch(true, snapshot);
        shows(&r, "signed out");
        assert!(!passed_over(&r));
        r.data.ready = true;
        r.data_ready().unwrap();
        shows(&r, "signed out");
    }
}

#[test]
fn a_refused_kept_answer_at_a_store_holding_secrets_shows_the_placeholder() {
    let r = launch(true, vec![token(), kept(1., "signed in as bob")]);
    shows(&r, "…");
    assert!(passed_over(&r));
}

#[test]
fn an_admitted_kept_answer_still_shows() {
    let r = launch(true, vec![token(), kept(0., "signed in as alice")]);
    shows(&r, "signed in as alice");
    assert!(!passed_over(&r));
}

#[test]
fn without_a_declared_placeholder_the_build_time_answer_stands() {
    let r = launch(false, vec![token()]);
    shows(&r, "signed out");
    assert!(!passed_over(&r));
    assert!(r
        .journal()
        .any(|l| l.contains("session shows its build-time answer")));
}

#[test]
fn the_launch_s_store_decides_not_a_write_before_readiness() {
    // Signed out at launch, signed in before the source is ready: the
    // build-time answer stays until the source answers.
    let mut r = launch(true, vec![]);
    r.store.set("token", "alice").unwrap();
    r.act("gen", vec![Value::Number(1.)]).unwrap();
    shows(&r, "signed out");
    r.data.ready = true;
    r.data_ready().unwrap();
    shows(&r, "signed in as alice");
    // Signed in at launch, signed out before: the placeholder stays.
    let mut r = launch(true, vec![token()]);
    r.store.forget("token").unwrap();
    r.act("gen", vec![Value::Number(1.)]).unwrap();
    shows(&r, "…");
    r.data.ready = true;
    r.data_ready().unwrap();
    shows(&r, "signed out");
}

/// `consume(use ? account : "")`: `consume` reads the store only once `use`
/// is set; its build-time answer and its placeholder are both `""`.
fn plan_switching() -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let boolean = b.primitive(TypeKind::Bool);
    let mut a = exact_plan::asm::Asm::new();
    a.number(0.);
    let a = b.code(a);
    let account = b.resource("account", "account", &[a], string, Some(&Value::str("x")));
    b.set_resource_reader(account, true);
    b.set_resource_initial_args(account, &[Value::Number(0.)]);
    let no = b.constant(&Value::Bool(false));
    let flag = b.slot("use", boolean, no);
    let empty = b.str("");
    let mut k = exact_plan::asm::Asm::new();
    let (otherwise, end) = (k.label(), k.label());
    k.load_slot(flag).jump_if_false(otherwise);
    k.load_resource(account).jump(end);
    k.place(otherwise).str(empty).place(end);
    let k = b.code(k);
    let consume = b.resource("consume", "consume", &[k], string, Some(&Value::str("")));
    b.set_resource_initial_args(consume, &[Value::str("")]);
    b.set_resource_placeholder_value(consume, &Value::str(""));
    let mut body = exact_plan::asm::Asm::new();
    body.load_param(0).store_slot(flag);
    let body = b.code(body);
    b.action("use", &[("value", boolean)], &[flag], body);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

#[test]
fn a_resource_that_comes_to_read_the_store_shows_a_placeholder_not_the_compiled_object() {
    let mut r = Runner::boot_stored(
        plan_switching(),
        Source::default(),
        Kernel::with_monospace(),
        vec![token()],
        Default::default(),
        "/",
    )
    .unwrap();
    let consume = r
        .plan
        .resources
        .iter()
        .position(|row| r.plan.str(row.name) == "consume")
        .unwrap();
    assert!(r.resources[consume].as_ref().unwrap().value.is_compiled());
    r.act("use", vec![Value::Bool(true)]).unwrap();
    let state = r.resources[consume].as_ref().unwrap();
    assert!(state.placeholder && !state.value.is_compiled());
}
