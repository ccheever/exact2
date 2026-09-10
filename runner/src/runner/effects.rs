//! Scope settlement and the existing tree's commit walk.
use super::requests::DeferredAnswer;
use super::*;
use crate::scope::{OwnedResource, ScopeRef};
use exact_plan::ResourcesId;

/// A mounted resource's boot seed, with authored keyed identity (never a token).
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedResourceSnapshot {
    /// The resource declaration, distinguishing static component uses.
    pub resource: ResourcesId,
    /// Every enclosing authored `each` key, outermost first.
    pub path: Vec<Value>,
    /// Arguments evaluated in this instance's live environment.
    pub args: Vec<Value>,
    /// The current typed answer or explicit pending fallback.
    pub value: Value,
    /// Whether this instance observed device or store data.
    pub reader: bool,
    /// A placeholder that must launch fresh work when the source is ready.
    pub pending: bool,
}

impl<D: DataSource> Runner<D> {
    /// Enumerate mounted resource values in tree and declaration order for bake.
    pub fn owned_resource_snapshots(&self) -> Vec<OwnedResourceSnapshot> {
        self.scope_frames()
            .into_iter()
            .flat_map(|frames| {
                let cell = frames.last().unwrap().scope.as_ref().unwrap().borrow();
                cell.resources
                    .iter()
                    .filter_map(|(i, r)| {
                        r.state.as_ref().map(|s| OwnedResourceSnapshot {
                            resource: ResourcesId(*i as u32),
                            path: cell.path.clone(),
                            args: s.args.clone(),
                            value: s.value.clone(),
                            reader: r.reader,
                            pending: r.pending || r.stale,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub(crate) fn scope_frames(&self) -> Vec<Vec<Frame>> {
        self.tree.as_ref().map(Tree::scopes).unwrap_or_default()
    }

    pub(super) fn lifetime_frames(&self, lifetime: u64) -> Option<Vec<Frame>> {
        self.scope_frames().into_iter().find(|frames| {
            frames
                .last()
                .unwrap()
                .scope
                .as_ref()
                .unwrap()
                .borrow()
                .lifetime
                == lifetime
        })
    }

    pub(super) fn target_scope(&self, target: Target) -> Option<ScopeRef> {
        let lifetime = match target {
            Target::OwnedResource(_, t) | Target::OwnedMutation(_, t) => t,
            _ => return None,
        };
        self.lifetime_frames(lifetime)?.last()?.scope.clone()
    }

    fn pending_scope_value(
        &self,
        i: usize,
        res: &mut OwnedResource,
        args: &[Value],
        frames: &[Frame],
    ) -> Result<Value, RunnerError> {
        if let Some(value) = res.settled.clone().or_else(|| {
            res.state
                .as_ref()
                .filter(|state| state.args == args)
                .map(|state| state.value.clone())
        }) {
            return Ok(value);
        }
        let row = &self.plan.resources[i];
        if row.fallback.len == 0 {
            return Err(RunnerError::Data { resource: self.plan.str(row.name).to_string(), error: DataError::Unavailable("pending resource has no matching boot seed, settled answer or explicit fallback".into()) });
        }
        let out = vm::eval(self.plan.code(row.fallback), &self.env(&[], frames), &[])?;
        res.reader |= out.store_dependent;
        Ok(out.value)
    }

    fn settle_scope(&mut self, frames: &[Frame]) -> Result<(), RunnerError> {
        let cell = frames
            .last()
            .and_then(|f| f.scope.as_ref())
            .expect("scope callback")
            .clone();
        let (region, lifetime, path) = {
            let c = cell.borrow();
            (c.region, c.lifetime, c.path.clone())
        };
        let which: Vec<_> = self
            .plan
            .resources
            .iter()
            .enumerate()
            .filter(|(_, r)| r.owner == Some(region))
            .map(|(i, _)| i)
            .collect();
        let previous = std::mem::take(&mut cell.borrow_mut().resources);
        let mut todo = which;
        let mut deferred = std::collections::BTreeMap::<usize, DeferredAnswer>::new();
        // Resources can depend on later declarations. Pending reads retry only
        // inside this scope, before any dependent child is realized.
        while !todo.is_empty() {
            let mut progress = false;
            let mut next = Vec::new();
            for i in todo {
                let row = self.plan.resources[i].clone();
                let mut args = Vec::new();
                let mut dependent = false;
                let mut wait = false;
                for a in row.args.iter() {
                    match vm::eval(
                        self.plan.code(self.plan.arg(a).expr),
                        &self.env(&[], frames),
                        &[],
                    ) {
                        Ok(out) => {
                            args.push(out.value);
                            dependent |= out.store_dependent;
                        }
                        Err(Trap::Pending { .. }) => {
                            wait = true;
                            break;
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
                if wait {
                    next.push(i);
                    continue;
                }
                let old = previous.get(&i).cloned();
                let mut res = old.unwrap_or_else(|| OwnedResource {
                    reader: row.reader,
                    ..Default::default()
                });
                let delivery = self.plan.str(row.source) == crate::delivery::SOURCE;
                res.reader |= dependent || delivery;
                let ready = delivery || self.data.ready();
                let forced = res.refresh || (res.stale && ready);
                let reuse = res.state.as_ref().is_some_and(|s| {
                    s.args == args
                        && !delivery
                        && !forced
                        && (!res.reader || res.stale || s.store_revision == self.store.revision())
                });
                if reuse {
                    cell.borrow_mut().resources.insert(i, res);
                    progress = true;
                    continue;
                }
                // Seeds are used during initial realization only; a removed
                // lifetime cannot recover any old answer on a later remount.
                if res.state.is_none() && self.booting && !delivery {
                    for seed in &self.plan.resource_boot {
                        if seed.resource.0 as usize != i {
                            continue;
                        }
                        if Value::from_bytes(self.plan.bytes(seed.path))
                            .map_err(RunnerError::Plan)?
                            != Value::list(path.clone())
                            || Value::from_bytes(self.plan.bytes(seed.args))
                                .map_err(RunnerError::Plan)?
                                != Value::list(args.clone())
                        {
                            continue;
                        }
                        let value = Value::from_bytes(self.plan.bytes(seed.value))
                            .map_err(RunnerError::Plan)?;
                        res.reader |= seed.reader;
                        res.pending = seed.pending;
                        res.stale = res.reader || seed.pending;
                        if !res.stale {
                            res.settled = Some(value.clone());
                        }
                        res.state = Some(ResourceState {
                            args: args.clone(),
                            value,
                            store_revision: self.store.revision(),
                        });
                        break;
                    }
                    if res.state.is_some() && (!res.stale || !ready) {
                        cell.borrow_mut().resources.insert(i, res);
                        progress = true;
                        continue;
                    }
                }
                let target = Target::OwnedResource(i, lifetime);
                if !ready {
                    let value = match self.pending_scope_value(i, &mut res, &args, frames) {
                        Ok(value) => value,
                        Err(RunnerError::Trap(Trap::Pending { .. })) => {
                            next.push(i);
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    self.check_shape(i, &value)?;
                    self.forget(target);
                    res.state = Some(ResourceState {
                        args,
                        value,
                        store_revision: self.store.revision(),
                    });
                    res.pending = true;
                    res.stale = true;
                    res.refresh = false;
                    cell.borrow_mut().resources.insert(i, res);
                    progress = true;
                    continue;
                }

                let (answer, context, revision) =
                    match deferred.remove(&i).filter(|pending| pending.args == args) {
                        Some(pending) => {
                            res.reader |= pending.reader;
                            res.refresh |= pending.forced;
                            (
                                Answer::Later(pending.request),
                                pending.context,
                                pending.revision,
                            )
                        }
                        None => {
                            let reads = self.store.reads();
                            let (answer, context) = self.query(i, &args)?;
                            res.reader |= self.store.reads() > reads;
                            (answer, context, self.store.revision())
                        }
                    };
                let value = match answer {
                    Answer::Now(value) => {
                        res.settled = Some(value.clone());
                        self.forget(target);
                        res.pending = false;
                        res.stale = !ready;
                        value
                    }
                    Answer::Later(request) => {
                        let value = match self.pending_scope_value(i, &mut res, &args, frames) {
                            Ok(value) => value,
                            Err(RunnerError::Trap(Trap::Pending { .. })) => {
                                deferred.insert(
                                    i,
                                    DeferredAnswer {
                                        args,
                                        request,
                                        context,
                                        reader: res.reader,
                                        revision,
                                        forced: res.refresh,
                                    },
                                );
                                next.push(i);
                                continue;
                            }
                            Err(error) => return Err(error),
                        };
                        self.check_shape(i, &value)?;
                        self.enqueue(
                            target,
                            self.plan.str(row.source).to_string(),
                            args.clone(),
                            request,
                            res.refresh,
                            context,
                        );
                        res.pending = true;
                        res.stale = !ready;
                        value
                    }
                };
                self.check_shape(i, &value)?;
                res.state = Some(ResourceState {
                    args,
                    value,
                    store_revision: revision,
                });
                res.refresh = false;
                cell.borrow_mut().resources.insert(i, res);
                progress = true;
            }
            if !progress {
                return Err(RunnerError::Cycle);
            }
            todo = next;
        }
        Ok(())
    }

    fn store_unsettled(&self) -> bool {
        let revision = self.store.revision();
        self.resources.iter().enumerate().any(|(i, s)| {
            self.plan.resources[i].owner.is_none()
                && self.store_readers[i]
                && !self.stale[i]
                && s.as_ref().is_some_and(|s| s.store_revision != revision)
        }) || self.scope_frames().iter().any(|frames| {
            frames
                .last()
                .unwrap()
                .scope
                .as_ref()
                .unwrap()
                .borrow()
                .resources
                .values()
                .any(|r| {
                    r.reader
                        && !r.stale
                        && r.state
                            .as_ref()
                            .is_some_and(|s| s.store_revision != revision)
                })
        })
    }

    /// One accumulating op batch across the bounded root/scoped store fixpoint.
    /// Once this starts, failures use the existing poison/restart boundary.
    pub(super) fn walk(
        &mut self,
    ) -> Result<(Vec<exact_kernel::Op>, Vec<SurfaceUpdate>), RunnerError> {
        let mut ops = Vec::new();
        let mut surfaces = Vec::new();
        let mut passes = 0;
        loop {
            passes += 1;
            if passes > self.plan.resources.len() + 2 {
                return Err(RunnerError::Cycle);
            }
            let before: std::collections::BTreeSet<_> = self
                .scope_frames()
                .iter()
                .map(|f| f.last().unwrap().scope.as_ref().unwrap().borrow().lifetime)
                .collect();
            let mut tree = self.tree.take();
            let mut ids = std::mem::take(&mut self.ids);
            // The plan is shared, and only the root value arrays are snapshotted;
            // instance cells stay in the single tree and settle in live frames.
            let plan = self.plan.clone();
            let slots = self.slots.clone();
            let derives = self.derives.clone();
            let derive_readers = self.derive_readers.clone();
            let resources = self.resource_values.clone();
            let pending_resources = self.pending_res.clone();
            let pending_mutations = self.pending_mut.clone();
            let readers = self.store_readers.clone();
            let now_ms = self.now_ms;
            let result = {
                let mut callback = |frames: &[Frame]| self.settle_scope(frames);
                let mut u = Update {
                    env: Env {
                        plan: &plan,
                        slots: &slots,
                        derives: &derives,
                        resources: &resources,
                        params: &[],
                        frames: &[],
                        now_ms,
                        pending_resources: &pending_resources,
                        pending_mutations: &pending_mutations,
                        store_dependent_derives: &derive_readers,
                        store_dependent_resources: &readers,
                    },
                    ids: &mut ids,
                    ops,
                    surfaces,
                    settle_scope: &mut callback,
                };
                let result = match tree.as_mut() {
                    Some(tree) => tree.update(&mut u),
                    None => Tree::create(&mut u).map(|t| tree = Some(t)),
                };
                ops = u.ops;
                surfaces = u.surfaces;
                result
            };
            self.ids = ids;
            self.tree = tree;
            result?;
            let after: std::collections::BTreeSet<_> = self
                .scope_frames()
                .iter()
                .map(|f| f.last().unwrap().scope.as_ref().unwrap().borrow().lifetime)
                .collect();
            for token in after.difference(&before) {
                self.log(format!("scope {token} mounted"));
            }
            for token in before.difference(&after) {
                self.log(format!("scope {token} unmounted"));
            }
            self.prune_requests();
            if !self.store_unsettled() {
                break;
            }
            self.settle(false)?;
        }
        // A later pass may destroy a node created in an earlier pass. Keep
        // destruction at the end of the one atomic batch (never apply mid-walk).
        let (mut live, gone): (Vec<_>, Vec<_>) = ops
            .into_iter()
            .partition(|op| !matches!(op, exact_kernel::Op::DestroyView { .. }));
        live.extend(gone);
        Ok((live, surfaces))
    }
}
