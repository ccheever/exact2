//! Resource and derive settlement, split from the runner's commit shell.

use super::{DataError, DataSource, ResourceState, Runner, RunnerError, Target};
use crate::request::{Answer, Request};
use crate::vm::{self, Env, Trap};
use exact_kernel::CommitReceipt;
use exact_plan::Value;

#[derive(Clone)]
enum RequestEffect {
    None,
    Answered,
    Later {
        args: Vec<Value>,
        request: Box<Request>,
        forced: bool,
    },
}

impl<D: DataSource> Runner<D> {
    /// Ask `which` resources again in one commit — what `data_ready` does
    /// when a TypeScript module finally loads (LLP 1027 D4) and what
    /// `set_delivery` does when a host hands the runner its delivery facts
    /// (LLP 1030 D7). Nothing survives a refusal: the store, the slots, and
    /// every settled resource go back as they were. `None` when the list
    /// is empty — there was nothing to ask.
    pub(super) fn recommit(
        &mut self,
        which: Vec<usize>,
        what: &str,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if which.is_empty() {
            return Ok(None);
        }
        let what = format!("{what} ({} asked again)", which.len());
        let was_poisoned = self.poisoned;
        let kept = self.store.checkpoint();
        let since = kept.writes;
        let saved_slots = self.slots.clone();
        let saved_resources = self.resources.clone();
        self.refresh_next.extend(which);
        let result = if self.poisoned {
            Err(RunnerError::Poisoned)
        } else {
            self.settle(false).and_then(|_| self.update())
        };
        match &result {
            Ok(_) => self.log_store_writes(since),
            Err(_) => {
                self.store.restore(kept);
                self.slots = saved_slots;
                self.resources = saved_resources;
            }
        }
        self.log_outcome(&what, &result, was_poisoned);
        result.map(Some)
    }

    pub(super) fn check_shape(&self, i: usize, value: &Value) -> Result<(), RunnerError> {
        let row = &self.plan.resources[i];
        #[cfg(test)]
        tests::SHAPE_CHECKS.with(|count| count.set(count.get() + 1));
        if value.conforms(&self.plan, row.ty) {
            Ok(())
        } else {
            Err(RunnerError::Shape {
                resource: self.plan.str(row.name).to_string(),
            })
        }
    }

    /// Settle every derive and resource against current state, in plan
    /// order, to a fixpoint: an expression that reads something not yet
    /// settled this pass is retried after it settles. Deterministic, and a
    /// cycle is a typed refusal. On boot a resource takes its compiled value
    /// if its evaluated arguments equal the baked arguments (LLP 1038 D5);
    /// afterwards it is re-requested only when its arguments changed, so
    /// every derive that reads it sees the new value in the same
    /// pass.
    pub(super) fn settle(&mut self, boot: bool) -> Result<(), RunnerError> {
        let mut effects = Vec::new();
        let result = self.settle_pass(boot, &mut effects);
        if result.is_err() {
            // A refused pass hands nothing out: the `Later` answers it
            // collected are dropped, and their sources forget the calls
            // (LLP 1027.002 D3, the cleanup of rolled-back calls).
            for effect in effects {
                if let RequestEffect::Later { request, .. } = effect {
                    self.discard_request(&request);
                }
            }
        }
        result
    }

    fn settle_pass(
        &mut self,
        boot: bool,
        effects: &mut Vec<RequestEffect>,
    ) -> Result<(), RunnerError> {
        // LLP 1016: what an action asked to re-request, the requests this
        // pass hands the host, and the pending flags as they will be —
        // published with the rest only when the pass succeeds.
        let mut force = std::mem::take(&mut self.refresh_next);
        // Work on a copy of the committed resource states; publish only when
        // the whole pass succeeds, so a failure leaves every cache as it was.
        let mut states: Vec<Option<ResourceState>> = self.resources.clone();
        *effects = vec![RequestEffect::None; states.len()];
        let mut passes = 0usize;
        loop {
            passes += 1;
            if passes > self.plan.resources.len() + 1 {
                return Err(RunnerError::Cycle);
            }
            let mut derives: Vec<Option<Value>> = vec![None; self.plan.derives.len()];
            let mut derive_store_dependent = vec![false; self.plan.derives.len()];
            let mut resources: Vec<Option<Value>> = vec![None; self.plan.resources.len()];
            let mut pending_res = self.pending_res.clone();
            for (i, effect) in effects.iter().enumerate() {
                match effect {
                    RequestEffect::None => {}
                    RequestEffect::Answered => pending_res[i] = false,
                    RequestEffect::Later { .. } => pending_res[i] = true,
                }
            }
            let mut settled_res = vec![false; states.len()];
            loop {
                let mut progress = false;
                let mut all = true;
                for i in 0..self.plan.derives.len() {
                    if derives[i].is_some() {
                        continue;
                    }
                    let code = self.plan.derives[i].body;
                    let result = {
                        let env = Env {
                            plan: &self.plan,
                            router: self.router.as_ref(),
                            slots: &self.slots,
                            derives: &derives,
                            resources: &resources,
                            params: &[],
                            frames: &[],
                            now_ms: self.now_ms,
                            pending_resources: &pending_res,
                            pending_mutations: &self.pending_mut,
                            store_dependent_derives: &derive_store_dependent,
                            store_dependent_resources: &self.store_readers,
                        };
                        vm::eval(self.plan.code(code), &env, &[])
                    };
                    match result {
                        Ok(o) => {
                            if !o.value.conforms(&self.plan, self.plan.derives[i].ty) {
                                return Err(RunnerError::DeriveType {
                                    derive: self.plan.str(self.plan.derives[i].name).to_string(),
                                });
                            }
                            derive_store_dependent[i] = o.store_dependent;
                            derives[i] = Some(o.value);
                            progress = true;
                        }
                        Err(Trap::Pending { .. }) => all = false,
                        Err(t) => return Err(t.into()),
                    }
                }
                for i in 0..self.plan.resources.len() {
                    if settled_res[i] {
                        continue;
                    }
                    let row = self.plan.resources[i].clone();
                    let mut args = Vec::with_capacity(row.args.len as usize);
                    let mut pending = false;
                    let mut store_dependent = false;
                    for a in row.args.iter() {
                        let code = self.plan.arg(a).expr;
                        let result = {
                            let env = Env {
                                plan: &self.plan,
                                router: self.router.as_ref(),
                                slots: &self.slots,
                                derives: &derives,
                                resources: &resources,
                                params: &[],
                                frames: &[],
                                now_ms: self.now_ms,
                                pending_resources: &pending_res,
                                pending_mutations: &self.pending_mut,
                                store_dependent_derives: &derive_store_dependent,
                                store_dependent_resources: &self.store_readers,
                            };
                            vm::eval(self.plan.code(code), &env, &[])
                        };
                        match result {
                            Ok(o) => {
                                store_dependent |= o.store_dependent;
                                args.push(o.value);
                            }
                            Err(Trap::Pending { .. }) => {
                                pending = true;
                                break;
                            }
                            Err(t) => return Err(t.into()),
                        }
                    }
                    if pending {
                        all = false;
                        continue;
                    }
                    // Store provenance follows the values used to form a
                    // resource query, including through derives. Such a
                    // resource is device data just like a direct reader.
                    self.store_readers[i] |= store_dependent;
                    let forced = force.contains(&i);
                    // A store-reading resource is reusable only at the exact
                    // store revision it observed. `answer` and `parse` both
                    // write through Store, so this is the one dirtying point.
                    // Host facts always come from this candidate, never a baked
                    // or carried delivery answer (LLP 1030 D7).
                    let reuse = states[i]
                        .as_ref()
                        .filter(|s| {
                            s.args == args
                                && self.plan.str(row.source) != crate::delivery::SOURCE
                                && self.plan.str(row.source) != crate::viewport::SOURCE
                                && self.plan.str(row.source) != crate::surface_record::SOURCE
                                && !forced
                                && (!self.store_readers[i]
                                    || s.store_revision == self.store.revision())
                        })
                        .map(|s| s.value.clone());
                    // Every ResourceState was checked for this index in this
                    // Runner's immutable plan: settlement checks new values,
                    // boot rechecks carried/kept values, and fulfil checks replies.
                    // Cloning that immutable value preserves its shape. Only this
                    // successful reuse skips validation; fresh/baked/fallback
                    // answers still cross the shape boundary below.
                    let reused = reuse.is_some();
                    let value = match reuse {
                        Some(v) => v,
                        None if (boot || !self.data.ready())
                            && self.plan.str(row.source) != crate::delivery::SOURCE
                            && self.plan.str(row.source) != crate::viewport::SOURCE
                            && self.plan.str(row.source) != crate::surface_record::SOURCE
                            && row.initial.len > 0
                            && (!self.data.ready()
                                || (Value::from_bytes(self.plan.bytes(row.initial_args))
                                    .map_err(RunnerError::Plan)?
                                    == Value::list(args.clone())
                                    && (!self.store_readers[i] || self.stale[i]))) =>
                        {
                            // Keep deferred placeholders until activation, even
                            // when timers or a deep launch change arguments after
                            // boot (LLP 1038 D5, LLP 1027 D4). data_ready asks the
                            // current arguments once the executor can answer.
                            if !self.data.ready() {
                                self.stale[i] = true;
                            }
                            Value::from_bytes(self.plan.bytes(row.initial))
                                .map_err(RunnerError::Plan)?
                        }
                        None => {
                            // A resource that consults the store is the device's,
                            // not the build's: bake gives it no compiled value
                            // (LLP 1018 D4).
                            let reads_before = self.store.reads();
                            // Asked again in a later pass: an earlier
                            // `Later` answer was never handed out.
                            if let RequestEffect::Later { request, .. } =
                                std::mem::replace(&mut effects[i], RequestEffect::None)
                            {
                                self.discard_request(&request);
                            }
                            pending_res[i] = self.pending_res[i];
                            let answer = self.query(i, &args)?;
                            force.retain(|forced| *forced != i);
                            if self.store.reads() > reads_before {
                                self.store_readers[i] = true;
                            }
                            match answer {
                                Answer::Now(v) => {
                                    if pending_res[i] {
                                        // Newer arguments answered now: the older
                                        // request's reply is no longer wanted.
                                        effects[i] = RequestEffect::Answered;
                                        pending_res[i] = false;
                                    }
                                    self.stale[i] = false;
                                    self.keep_answer(i, &args, &v);
                                    v
                                }
                                Answer::Later(request) => {
                                    // The host will run it. Meanwhile the resource
                                    // keeps the value it had — its last answer, or
                                    // its compiled boot value (LLP 1016 D3).
                                    let kept =
                                        states[i].as_ref().map(|s| s.value.clone()).or_else(|| {
                                            (row.initial.len > 0
                                                && Value::from_bytes(
                                                    self.plan.bytes(row.initial_args),
                                                )
                                                .ok()
                                                    == Some(Value::list(args.clone())))
                                            .then(|| {
                                                Value::from_bytes(self.plan.bytes(row.initial)).ok()
                                            })
                                            .flatten()
                                        });
                                    let Some(kept) = kept else {
                                        return Err(RunnerError::Data {
                                            resource: self.plan.str(row.name).to_string(),
                                            error: DataError::Unavailable(
                                                "answers later at boot: declare boot arguments the source answers now"
                                                    .into(),
                                            ),
                                        });
                                    };
                                    effects[i] = RequestEffect::Later {
                                        args: args.clone(),
                                        request: Box::new(request),
                                        forced,
                                    };
                                    pending_res[i] = true;
                                    kept
                                }
                            }
                        }
                    };
                    if !reused {
                        self.check_shape(i, &value)?;
                    }
                    resources[i] = Some(value.clone());
                    states[i] = Some(ResourceState {
                        args,
                        value,
                        store_revision: self.store.revision(),
                    });
                    settled_res[i] = true;
                    progress = true;
                }
                if all && settled_res.iter().all(|s| *s) {
                    break;
                }
                if !progress {
                    return Err(RunnerError::Cycle);
                }
            }

            let revision = self.store.revision();
            // A resource later in plan order may have written after an
            // earlier reader settled. Repeat against the tentative states;
            // the writer's state already carries the new revision, so it
            // does not refresh itself.
            let stale_store_reader = states.iter().enumerate().any(|(i, state)| {
                self.store_readers[i]
                    && state
                        .as_ref()
                        .is_none_or(|state| state.store_revision != revision)
            });
            if stale_store_reader {
                continue;
            }

            self.derives = derives;
            self.resource_values = resources;
            self.resources = states;
            for (i, effect) in effects.iter().enumerate() {
                if matches!(effect, RequestEffect::Answered) {
                    self.forget(Target::Resource(i));
                }
            }
            for (i, effect) in std::mem::take(effects).into_iter().enumerate() {
                if let RequestEffect::Later {
                    args,
                    request,
                    forced,
                } = effect
                {
                    let source = self.plan.str(self.plan.resources[i].source).to_string();
                    self.enqueue(Target::Resource(i), source, args, *request, forced);
                }
            }
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Carried, Outcome, Store};
    use exact_kernel::{Kernel, NodeType};
    use exact_plan::{asm::Asm, builder::PlanBuilder, Plan, Stdlib, TypeKind};
    use std::{cell::Cell, rc::Rc};

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
            }
        }
    }

    impl DataSource for Data {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!("the fixture implements answer")
        }

        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            self.queries += 1;
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
            Ok(Answer::Now(self.value.clone()))
        }

        fn ready(&self) -> bool {
            self.ready
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
        assert!(Rc::ptr_eq(before, after));
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
        Rc::make_mut(items)[24_999] = Value::record(vec![Value::str("invalid tail")]);
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
    fn async_answers_validate_before_reuse_and_refusal_keeps_ticket_and_store() {
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
        assert!(matches!(
            r.fulfill(ticket, Outcome::Storage(vec![])),
            Err(RunnerError::Shape { .. })
        ));
        assert_eq!(r.carry(), before);
        r.data().value = records(2);
        assert!(r
            .fulfill(ticket, Outcome::Storage(vec![]))
            .unwrap()
            .is_some());
        assert_eq!(r.resource("rows"), Some(&records(2)));
        assert!(r
            .carry()
            .store
            .contains(&("token".into(), "candidate".into())));
        checks();
        tick(&mut r);
        assert_eq!(checks(), 0);
        assert!(r
            .fulfill(ticket, Outcome::Storage(vec![]))
            .unwrap()
            .is_none());
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
        assert!(matches!(
            r.fulfill(ticket, Outcome::Storage(vec![])),
            Err(RunnerError::Shape { .. })
        ));
        assert_eq!(r.resource("rows"), Some(&records(1)));
        assert_eq!(r.carry(), before);
        assert_eq!(r.kernel().export(None).unwrap(), tree);
        assert!(!r.is_poisoned());
        r.data().bad_guard = false;
        assert!(r
            .fulfill(ticket, Outcome::Storage(vec![]))
            .unwrap()
            .is_some());
        assert_eq!(r.resource("rows"), Some(&records(2)));
        checks();
        tick(&mut r);
        assert_eq!(checks(), 0);
    }
}
