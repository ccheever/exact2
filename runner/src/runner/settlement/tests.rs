use super::*;
use crate::{Carried, Outcome, Store};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, Items, Plan, Stdlib, TypeKind};
use std::cell::Cell;

// Count actual resource-validation entries, not elapsed time or a duplicate
// traversal. Zero entries means the 25k list cannot be walked by check_shape.
thread_local! {
    pub(super) static SHAPE_CHECKS: Cell<usize> = const { Cell::new(0) };
}

fn checks() -> usize {
    SHAPE_CHECKS.with(|count| count.replace(0))
}

struct Data {
    value: Value,
    queries: usize,
    later: bool,
    ready: bool,
    bad_guard: bool,
    write_on_parse: bool,
    read_store: bool,
    fail_parse: bool,
    refuse: Option<DataError>,
    adopted: Vec<(String, Vec<Value>, Value)>,
}

impl Data {
    fn new(value: Value) -> Self {
        Self {
            value,
            queries: 0,
            later: false,
            ready: true,
            bad_guard: false,
            write_on_parse: false,
            read_store: false,
            fail_parse: false,
            refuse: None,
            adopted: Vec::new(),
        }
    }
}

impl DataSource for Data {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        self.queries += 1;
        if let (Some(e), "rows") = (&self.refuse, source) {
            return Err(e.clone());
        }
        if self.read_store && source == "rows" {
            store.get("token");
        }
        Ok(if source == "guard" {
            Answer::Now(if self.bad_guard {
                Value::str("bad")
            } else {
                Value::Bool(true)
            })
        } else if self.later {
            Answer::Later(Request::continuation(1))
        } else {
            Answer::Now(self.value.clone())
        })
    }

    fn parse(
        &mut self,
        store: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        if self.write_on_parse {
            store.set("token", "candidate")?;
        }
        if self.fail_parse {
            return Err(DataError::Unavailable(
                "took 180 ms, over the 100 ms budget".into(),
            ));
        }
        Ok(Answer::Now(self.value.clone()))
    }

    fn ready(&self) -> bool {
        self.ready
    }
    fn adopt(&mut self, source: &str, args: &[Value], value: &Value) {
        self.adopted
            .push((source.to_string(), args.to_vec(), value.clone()));
    }
    fn grants(&self) -> &str {
        "secret.keep token\n"
    }
}

fn records(count: usize) -> Value {
    Value::list(
        (0..count)
            .map(|i| Value::record(vec![Value::Number(i as f64)]))
            .collect(),
    )
}

fn plan(kind: TypeKind, baked: Option<&Value>, reader: bool, guard: bool) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let field = b.primitive(kind);
    let item = b.record("Item", &[("value", field)]);
    let list = b.list(item);
    let number = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.));
    let revision = b.slot("revision", number, zero);
    let tick = b.slot("tick", number, zero);
    let mut arg = Asm::new();
    arg.load_slot(revision);
    let arg = b.code(arg);
    let rows = b.resource("rows", "rows", &[arg], list, baked);
    if baked.is_some() {
        b.set_resource_initial_args(rows, &[Value::Number(0.)]);
    }
    b.set_resource_reader(rows, reader);
    for (name, slot) in [("tick", tick), ("change", revision)] {
        let mut body = Asm::new();
        body.load_param(0).store_slot(slot);
        let body = b.code(body);
        b.action(name, &[("value", number)], &[slot], body);
    }
    let mut body = Asm::new();
    body.refresh(rows);
    let body = b.code(body);
    b.action("refresh", &[], &[], body);
    if guard {
        let boolean = b.primitive(TypeKind::Bool);
        let mut arg = Asm::new();
        arg.load_resource(rows).call(Stdlib::Length);
        let arg = b.code(arg);
        b.resource("guard", "guard", &[arg], boolean, None);
    }
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

fn boot(plan: Plan, data: Data) -> Runner<Data> {
    Runner::boot(
        plan,
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn tick(r: &mut Runner<Data>) {
    r.act("tick", vec![Value::Number(1.)]).unwrap();
}

/// A reply the source cannot take is let go, not restored as pending
/// with nobody left to deliver it (FRICTION F2 in the grnl port).
#[test]
fn a_reply_the_source_fails_to_parse_is_no_longer_pending() {
    let mut data = Data::new(records(1));
    data.later = true;
    let mut r = boot(plan(TypeKind::Number, None, false, false), data);
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    assert!(r.has_pending());
    r.data().fail_parse = true;
    let receipt = r
        .fulfill(
            requests[0].ticket,
            Outcome::Response(crate::Response {
                status: 200,
                headers: vec![],
                body: vec![],
            }),
        )
        .expect("the release commits");
    assert!(receipt.is_some());
    assert!(!r.has_pending(), "the failed reply is let go");
    assert!(!r.holds(requests[0].ticket));
    assert!(
        r.take_requests().is_empty(),
        "and not asked again by itself"
    );
    // `refresh` still asks.
    r.data().fail_parse = false;
    r.act("refresh", vec![]).unwrap();
    let again = r.take_requests();
    assert_eq!(again.len(), 1);
    r.fulfill(
        again[0].ticket,
        Outcome::Response(crate::Response {
            status: 200,
            headers: vec![],
            body: vec![],
        }),
    )
    .unwrap();
    assert!(!r.has_pending());
}

/// A Rust module's seam that cannot carry the call (an answer over its
/// bound) fails the resource: the commit stands, the value it had stays
/// and `failed` is set; new arguments ask again. A source's own refusal
/// still refuses the commit.
#[test]
fn a_source_that_cannot_answer_now_fails_the_resource() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    let rows = |r: &Runner<Data>| r.resource("rows").cloned();
    assert_eq!(rows(&r), Some(records(2)));
    r.data().refuse = Some(DataError::Interface("logic ABI message too large".into()));
    r.act("change", vec![Value::Number(1.)])
        .expect("the commit stands");
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.)));
    assert_eq!(rows(&r), Some(records(2)), "the value it had stays");
    assert_eq!(r.failed_args[0], Some(vec![Value::Number(1.)]));
    assert!(r
        .journal()
        .any(|l| l.contains("resource rows failed: logic ABI message too large")));
    r.data().refuse = Some(DataError::BadArguments("no".into()));
    assert!(r.act("change", vec![Value::Number(2.)]).is_err());
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.)));
    r.data().refuse = None;
    r.data().value = records(3);
    r.act("change", vec![Value::Number(3.)]).unwrap();
    assert_eq!(rows(&r), Some(records(3)));
    assert_eq!(r.failed_args[0], None);
}

/// A TypeScript answer that threw, rejected or answered outside its shape
/// (a refused ambient read, issue #124) fails its resource, as the web
/// takes it: the commit that asked stands, the value it had stays,
/// `failed` is set and an older request's reply is no longer wanted; the
/// same arguments are not asked again, new ones are (LLP 1027.000 D3).
#[test]
fn a_typescript_answer_that_failed_now_fails_the_resource() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    let rows = |r: &Runner<Data>| r.resource("rows").cloned();
    r.data().later = true;
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let older = r.take_requests();
    assert_eq!(older.len(), 1);
    let why = "setTimeout() is unavailable in data sources: there are no timers";
    r.data().refuse = Some(DataError::Failed(why.into()));
    r.act("change", vec![Value::Number(2.)])
        .expect("the commit stands");
    assert_eq!(r.slot("revision"), Some(&Value::Number(2.)));
    assert_eq!(rows(&r), Some(records(2)), "the value it had stays");
    assert_eq!(r.failed_args[0], Some(vec![Value::Number(2.)]));
    assert_eq!(
        r.failed_resources(),
        [("rows", why)],
        "`state.failed` says why"
    );
    assert!(!r.holds(older[0].ticket), "the older reply is not wanted");
    assert!(!r.has_pending());
    assert!(r
        .journal()
        .any(|l| l.contains(&format!("resource rows failed: {why}"))));
    let asked = r.data().queries;
    tick(&mut r);
    assert_eq!(r.data().queries, asked, "not asked again for them");
    r.data().refuse = None;
    r.data().later = false;
    r.data().value = records(3);
    r.act("change", vec![Value::Number(3.)]).unwrap();
    assert_eq!(rows(&r), Some(records(3)));
    assert_eq!(r.failed_args[0], None);
}

/// A TypeScript send whose answer failed now ends unsent, as a failed reply
/// ends one: the commit that asked stands, the slot stays as it was and an
/// older send's reply is no longer wanted. A source's own refusal still
/// refuses the action (LLP 1027.000 D3, amended 2026-10-07).
#[test]
fn a_typescript_send_that_failed_now_ends_unsent() {
    struct Sends(Option<DataError>);
    impl DataSource for Sends {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!("the fixture implements answer")
        }
        fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            match &self.0 {
                Some(e) => Err(e.clone()),
                None => Ok(Answer::Later(Request::get("https://fixture.test/save"))),
            }
        }
    }
    let plan = contract::compile(
        "component App\n  state note = \"\"\n  mutation saved as shape string\n  action post(v: string)\n    note = v\n    send saved = save(v)\n  view\n    text note\n",
    )
    .unwrap();
    let mut r = Runner::boot(
        plan,
        Sends(None),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("post", vec![Value::str("a")]).unwrap();
    let older = r.take_requests();
    assert_eq!(older.len(), 1);
    let why = "Date.now() is unavailable in data sources";
    r.data().0 = Some(DataError::Failed(why.into()));
    r.act("post", vec![Value::str("b")])
        .expect("the commit stands");
    assert_eq!(r.slot("note"), Some(&Value::str("b")));
    assert_eq!(r.slot("saved"), Some(&Value::NONE), "its slot as it was");
    assert!(!r.holds(older[0].ticket), "the older reply is not wanted");
    assert!(!r.has_pending() && r.take_requests().is_empty());
    assert!(r
        .journal()
        .any(|l| l.contains(&format!("send saved failed: {why}; it ends unsent"))));
    r.data().0 = Some(DataError::Unavailable("refused".into()));
    assert!(r.act("post", vec![Value::str("c")]).is_err());
    assert_eq!(r.slot("note"), Some(&Value::str("b")));
}

#[test]
fn unchanged_25k_resource_reuse_never_enters_shape_validation() {
    checks();
    let value = records(25_000);
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(value.clone()),
    );
    assert_eq!(checks(), 1, "the fresh 25k answer is validated");
    for i in 0..120 {
        r.act("tick", vec![Value::Number(i as f64)]).unwrap();
    }
    assert_eq!(
        checks(),
        0,
        "unchanged reuse must not walk the resource shape"
    );
    assert_eq!(r.data().queries, 1);
    let (Value::List(before), Some(Value::List(after))) = (&value, r.resource("rows")) else {
        panic!()
    };
    assert!(Items::ptr_eq(before, after));
    r.act("refresh", vec![]).unwrap();
    assert_eq!(
        checks(),
        1,
        "a forced fresh answer is checked even at the same pointer"
    );
    assert_eq!(r.data().queries, 2);
}

#[test]
fn fresh_malformed_tail_refuses_and_rolls_back_without_losing_checked_state() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(25_000)),
    );
    let before = r.carry();
    let tree = r.kernel().export(None).unwrap();
    let Value::List(items) = &mut r.data().value else {
        panic!()
    };
    let mut tail = items.to_vec();
    tail[24_999] = Value::record(vec![Value::str("invalid tail")]);
    *items = Items::from(tail);
    checks();
    assert!(matches!(
        r.act("change", vec![Value::Number(1.)]),
        Err(RunnerError::Shape { .. })
    ));
    assert_eq!(checks(), 1);
    assert_eq!(r.carry(), before);
    assert_eq!(r.kernel().export(None).unwrap(), tree);
    assert!(!r.is_poisoned());
    tick(&mut r);
    assert_eq!(checks(), 0, "the previous checked state survived refusal");
    r.data().value = records(2);
    r.act("change", vec![Value::Number(1.)]).unwrap();
    assert_eq!(checks(), 1);
    assert_eq!(r.resource("rows"), Some(&records(2)));
}

#[test]
fn baked_values_are_checked_before_they_become_reusable() {
    for ready in [false, true] {
        let value = records(3);
        let mut data = Data::new(Value::Unit);
        data.ready = ready;
        checks();
        let mut r = boot(plan(TypeKind::Number, Some(&value), false, false), data);
        assert_eq!(checks(), 1);
        assert_eq!(r.data().queries, 0);
        tick(&mut r);
        assert_eq!(checks(), 0);
        let invalid = Value::list(vec![Value::record(vec![Value::str("bad")])]);
        let mut data = Data::new(value);
        data.ready = ready;
        assert!(matches!(
            Runner::boot(
                plan(TypeKind::Number, Some(&invalid), false, false),
                data,
                Kernel::with_monospace(),
                Default::default(),
                "/"
            ),
            Err(RunnerError::Shape { .. })
        ));
        assert_eq!(checks(), 1);
    }
}

#[test]
fn a_compiled_value_is_adopted_once_as_the_runner_holds_it() {
    let baked = records(3);
    let mut r = boot(
        plan(TypeKind::Number, Some(&baked), false, false),
        Data::new(records(5)),
    );
    assert_eq!(r.data().queries, 0, "the compiled value answers boot");
    let adopted = std::mem::take(&mut r.data().adopted);
    assert_eq!(adopted.len(), 1);
    let (source, args, value) = &adopted[0];
    assert_eq!(
        (source.as_str(), args.as_slice()),
        ("rows", &[Value::Number(0.)][..])
    );
    let (Value::List(given), Some(Value::List(held))) = (value, r.resource("rows")) else {
        panic!()
    };
    assert!(Items::ptr_eq(given, held), "the one decoded copy, shared");
    tick(&mut r);
    r.act("change", vec![Value::Number(1.)]).unwrap();
    assert_eq!(r.data().queries, 1, "new arguments ask the source");
    assert!(r.data().adopted.is_empty(), "an answer is never adopted");
    let fresh = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(3)),
    );
    assert!(
        fresh.data.adopted.is_empty(),
        "nothing compiled, nothing adopted"
    );
}

#[test]
fn carry_is_revalidated_under_the_new_plan_even_when_type_indices_match() {
    let original = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(25_000)),
    );
    let mut carried = original.carry();
    let reload = |plan, data, carried: &Carried| {
        Runner::boot_carrying(
            plan,
            data,
            Kernel::with_monospace(),
            carried,
            Default::default(),
            "/",
        )
        .unwrap()
    };
    checks();
    let mut same = reload(original.plan().clone(), Data::new(Value::Unit), &carried);
    assert_eq!(
        checks(),
        1,
        "validate at the carry boundary, not again in settle"
    );
    assert_eq!(same.data().queries, 0);
    tick(&mut same);
    assert_eq!(checks(), 0);

    let strings = Value::list(vec![Value::record(vec![Value::str("new shape")])]);
    let changed_plan = plan(TypeKind::String, None, false, false);
    assert_eq!(
        changed_plan.resources[0].ty,
        original.plan().resources[0].ty
    );
    let mut changed = reload(changed_plan, Data::new(strings.clone()), &carried);
    assert_eq!(
        checks(),
        2,
        "reject old shape and validate the fresh replacement"
    );
    assert_eq!(changed.data().queries, 1);
    assert_eq!(changed.resource("rows"), Some(&strings));

    // Carried is public input, not a certificate from an earlier Runner.
    carried.resources[0].3 = Value::Unit;
    let mut repaired = reload(original.plan().clone(), Data::new(records(1)), &carried);
    assert_eq!(checks(), 2);
    assert_eq!(repaired.data().queries, 1);
    assert_eq!(repaired.resource("rows"), Some(&records(1)));
}

#[test]
fn persisted_seeds_check_current_shape_and_data_ready_checks_fresh_answers() {
    for seed in [
        records(2),
        Value::list(vec![Value::record(vec![Value::str("old type")])]),
    ] {
        let valid = seed == records(2);
        let snapshot = vec![(
            super::super::kept::kept_name("rows"),
            super::super::kept::encode(&[Value::Number(0.)], &seed),
        )];
        let mut data = Data::new(records(3));
        data.ready = false;
        checks();
        let mut r = Runner::boot_stored(
            plan(TypeKind::Number, Some(&records(0)), true, false),
            data,
            Kernel::with_monospace(),
            snapshot,
            Default::default(),
            "/",
        )
        .unwrap();
        assert_eq!(checks(), if valid { 1 } else { 2 });
        assert_eq!(
            r.resource("rows"),
            Some(&if valid { records(2) } else { records(0) })
        );
        tick(&mut r);
        assert_eq!(checks(), 0);
        r.data().ready = true;
        r.data_ready().unwrap();
        assert_eq!(checks(), 1);
        assert_eq!(r.resource("rows"), Some(&records(3)));
    }
}

#[test]
fn async_answers_validate_before_reuse_and_refusal_releases_ticket_but_keeps_store() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(1)),
    );
    r.data().later = true;
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let ticket = r.take_requests().remove(0).ticket;
    let before = r.carry();
    r.data().write_on_parse = true;
    r.data().value = Value::list(vec![Value::record(vec![Value::str("bad")])]);
    // The refused reply rolls back, and its ticket is let go: no host
    // delivers a ticket twice, so a kept one would be pending forever.
    assert!(r
        .fulfill(ticket, Outcome::Storage(vec![]))
        .unwrap()
        .is_some());
    assert!(!r.has_pending());
    let failed = r.failed_resources(); // `state.failed` says why
    assert!(
        matches!(failed[..], [("rows", why)] if why.contains("outside its shape")),
        "{failed:?}"
    );
    assert_eq!(
        r.resource("rows"),
        Some(&records(1)),
        "it keeps its last value"
    );
    assert_eq!(
        r.carry().store,
        before.store,
        "the refused parse wrote nothing"
    );
    assert!(r
        .fulfill(ticket, Outcome::Storage(vec![]))
        .unwrap()
        .is_none());
    r.data().value = records(2);
    r.act("refresh", vec![]).unwrap();
    let again = r.take_requests().remove(0).ticket;
    assert!(r
        .fulfill(again, Outcome::Storage(vec![]))
        .unwrap()
        .is_some());
    assert_eq!(r.resource("rows"), Some(&records(2)));
    assert!(r.failed_resources().is_empty());
    assert!(r
        .carry()
        .store
        .contains(&("token".into(), "candidate".into())));
    checks();
    tick(&mut r);
    assert_eq!(checks(), 0);
}

#[test]
fn async_downstream_refusal_restores_previous_resource_before_retry() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, true),
        Data::new(records(1)),
    );
    r.data().later = true;
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let ticket = r.take_requests().remove(0).ticket;
    let before = r.carry();
    let tree = r.kernel().export(None).unwrap();
    r.data().value = records(2);
    r.data().write_on_parse = true;
    r.data().bad_guard = true;
    assert!(r
        .fulfill(ticket, Outcome::Storage(vec![]))
        .unwrap()
        .is_some());
    assert_eq!(r.resource("rows"), Some(&records(1)));
    assert_eq!(r.carry().store, before.store);
    assert_eq!(r.kernel().export(None).unwrap(), tree);
    assert!(!r.is_poisoned());
    assert!(!r.has_pending(), "the refused reply is let go");
    r.data().bad_guard = false;
    r.act("refresh", vec![]).unwrap();
    let again = r.take_requests().remove(0).ticket;
    assert!(r
        .fulfill(again, Outcome::Storage(vec![]))
        .unwrap()
        .is_some());
    assert_eq!(r.resource("rows"), Some(&records(2)));
    checks();
    tick(&mut r);
    assert_eq!(checks(), 0);
}

#[test]
fn a_refused_pass_leaves_a_deferred_resource_stale_for_the_next_activation() {
    let mut data = Data::new(records(3));
    data.ready = false;
    let mut r = boot(plan(TypeKind::Number, Some(&records(0)), true, true), data);
    assert_eq!(r.resource("rows"), Some(&records(0)));
    r.data().ready = true;
    r.data().bad_guard = true;
    assert!(matches!(r.data_ready(), Err(RunnerError::Shape { .. })));
    assert_eq!(r.resource("rows"), Some(&records(0)), "rolled back");
    r.data().bad_guard = false;
    assert!(r.data_ready().unwrap().is_some(), "asked again");
    assert_eq!(r.resource("rows"), Some(&records(3)));
}

#[test]
fn a_refused_pass_does_not_mark_a_resource_as_reading_the_store() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, true),
        Data::new(records(1)),
    );
    assert!(!r.resource_reads_store("rows"));
    r.data().read_store = true;
    r.data().bad_guard = true;
    r.data().value = records(2);
    assert!(matches!(
        r.act("change", vec![Value::Number(1.)]),
        Err(RunnerError::Shape { .. })
    ));
    assert!(
        !r.resource_reads_store("rows"),
        "the refused query's store read is not provenance"
    );
}

#[test]
fn strings_past_the_limit_are_refused_where_they_would_be_built_or_kept() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let init = b.constant(&Value::str("ab"));
    let text = b.slot("text", string, init);
    // text = text + text, 26 times: 2 << 26 bytes, past MAX_STRING.
    let mut body = Asm::new();
    body.load_slot(text);
    for _ in 0..26 {
        body.bind_local()
            .load_local(0)
            .load_local(0)
            .op(exact_plan::Opcode::Concat, &[])
            .drop_local();
    }
    body.store_slot(text).op(exact_plan::Opcode::Unit, &[]);
    let body = b.code(body);
    b.action("double", &[], &[text], body);
    let mut keep = Asm::new();
    keep.load_param(0)
        .store_slot(text)
        .op(exact_plan::Opcode::Unit, &[]);
    let keep = b.code(keep);
    b.action("keep", &[("value", string)], &[text], keep);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let mut r = boot(b.finish().unwrap(), Data::new(Value::Unit));
    assert!(matches!(
        r.act("double", vec![]),
        Err(RunnerError::Trap(crate::vm::Trap::StringTooLong { .. }))
    ));
    let long = "x".repeat(crate::vm::MAX_STRING + 1);
    assert!(matches!(
        r.act("keep", vec![Value::str(&long)]),
        Err(RunnerError::StringTooLong { .. })
    ));
    assert_eq!(r.slot("text"), Some(&Value::str("ab")));
    assert!(!r.is_poisoned());
    r.act("keep", vec![Value::str("fits")]).unwrap();
}
