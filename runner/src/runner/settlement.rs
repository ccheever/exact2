//! Resource and derive settlement, split from the runner's commit shell.

use super::{DataError, DataSource, ResourceState, Runner, RunnerError, Target};
use crate::request::{Answer, Request};
use crate::vm::{self, Env, Trap};
use exact_plan::Value;

#[derive(Clone)]
enum RequestEffect {
    None,
    Answered,
    Later {
        args: Vec<Value>,
        request: Request,
        forced: bool,
    },
}

impl<D: DataSource> Runner<D> {
    pub(super) fn check_shape(&self, i: usize, value: &Value) -> Result<(), RunnerError> {
        let row = &self.plan.resources[i];
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
    /// if it has one; afterwards it is re-requested only when its arguments
    /// changed, so every derive that reads it sees the new value in the same
    /// pass.
    pub(super) fn settle(&mut self, boot: bool) -> Result<(), RunnerError> {
        // LLP 1016: what an action asked to re-request, the requests this
        // pass hands the host, and the pending flags as they will be —
        // published with the rest only when the pass succeeds.
        let mut force = std::mem::take(&mut self.refresh_next);
        // Work on a copy of the committed resource states; publish only when
        // the whole pass succeeds, so a failure leaves every cache as it was.
        let mut states: Vec<Option<ResourceState>> = self.resources.clone();
        let mut effects = vec![RequestEffect::None; states.len()];
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
                    let reuse = states[i]
                        .as_ref()
                        .filter(|s| {
                            s.args == args
                                && !forced
                                && (!self.store_readers[i]
                                    || s.store_revision == self.store.revision())
                        })
                        .map(|s| s.value.clone());
                    let value = match reuse {
                        Some(v) => v,
                        None if boot
                            && row.initial.len > 0
                            && (!self.store_readers[i] || self.stale[i]) =>
                        {
                            // A store-reading resource whose source is not
                            // ready at boot takes its compiled empty-store
                            // placeholder (LLP 1027 D4) and is asked again at
                            // `data_ready`.
                            Value::from_bytes(self.plan.bytes(row.initial))
                                .map_err(RunnerError::Plan)?
                        }
                        None => {
                            // A resource that consults the store is the device's,
                            // not the build's: bake gives it no compiled value
                            // (LLP 1018 D4).
                            let reads_before = self.store.reads();
                            effects[i] = RequestEffect::None;
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
                                            (row.initial.len > 0)
                                                .then(|| {
                                                    Value::from_bytes(self.plan.bytes(row.initial))
                                                        .ok()
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
                                        request,
                                        forced,
                                    };
                                    pending_res[i] = true;
                                    kept
                                }
                            }
                        }
                    };
                    self.check_shape(i, &value)?;
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
            for (i, effect) in effects.into_iter().enumerate() {
                if let RequestEffect::Later {
                    args,
                    request,
                    forced,
                } = effect
                {
                    let source = self.plan.str(self.plan.resources[i].source).to_string();
                    self.enqueue(Target::Resource(i), source, args, request, forced);
                }
            }
            return Ok(());
        }
    }
}
