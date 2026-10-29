//! Settlement: reuse, stand-ins, refusals and their rollback.
use super::*;
use crate::{CallOrigin, Carried, Outcome, Store};
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

#[test]
fn resource_inspection_distinguishes_retained_requests_equal_refresh_and_recovery() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(1)),
    );
    let status = |r: &Runner<Data>| -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(&crate::agent::state(r)).unwrap()
            ["resourceStatus"]["rows"]
            .clone()
    };
    assert!(!crate::agent::state(&r).contains("resourceStatus"));
    r.measure(true, false);
    let first = status(&r);
    assert_eq!(first["displayed"], "current");
    assert_eq!(first["argumentsEqual"], true);
    r.data().later = true;
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let b = r.take_requests().pop().unwrap();
    let pending = status(&r);
    assert_eq!(pending["displayed"], "retained");
    assert_eq!(pending["argumentsEqual"], false);
    assert_eq!(pending["displayedGeneration"], first["displayedGeneration"]);
    assert_ne!(pending["requestGeneration"], first["requestGeneration"]);
    assert_eq!(pending["pending"], true);
    r.data().value = Value::list(vec![Value::record(vec![Value::str("invalid")])]);
    r.fulfill(b.ticket, Outcome::Storage(vec![])).unwrap();
    assert_eq!(status(&r)["pending"], false);
    assert_eq!(status(&r)["argumentsEqual"], false);
    assert!(r.failed_resources()[0].1.contains("value"));
    assert_eq!(r.failure_counts()[0].1.count, 1);

    // A -> B -> A has equal arguments, but its value still answers the old A.
    r.act("change", vec![Value::Number(0.)]).unwrap();
    assert!(r.take_requests().is_empty());
    assert_eq!(status(&r)["displayed"], "retained");
    assert_eq!(status(&r)["argumentsEqual"], true);
    r.act("refresh", vec![]).unwrap();
    let a = r.take_requests().pop().unwrap();
    r.data().value = records(2);
    assert!(r
        .fulfill(b.ticket, Outcome::Storage(vec![]))
        .unwrap()
        .is_none());
    r.fulfill(a.ticket, Outcome::Storage(vec![])).unwrap();
    assert_eq!(status(&r)["displayed"], "current");
    let recovered = status(&r);
    r.act("refresh", vec![]).unwrap();
    let refresh = r.take_requests().pop().unwrap();
    r.data().fail_parse = true;
    r.fulfill(refresh.ticket, Outcome::Storage(vec![])).unwrap();
    assert_eq!(status(&r)["displayed"], "retained");
    assert_eq!(status(&r)["argumentsEqual"], true);
    assert_eq!(
        status(&r)["lastSuccessfulGeneration"],
        recovered["displayedGeneration"]
    );
    assert_eq!(r.failure_counts()[0].1.count, 2);
    r.data().fail_parse = false;
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let old = r.take_requests().pop().unwrap();
    r.act("change", vec![Value::Number(2.)]).unwrap();
    let current = r.take_requests().pop().unwrap();
    let generation = status(&r)["requestGeneration"].clone();
    assert!(r
        .fulfill(old.ticket, Outcome::Storage(vec![]))
        .unwrap()
        .is_none());
    assert_eq!(status(&r)["requestGeneration"], generation);
    r.fulfill(current.ticket, Outcome::Storage(vec![])).unwrap();
    assert_eq!(status(&r)["displayed"], "current");
    assert_eq!(r.failure_counts()[0].1.count, 2);
    r.measure(false, false);
    assert!(!crate::agent::state(&r).contains("resourceStatus"));
}

#[test]
fn resource_inspection_restores_generations_when_a_later_resource_refuses() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, true),
        Data::new(records(1)),
    );
    r.measure(true, false);
    let before = r.ids.work.resources.clone();
    let generation = r.resources[0].as_ref().unwrap().inspection_generation;
    r.data().bad_guard = true;
    r.data().value = records(2);
    assert!(r.act("change", vec![Value::Number(1.)]).is_err());
    assert_eq!(r.ids.work.resources[0].request, before[0].request);
    assert_eq!(
        r.resources[0].as_ref().unwrap().inspection_generation,
        generation
    );
}

struct Data {
    value: Value,
    queries: usize,
    later: bool,
    ready: bool,
    bad_guard: bool,
    guard_writes: bool,
    rows_writes: bool,
    write_on_parse: bool,
    read_store: bool,
    fail_parse: bool,
    /// What a reply's parse refuses with, when set (`overrun.rs`'s tests).
    parse_error: Option<DataError>,
    refuse: Option<DataError>,
    /// `guard` opens a stream, and refuses with this when set.
    guard_stream: bool,
    guard_refuse: Option<DataError>,
    /// `rows` answers later with a plain GET, a request a re-ask may keep.
    later_get: bool,
    /// `watch` answers later (else now, false).
    watch_later: bool,
    /// `rows`'s next turn answers later, whatever `refuse` says.
    later_once: bool,
    /// `rows`'s next turn is refused with this.
    refuse_once: Option<DataError>,
    /// `guard` writes the store once.
    guard_writes_once: bool,
    /// Moved by `rows`'s refused turn: a turn that took this long, on a live clock.
    turn_ms: Option<(std::sync::Arc<std::sync::Mutex<f64>>, f64)>,
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
            guard_writes: false,
            rows_writes: false,
            write_on_parse: false,
            read_store: false,
            fail_parse: false,
            parse_error: None,
            refuse: None,
            guard_stream: false,
            guard_refuse: None,
            later_get: false,
            watch_later: false,
            later_once: false,
            refuse_once: None,
            guard_writes_once: false,
            turn_ms: None,
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
        if self.read_store && source == "rows" {
            store.get("token");
        }
        if source == "watch" {
            return Ok(if self.watch_later {
                Answer::Later(Request::continuation(2))
            } else {
                Answer::Now(Value::Bool(false))
            });
        }
        if source == "guard" && std::mem::take(&mut self.guard_writes_once) {
            store.set("token", "once")?;
        }
        if source == "rows" && std::mem::take(&mut self.later_once) {
            return Ok(Answer::Later(Request::continuation(1)));
        }
        if let Some(e) = self.refuse_once.take().filter(|_| source == "rows") {
            return Err(e);
        }
        if let (Some(e), "rows") = (&self.refuse, source) {
            if let Some((clock, ms)) = &self.turn_ms {
                *clock.lock().unwrap() += ms;
            }
            return Err(e.clone());
        }
        if let (Some(e), "guard") = (&self.guard_refuse, source) {
            return Err(e.clone());
        }
        if self.guard_stream && source == "guard" {
            return Ok(Answer::stream(Request::get("https://example.test/guard")));
        }
        if self.guard_writes && source == "guard" || self.rows_writes && source == "rows" {
            store.set("token", "guarded")?;
        }
        Ok(if source == "guard" {
            Answer::Now(if self.bad_guard {
                Value::str("bad")
            } else {
                Value::Bool(true)
            })
        } else if self.later_get {
            Answer::Later(Request::get("https://example.test/rows"))
        } else if self.later {
            Answer::Later(Request::continuation(1))
        } else {
            Answer::Now(self.value.clone())
        })
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        if source == "guard" {
            return Ok(Answer::Now(Value::Bool(true)));
        }
        if self.write_on_parse {
            store.set("token", "candidate")?;
        }
        if let Some(e) = &self.parse_error {
            return Err(e.clone());
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
    plan_with(kind, baked, reader, guard, false)
}

/// [`plan`], and with `watch` a resource `watch` asked with `failed(rows)`.
fn plan_with(
    kind: TypeKind,
    baked: Option<&Value>,
    reader: bool,
    guard: bool,
    watch: bool,
) -> Plan {
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
    if watch {
        let boolean = b.primitive(TypeKind::Bool);
        let mut arg = Asm::new();
        arg.failed_resource(rows);
        let arg = b.code(arg);
        b.resource("watch", "watch", &[arg], boolean, None);
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
fn a_deferred_answer_outside_its_shape_fails_it_until_the_next_activation() {
    let mut data = Data::new(records(3));
    data.ready = false;
    let mut r = boot(plan(TypeKind::Number, Some(&records(0)), true, true), data);
    assert_eq!(r.resource("rows"), Some(&records(0)));
    r.data().ready = true;
    r.data().bad_guard = true;
    // `guard`'s answer outside its shape fails it, not the activation
    assert!(r.data_ready().is_ok());
    assert_eq!(r.resource("rows"), Some(&records(3)));
    assert!(r
        .failed_resources()
        .iter()
        .any(|(name, _)| *name == "guard"));
    r.data().bad_guard = false;
    assert!(r.data_ready().unwrap().is_some(), "asked again");
    assert!(r.failed_resources().is_empty());
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

#[test]
fn a_stand_in_failed_at_activation_is_not_pending_after_another_pass() {
    let mut data = Data::new(records(3));
    data.ready = false;
    let mut r = boot(plan(TypeKind::Number, None, true, true), data);
    assert!(r.pending_res[0], "the placeholder is pending");
    r.data().ready = true;
    r.data().read_store = true;
    r.data().refuse = Some(DataError::Unavailable("down".into()));
    // `guard`, later in plan order, writes the store `rows` read: the pass
    // repeats, and `rows` stays failed and not pending.
    r.data().guard_writes = true;
    assert!(r.data_ready().is_ok());
    assert!(r.failed_args[0].is_some());
    assert!(!r.pending_res[0] && !r.awaiting[0]);
}

/// Two sources that read the store and, as they answer, keep a value it
/// already keeps: keeping it again is no store change, so neither makes the
/// other stale, and settlement converges instead of refusing with `Cycle`.
#[test]
fn sources_that_keep_what_is_kept_settle() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let boolean = b.primitive(TypeKind::Bool);
    let item = b.record("Item", &[("value", number)]);
    let list = b.list(item);
    let rows = b.resource("rows", "rows", &[], list, None);
    b.set_resource_reader(rows, true);
    let guard = b.resource("guard", "guard", &[], boolean, None);
    b.set_resource_reader(guard, true);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let mut data = Data::new(records(1));
    data.read_store = true;
    data.rows_writes = true;
    data.guard_writes = true;
    let mut r = Runner::boot(
        b.finish().unwrap(),
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .expect("settles");
    assert_eq!(r.store.get("token"), Some("guarded"));
    let writes = r.store.take_writes();
    assert_eq!(
        writes.len(),
        1,
        "the first keep writes, the second does not"
    );
    assert_eq!(writes[0].name, "token");
    assert_eq!(writes[0].value.as_deref(), Some(&b"guarded"[..]));
}

fn overrun() -> DataError {
    DataError::Overrun(
        "`rows` took 180.0 ms of its own (CPU) time, over the 100 ms budget for one turn".into(),
    )
}

/// A turn over its budget in the first turn of an action's commit (theme2's
/// 18:06 ruling): the action stands, the resource keeps its value and reads as
/// an ask in flight (pending, no failure), and is asked again at 250 ms on the
/// runner's own clock.
#[test]
fn an_overrun_first_turn_leaves_the_action_standing_and_asks_again_at_250_ms() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    let rows = |r: &Runner<Data>| r.resource("rows").cloned();
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)])
        .expect("the action stands");
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.)));
    assert_eq!(rows(&r), Some(records(2)), "it keeps what it shows");
    assert!(r.pending_res[0], "it reads as an ask in flight");
    assert!(r.failed_resources().is_empty(), "no failure shows");
    assert_eq!(r.failed_args[0], None);
    assert!(r
        .journal()
        .any(|l| l.contains("resource rows:") && l.contains("asked again in 250 ms (1 of 2)")));
    let (t0, asked) = (r.now_ms(), r.data().queries);
    assert_eq!(r.timer_due_ms(), Some(t0 + 250.));
    r.data().refuse = None;
    r.data().value = records(3);
    r.advance(t0 + 249.).unwrap();
    assert_eq!(r.data().queries, asked, "not before its time");
    r.advance(t0 + 250.).unwrap();
    assert_eq!(rows(&r), Some(records(3)), "asked again, and answered");
    assert!(!r.pending_res[0]);
    assert_eq!(r.reask_due_ms(), None);
    assert_eq!(r.timer_due_ms(), None);
}

/// Three turns over budget in a row for the same arguments: asked again at 250
/// ms and at 1 s, then failed exactly as any refused resource, and not asked
/// again by itself.
#[test]
fn a_third_overrun_fails_the_resource_as_today() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let t0 = r.now_ms();
    r.advance(t0 + 250.).unwrap();
    assert!(r
        .journal()
        .any(|l| l.contains("asked again in 1000 ms (2 of 2)")));
    assert!(r.pending_res[0] && r.failed_resources().is_empty());
    r.advance(t0 + 1250.).unwrap();
    assert!(!r.pending_res[0]);
    assert_eq!(r.failed_args[0], Some(vec![Value::Number(1.)]));
    assert!(r.failed_resources()[0].1.contains("over the 100 ms budget"));
    assert!(r.journal().any(|l| l.contains("resource rows failed:")));
    let asked = r.data().queries;
    r.advance(t0 + 60_000.).unwrap();
    assert_eq!(r.data().queries, asked, "not asked again by itself");
    assert_eq!(r.timer_due_ms(), None);
}

/// Arguments that change during the wait ask for the new ones at once; the
/// wait for the old ones is dropped, with no stale re-ask at its time.
#[test]
fn an_argument_change_during_the_wait_fires_no_stale_reask() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let t0 = r.now_ms();
    r.data().refuse = None;
    r.data().value = records(4);
    r.act("change", vec![Value::Number(2.)]).unwrap();
    assert_eq!(r.resource("rows").cloned(), Some(records(4)));
    assert_eq!(r.reask_due_ms(), None);
    assert!(!r.pending_res[0]);
    let asked = r.data().queries;
    r.advance(t0 + 5_000.).unwrap();
    assert_eq!(r.data().queries, asked, "no stale re-ask");
}

/// A reply refused by a turn over its budget (the reply path, and every
/// worker or ordered source's) lets its request go, keeps the resource pending
/// with its value, and asks again at 250 ms with a new request.
#[test]
fn an_overrun_reply_asks_again_with_a_new_request() {
    let mut data = Data::new(records(1));
    data.later = true;
    let mut r = boot(plan(TypeKind::Number, None, false, false), data);
    let first = r.take_requests();
    assert_eq!(first.len(), 1);
    r.data().parse_error = Some(overrun());
    let reply = || {
        Outcome::Response(crate::Response {
            status: 200,
            headers: vec![],
            body: vec![],
        })
    };
    r.fulfill(first[0].ticket, reply()).unwrap();
    assert!(!r.holds(first[0].ticket), "the request is let go");
    assert!(r.pending_res[0], "but the resource reads as pending");
    assert!(r.failed_resources().is_empty());
    assert!(
        r.take_requests().is_empty(),
        "nothing asked before its time"
    );
    let t0 = r.now_ms();
    r.data().parse_error = None;
    r.advance(t0 + 250.).unwrap();
    let again = r.take_requests();
    assert_eq!(again.len(), 1, "asked again");
    r.fulfill(again[0].ticket, reply()).unwrap();
    assert!(!r.pending_res[0]);
    assert_eq!(r.reask_due_ms(), None);
}

// Round 1's findings on the first build (theme2's 18:55 condition (a)): each
// test below was red on it (ad991effff..f9d20916a9).

fn reply() -> Outcome {
    Outcome::Response(crate::Response {
        status: 200,
        headers: vec![],
        body: vec![],
    })
}

/// The re-ask keeps the original ask's origin and invocation, from a refused
/// turn of the ask (the query) and from a refused reply alike.
#[test]
fn a_reask_keeps_the_asks_origin_and_invocation() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.as_invocation(CallOrigin::Tool, 7, |r| {
        r.act("change", vec![Value::Number(1.)])
    })
    .unwrap();
    let t0 = r.now_ms();
    r.data().refuse = None;
    r.data().later = true;
    r.advance(t0 + 250.).unwrap();
    let asked = r.take_requests();
    assert_eq!(asked.len(), 1, "asked again");
    assert_eq!(r.ticket_origin(asked[0].ticket), CallOrigin::Tool);
    assert_eq!(r.ticket_id(asked[0].ticket), 7);
    // Its reply refused by a turn over its budget: asked again, the same.
    r.data().parse_error = Some(overrun());
    r.fulfill(asked[0].ticket, reply()).unwrap();
    r.data().parse_error = None;
    let t1 = r.now_ms();
    r.advance(t1 + 1000.).unwrap();
    let again = r.take_requests();
    assert_eq!(again.len(), 1, "asked again after the reply's refusal");
    assert_eq!(r.ticket_origin(again[0].ticket), CallOrigin::Tool);
    assert_eq!(r.ticket_id(again[0].ticket), 7);
}

/// A re-ask whose commit is refused (here a downstream answer outside its
/// shape) leaves no resource pending forever: the wait comes due again.
#[test]
fn a_refused_reask_is_not_left_pending_forever() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, true),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let t0 = r.now_ms();
    r.data().refuse = None;
    r.data().value = records(3);
    r.data().bad_guard = true;
    assert!(
        r.advance(t0 + 250.).is_err(),
        "the re-ask's commit is refused"
    );
    assert!(r.pending_res[0], "still waiting, as an ask in flight");
    assert!(r.timer_due_ms().is_some(), "and due again");
    r.data().bad_guard = false;
    r.advance(t0 + 60_000.).unwrap();
    assert_eq!(r.resource("rows").cloned(), Some(records(3)));
    assert!(!r.pending_res[0]);
}

/// Commits that don't ask the resource again (its arguments unchanged) burn
/// no try: it is not asked in them, and its wait stands as it was.
#[test]
fn an_unrelated_commit_burns_no_try() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let (t0, asked) = (r.now_ms(), r.data().queries);
    tick(&mut r);
    r.act("tick", vec![Value::Number(2.)]).unwrap();
    assert_eq!(r.data().queries, asked, "not asked in unrelated commits");
    assert!(r.pending_res[0] && r.failed_resources().is_empty());
    assert_eq!(r.timer_due_ms(), Some(t0 + 250.));
    assert!(!r.journal().any(|l| l.contains("(2 of 2)")));
}

/// A downstream resource's turn over its budget, in the commit of another's
/// reply, is not that reply's: the reply's resource is not asked again.
#[test]
fn a_downstream_overrun_is_not_the_replys() {
    let mut data = Data::new(records(1));
    data.later = true;
    data.guard_stream = true;
    let mut r = boot(plan(TypeKind::Number, None, false, true), data);
    let out = r.take_requests();
    let rows = out.iter().find(|q| q.target == "rows").unwrap().ticket;
    let guard = out.iter().find(|q| q.target == "guard").unwrap().ticket;
    // Its stream open, a message landed: a re-ask of it now fails, not waits.
    let message = Outcome::Message(crate::Message {
        data: "true".into(),
        ..crate::Message::default()
    });
    r.fulfill(guard, message).unwrap();
    r.data().guard_refuse = Some(overrun());
    let _ = r.fulfill(rows, reply());
    let t0 = r.now_ms();
    r.data().guard_refuse = None;
    let _ = r.advance(t0 + 5_000.);
    assert!(
        !r.take_requests().iter().any(|q| q.target == "rows"),
        "rows answered; guard's turn was refused"
    );
    assert!(!r
        .journal()
        .any(|l| l.contains("resource rows:") && l.contains("asked again")));
}

/// One advance across both delays fires each re-ask at its own time, in
/// deadline order, not once at the advance's end.
#[test]
fn reasks_fire_at_their_own_times_under_advance() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let t0 = r.now_ms();
    let a = r.advance_timed(t0 + 5_000.);
    assert!(a.error.is_none());
    let at: Vec<f64> = a.receipts.iter().map(|t| t.at_ms).collect();
    assert_eq!(at, vec![t0 + 250., t0 + 1250.]);
    assert!(!r.pending_res[0], "its streak spent within the advance");
    assert!(r.failed_resources()[0].1.contains("over the 100 ms budget"));
}

// Round 1 of the redesign (Astra): each red before its fix.

/// Two re-asks whose commits are refused: the commit that fails the resource
/// is the advance's receipt, so a host sees it leave `pending`.
#[test]
fn a_spent_streak_of_refused_reasks_hands_the_host_its_failure() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, true),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let t0 = r.now_ms();
    r.data().refuse = None;
    r.data().value = records(3);
    r.data().bad_guard = true;
    assert!(r.advance(t0 + 250.).is_err());
    let receipts = r.advance(t0 + 5_000.).expect("the failing commit stands");
    assert_eq!(receipts.len(), 1);
    assert!(!r.pending_res[0]);
    assert!(r.failed_resources()[0].1.contains("its commit was refused"));
    assert_eq!(r.timer_due_ms(), None);
}

/// A re-ask is a request's, not an authored timer's: it comes due while the
/// timers wait (an unseen wake or call, and `land_then`, advance with them
/// held).
#[test]
fn a_reask_comes_due_while_timers_wait() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    r.data().refuse = None;
    r.data().value = records(3);
    r.now_ms += 300.;
    let landed = r.land_then();
    assert!(landed.error.is_none());
    assert_eq!(r.resource("rows").cloned(), Some(records(3)));
    assert!(!r.pending_res[0]);
}

/// A refresh that a turn over its budget refuses again replaces the standing
/// wait: its own origin and invocation, its own time.
#[test]
fn a_refresh_replaces_another_invocations_wait() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.as_invocation(CallOrigin::Tool, 7, |r| {
        r.act("change", vec![Value::Number(1.)])
    })
    .unwrap();
    let t0 = r.now_ms();
    r.advance(t0 + 100.).unwrap();
    r.act("refresh", vec![]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(t0 + 350.), "its own time");
    r.data().refuse = None;
    r.data().later = true;
    r.advance(t0 + 350.).unwrap();
    let asked = r.take_requests();
    assert_eq!(asked.len(), 1);
    assert_eq!(r.ticket_origin(asked[0].ticket), CallOrigin::App);
    assert_eq!(r.ticket_id(asked[0].ticket), 0);
}

/// A request kept across an argument change (LLP 1054.000.000 D3) starts a
/// new streak: its first turn over budget waits 250 ms, not 1 s.
#[test]
fn new_arguments_on_a_kept_request_start_a_new_streak() {
    let mut data = Data::new(records(1));
    data.later_get = true;
    let mut r = boot(plan(TypeKind::Number, None, false, false), data);
    let first = r.take_requests();
    r.data().parse_error = Some(overrun());
    r.fulfill(first[0].ticket, reply()).unwrap();
    let t0 = r.now_ms();
    r.advance(t0 + 250.).unwrap();
    let again = r.take_requests();
    assert_eq!(again.len(), 1, "asked again: one re-ask in the streak");
    r.act("change", vec![Value::Number(2.)]).unwrap();
    assert!(r.take_requests().is_empty(), "the request is kept");
    r.fulfill(again[0].ticket, reply()).unwrap();
    assert_eq!(r.timer_due_ms(), Some(r.now_ms() + 250.));
}

/// A store reader asked again unforced under its wait keeps the streak's
/// count, however many store commits come: its next turn over budget waits
/// 1 s, the second of two.
#[test]
fn a_store_readers_new_ask_keeps_its_streak() {
    let mut data = Data::new(records(2));
    data.read_store = true;
    let mut r = boot(plan(TypeKind::Number, None, true, false), data);
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let t0 = r.now_ms();
    r.advance(t0 + 250.).unwrap();
    assert!(
        r.journal().any(|l| l.contains("(2 of 2)")),
        "one re-ask made"
    );
    r.data().refuse = None;
    r.data().later = true;
    r.store.set("token", "moved").unwrap();
    r.commit_again(Vec::new(), "the store moved").unwrap();
    let asked = r.take_requests();
    assert_eq!(asked.len(), 1, "asked again for the store");
    r.data().parse_error = Some(overrun());
    r.fulfill(asked[0].ticket, reply()).unwrap();
    assert_eq!(r.timer_due_ms(), Some(r.now_ms() + 1000.));
}

// Round 2 of the redesign (Astra, Grok): each red before its fix.

/// A spent streak's failing commit is made under its ask's origin and
/// invocation, as each re-ask was: what it asks (here `watch`, which reads
/// `failed(rows)`) is stamped so.
#[test]
fn a_spent_streaks_failure_keeps_its_asks_provenance() {
    let mut r = boot(
        plan_with(TypeKind::Number, None, false, true, true),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.as_invocation(CallOrigin::Tool, 7, |r| {
        r.act("change", vec![Value::Number(1.)])
    })
    .unwrap();
    let t0 = r.now_ms();
    r.data().refuse = None;
    r.data().value = records(3);
    r.data().bad_guard = true;
    r.data().watch_later = true;
    assert!(r.advance(t0 + 250.).is_err());
    r.advance(t0 + 5_000.).unwrap();
    let asked = r.take_requests();
    let watch = asked
        .iter()
        .find(|q| q.target == "watch")
        .expect("watch asked");
    assert_eq!(r.ticket_origin(watch.ticket), CallOrigin::Tool);
    assert_eq!(r.ticket_id(watch.ticket), 7);
}

/// A refresh refused by a turn over its budget, asked again in a later pass
/// of the same settlement (a store reader after a later resource wrote),
/// still replaces another invocation's wait.
#[test]
fn a_refresh_asked_again_in_a_later_pass_still_replaces_the_wait() {
    let mut data = Data::new(records(2));
    data.read_store = true;
    let mut r = boot(plan(TypeKind::Number, None, true, true), data);
    r.data().refuse = Some(overrun());
    r.as_invocation(CallOrigin::Tool, 7, |r| {
        r.act("change", vec![Value::Number(1.)])
    })
    .unwrap();
    let t0 = r.now_ms();
    r.advance(t0 + 100.).unwrap();
    r.data().guard_writes_once = true;
    let asked = r.data().queries;
    r.commit_again(vec![0, 1], "a refresh of both").unwrap();
    assert!(
        r.data().queries >= asked + 3,
        "rows asked again in a second pass"
    );
    assert_eq!(r.timer_due_ms(), Some(t0 + 350.), "the refresh's own time");
    r.data().refuse = None;
    r.data().later = true;
    r.advance(t0 + 350.).unwrap();
    let asked = r.take_requests();
    let rows = asked
        .iter()
        .find(|q| q.target == "rows")
        .expect("rows asked");
    assert_eq!(r.ticket_origin(rows.ticket), CallOrigin::App);
}

/// Arguments that change by the re-ask's time start a new streak: their first
/// turn over budget waits 250 ms again, not the spent streak's failure.
#[test]
fn new_arguments_at_the_reasks_time_start_a_new_streak() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let t0 = r.now_ms();
    r.advance(t0 + 250.).unwrap();
    // Arguments read anew at the second re-ask (a clock-derived argument).
    r.slots[0] = Value::Number(2.);
    r.advance(t0 + 1250.).unwrap();
    assert!(
        r.failed_resources().is_empty(),
        "not the spent streak's failure"
    );
    assert!(r.pending_res[0]);
    assert_eq!(r.timer_due_ms(), Some(t0 + 1500.));
}

/// On a live clock the delay runs from the refusal: a 400 ms turn doesn't
/// spend its own 250 ms.
#[test]
fn a_slow_turn_does_not_spend_its_own_delay() {
    let mut r = boot(
        plan(TypeKind::Number, None, false, false),
        Data::new(records(2)),
    );
    let clock = std::sync::Arc::new(std::sync::Mutex::new(1000.));
    let read = clock.clone();
    r.set_live_clock(Some(Box::new(move || *read.lock().unwrap())));
    r.data().refuse = Some(overrun());
    r.data().turn_ms = Some((clock, 400.));
    r.act("change", vec![Value::Number(1.)]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(1650.));
}

// Round 3 of the redesign (Astra, Grok): each red before its fix.

/// A refresh whose first pass hands out a request, asked again in a later pass
/// and refused there by a turn over its budget, still replaces another
/// invocation's wait.
#[test]
fn a_refresh_that_first_asked_later_still_replaces_the_wait() {
    let mut data = Data::new(records(2));
    data.read_store = true;
    let mut r = boot(plan(TypeKind::Number, None, true, true), data);
    r.data().refuse = Some(overrun());
    r.as_invocation(CallOrigin::Tool, 7, |r| {
        r.act("change", vec![Value::Number(1.)])
    })
    .unwrap();
    let t0 = r.now_ms();
    r.advance(t0 + 100.).unwrap();
    r.data().guard_writes_once = true;
    r.data().later_once = true;
    r.commit_again(vec![0, 1], "a refresh of both").unwrap();
    assert!(
        r.take_requests().is_empty(),
        "the first pass's request, discarded"
    );
    assert_eq!(r.timer_due_ms(), Some(t0 + 350.), "the refresh's own time");
}

/// A refresh refused in its first pass whose later pass hands out a request:
/// that request starts a new streak, so its own refusal waits 250 ms.
#[test]
fn a_refreshs_request_from_a_later_pass_starts_a_new_streak() {
    let mut data = Data::new(records(2));
    data.read_store = true;
    let mut r = boot(plan(TypeKind::Number, None, true, true), data);
    r.data().refuse = Some(overrun());
    r.act("change", vec![Value::Number(1.)]).unwrap();
    let t0 = r.now_ms();
    r.advance(t0 + 250.).unwrap();
    assert!(
        r.journal().any(|l| l.contains("(2 of 2)")),
        "one re-ask made"
    );
    r.data().refuse = None;
    r.data().later = true;
    r.data().refuse_once = Some(overrun());
    r.data().guard_writes_once = true;
    r.commit_again(vec![0, 1], "a refresh of both").unwrap();
    let asked = r.take_requests();
    let rows = asked
        .iter()
        .find(|q| q.target == "rows")
        .expect("rows asked");
    r.data().parse_error = Some(overrun());
    r.fulfill(rows.ticket, reply()).unwrap();
    assert_eq!(r.timer_due_ms(), Some(r.now_ms() + 250.));
}
