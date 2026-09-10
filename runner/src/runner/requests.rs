//! Lifetime-qualified requests and completions.
use super::*;

/// One already-started call waiting only for its fallback dependencies.
#[derive(Clone)]
pub(super) struct DeferredAnswer {
    pub(super) args: Vec<Value>,
    pub(super) request: Request,
    pub(super) context: u64,
    pub(super) reader: bool,
    pub(super) revision: u64,
    pub(super) forced: bool,
}

impl<D: DataSource> Runner<D> {
    pub(super) fn answer_call(
        &mut self,
        source: &str,
        args: &[Value],
    ) -> Result<(Answer, u64), DataError> {
        let context = self.next_context;
        self.next_context += 1;
        let answer = self
            .data
            .answer_scoped(context, &mut self.store, source, args);
        match &answer {
            Ok(Answer::Later(_)) => {
                self.contexts.insert(context);
            }
            // Terminal calls have no executor work to retain. Even a source
            // refusal may have allocated state before discovering its error.
            _ => self.data.cancel_scoped(context),
        }
        answer.map(|answer| (answer, context))
    }

    pub(super) fn flush_contexts(&mut self) {
        let keep = self.pending.iter().map(|p| p.context).collect();
        self.retain_contexts(&keep);
    }

    pub(super) fn retain_contexts(&mut self, keep: &std::collections::BTreeSet<u64>) {
        let gone: Vec<_> = self.contexts.difference(keep).copied().collect();
        for context in gone {
            self.contexts.remove(&context);
            self.data.cancel_scoped(context);
        }
    }

    pub(super) fn target_name(&self, t: Target) -> String {
        match t {
            Target::Resource(i) => self.plan.str(self.plan.resources[i].name).to_string(),
            Target::OwnedResource(i, t) => {
                format!("{}@{t}", self.plan.str(self.plan.resources[i].name))
            }
            Target::Mutation(m) => self.plan.str(self.plan.mutations[m].name).to_string(),
            Target::OwnedMutation(m, t) => {
                format!("{}@{t}", self.plan.str(self.plan.mutations[m].name))
            }
        }
    }

    /// Drop the request in flight for `target`, if any: its reply, when it
    /// comes, is dropped too (a `POST` already sent is not unsent — LLP 1016 D5).
    pub(super) fn forget(&mut self, target: Target) {
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.requests.retain(|r| r.ticket != t);
            self.log(format!("forget request {t} ({})", self.target_name(target)));
        }
        self.set_target_pending(target, false);
        self.sync_pending_flags();
    }

    /// An explicit send still goes out when the same action assigns its
    /// mutation slot (for example logout then session = none). Keep executor
    /// ownership until completion or teardown, but discard reply interest.
    pub(super) fn discard_reply(&mut self, target: Target) {
        if let Some(pending) = self.pending.iter_mut().find(|p| p.target == target) {
            pending.publish = false;
            let ticket = pending.ticket;
            self.log(format!(
                "forget request {ticket} reply ({}): send retained",
                self.target_name(target)
            ));
        }
        self.set_target_pending(target, false);
        self.sync_pending_flags();
    }

    /// Hand `request` to the host under a fresh ticket, replacing any
    /// request in flight for the same target.
    pub(super) fn enqueue(
        &mut self,
        target: Target,
        source: String,
        args: Vec<Value>,
        request: Request,
        forced: bool,
        context: u64,
    ) {
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.requests.retain(|r| r.ticket != t);
            self.log(format!("forget request {t} ({})", self.target_name(target)));
        }
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let name = self.target_name(target);
        self.log(match request.continuation {
            Some(token) => format!("continuation {ticket} ({name}): executor token {token}"),
            None => format!(
                "request {ticket} ({name}): {} {}",
                request.method, request.url
            ),
        });
        self.pending.push(PendingReq {
            ticket,
            context,
            publish: true,
            target,
            source,
            args,
        });
        self.requests.push(RequestOut {
            ticket,
            target: name,
            request,
            forced,
        });
        self.set_target_pending(target, true);
        self.sync_pending_flags();
    }

    pub(super) fn sync_pending_flags(&mut self) {
        self.pending_res = vec![false; self.plan.resources.len()];
        self.pending_mut = vec![false; self.plan.mutations.len()];
        for p in self.pending.iter().filter(|p| p.publish) {
            match p.target {
                Target::Resource(i) => self.pending_res[i] = true,
                Target::Mutation(m) => self.pending_mut[m] = true,
                Target::OwnedResource(..) | Target::OwnedMutation(..) => {}
            }
        }
    }

    pub(super) fn set_target_pending(&mut self, target: Target, pending: bool) {
        match target {
            Target::Resource(i) => self.pending_res[i] = pending,
            Target::Mutation(i) => self.pending_mut[i] = pending,
            Target::OwnedResource(i, _) => {
                if let Some(cell) = self.target_scope(target) {
                    if let Some(r) = cell.borrow_mut().resources.get_mut(&i) {
                        r.pending = pending;
                    }
                }
            }
            Target::OwnedMutation(i, _) => {
                if let Some(cell) = self.target_scope(target) {
                    cell.borrow_mut().mutations.insert(i, pending);
                }
            }
        }
    }

    pub(super) fn prune_requests(&mut self) {
        let live: std::collections::BTreeSet<_> = self
            .scope_frames()
            .iter()
            .map(|f| f.last().unwrap().scope.as_ref().unwrap().borrow().lifetime)
            .collect();
        let gone: Vec<_> = self
            .pending
            .iter()
            .filter(|p| match p.target {
                Target::OwnedResource(_, t) | Target::OwnedMutation(_, t) => !live.contains(&t),
                _ => false,
            })
            .map(|p| p.target)
            .collect();
        for target in gone {
            self.forget(target);
        }
    }

    /// The requests the host is to run since the last take (LLP 1016 D2).
    pub fn take_requests(&mut self) -> Vec<RequestOut> {
        std::mem::take(&mut self.requests)
    }

    /// Every request in flight: the resource's or mutation's name and its ticket.
    pub fn pending(&self) -> Vec<(String, u64)> {
        self.pending
            .iter()
            .filter(|p| p.publish)
            .map(|p| (self.target_name(p.target), p.ticket))
            .collect()
    }

    /// Whether any request is in flight (the agent's `settle` waits on it).
    pub fn has_pending(&self) -> bool {
        self.pending.iter().any(|p| p.publish)
    }

    /// The host brought back the outcome of request `ticket`: the source
    /// parses it, the resource takes its value or the mutation's slot its
    /// `some`, and everything downstream settles as after an action — one
    /// commit. A ticket no longer held (forgotten, D5) is dropped with a
    /// journal line and no commit.
    pub fn fulfill(
        &mut self,
        ticket: u64,
        outcome: Outcome,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        self.prune_requests();
        let Some(pos) = self.pending.iter().position(|p| p.ticket == ticket) else {
            self.log(format!("reply {ticket} dropped: no such request in flight"));
            return Ok(None);
        };
        if !self.pending[pos].publish {
            let target = self.pending[pos].target;
            self.forget(target);
            self.flush_contexts();
            self.log(format!(
                "reply {ticket} dropped: the action assigned its mutation slot"
            ));
            return Ok(None);
        }
        let target = self.pending[pos].target;
        let frames = match target {
            Target::OwnedResource(_, t) | Target::OwnedMutation(_, t) => {
                self.lifetime_frames(t).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        let saved = self.checkpoint(&frames);
        let since = self.store.writes().len();
        let p = self.pending.remove(pos);
        self.requests.retain(|r| r.ticket != ticket);
        self.set_target_pending(p.target, false);
        self.sync_pending_flags();
        let what = format!("fulfil {ticket} ({})", self.target_name(p.target));
        let was_poisoned = self.poisoned;
        let result = self.fulfill_inner(p, outcome);
        match &result {
            Ok(_) => self.log_store_writes(since),
            Err(_) => self.restore(saved),
        }
        self.log_outcome(&what, &result, was_poisoned);
        result.map(Some)
    }

    fn fulfill_inner(
        &mut self,
        p: PendingReq,
        outcome: Outcome,
    ) -> Result<CommitReceipt, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let name = self.target_name(p.target);
        let ty = match p.target {
            Target::Resource(i) | Target::OwnedResource(i, _) => self.plan.resources[i].ty,
            Target::Mutation(m) | Target::OwnedMutation(m, _) => self.plan.mutations[m].ty,
        };
        let reads = self.store.reads();
        let parsed = self
            .data
            .parse_scoped(p.context, &mut self.store, &p.source, &p.args, outcome)
            .map_err(|error| RunnerError::Data {
                resource: name.clone(),
                error,
            })?;
        // Parsing participates in store provenance too. A continuation's
        // pending value observed this revision: don't replace its next
        // request with a fresh answer() while settling other store readers.
        let reader = self.store.reads() > reads;
        let revision = self.store.revision();
        match p.target {
            Target::Resource(i) => {
                self.store_readers[i] |= reader;
                if let Some(state) = &mut self.resources[i] {
                    state.store_revision = revision;
                }
            }
            Target::OwnedResource(i, _) => {
                if let Some(scope) = self.target_scope(p.target) {
                    if let Some(res) = scope.borrow_mut().resources.get_mut(&i) {
                        res.reader |= reader;
                        if let Some(state) = &mut res.state {
                            state.store_revision = revision;
                        }
                    };
                }
            }
            _ => {}
        }
        let value = match parsed {
            Answer::Now(value) => value,
            Answer::Later(request) => {
                // One more round (LLP 1027 D1a): the target keeps its value,
                // a new ticket goes out for the same arguments, and this
                // commit changes nothing but the pending set.
                self.log(format!("{name}: the reply asks for one more request"));
                self.enqueue(p.target, p.source, p.args, request, false, p.context);
                self.settle(false)?;
                return self.update();
            }
        };
        if !value.conforms(&self.plan, ty) {
            return Err(RunnerError::Shape { resource: name });
        }
        match p.target {
            Target::OwnedResource(i, _) => {
                let scope = self.target_scope(p.target).expect("live target");
                let mut cell = scope.borrow_mut();
                let res = cell.resources.get_mut(&i).expect("settled resource");
                res.settled = Some(value.clone());
                res.state = Some(ResourceState {
                    args: p.args,
                    value,
                    store_revision: self.store.revision(),
                });
                res.pending = false;
                res.stale = false;
            }
            Target::OwnedMutation(m, _) => {
                let slot = self.mutation_slot(m)?;
                let scope = self.target_scope(p.target).expect("live target");
                scope
                    .borrow()
                    .slots
                    .borrow_mut()
                    .insert(slot as u32, Value::some(value));
            }
            Target::Resource(i) => {
                self.stale[i] = false;
                self.keep_answer(i, &p.args, &value);
                self.resources[i] = Some(ResourceState {
                    args: p.args,
                    value,
                    store_revision: self.store.revision(),
                });
            }
            Target::Mutation(m) => {
                let slot = self.mutation_slot(m)?;
                self.slots[slot] = Value::some(value);
            }
        }
        self.settle(false)?;
        self.update()
    }
}
