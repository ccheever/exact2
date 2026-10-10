//! A kept answer's identity read from another resource (LLP 1027.005 D3,
//! 2026-10-10): `feeds(gen, account)`, where `account` reads the store.
use super::*;
use crate::{Answer, Outcome, Request, Store};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, Plan, TypeKind, Value};

#[derive(Default)]
struct Source {
    ready: bool,
    asks: Vec<(String, Vec<Value>)>,
}

impl DataSource for Source {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        assert!(
            self.ready,
            "a deferred source must not run before readiness"
        );
        self.asks.push((source.to_string(), args.to_vec()));
        let key = store.get("token").unwrap_or("public").to_string();
        Ok(match source {
            "account" => Answer::Now(Value::str(&key)),
            _ => Answer::Later(Request::get(&format!(
                "https://example.test/{}",
                args[1].text()
            ))),
        })
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(Value::str(&format!(
            "feeds of {}",
            args[1].text()
        ))))
    }

    fn ready(&self) -> bool {
        self.ready
    }

    fn grants(&self) -> &str {
        "secret.keep token\nnet.fetch https://example.test"
    }
}

/// `account(0)` (placeholder "nobody") and `feeds(gen, key)`, the key read
/// straight from `account` or, with `through_derive`, through a derive.
fn plan(through_derive: bool) -> Plan {
    plan_with(through_derive, false, true)
}

/// As [`plan`]; `key_is_context` makes the key `feeds`' `with` context, and
/// `account_reads_store` false makes `account` a plain compiled resource.
fn plan_with(through_derive: bool, key_is_context: bool, account_reads_store: bool) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let number = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.));
    let gen = b.slot("gen", number, zero);
    let mut a = Asm::new();
    a.number(0.);
    let a = b.code(a);
    let account = b.resource(
        "account",
        "account",
        &[a],
        string,
        Some(&Value::str("nobody")),
    );
    b.set_resource_reader(account, account_reads_store);
    b.set_resource_initial_args(account, &[Value::Number(0.)]);
    let mut g = Asm::new();
    g.load_slot(gen);
    let g = b.code(g);
    let mut k = Asm::new();
    if through_derive {
        let mut d = Asm::new();
        d.load_resource(account);
        let d = b.code(d);
        let key = b.derive("key", string, d);
        k.load_derive(key);
    } else {
        k.load_resource(account);
    }
    let k = b.code(k);
    let feeds = b.resource(
        "feeds",
        "feeds",
        &[g, k],
        string,
        Some(&Value::str("loading")),
    );
    b.set_resource_reader(feeds, true);
    b.set_resource_context(feeds, u16::from(key_is_context));
    b.set_resource_initial_args(feeds, &[Value::Number(0.), Value::str("nobody")]);
    let mut body = Asm::new();
    body.load_param(0).store_slot(gen);
    let body = b.code(body);
    b.action("gen", &[("value", number)], &[gen], body);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

fn launch(through_derive: bool, snapshot: Vec<(String, String)>) -> Runner<Source> {
    launch_plan(plan(through_derive), snapshot)
}

fn launch_plan(plan: Plan, snapshot: Vec<(String, String)>) -> Runner<Source> {
    Runner::boot_stored(
        plan,
        Source::default(),
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap()
}

fn settle_requests(r: &mut Runner<Source>) {
    for q in r.take_requests() {
        r.fulfill(q.ticket, Outcome::Storage(vec![])).unwrap();
    }
}

fn shows(r: &Runner<Source>, name: &str, value: &str) {
    assert_eq!(r.resource(name), Some(&Value::str(value)), "{name}");
}

fn feeds_kept(args: &[Value], value: &str) -> (String, String) {
    (
        kept::kept_name("feeds"),
        kept::encode(args, &Value::str(value)),
    )
}

#[test]
fn a_sign_in_rekeeps_both_and_the_next_launch_shows_the_signed_in_answers() {
    let mut r = launch(false, vec![]);
    r.data.ready = true;
    r.data_ready().unwrap();
    settle_requests(&mut r);
    shows(&r, "feeds", "feeds of public");
    // Signing in writes the secret; the store-reading resources answer again.
    r.store.set("token", "alice").unwrap();
    r.act("gen", vec![Value::Number(0.)]).unwrap();
    settle_requests(&mut r);
    shows(&r, "account", "alice");
    shows(&r, "feeds", "feeds of alice");
    let next = launch(false, r.store.snapshot());
    shows(&next, "account", "alice");
    shows(&next, "feeds", "feeds of alice");
    assert!(next.journal().any(
        |l| l.contains("feeds shows its kept answer from the last session, kept for [0,\"alice\"]")
    ));
}

#[test]
fn an_identity_read_from_a_resource_on_its_placeholder_does_not_admit_a_kept_answer() {
    for through_derive in [false, true] {
        // The account kept nothing (a first launch after an update, an
        // answer over the budget), so the first frame's key is the
        // placeholder's, which another launch's entry happens to match.
        let mut r = launch(
            through_derive,
            vec![feeds_kept(
                &[Value::Number(0.), Value::str("nobody")],
                "feeds of somebody else",
            )],
        );
        shows(&r, "account", "nobody");
        shows(&r, "feeds", "loading");
        assert!(!r.journal().any(|l| l.contains("shows its kept answer")));
        // At readiness both are asked for what they are now.
        r.data.ready = true;
        r.data_ready().unwrap();
        settle_requests(&mut r);
        shows(&r, "feeds", "feeds of public");
    }
}

#[test]
fn an_identity_read_from_a_kept_answer_still_admits_its_dependent() {
    for through_derive in [false, true] {
        let r = launch(
            through_derive,
            vec![
                (
                    kept::kept_name("account"),
                    kept::encode(&[Value::Number(0.)], &Value::str("alice")),
                ),
                feeds_kept(&[Value::Number(0.), Value::str("alice")], "feeds of alice"),
            ],
        );
        shows(&r, "account", "alice");
        shows(&r, "feeds", "feeds of alice");
        assert!(r
            .journal()
            .any(|l| l.contains("account shows its kept answer")));
    }
}

#[test]
fn a_placeholder_read_only_by_with_context_does_not_refuse_the_identity() {
    // `feeds(gen) with key`: the identity is `gen`, which is known.
    let r = launch_plan(
        plan_with(false, true, true),
        vec![feeds_kept(&[Value::Number(0.)], "feeds kept for gen 0")],
    );
    shows(&r, "account", "nobody");
    shows(&r, "feeds", "feeds kept for gen 0");
}

#[test]
fn a_compiled_value_of_a_resource_that_reads_no_store_is_a_known_identity() {
    let r = launch_plan(
        plan_with(false, false, false),
        vec![feeds_kept(
            &[Value::Number(0.), Value::str("nobody")],
            "feeds of nobody",
        )],
    );
    shows(&r, "feeds", "feeds of nobody");
}

#[test]
fn a_long_identity_is_journaled_shortened_with_a_mark() {
    let line = super::lines::kept_shown("feeds", &"x".repeat(300));
    assert!(
        line.ends_with(&format!("kept for {}…", "x".repeat(200))),
        "{line}"
    );
    let line = super::lines::kept_shown("feeds", "[0]");
    assert!(line.ends_with("kept for [0]"), "{line}");
}
