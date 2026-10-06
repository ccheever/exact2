//! Persisted-answer identity and request context (LLP 1027.005 D3–D6).
use super::*;
use crate::{Answer, Outcome, Request, Store};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, Opcode, Plan, TypeKind, Value};

struct Source {
    ready: bool,
    later: bool,
    same_request: bool,
    fail: bool,
    value: Value,
    asks: Vec<Vec<Value>>,
    parses: Vec<Vec<Value>>,
    /// Grants Health (LLP 1069.008.000 D7).
    health: bool,
}

impl Default for Source {
    fn default() -> Self {
        Self {
            ready: false,
            later: false,
            same_request: false,
            fail: false,
            value: Value::str("fresh"),
            asks: Vec::new(),
            parses: Vec::new(),
            health: false,
        }
    }
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
        if source == "guard" {
            return Ok(Answer::Now(Value::Number(1.)));
        }
        assert!(
            self.ready,
            "a deferred source must not run before readiness"
        );
        store.get("token");
        self.asks.push(args.to_vec());
        Ok(if self.later {
            let suffix = if self.same_request {
                "same"
            } else {
                args[args.len() - 2].text()
            };
            Answer::Later(Request::get(&format!("https://example.test/{suffix}")))
        } else {
            Answer::Now(self.value.clone())
        })
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        self.parses.push(args.to_vec());
        if self.fail {
            return Err(DataError::Unavailable("failed activation".into()));
        }
        Ok(Answer::Now(self.value.clone()))
    }

    fn ready(&self) -> bool {
        self.ready
    }

    fn grants(&self) -> &str {
        if self.health {
            "secret.keep token\nnet.fetch https://example.test\ndevice.health-read purpose.health"
        } else {
            "secret.keep token\nnet.fetch https://example.test"
        }
    }
}

fn full(car: &str, minute: &str, revision: f64) -> Vec<Value> {
    vec![Value::str(car), Value::str(minute), Value::Number(revision)]
}

fn plan(context: u16, empty_call: bool) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let number = b.primitive(TypeKind::Number);
    let optional = b.option(number);
    let a = b.constant(&Value::str("A"));
    let car = b.slot("car", string, a);
    let initial_minute = b.constant(&Value::str("100"));
    let minute = b.slot("minute", string, initial_minute);
    let zero = b.constant(&Value::Number(0.));
    let revision = b.slot("revision", number, zero);
    let tick = b.slot("tick", number, zero);
    let one = b.constant(&Value::some(Value::Number(1.)));
    let guard = b.slot("guard", optional, one);
    let args: Vec<_> = [car, minute, revision]
        .into_iter()
        .skip(usize::from(empty_call))
        .map(|slot| {
            let mut a = Asm::new();
            a.load_slot(slot);
            b.code(a)
        })
        .collect();
    let answer = b.resource(
        "answer",
        "answer",
        &args,
        string,
        Some(&Value::str("loading")),
    );
    b.set_resource_context(answer, context);
    b.set_resource_reader(answer, true);
    let initial = full("A", "100", 0.);
    b.set_resource_initial_args(answer, &initial[usize::from(empty_call)..]);
    for (name, slot, ty) in [
        ("car", car, string),
        ("minute", minute, string),
        ("revision", revision, number),
        ("tick", tick, number),
    ] {
        let mut body = Asm::new();
        body.load_param(0).store_slot(slot);
        let body = b.code(body);
        b.action(name, &[("value", ty)], &[slot], body);
    }
    let mut body = Asm::new();
    body.refresh(answer);
    let body = b.code(body);
    b.action("refresh", &[], &[], body);
    let mut body = Asm::new();
    body.load_param(0).store_slot(car);
    body.simple(Opcode::None).store_slot(guard);
    let body = b.code(body);
    b.action("refuse", &[("car", string)], &[car, guard], body);
    // The answer's tentative state is visited before this argument refuses.
    let mut arg = Asm::new();
    arg.load_slot(guard).simple(Opcode::Unwrap);
    let arg = b.code(arg);
    let guard = b.resource("guard", "guard", &[arg], number, Some(&Value::Number(1.)));
    b.set_resource_initial_args(guard, &[Value::Number(1.)]);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

fn entry(args: &[Value], value: &Value) -> Vec<(String, String)> {
    vec![(kept::kept_name("answer"), kept::encode(args, value))]
}

fn boot(plan: Plan, source: Source, snapshot: Vec<(String, String)>) -> Runner<Source> {
    Runner::boot_stored(
        plan,
        source,
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap()
}

fn seeded() -> Runner<Source> {
    boot(
        plan(2, false),
        Source::default(),
        entry(&[Value::str("A")], &Value::str("kept")),
    )
}

fn shows(r: &Runner<Source>, value: &str) {
    assert_eq!(r.resource("answer"), Some(&Value::str(value)));
}

fn state(r: &Runner<Source>) -> &ResourceState {
    r.resources[0].as_ref().unwrap()
}

fn answer_writes(r: &Runner<Source>) -> usize {
    r.store
        .writes()
        .iter()
        .filter(|write| write.name == kept::kept_name("answer"))
        .count()
}

fn provisional(r: &Runner<Source>) {
    assert!(r.carry().resources.iter().all(|(n, ..)| n != "answer"));
    let checkpoint = r.document_checkpoint("/");
    assert!(checkpoint.answers.iter().all(|(n, ..)| n != "answer"));
    assert!(checkpoint.pending.iter().any(|name| name == "answer"));
    assert_eq!(answer_writes(r), 0);
}

#[test]
fn seed_survives_two_context_changes_and_activation_asks_the_latest_full_arguments() {
    let mut r = seeded();
    shows(&r, "kept");
    assert!(state(&r).kept_seed);
    provisional(&r);
    r.act("minute", vec![Value::str("200")]).unwrap();
    r.act("revision", vec![Value::Number(2.)]).unwrap();
    shows(&r, "kept");
    assert_eq!(state(&r).args, [Value::str("A")]);
    assert!(state(&r).kept_seed);
    assert!(r.data.asks.is_empty());
    provisional(&r);
    r.data.ready = true;
    assert!(r.data_ready().unwrap().is_some());
    assert_eq!(r.data.asks, [full("A", "200", 2.)]);
    shows(&r, "fresh");
    assert!(!state(&r).kept_seed);
    assert!(!r.stale[0]);
    assert_eq!(state(&r).args, full("A", "200", 2.));
    assert_eq!(
        r.store.kept(&kept::kept_name("answer")),
        Some(kept::encode(&[Value::str("A")], &Value::str("fresh")).as_str())
    );
    assert_eq!(answer_writes(&r), 1);
    assert!(r.data_ready().unwrap().is_none());
}

#[test]
fn an_identity_mismatch_discards_the_seed_and_changing_back_does_not_readmit_it() {
    let mut r = seeded();
    r.act("car", vec![Value::str("B")]).unwrap();
    shows(&r, "loading");
    assert!(!state(&r).kept_seed);
    r.act("car", vec![Value::str("A")]).unwrap();
    r.act("minute", vec![Value::str("200")]).unwrap();
    shows(&r, "loading");
    assert!(!state(&r).kept_seed);
    assert!(r.data.asks.is_empty());
    assert_eq!(answer_writes(&r), 0);
}

#[test]
fn refusal_before_readiness_restores_the_seed_and_its_identity_eligibility() {
    let mut r = seeded();
    let before = r.kernel().export(None).unwrap();
    assert!(matches!(
        r.act("refuse", vec![Value::str("B")]),
        Err(RunnerError::Trap(_))
    ));
    assert_eq!(r.kernel().export(None).unwrap(), before);
    shows(&r, "kept");
    assert!(state(&r).kept_seed);
    assert_eq!(state(&r).args, [Value::str("A")]);
    r.act("minute", vec![Value::str("200")]).unwrap();
    shows(&r, "kept");
    assert!(state(&r).kept_seed);
    provisional(&r);
}

#[test]
fn a_refused_activation_restores_seed_provenance_and_does_not_persist_its_answer() {
    let mut r = seeded();
    r.data.ready = true;
    r.data.value = Value::Bool(false);
    assert!(matches!(r.data_ready(), Err(RunnerError::Shape { .. })));
    shows(&r, "kept");
    assert!(state(&r).kept_seed);
    assert!(r.stale[0]);
    assert_eq!(answer_writes(&r), 0);
    r.data.ready = false;
    r.act("minute", vec![Value::str("200")]).unwrap();
    shows(&r, "kept");
    r.data.ready = true;
    r.data.value = Value::str("valid");
    r.data_ready().unwrap();
    shows(&r, "valid");
    assert!(!state(&r).kept_seed);
    assert_eq!(r.data.asks.last(), Some(&full("A", "200", 0.)));
    assert_eq!(answer_writes(&r), 1);
}

#[test]
fn async_activation_is_provisional_and_a_context_change_supersedes_its_full_request() {
    let mut r = seeded();
    r.data.ready = true;
    r.data.later = true;
    r.data_ready().unwrap();
    let first = r.take_requests().remove(0).ticket;
    shows(&r, "kept");
    assert!(
        !state(&r).kept_seed,
        "the committed activation consumes admission"
    );
    assert_eq!(state(&r).args, [Value::str("A")]);
    assert_eq!(r.pending[0].args, full("A", "100", 0.));
    provisional(&r);
    r.act("tick", vec![Value::Number(1.)]).unwrap();
    assert_eq!(r.data.asks.len(), 1, "an identical full ask is reused");
    r.act("minute", vec![Value::str("200")]).unwrap();
    let second = r.take_requests().remove(0).ticket;
    assert_ne!(first, second);
    assert!(!r.holds(first));
    assert_eq!(r.pending[0].args, full("A", "200", 0.));
    assert_eq!(state(&r).args, [Value::str("A")]);
    provisional(&r);
    assert!(r
        .fulfill(first, Outcome::Storage(vec![]))
        .unwrap()
        .is_none());
    assert!(r.data.parses.is_empty());
    provisional(&r);
    r.fulfill(second, Outcome::Storage(vec![])).unwrap();
    shows(&r, "fresh");
    assert_eq!(r.data.parses, [full("A", "200", 0.)]);
    assert_eq!(state(&r).args, full("A", "200", 0.));
    assert_eq!(answer_writes(&r), 1);
    assert!(r.carry().resources.iter().any(|(n, ..)| n == "answer"));
    assert!(r
        .document_checkpoint("/")
        .answers
        .iter()
        .any(|(n, ..)| n == "answer"));
}

#[test]
fn identical_transport_keeps_its_ticket_but_parses_with_the_latest_full_arguments() {
    let mut r = seeded();
    r.data.ready = true;
    r.data.later = true;
    r.data.same_request = true;
    r.data_ready().unwrap();
    let ticket = r.take_requests().remove(0).ticket;
    r.act("minute", vec![Value::str("200")]).unwrap();
    r.act("revision", vec![Value::Number(2.)]).unwrap();
    assert_eq!(r.data.asks.len(), 3);
    assert!(r.take_requests().is_empty());
    assert!(r.holds(ticket));
    assert_eq!(r.pending[0].args, full("A", "200", 2.));
    provisional(&r);
    r.fulfill(ticket, Outcome::Storage(vec![])).unwrap();
    assert_eq!(r.data.parses, [full("A", "200", 2.)]);
    assert_eq!(state(&r).args, full("A", "200", 2.));
    assert_eq!(answer_writes(&r), 1);
}

#[test]
fn failed_activation_writes_nothing_and_only_the_full_failed_arguments_suppress_retry() {
    let mut r = seeded();
    r.data.ready = true;
    r.data.later = true;
    r.data.fail = true;
    r.data_ready().unwrap();
    let ticket = r.take_requests().remove(0).ticket;
    r.fulfill(ticket, Outcome::Storage(vec![])).unwrap();
    assert!(!r.has_pending());
    assert_eq!(r.failed_args[0], Some(full("A", "100", 0.)));
    assert!(!state(&r).kept_seed);
    shows(&r, "kept");
    provisional(&r);
    r.act("tick", vec![Value::Number(1.)]).unwrap();
    assert_eq!(r.data.asks.len(), 1);
    r.data.fail = false;
    r.act("minute", vec![Value::str("200")]).unwrap();
    assert_eq!(r.data.asks.len(), 2);
    assert!(r.failed_args[0].is_none());
    let ticket = r.take_requests().remove(0).ticket;
    r.fulfill(ticket, Outcome::Storage(vec![])).unwrap();
    shows(&r, "fresh");
    assert_eq!(answer_writes(&r), 1);
}

#[test]
fn context_only_changes_reask_but_equal_answers_do_not_write_the_kept_entry_again() {
    let mut r = seeded();
    r.data.ready = true;
    r.data.value = Value::str("kept");
    r.data_ready().unwrap();
    assert_eq!(answer_writes(&r), 0);
    r.act("minute", vec![Value::str("200")]).unwrap();
    r.act("revision", vec![Value::Number(2.)]).unwrap();
    assert_eq!(
        r.data.asks,
        [
            full("A", "100", 0.),
            full("A", "200", 0.),
            full("A", "200", 2.)
        ]
    );
    assert_eq!(answer_writes(&r), 0);
    r.data.value = Value::str("changed");
    r.act("minute", vec![Value::str("300")]).unwrap();
    assert_eq!(answer_writes(&r), 1);
    r.act("car", vec![Value::str("B")]).unwrap();
    assert_eq!(
        answer_writes(&r),
        2,
        "identity is persisted even for an equal value"
    );
}

#[test]
fn no_with_preserves_entry_bytes_and_requires_all_arguments_to_match() {
    let old_args = full("A", "100", 0.);
    let snapshot = entry(&old_args, &Value::str("kept"));
    let old_bytes = snapshot[0].1.clone();
    let mut r = boot(plan(0, false), Source::default(), snapshot);
    shows(&r, "kept");
    assert_eq!(state(&r).args, old_args);
    r.data.ready = true;
    r.data.value = Value::str("kept");
    r.data_ready().unwrap();
    assert_eq!(
        r.store.kept(&kept::kept_name("answer")),
        Some(old_bytes.as_str())
    );
    assert_eq!(answer_writes(&r), 0);
    r.act("minute", vec![Value::str("200")]).unwrap();
    assert_eq!(answer_writes(&r), 1);
    assert_eq!(
        r.store.kept(&kept::kept_name("answer")),
        Some(kept::encode(&full("A", "200", 0.), &Value::str("kept")).as_str())
    );
    let mismatch = boot(
        plan(0, false),
        Source::default(),
        entry(&full("A", "yesterday", 0.), &Value::str("kept")),
    );
    shows(&mismatch, "loading");
}

#[test]
fn an_empty_call_keeps_one_answer_and_still_passes_all_context_to_the_source() {
    let mut r = boot(
        plan(2, true),
        Source::default(),
        entry(&[], &Value::str("kept")),
    );
    shows(&r, "kept");
    r.act("minute", vec![Value::str("200")]).unwrap();
    shows(&r, "kept");
    assert!(state(&r).args.is_empty());
    r.data.ready = true;
    r.data_ready().unwrap();
    assert_eq!(r.data.asks, [vec![Value::str("200"), Value::Number(0.)]]);
    assert_eq!(
        r.store.kept(&kept::kept_name("answer")),
        Some(kept::encode(&[], &Value::str("fresh")).as_str())
    );
}

#[test]
fn damaged_wrong_length_wrong_identity_and_wrong_shape_entries_use_the_fallback() {
    let mut snapshots = vec![vec![(kept::kept_name("answer"), "damaged".into())]];
    for (args, value) in [
        (vec![], Value::str("kept")),
        (full("A", "100", 0.), Value::str("kept")),
        (vec![Value::str("B")], Value::str("kept")),
        (vec![Value::str("A")], Value::Bool(true)),
        (vec![Value::str("A")], Value::str(&"x".repeat(5000))),
    ] {
        snapshots.push(entry(&args, &value));
    }
    for snapshot in snapshots {
        let r = boot(plan(2, false), Source::default(), snapshot);
        shows(&r, "loading");
        assert!(!state(&r).kept_seed);
        assert_eq!(answer_writes(&r), 0);
    }
}

#[test]
fn the_kept_budget_counts_identity_and_value_but_not_context() {
    let mut r = seeded();
    let large = Value::str(&"x".repeat(5000));
    r.act("minute", vec![large.clone()]).unwrap();
    r.data.ready = true;
    r.data_ready().unwrap();
    assert_eq!(r.data.asks[0][1], large);
    assert_eq!(
        answer_writes(&r),
        1,
        "large context is absent from the entry"
    );
    r.act("car", vec![large.clone()]).unwrap();
    assert_eq!(
        answer_writes(&r),
        1,
        "large identity exceeds the entry budget"
    );
    r.data.value = large;
    r.act("car", vec![Value::str("A")]).unwrap();
    assert_eq!(
        answer_writes(&r),
        1,
        "large answer exceeds the entry budget"
    );
}

#[test]
fn carried_and_document_answers_keep_full_argument_admission() {
    for args in [vec![Value::str("A")], full("A", "yesterday", 0.)] {
        let carried = Carried {
            resources: vec![(
                "answer".into(),
                "answer".into(),
                args.clone(),
                Value::str("carried"),
            )],
            ..Default::default()
        };
        let reloaded = Runner::boot_carrying(
            plan(2, false),
            Source::default(),
            Kernel::with_monospace(),
            &carried,
            Default::default(),
            "/",
        )
        .unwrap();
        shows(&reloaded, "loading");
        assert!(!state(&reloaded).kept_seed);
        let checkpoint = super::Checkpoint {
            answers: vec![(
                "answer".into(),
                "answer".into(),
                args,
                Value::str("rendered"),
            )],
            ..Default::default()
        };
        let adopted = Runner::boot_checkpoint(
            plan(2, false),
            Source::default(),
            Kernel::with_monospace(),
            &checkpoint,
            vec![],
            Default::default(),
            Default::default(),
            "/",
        )
        .unwrap();
        shows(&adopted, "loading");
        assert!(!state(&adopted).kept_seed);
    }
}

#[test]
fn a_ready_source_gets_no_kept_seed_even_when_its_first_answer_is_slow() {
    let mut r = boot(
        plan(2, false),
        Source {
            ready: true,
            later: true,
            ..Default::default()
        },
        entry(&[Value::str("A")], &Value::str("kept")),
    );
    shows(&r, "loading");
    assert!(!state(&r).kept_seed);
    assert_eq!(r.data.asks, [full("A", "100", 0.)]);
    let ticket = r.take_requests().remove(0).ticket;
    r.fulfill(ticket, Outcome::Storage(vec![])).unwrap();
    shows(&r, "fresh");
    assert_eq!(answer_writes(&r), 0, "ready-at-boot sources keep nothing");
}

#[test]
fn runner_owned_facts_never_admit_or_persist_kept_answers() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let time = b.record("Time", &[("epochAtZero", number)]);
    let answer = b.resource("answer", "exactTime", &[], time, None);
    b.set_resource_reader(answer, true);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let mut r = boot(
        b.finish().unwrap(),
        Source::default(),
        entry(&[], &Value::record(vec![Value::Number(999.)])),
    );
    assert_eq!(
        r.resource("answer"),
        Some(&Value::record(vec![Value::Number(0.)]))
    );
    assert!(!state(&r).kept_seed);
    r.set_time(1000., 0.).unwrap();
    assert_eq!(
        r.resource("answer"),
        Some(&Value::record(vec![Value::Number(1000.)]))
    );
    assert_eq!(answer_writes(&r), 0);
    assert!(r.data.asks.is_empty());
}

#[test]
fn a_cold_boot_forgets_kept_answers_no_declared_reader_seeds() {
    // `answer` is a declared reader; `gone` names no resource, as after a
    // reader is removed or made transient (the Brooks port's Health summary).
    let mut snapshot = entry(&[Value::str("A")], &Value::str("kept"));
    let gone = kept::kept_name("gone");
    snapshot.push((gone.clone(), kept::encode(&[], &Value::str("private"))));
    let r = boot(plan(2, false), Source::default(), snapshot);
    shows(&r, "kept");
    assert_eq!(r.store.kept(&gone), None);
    let removed: Vec<_> = r
        .store
        .writes()
        .iter()
        .filter(|w| w.value.is_none())
        .map(|w| w.name.as_str())
        .collect();
    assert_eq!(removed, [gone.as_str()]);
    assert_eq!(answer_writes(&r), 0);
}

#[test]
fn an_app_granting_health_neither_seeds_nor_keeps_answers() {
    // A reader's answer kept before the grant (or by an older build) is
    // forgotten on disk and does not paint the first frame; the fresh answer
    // is never kept (LLP 1069.008.000 D7).
    let source = Source {
        health: true,
        ..Source::default()
    };
    let mut r = boot(
        plan(2, false),
        source,
        entry(&[Value::str("A")], &Value::str("kept")),
    );
    shows(&r, "loading");
    assert!(!state(&r).kept_seed);
    assert_eq!(r.store.kept(&kept::kept_name("answer")), None);
    let removed: Vec<_> = r
        .store
        .writes()
        .iter()
        .filter(|w| w.value.is_none())
        .map(|w| w.name.clone())
        .collect();
    assert_eq!(removed, [kept::kept_name("answer")]);
    r.data.ready = true;
    assert!(r.data_ready().unwrap().is_some());
    shows(&r, "fresh");
    assert_eq!(r.store.kept(&kept::kept_name("answer")), None);
    assert_eq!(answer_writes(&r), 1, "only the removal");
}
