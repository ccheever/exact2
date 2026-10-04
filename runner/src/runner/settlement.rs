//! Resource and derive settlement, split from the runner's commit shell.

use super::{DataError, DataSource, ResourceState, Runner, RunnerError, Target};
use crate::held::Held;
use crate::request::{Answer, Request};
use crate::vm::{self, Env, Trap};
use exact_kernel::CommitReceipt;
use exact_plan::{ResourcesRow, TypeKind, Value};

/// What the published derives were computed against: an input equal to
/// its value here has not changed since the last successful settlement.
pub(super) struct Settled {
    slots: Vec<Value>,
    now_ms: f64,
    pending_res: Vec<bool>,
    failed_res: Vec<bool>,
    pending_mut: Vec<bool>,
    store_readers: Vec<bool>,
    resource_args: Vec<Vec<Value>>,
}

/// Which derives and resources this pass has settled, and which of them
/// differ from what was published last — in value or in store provenance,
/// which travels on even when the value does not change.
struct Progress<'a> {
    derives: &'a [Option<Value>],
    derive_changed: &'a [bool],
    resources: &'a [bool],
    resource_changed: &'a [bool],
    pending_res: &'a [bool],
}

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
        self.commit_again(which, what).map(Some)
    }

    /// [`Runner::recommit`]'s one commit, even when it asks nothing again:
    /// a pending set that changed outside a commit still reaches the view.
    pub(super) fn commit_again(
        &mut self,
        which: Vec<usize>,
        what: &str,
    ) -> Result<CommitReceipt, RunnerError> {
        let what = format!("{what} ({} asked again)", which.len());
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(false);
        self.refresh_next.extend(which);
        let result = if self.poisoned {
            Err(RunnerError::Poisoned)
        } else {
            self.settle(false).and_then(|_| self.update())
        };
        self.conclude(checkpoint, &result, was_poisoned);
        self.arm_next(result.is_ok());
        self.log_outcome(&what, &result, was_poisoned);
        result
    }

    /// What a resource shows while its source hasn't answered and nothing
    /// is kept (LLP 1048.003 D6): its declared placeholder row's value this
    /// pass, or a list's or an option's empty value.
    fn placeholder(
        &self,
        i: usize,
        row: &ResourcesRow,
        resources: &[Option<Held>],
    ) -> Option<Value> {
        match row.placeholder {
            Some(p) => resources[p.0 as usize]
                .as_ref()
                .map(|h| h.get(&self.plan).clone()),
            // @ref LLP 1054.000.002 D1/D3 — a declared `else empty(…)`, or
            // the type's zero.
            None if row.placeholder_value.len > 0 => {
                Value::from_bytes(self.plan.bytes(row.placeholder_value)).ok()
            }
            // A declared `else source()` row must answer now: no zero hides
            // one that answers later (its owner is refused, by name).
            None if self.is_placeholder_row(i) => None,
            None => zero(&self.plan, row.ty),
        }
    }

    /// The journal's word when an answer lands on a build-time one shown
    /// until its source was ready (LLP 1048.003 D6).
    pub(super) fn revalidated(&mut self, i: usize, shown: Option<&ResourceState>, answer: &Value) {
        let Some(shown) = shown.filter(|s| self.stale[i] && s.value.is_compiled()) else {
            return;
        };
        let same = crate::compare::equivalent(shown.value.get(&self.plan), answer);
        let line = super::lines::revalidated(self.plan.str(self.plan.resources[i].name), same);
        self.log(line);
    }

    /// Whether resource `i` is another's `else` row (LLP 1048.003 D6).
    fn is_placeholder_row(&self, i: usize) -> bool {
        self.plan
            .resources
            .iter()
            .any(|r| r.placeholder.is_some_and(|p| p.0 as usize == i))
    }

    /// A resource whose source answers later with nothing to show: no
    /// answer kept for its arguments and no placeholder (LLP 1048.003 D6).
    fn unanswerable(&self, i: usize) -> RunnerError {
        let name = self.plan.str(self.plan.resources[i].name);
        let owner = self
            .plan
            .resources
            .iter()
            .find(|r| r.placeholder.is_some_and(|p| p.0 as usize == i));
        let (resource, message) = match owner {
            Some(owner) => (
                self.plan.str(owner.name),
                "its placeholder answers later; a placeholder answers now".to_string(),
            ),
            None => (
                name,
                format!("answers later, with nothing kept for these arguments: declare a placeholder, `resource {name} = … else source()`"),
            ),
        };
        RunnerError::Data {
            resource: resource.to_string(),
            error: DataError::Unavailable(message),
        }
    }

    pub(super) fn check_shape(&self, i: usize, value: &Value) -> Result<(), RunnerError> {
        let row = &self.plan.resources[i];
        #[cfg(test)]
        tests::SHAPE_CHECKS.with(|count| count.set(count.get() + 1));
        if self.conforms(value, row.ty) {
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

    /// Whether every input `reads` names has settled in this pass to the
    /// value it had when the published values were computed.
    fn unchanged(&self, reads: &crate::instance::DepReads, now: &Progress<'_>) -> bool {
        use crate::instance::Input;
        let Some(base) = self
            .settled
            .as_ref()
            .filter(|_| !self.full && !reads.opaque)
        else {
            return false;
        };
        self.sites.deps().inputs(reads).all(|input| match input {
            Input::Slot(k) => crate::compare::equivalent(&base.slots[k], &self.slots[k]),
            Input::Derive(j) => now.derives[j].is_some() && !now.derive_changed[j],
            Input::Resource(r) => {
                now.resources[r]
                    && !now.resource_changed[r]
                    && base.store_readers[r] == self.store_readers[r]
            }
            Input::PendingResource(r) => {
                now.resources[r] && base.pending_res[r] == now.pending_res[r]
            }
            Input::FailedResource(r) => {
                now.resources[r] && base.failed_res[r] == self.failed_args[r].is_some()
            }
            Input::PendingMutation(m) => base.pending_mut[m] == self.pending_mut[m],
            Input::Clock => base.now_ms.to_bits() == self.now_ms.to_bits(),
        })
    }

    fn settle_pass(
        &mut self,
        boot: bool,
        effects: &mut Vec<RequestEffect>,
    ) -> Result<(), RunnerError> {
        // Store provenance as the derives below will see it.
        let store_readers = self.store_readers.clone();
        // LLP 1016: what an action asked to re-request, the requests this
        // pass hands the host, and the pending flags as they will be —
        // published with the rest only when the pass succeeds.
        let mut force = std::mem::take(&mut self.refresh_next);
        // LLP 1054.000.000 D1: what a send changed, asked again for what the
        // source knows now — a request it hands back is not sent.
        let reread = std::mem::take(&mut self.reread_next);
        // Work on a copy of the committed resource states; publish only when
        // the whole pass succeeds, so a failure leaves every cache as it was.
        let mut states: Vec<Option<ResourceState>> = self.resources.clone();
        effects.clear();
        effects.resize_with(states.len(), || RequestEffect::None);
        // Which resources took their compiled value in this settlement: each
        // source is told once it is published (LLP 1027 D11).
        let mut adopted = vec![false; states.len()];
        let mut resource_args = vec![Vec::new(); states.len()];
        let mut passes = 0usize;
        loop {
            passes += 1;
            if passes > self.plan.resources.len() + 1 {
                return Err(RunnerError::Cycle);
            }
            let mut derives: Vec<Option<Value>> = vec![None; self.plan.derives.len()];
            let mut derive_store_dependent = vec![false; self.plan.derives.len()];
            let mut resources: Vec<Option<Held>> = vec![None; self.plan.resources.len()];
            let mut pending_res = self.pending_res.clone();
            let mut awaiting = self.awaiting.clone();
            for (i, effect) in effects.iter().enumerate() {
                match effect {
                    RequestEffect::None => {}
                    RequestEffect::Answered => pending_res[i] = false,
                    RequestEffect::Later { .. } => pending_res[i] = true,
                }
            }
            let mut settled_res = vec![false; states.len()];
            let mut derive_changed = vec![false; self.plan.derives.len()];
            let mut resource_changed = vec![false; states.len()];
            loop {
                let mut progress = false;
                let mut all = true;
                for i in 0..self.plan.derives.len() {
                    if derives[i].is_some() {
                        continue;
                    }
                    // Unchanged inputs, unchanged value: no evaluation.
                    let now = Progress {
                        derives: &derives,
                        derive_changed: &derive_changed,
                        resources: &settled_res,
                        resource_changed: &resource_changed,
                        pending_res: &pending_res,
                    };
                    if self.derives[i].is_some()
                        && self.unchanged(&self.sites.deps().derives[i], &now)
                    {
                        derives[i] = self.derives[i].clone();
                        derive_store_dependent[i] = self.derive_store_dependent[i];
                        progress = true;
                        continue;
                    }
                    let code = self.plan.derives[i].body;
                    self.derives_evaluated += 1;
                    let result = {
                        let env = Env {
                            plan: &self.plan,
                            strings: &self.strings,
                            router: self.router.as_deref(),
                            lists: self.links.lists,
                            format: self.links.format,
                            geometry: None,
                            slots: &self.slots,
                            derives: &derives,
                            resources: &resources,
                            params: &[],
                            frames: &[],
                            now_ms: self.now_ms,
                            pending_resources: &pending_res,
                            failed_resources: &self.failed_args,
                            pending_mutations: &self.pending_mut,
                            store_dependent_derives: &derive_store_dependent,
                            store_dependent_resources: &self.store_readers,
                        };
                        vm::eval(self.plan.code(code), &env, &[])
                    };
                    match result {
                        Ok(o) => {
                            if !self.conforms(&o.value, self.plan.derives[i].ty) {
                                return Err(RunnerError::DeriveType {
                                    derive: self.plan.str(self.plan.derives[i].name).to_string(),
                                });
                            }
                            // Provenance is part of what a reader inherits.
                            derive_changed[i] =
                                self.derive_store_dependent.get(i) != Some(&o.store_dependent);
                            derive_store_dependent[i] = o.store_dependent;
                            // An equivalent result keeps the previous object,
                            // so identity survives for every memo downstream.
                            derives[i] = Some(match &self.derives[i] {
                                Some(old) if crate::compare::equivalent(old, &o.value) => {
                                    old.clone()
                                }
                                _ => {
                                    derive_changed[i] = true;
                                    o.value
                                }
                            });
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
                    let now = Progress {
                        derives: &derives,
                        derive_changed: &derive_changed,
                        resources: &settled_res,
                        resource_changed: &resource_changed,
                        pending_res: &pending_res,
                    };
                    let kept_args = self
                        .settled
                        .as_ref()
                        .filter(|_| self.unchanged(&self.sites.deps().resource_args[i], &now))
                        .map(|s| s.resource_args[i].clone());
                    for a in row.args.iter().filter(|_| kept_args.is_none()) {
                        let code = self.plan.arg(a).expr;
                        let result = {
                            let env = Env {
                                plan: &self.plan,
                                strings: &self.strings,
                                router: self.router.as_deref(),
                                lists: self.links.lists,
                                format: self.links.format,
                                geometry: None,
                                slots: &self.slots,
                                derives: &derives,
                                resources: &resources,
                                params: &[],
                                frames: &[],
                                now_ms: self.now_ms,
                                pending_resources: &pending_res,
                                failed_resources: &self.failed_args,
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
                    // @ref LLP 1048.003 D6 — a placeholder row settles first,
                    // so a source that answers later has something to show.
                    if row.placeholder.is_some_and(|p| !settled_res[p.0 as usize]) {
                        all = false;
                        continue;
                    }
                    if let Some(kept) = kept_args {
                        args = kept;
                    }
                    resource_args[i] = args.clone();
                    // Store provenance follows the values used to form a
                    // resource query, including through derives. Such a
                    // resource is device data just like a direct reader.
                    self.store_readers[i] |= store_dependent;
                    let forced = force.contains(&i);
                    if forced
                        || self.failed_args[i]
                            .as_ref()
                            .is_some_and(|failed| *failed != args)
                    {
                        self.failed_args[i] = None;
                    }
                    let failed = self.failed_args[i].as_ref() == Some(&args);
                    // The value may still answer older arguments while a
                    // request is in flight. Reuse follows the current ask;
                    // the standing value keeps its own provenance.
                    let asked_args = match &effects[i] {
                        RequestEffect::Later { args, .. } => Some(args),
                        RequestEffect::Answered => None,
                        RequestEffect::None => self
                            .pending
                            .iter()
                            .find(|p| p.target == Target::Resource(i))
                            .map(|p| &p.args),
                    };
                    // Only for the same arguments: new ones need their request.
                    let reread = !forced
                        && reread.contains(&i)
                        && states[i]
                            .as_ref()
                            .is_some_and(|s| asked_args.unwrap_or(&s.args) == &args);
                    // @ref LLP 1027.005 D3/D4 — only a persisted boot seed
                    // ignores context, and only until its source is ready.
                    // Consume the mark on mismatch or activation even when
                    // the old value stands during an asynchronous request.
                    // `states` is tentative, so a refused pass restores both.
                    let kept_seed = states[i].as_ref().is_some_and(|s| {
                        s.kept_seed
                            && !self.data.ready()
                            && s.args == args[..row.args.len as usize - usize::from(row.context)]
                    });
                    if let Some(state) = &mut states[i] {
                        state.kept_seed = kept_seed;
                    }
                    // A store-reading resource is reusable only at the exact
                    // store revision it observed. `answer` and `parse` both
                    // write through Store, so this is the one dirtying point.
                    // Host facts always come from this candidate, never a baked
                    // or carried delivery answer (LLP 1030 D7).
                    let reuse = states[i]
                        .as_ref()
                        .filter(|s| {
                            kept_seed
                                || failed
                                || (asked_args.unwrap_or(&s.args) == &args
                                && self.plan.str(row.source) != crate::delivery::SOURCE
                                && self.plan.str(row.source) != crate::viewport::SOURCE
                                && self.plan.str(row.source) != crate::time::SOURCE
                                && self.plan.str(row.source) != crate::page::SOURCE
                                && self.plan.str(row.source) != crate::surface_record::SOURCE
                                && !forced
                                && !reread
                                && (!self.store_readers[i]
                                    || s.store_revision == self.store.revision())
                                // @ref LLP 1054.000.002 D4 — a placeholder stands
                                // in only while its answer is on the way.
                                && (!s.placeholder || pending_res[i] || awaiting[i]))
                        })
                        .map(|s| s.value.clone());
                    let mut value_args = if reuse.is_some() {
                        states[i].as_ref().expect("reused").args.clone()
                    } else {
                        args.clone()
                    };
                    // Whether what shows is a stand-in, not an answer (D4).
                    let mut placeholder =
                        reuse.is_some() && states[i].as_ref().is_some_and(|s| s.placeholder);
                    // Every ResourceState was checked for this index in this
                    // Runner's immutable plan: settlement checks new values,
                    // boot rechecks carried/kept values, and fulfil checks replies.
                    // Cloning that immutable value preserves its shape. Only this
                    // successful reuse skips validation; fresh/baked/fallback
                    // answers still cross the shape boundary below.
                    let reused = reuse.is_some();
                    // A reused value keeps what an earlier pass recorded;
                    // anything else is recorded by the branch that takes it.
                    let compiled = std::mem::replace(&mut adopted[i], false);
                    let value = match reuse {
                        Some(v) => v,
                        None if (boot || !self.data.ready())
                            && self.plan.str(row.source) != crate::delivery::SOURCE
                            && self.plan.str(row.source) != crate::viewport::SOURCE
                            && self.plan.str(row.source) != crate::time::SOURCE
                            && self.plan.str(row.source) != crate::page::SOURCE
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
                            // boot (LLP 1038 D5, LLP 1027 D4). The bake's answer
                            // is the first frame, not the answer: data_ready asks
                            // the current arguments once the executor can answer,
                            // as the web asks its module at launch (LLP 1048.003
                            // D6, 2026-10-04; feed F24). An `else` row is the
                            // bake's for every launch and is never asked again
                            // (each was a worker turn, c318ed04), unless forced.
                            if !self.data.ready()
                                && (forced || self.store_readers[i] || !self.is_placeholder_row(i))
                            {
                                self.stale[i] = true;
                            }
                            awaiting[i] = false;
                            adopted[i] = true;
                            Held::compiled(
                                Value::from_bytes(self.plan.bytes(row.initial))
                                    .map_err(RunnerError::Plan)?,
                                row.initial,
                            )
                        }
                        // @ref LLP 1048.003 D6 — the source can't answer yet
                        // (its module isn't loaded) and nothing is compiled:
                        // the placeholder shows, pending; `data_ready` asks.
                        None if !self.data.ready()
                            && !exact_plan::runner_owned_source(self.plan.str(row.source))
                            && self.placeholder(i, &row, &resources).is_some() =>
                        {
                            self.stale[i] = true;
                            pending_res[i] = true;
                            awaiting[i] = true;
                            placeholder = true;
                            Held::new(self.placeholder(i, &row, &resources).expect("checked"))
                        }
                        None => {
                            awaiting[i] = false;
                            // A resource that consults the store is the device's,
                            // not the build's: bake gives it no compiled value
                            // (LLP 1018 D4).
                            let reads_before = self.store.reads();
                            let draws_before = self.store.entropy_draws();
                            // Asked again in a later pass: an earlier
                            // `Later` answer was never handed out.
                            if let RequestEffect::Later { request, .. } =
                                std::mem::replace(&mut effects[i], RequestEffect::None)
                            {
                                self.discard_request(&request);
                            }
                            pending_res[i] = self.pending_res[i];
                            self.store.take_topics();
                            let answer = match self.query(i, &args) {
                                // A Rust module's seam that could not carry
                                // the call (an answer over its bound, a trap)
                                // fails the resource, as a failed reply does:
                                // the value it had stands and `failed` says
                                // so. A source's own refusal still refuses
                                // the commit.
                                Err(RunnerError::Data {
                                    error: DataError::Interface(why),
                                    ..
                                }) if !exact_plan::runner_owned_source(
                                    self.plan.str(row.source),
                                ) && states[i].is_some() =>
                                {
                                    self.log(super::lines::failed_now(
                                        self.plan.str(row.name),
                                        &why,
                                    ));
                                    self.failed_args[i] = Some(args.clone());
                                    force.retain(|forced| *forced != i);
                                    let state = states[i].as_ref().expect("checked");
                                    resources[i] = Some(state.value.clone());
                                    settled_res[i] = true;
                                    progress = true;
                                    continue;
                                }
                                answer => answer?,
                            };
                            force.retain(|forced| *forced != i);
                            if self.store.reads() > reads_before {
                                self.store_readers[i] = true;
                            }
                            if self.store.entropy_draws() > draws_before {
                                self.entropy_readers[i] = true;
                            }
                            // What a fresh answer watches replaces what the
                            // last one did; its reply's parse may add more.
                            self.watching[i] = self.store.take_topics();
                            match answer {
                                Answer::Now(v) => {
                                    // A re-read shows the source's answer and
                                    // leaves a request in flight to land.
                                    if (pending_res[i] || self.streaming(i)) && !reread {
                                        // Newer arguments answered now: the older
                                        // request's reply is no longer wanted.
                                        effects[i] = RequestEffect::Answered;
                                        pending_res[i] = false;
                                    }
                                    self.revalidated(i, states[i].as_ref(), &v);
                                    self.stale[i] = false;
                                    self.keep_answer(i, &args, &v);
                                    Held::new(v)
                                }
                                Answer::Later(request) if reread => {
                                    // Nothing newer to show before the write
                                    // lands; the reply's refresh asks the host.
                                    // A source that parks calls hears what is
                                    // in flight and drops the one parked here.
                                    self.discard_request(&request);
                                    self.forgot = true;
                                    let state = states[i].as_ref().expect("checked");
                                    placeholder = state.placeholder;
                                    value_args = state.args.clone();
                                    state.value.clone()
                                }
                                Answer::Later(request) => {
                                    // The host will run it. Meanwhile the resource
                                    // keeps the value it had — its last answer, or
                                    // its compiled boot value (LLP 1016 D3).
                                    placeholder = states[i].as_ref().is_some_and(|s| s.placeholder);
                                    if let Some(state) = &states[i] {
                                        value_args = state.args.clone();
                                    }
                                    let kept =
                                        states[i].as_ref().map(|s| s.value.clone()).or_else(|| {
                                            (row.initial.len > 0
                                                && Value::from_bytes(
                                                    self.plan.bytes(row.initial_args),
                                                )
                                                .ok()
                                                    == Some(Value::list(args.clone())))
                                            .then(|| {
                                                Value::from_bytes(self.plan.bytes(row.initial))
                                                    .ok()
                                                    .map(|v| Held::compiled(v, row.initial))
                                            })
                                            .flatten()
                                        });
                                    // @ref LLP 1048.003 D6 — nothing kept for these
                                    // arguments: the placeholder shows, pending.
                                    let kept = kept.or_else(|| {
                                        placeholder = true;
                                        self.placeholder(i, &row, &resources).map(Held::new)
                                    });
                                    let Some(kept) = kept else {
                                        return Err(self.unanswerable(i));
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
                    let value = if reused {
                        adopted[i] = compiled;
                        value
                    } else {
                        self.check_shape(i, value.get(&self.plan))?;
                        // A fresh answer equal to the last keeps its object.
                        match &states[i] {
                            Some(s) if Held::equivalent(&s.value, &value) => s.value.clone(),
                            _ => value,
                        }
                    };
                    resource_changed[i] = !matches!(
                        &self.resource_values[i],
                        Some(old) if Held::same(old, &value)
                    );
                    resources[i] = Some(value.clone());
                    states[i] = Some(ResourceState {
                        args: value_args,
                        value,
                        store_revision: self.store.revision(),
                        placeholder,
                        kept_seed,
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

            self.settled = Some(Settled {
                slots: self.slots.clone(),
                now_ms: self.now_ms,
                pending_res,
                failed_res: self.failed_args.iter().map(Option::is_some).collect(),
                pending_mut: self.pending_mut.clone(),
                store_readers,
                resource_args,
            });
            self.derive_store_dependent = derive_store_dependent;
            self.derives = derives;
            self.resource_values = resources;
            self.resources = states;
            self.awaiting = awaiting;
            self.adopt(&adopted);
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
            // The flags follow the tickets and what awaits its source.
            self.sync_pending_flags();
            // A forced resource not asked in this settlement (an argument
            // still pending, a source not ready) is forced at the next one
            // that can ask it (LLP 1054.000.000 D2).
            self.refresh_next = force;
            return Ok(());
        }
    }

    /// Hand each source the compiled value its resource just took, with the
    /// arguments the bake answered it for (LLP 1027 D11): the one decoded
    /// copy, shared.
    fn adopt(&mut self, adopted: &[bool]) {
        for (i, _) in adopted.iter().enumerate().filter(|(_, a)| **a) {
            let row = &self.plan.resources[i];
            let (Ok(Value::List(args)), Some(state)) = (
                Value::from_bytes(self.plan.bytes(row.initial_args)),
                self.resources[i].as_ref(),
            ) else {
                continue;
            };
            self.data.adopt(
                self.plan.str(row.source),
                &args,
                state.value.get(&self.plan),
            );
        }
    }
}

impl<D: DataSource> Runner<D> {
    /// Release each compiled resource value that nothing outside the
    /// runner holds any more (`crate::held`): a live answer replaced it on
    /// screen, or a source adopted it and answers its edits another way.
    /// The plan's bytes stay; a later read decodes them again. After an
    /// update, when what the tree shows is what it last saw.
    pub(super) fn release_compiled(&mut self) {
        for i in 0..self.resources.len() {
            let (Some(state), Some(Some(read))) =
                (self.resources[i].as_mut(), self.resource_values.get_mut(i))
            else {
                continue;
            };
            let Some(released) = state.value.released() else {
                continue;
            };
            // Every copy must go, or the value stays: the settled state and
            // what expressions read share one cell (settle_pass).
            if !Held::same(read, &state.value) {
                continue;
            }
            state.value = released.clone();
            *read = released.clone();
            if let Some(tree) = self.tree.as_mut() {
                tree.release_resource(i, &released);
            }
        }
    }
}

/// `ty`'s zero (LLP 1054.000.002 D1): `0`, `""`, `false`, `()`, `none`, `[]`,
/// and a record's fields' zeros in order — the compiler's
/// `contract_types::placeholder::zero`, over the plan's tables.
pub(super) fn zero(plan: &exact_plan::Plan, ty: exact_plan::TypesId) -> Option<Value> {
    let row = plan.types.get(ty.0 as usize)?;
    Some(match row.kind {
        TypeKind::Number => Value::Number(0.0),
        TypeKind::String => Value::str(""),
        TypeKind::Bool => Value::Bool(false),
        TypeKind::Unit => Value::Unit,
        TypeKind::Option => Value::NONE,
        TypeKind::List => Value::list(Vec::new()),
        TypeKind::Record => Value::record(
            plan.fields
                .get(row.fields.start as usize..(row.fields.start + row.fields.len) as usize)?
                .iter()
                .map(|f| zero(plan, f.ty))
                .collect::<Option<Vec<_>>>()?,
        ),
    })
}

#[cfg(test)]
mod tests {
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
}
