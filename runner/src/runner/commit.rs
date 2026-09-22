//! Commits: an action, a timer's action, a request's reply — each one
//! transaction that settles, updates the tree and applies one batch, or
//! refuses and leaves the kernel as it was.

use super::*;

/// A string argument or write past [`crate::vm::MAX_STRING`] bytes.
fn too_long(value: &Value) -> bool {
    matches!(value, Value::Str(s) if s.len() > crate::vm::MAX_STRING)
}

/// Everything a refused commit leaves as it was (§6 atomicity): state, the
/// store, the request book-keeping, and the flags a settlement pass sets on
/// its way — a deferred resource's staleness, store provenance, and the
/// refreshes an action asked for.
pub(super) struct Checkpoint {
    store: crate::store::StoreCheckpoint,
    slots: Vec<Value>,
    resources: Option<Vec<Option<ResourceState>>>,
    stale: Vec<bool>,
    store_readers: Vec<bool>,
    refresh_next: Vec<usize>,
    pending: Vec<PendingReq>,
    commands: usize,
}

impl<D: DataSource> Runner<D> {
    /// Before a commit. `resources`: the commit edits the settled caches
    /// before settling (a reply does).
    pub(super) fn checkpoint(&self, resources: bool) -> Checkpoint {
        Checkpoint {
            store: self.store.checkpoint(),
            slots: self.slots.clone(),
            resources: resources.then(|| self.resources.clone()),
            stale: self.stale.clone(),
            store_readers: self.store_readers.clone(),
            refresh_next: self.refresh_next.clone(),
            pending: self.pending.clone(),
            commands: self.commands.len(),
        }
    }

    /// After a commit: journal the store's writes if it stood; otherwise
    /// put everything back — only the store, when this very commit poisoned
    /// the runner (its tree matches nothing to go back to).
    pub(super) fn conclude<T>(
        &mut self,
        c: Checkpoint,
        result: &Result<T, RunnerError>,
        was_poisoned: bool,
    ) {
        match result {
            Ok(_) => self.log_store_writes(c.store.writes),
            Err(_) if self.poisoned && !was_poisoned => self.store.restore(c.store),
            Err(_) => {
                self.store.restore(c.store);
                self.slots = c.slots;
                if let Some(resources) = c.resources {
                    self.resources = resources;
                }
                self.stale = c.stale;
                self.store_readers = c.store_readers;
                self.refresh_next = c.refresh_next;
                self.pending = c.pending;
                self.sync_pending_flags();
                self.commands.truncate(c.commands);
            }
        }
    }

    /// Run an action by name with `args` — what a test or an agent does.
    pub fn act(&mut self, name: &str, args: Vec<Value>) -> Result<CommitReceipt, RunnerError> {
        let what = format!("act {name}");
        let was_poisoned = self.poisoned;
        let result = match self
            .plan
            .actions
            .iter()
            .position(|a| self.plan.str(a.name) == name)
            .map(|i| ActionsId(i as u32))
        {
            Some(id) => self.run_action(id, args, &[]),
            None => Err(RunnerError::NoHandler {
                view: 0,
                event: "action",
            }),
        };
        self.log_outcome(&what, &result, was_poisoned);
        result
    }

    /// Move the clock to `now_ms`, firing every timer due, in order — the
    /// commits alone, or the refusal that stopped it. Tests use this; a host
    /// uses [`Runner::advance_timed`], which keeps the commits before a
    /// refusal and the time each was made.
    pub fn advance(&mut self, now_ms: f64) -> Result<Vec<CommitReceipt>, RunnerError> {
        let a = self.advance_timed(now_ms);
        match a.error {
            Some(e) => Err(e),
            None => Ok(a.receipts.into_iter().map(|t| t.receipt).collect()),
        }
    }

    /// Move the clock to `now_ms`, firing every timer due, in order, each at
    /// its own due time. A refusal stops the advance there: the commits so
    /// far are returned with their times, the clock stays at the refusing
    /// timer's due time, and the refusal rides along.
    pub fn advance_timed(&mut self, now_ms: f64) -> Advanced {
        let mut receipts = Vec::new();
        if !now_ms.is_finite() {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: Some(RunnerError::NonFiniteClock),
            };
        }
        if now_ms < self.now_ms {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: None,
            };
        }
        if now_ms > MAX_CLOCK_MS {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: Some(RunnerError::ClockOutOfRange),
            };
        }
        loop {
            // The earliest due timer, deterministic by index on ties.
            let due = self
                .timers
                .iter()
                .enumerate()
                .filter(|(_, t)| t.next_ms <= now_ms)
                .min_by(|(ia, a), (ib, b)| {
                    a.next_ms.partial_cmp(&b.next_ms).unwrap().then(ia.cmp(ib))
                })
                .map(|(i, t)| (i, t.next_ms));
            let Some((i, at)) = due else { break };
            if receipts.len() == TIMER_FIRE_LIMIT {
                return Advanced {
                    receipts,
                    now_ms: self.now_ms,
                    error: Some(RunnerError::TimerFireLimit {
                        limit: TIMER_FIRE_LIMIT,
                    }),
                };
            }
            let interval = self.plan.timers[i].interval_ms as f64;
            let next_ms = at + interval;
            if !next_ms.is_finite() || next_ms <= at {
                return Advanced {
                    receipts,
                    now_ms: self.now_ms,
                    error: Some(RunnerError::ClockDidNotAdvance { timer: i }),
                };
            }
            self.now_ms = at;
            self.timers[i].next_ms = next_ms;
            let action = self.plan.timers[i].action;
            let was_poisoned = self.poisoned;
            let result = self.run_action(action, Vec::new(), &[]);
            match result {
                Ok(receipt) => receipts.push(Timed { at_ms: at, receipt }),
                Err(e) => {
                    let what = format!(
                        "timer {} ({})",
                        i,
                        self.plan.str(self.plan.action(action).name)
                    );
                    let failed = Err(e);
                    self.log_outcome(&what, &failed, was_poisoned);
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: failed.err(),
                    };
                }
            }
        }
        self.now_ms = now_ms;
        if !receipts.is_empty() {
            let line = format!(
                "advance → {} timer{} fired, epoch {}",
                receipts.len(),
                if receipts.len() == 1 { "" } else { "s" },
                receipts.last().map_or(0, |t| t.receipt.epoch)
            );
            self.log(line);
        }
        Advanced {
            receipts,
            now_ms,
            error: None,
        }
    }

    /// Run an action: its body, its sends, its writes, settlement, the
    /// update — one commit. What it kept in the store is journaled once the
    /// commit stands and rolled back with everything else when it does not
    /// (LLP 1018 D1): nothing reaches the host from a refused action.
    pub(super) fn run_action(
        &mut self,
        action: ActionsId,
        args: Vec<Value>,
        frames: &[Frame],
    ) -> Result<CommitReceipt, RunnerError> {
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(false);
        let result = self.run_action_inner(action, args, frames);
        self.conclude(checkpoint, &result, was_poisoned);
        result
    }

    /// Journal the store's writes from index `since`: the names, never the
    /// values.
    pub(super) fn log_store_writes(&mut self, since: usize) {
        let writes = self.store.writes();
        let lines: Vec<String> = writes[since.min(writes.len())..]
            .iter()
            .map(|w| match &w.value {
                Some(_) => format!("store {}", w.name),
                None => format!("forget {}", w.name),
            })
            .collect();
        for line in lines {
            self.log(line);
        }
    }

    pub(super) fn run_action_inner(
        &mut self,
        action: ActionsId,
        args: Vec<Value>,
        frames: &[Frame],
    ) -> Result<CommitReceipt, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let row = self.plan.action(action).clone();
        let expected = row.params.len as usize;
        if args.len() != expected {
            return Err(RunnerError::Arity {
                action: self.plan.str(row.name).to_string(),
                expected,
                actual: args.len(),
            });
        }
        for (i, p) in row.params.iter().enumerate() {
            let param = self.plan.param(p);
            if !args[i].conforms(&self.plan, param.ty) {
                return Err(RunnerError::ArgumentType {
                    action: self.plan.str(row.name).to_string(),
                    param: self.plan.str(param.name).to_string(),
                });
            }
            if too_long(&args[i]) {
                return Err(RunnerError::StringTooLong {
                    name: self.plan.str(param.name).to_string(),
                });
            }
        }
        let allowed: Vec<u32> = row
            .writes
            .iter()
            .map(|w| self.plan.write(w).slot.0)
            .collect();
        let outcome = {
            // The frames in force at the view the event hit (LLP 1017 P4c):
            // a row action reads and writes its row through them.
            let env = self.env(&args, frames);
            vm::eval(self.plan.code(row.body), &env, &allowed)?
        };
        // Sends (LLP 1016 §4): each asks the source now. An answer lands in
        // the mutation's slot inside this commit; a request goes to the host
        // once the commit stands.
        let mut later: Vec<(usize, String, Vec<Value>, Request)> = Vec::new();
        let mut answered: Vec<(u32, Value)> = Vec::new();
        for (m, source, sargs) in &outcome.sends {
            let m = *m as usize;
            let mrow = self.plan.mutations[m].clone();
            let name = self.plan.str(mrow.name).to_string();
            let answer = match self.data.answer(&mut self.store, source, sargs) {
                Ok(answer) => answer,
                Err(error) => {
                    self.discard_later(&later);
                    return Err(RunnerError::Data {
                        resource: name,
                        error,
                    });
                }
            };
            match answer {
                Answer::Now(v) => {
                    if !v.conforms(&self.plan, mrow.ty) {
                        self.discard_later(&later);
                        return Err(RunnerError::Shape { resource: name });
                    }
                    let slot = match self.mutation_slot(m) {
                        Ok(slot) => slot,
                        Err(e) => {
                            self.discard_later(&later);
                            return Err(e);
                        }
                    };
                    answered.push((slot as u32, Value::some(v)));
                }
                Answer::Later(request) => later.push((m, source.clone(), sargs.clone(), request)),
            }
        }
        // Commit the writes, then everything downstream. If settlement refuses
        // (a data source or shape refusal), the writes and commands roll back
        // and the kernel is exactly as it was.
        for (slot, value) in outcome
            .writes
            .iter()
            .map(|(s, v)| (s, v))
            .chain(outcome.row_writes.iter().map(|(s, v, _)| (s, v)))
        {
            let row = &self.plan.slots[*slot as usize];
            let refusal = if !value.conforms(&self.plan, row.ty) {
                Some(RunnerError::SlotType {
                    slot: self.plan.str(row.name).to_string(),
                })
            } else if too_long(value) {
                Some(RunnerError::StringTooLong {
                    name: self.plan.str(row.name).to_string(),
                })
            } else {
                None
            };
            if let Some(refusal) = refusal {
                self.discard_later(&later);
                return Err(refusal);
            }
        }
        // Row writes land in their rows now, remembered for a rollback.
        let mut row_undo: Vec<(RowSlots, u32, Option<Value>)> = Vec::new();
        for (slot, value, rows) in outcome.row_writes {
            let old = rows.borrow_mut().insert(slot, value);
            row_undo.push((rows, slot, old));
        }
        let first_command = self.commands.len();
        for (slot, value) in answered {
            self.slots[slot as usize] = value;
        }
        let written: Vec<u32> = outcome.writes.iter().map(|(s, _)| *s).collect();
        for (slot, value) in outcome.writes {
            self.slots[slot as usize] = value;
        }
        for (name, args) in outcome.commands {
            self.commands.push(Command { name, args });
        }
        // An assignment to a mutation's slot tentatively makes it not
        // pending. The pending map is changed only after settlement stands:
        // a refused assignment did not change what reply the view wants.
        let assigned: Vec<usize> = (0..self.plan.mutations.len())
            .filter(|m| written.contains(&self.plan.mutations[*m].slot.0))
            .collect();
        for m in &assigned {
            self.pending_mut[*m] = false;
        }
        for (m, _, _, _) in &later {
            if !assigned.contains(m) {
                self.pending_mut[*m] = true;
            }
        }
        self.refresh_next = outcome.refreshes.iter().map(|r| *r as usize).collect();
        // A refusal from here is put back by the checkpoint (run_action);
        // row slots live in the tree, so they are undone here.
        if let Err(e) = self.router_change().and_then(|_| self.settle(false)) {
            self.discard_later(&later);
            for (rows, slot, old) in row_undo.into_iter().rev() {
                match old {
                    Some(v) => rows.borrow_mut().insert(slot, v),
                    None => rows.borrow_mut().remove(&slot),
                };
            }
            return Err(e);
        }
        for (rows, slot, _) in &row_undo {
            self.row_writes.record(frames, rows, *slot);
        }
        for m in &assigned {
            self.forget(Target::Mutation(*m));
        }
        for (m, source, args, request) in later {
            self.enqueue(Target::Mutation(m), source, args, request, false);
            if assigned.contains(&m) {
                self.forget(Target::Mutation(m));
            }
        }
        let commands: Vec<String> = self.commands[first_command..]
            .iter()
            .map(|c| {
                let mut s = format!("command {}(", c.name);
                for (i, a) in c.args.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    crate::agent::untyped_json(a, &mut s);
                }
                s.push(')');
                s
            })
            .collect();
        let result = self.update();
        // Commands are journaled only once the update committed: a failure
        // there poisons the runner and clears them.
        if result.is_ok() {
            for line in commands {
                self.log(line);
            }
        }
        result
    }

    pub(super) fn poison(&mut self) {
        self.poisoned = true;
        self.commands.clear();
        self.requests.clear();
        self.pending.clear();
        self.sync_pending_flags();
    }

    /// Recheck the plan relation at the write boundary instead of trusting
    /// that a decoded plan is the only possible caller.
    pub(super) fn mutation_slot(&self, mutation: usize) -> Result<usize, RunnerError> {
        let mutation = MutationsId(mutation as u32);
        self.plan
            .validate_mutation_slot(mutation)
            .map_err(RunnerError::Plan)?;
        Ok(self.plan.mutation(mutation).slot.0 as usize)
    }

    pub(super) fn target_name(&self, t: Target) -> String {
        match t {
            Target::Resource(i) => self.plan.str(self.plan.resources[i].name).to_string(),
            Target::Mutation(m) => self.plan.str(self.plan.mutations[m].name).to_string(),
        }
    }

    /// Drop the request in flight for `target`, if any: its reply, when it
    /// comes, is dropped too (a `POST` already sent is not unsent — LLP 1016 D5).
    pub(super) fn forget(&mut self, target: Target) {
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.log(format!("forget request {t} ({})", self.target_name(target)));
        }
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
    ) {
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.log(format!("forget request {t} ({})", self.target_name(target)));
        }
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let name = self.target_name(target);
        self.log(match request.continuation {
            Some(token) => format!("continuation {ticket} ({name}): executor token {token}"),
            None if request.storage.is_some() => format!("storage {ticket} ({name})"),
            None => format!(
                "request {ticket} ({name}): {} {}",
                request.method, request.url
            ),
        });
        self.pending.push(PendingReq {
            refusal: None,
            ticket,
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
        self.sync_pending_flags();
    }

    pub(super) fn sync_pending_flags(&mut self) {
        self.pending_res = vec![false; self.plan.resources.len()];
        self.pending_mut = vec![false; self.plan.mutations.len()];
        for p in &self.pending {
            match p.target {
                Target::Resource(i) => self.pending_res[i] = true,
                Target::Mutation(m) => self.pending_mut[m] = true,
            }
        }
    }

    /// The requests the host is to run since the last take (LLP 1016 D2).
    pub fn take_requests(&mut self) -> Vec<RequestOut> {
        std::mem::take(&mut self.requests)
    }

    /// The work behind continuation `token` (LLP 1027.002 D3): a host asks
    /// on this thread, after the commit that handed the request out, so a
    /// worker's snapshot is the store as committed then.
    pub fn dispatch_work(&mut self, token: u64) -> Dispatch {
        self.data.dispatch(token, &self.store)
    }

    /// Work the source held at dispatch and the last commit releases, in
    /// order; a host asks after every commit.
    pub fn release_work(&mut self) -> Vec<(u64, Dispatch)> {
        self.data.release(&self.store)
    }

    /// A request a refused transaction dropped before it was handed out:
    /// its continuation token is never dispatched, so the source forgets it.
    pub(super) fn discard_request(&mut self, request: &Request) {
        if let Some(token) = request.continuation {
            self.data.discard(token);
        }
    }

    /// The `Later` sends an action collected before a refusal dropped them.
    pub(super) fn discard_later(&mut self, later: &[(usize, String, Vec<Value>, Request)]) {
        for (_, _, _, request) in later {
            self.discard_request(request);
        }
    }

    /// Every request in flight: the resource's or mutation's name and its ticket.
    pub fn pending(&self) -> Vec<(String, u64)> {
        self.pending
            .iter()
            .map(|p| (self.target_name(p.target), p.ticket))
            .collect()
    }

    /// Whether any request is in flight (the agent's `settle` waits on it).
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
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
        let Some(pos) = self.pending.iter().position(|p| p.ticket == ticket) else {
            self.log(format!("reply {ticket} dropped: no such request in flight"));
            return Ok(None);
        };
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(true);
        let p = self.pending.remove(pos);
        self.sync_pending_flags();
        let what = format!("fulfil {ticket} ({})", self.target_name(p.target));
        let result = self.fulfill_inner(p, outcome);
        self.conclude(checkpoint, &result, was_poisoned);
        self.log_outcome(&what, &result, was_poisoned);
        result.map(Some)
    }

    pub(super) fn fulfill_inner(
        &mut self,
        p: PendingReq,
        outcome: Outcome,
    ) -> Result<CommitReceipt, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let name = self.target_name(p.target);
        let ty = match p.target {
            Target::Resource(i) => self.plan.resources[i].ty,
            Target::Mutation(m) => self.plan.mutations[m].ty,
        };
        let value = match self
            .data
            .parse(&mut self.store, &p.source, &p.args, outcome)
            .map_err(|error| RunnerError::Data {
                resource: name.clone(),
                error,
            })? {
            Answer::Now(value) => value,
            Answer::Later(request) => {
                // One more round (LLP 1027 D1a): the target keeps its value,
                // a new ticket goes out for the same arguments, and this
                // commit changes nothing but the pending set.
                self.log(format!("{name}: the reply asks for one more request"));
                self.enqueue(p.target, p.source, p.args, request, false);
                return self.update();
            }
        };
        if !value.conforms(&self.plan, ty) {
            return Err(RunnerError::Shape { resource: name });
        }
        match p.target {
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
        self.router_change().and_then(|_| self.settle(false))?;
        self.update()
    }
}
