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
    watching: Vec<Vec<String>>,
    refresh_next: Vec<usize>,
    reread_next: Vec<usize>,
    pending: Vec<PendingReq>,
    commands: usize,
}

impl<D: DataSource> Runner<D> {
    /// Before a commit. `resources`: the commit edits the settled caches
    /// before settling (a reply does).
    pub(super) fn checkpoint(&mut self, resources: bool) -> Checkpoint {
        self.derives_evaluated = 0;
        self.copied_at_checkpoint = self.store.copied_bytes();
        Checkpoint {
            store: self.store.checkpoint(),
            slots: self.slots.clone(),
            resources: resources.then(|| self.resources.clone()),
            stale: self.stale.clone(),
            store_readers: self.store_readers.clone(),
            watching: self.watching.clone(),
            refresh_next: self.refresh_next.clone(),
            reread_next: self.reread_next.clone(),
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
                // What the refused commit asked is let go with it.
                self.forgot = true;
                self.store.restore(c.store);
                self.slots = c.slots;
                if let Some(resources) = c.resources {
                    self.resources = resources;
                }
                self.stale = c.stale;
                self.store_readers = c.store_readers;
                self.watching = c.watching;
                self.refresh_next = c.refresh_next;
                self.reread_next = c.reread_next;
                self.pending = c.pending;
                self.sync_pending_flags();
                self.commands.truncate(c.commands);
            }
        }
        // After any restore: the source hears what is really in flight.
        if std::mem::take(&mut self.forgot) {
            let in_flight: Vec<InFlight<'_>> = self
                .pending
                .iter()
                .map(|p| InFlight {
                    target: p.target,
                    source: &p.source,
                    args: &p.args,
                    continuation: p.continuation,
                })
                .collect();
            self.data.forgotten(&in_flight);
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
        self.advance_within(now_ms, false)
    }

    /// [`Runner::advance_timed`], stopping after the first timer whose
    /// commit hands the host a request: the clock stays at that timer's due
    /// time, for the host to land the reply before advancing again. The
    /// runner keeps one request per target (LLP 1016 D5), so the next tick's
    /// send would drop it. An agent's clock jump advances this way (LLP
    /// 1012); on the wall clock, replies land between ticks by themselves.
    pub fn advance_until_request(&mut self, now_ms: f64) -> Advanced {
        self.advance_within(now_ms, true)
    }

    fn advance_within(&mut self, now_ms: f64, until_request: bool) -> Advanced {
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
        let mut landed = now_ms;
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
            // An answer's `then` goes before a timer due at the same time: the
            // answer landed first.
            let then = self
                .then_due
                .iter()
                .enumerate()
                .filter(|(_, at)| **at <= now_ms)
                .min_by(|(ia, a), (ib, b)| a.partial_cmp(b).unwrap().then(ia.cmp(ib)))
                .map(|(m, at)| (m, *at))
                .filter(|(_, at)| due.is_none_or(|(_, timer)| *at <= timer));
            if let Some((m, at)) = then {
                if receipts.len() == TIMER_FIRE_LIMIT {
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: Some(RunnerError::TimerFireLimit {
                            limit: TIMER_FIRE_LIMIT,
                        }),
                    };
                }
                self.now_ms = self.now_ms.max(at);
                self.then_due[m] = f64::INFINITY;
                let action = self.plan.mutations[m].then.expect("armed only with a then");
                let was_poisoned = self.poisoned;
                let ticket = self.next_ticket;
                match self.run_action(action, Vec::new(), &[]) {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        let what = format!(
                            "{} then {}",
                            self.plan.str(self.plan.mutations[m].name),
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
                if until_request && self.next_ticket != ticket {
                    landed = self.now_ms;
                    break;
                }
                continue;
            }
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
            let row = &self.plan.timers[i];
            // A one-shot timer is spent: an infinite deadline is never due
            // and never reported (`timer_due_ms`), so it keeps no host awake.
            let next_ms = if row.once {
                f64::INFINITY
            } else {
                at + row.interval_ms as f64
            };
            if !row.once && (!next_ms.is_finite() || next_ms <= at) {
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
            let ticket = self.next_ticket;
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
            if until_request && self.next_ticket != ticket {
                landed = at;
                break;
            }
        }
        self.now_ms = landed;
        if !receipts.is_empty() {
            let line = super::lines::advanced(
                receipts.len(),
                receipts.last().map_or(0, |t| t.receipt.epoch),
            );
            self.log(line);
        }
        Advanced {
            receipts,
            now_ms: landed,
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
        self.arm_then(result.is_ok());
        result
    }

    /// Arm the `then` action of each mutation answered in the commit just
    /// made, if it stood. It runs as its own commit when the host next
    /// advances the clock, which it does at once for a due time already
    /// past (`timer_due_ms`): the answer's commit is never extended by
    /// what it causes, and a refused `then` leaves the answer standing.
    pub(super) fn arm_then(&mut self, stood: bool) {
        for m in std::mem::take(&mut self.landed) {
            if stood && self.plan.mutations[m].then.is_some() {
                self.then_due[m] = self.now_ms;
            }
        }
    }

    /// Journal the store's writes from index `since`: the names, never the
    /// values.
    pub(super) fn log_store_writes(&mut self, since: usize) {
        let writes = self.store.writes();
        let lines: Vec<String> = writes[since.min(writes.len())..]
            .iter()
            .map(|w| super::lines::store_write(&w.name, w.value.is_some()))
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
            let target = Target::Mutation(m);
            let answer = match self.data.answer_for(target, &mut self.store, source, sargs) {
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
                    if !self.conforms(&v, mrow.ty) {
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
                    self.landed.push(m);
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
            let refusal = if !self.conforms(value, row.ty) {
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
        // What the action refreshes, added to what is already waiting
        // (LLP 1054.000.000 D2); and what each mutation it sent to declares
        // it changes, read again now that every send has asked its source.
        for r in outcome.refreshes.iter() {
            self.force_refresh(*r as usize);
        }
        self.reread_next = outcome
            .sends
            .iter()
            .flat_map(|(m, _, _)| self.declared_refreshes(*m as usize))
            .collect();
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

    /// Whether `value` conforms to `ty`, checking only the list items not
    /// already found conforming ([`crate::conform::Conformed`]).
    pub(super) fn conforms(&self, value: &Value, ty: exact_plan::TypesId) -> bool {
        self.conformed.borrow_mut().conforms(&self.plan, value, ty)
    }

    pub(super) fn poison(&mut self) {
        self.poisoned = true;
        self.notes.clear();
        self.commands.clear();
        self.requests.clear();
        self.forgot |= !self.pending.is_empty();
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
            self.forgot = true;
            self.log(super::lines::forgot(t, &self.target_name(target)));
        }
        self.sync_pending_flags();
    }

    /// The resources mutation `m` declares it refreshes.
    pub(super) fn declared_refreshes(&self, m: usize) -> Vec<usize> {
        let range = self.plan.mutations[m].refreshes;
        self.plan.mutation_refreshes[range.start as usize..(range.start + range.len) as usize]
            .iter()
            .map(|row| row.resource.0 as usize)
            .collect()
    }

    /// Ask resource `r` again, forced, at the next settlement that can.
    pub(super) fn force_refresh(&mut self, r: usize) {
        if !self.refresh_next.contains(&r) {
            self.refresh_next.push(r);
        }
    }

    /// Hand `request` to the host under a fresh ticket, replacing any
    /// request in flight for the same target — unless only the arguments
    /// moved and the request is the one already in flight (LLP 1054.000.000
    /// D3, amending LLP 1016 D5): then that ticket is kept, to be parsed
    /// with the newer arguments, and nothing is sent.
    pub(super) fn enqueue(
        &mut self,
        target: Target,
        source: String,
        args: Vec<Value>,
        request: Request,
        forced: bool,
    ) {
        let keepable = matches!(target, Target::Resource(_))
            && request.continuation.is_none()
            && request.storage.is_none()
            && request.surface.is_none();
        if keepable && !forced {
            if let Some(p) = self.pending.iter_mut().find(|p| {
                p.target == target
                    && !p.refused
                    && p.refusal.is_none()
                    && p.args != args
                    && p.keepable.as_ref() == Some(&request)
            }) {
                p.args = args;
                let ticket = p.ticket;
                // A source that parks calls hears the kept ticket's newer
                // arguments and drops the call parked under the old ones.
                self.forgot = true;
                self.log(super::lines::kept(ticket, &self.target_name(target)));
                return;
            }
        }
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.forgot = true;
            self.log(super::lines::forgot(t, &self.target_name(target)));
        }
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let name = self.target_name(target);
        self.log(super::lines::enqueued(ticket, &name, &request));
        self.pending.push(PendingReq {
            refusal: None,
            refused: false,
            ticket,
            target,
            source,
            args,
            continuation: request.continuation,
            keepable: keepable.then(|| request.clone()),
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
        // A placeholder shown until the source can answer is pending too.
        for (i, awaiting) in self.awaiting.iter().enumerate() {
            self.pending_res[i] |= *awaiting;
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

    /// The work behind a long native call ([`Request::is_native`]): hand its
    /// body to the source's native handler with the reply, on the host's
    /// worker, which returns at once — the module's own thread answers.
    pub fn native_work(&mut self, request: &Request) -> Dispatch {
        let handler = self.data.native().and_then(|n| n.handler());
        let body = request.body.clone();
        Dispatch::Run(crate::Work::Later(Box::new(move |reply| match handler {
            Some(handler) => handler(body, reply),
            None => reply.send(crate::Outcome::Failed {
                kind: crate::FailureKind::Unsupported,
                message: "no native module here takes long calls".into(),
            }),
        })))
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

    /// Whether request `ticket` is still wanted. A host asks after each
    /// commit and lets go of the work for any it holds that isn't (LLP 1016
    /// D5): a superseded or forgotten reply would only be dropped here.
    pub fn holds(&self, ticket: u64) -> bool {
        self.pending.iter().any(|p| p.ticket == ticket)
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
        let summary = outcome.summary();
        let Some(pos) = self.pending.iter().position(|p| p.ticket == ticket) else {
            self.log(super::lines::dropped(ticket, &summary));
            return Ok(None);
        };
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(true);
        let p = self.pending.remove(pos);
        self.sync_pending_flags();
        let what = super::lines::fulfilling(ticket, &self.target_name(p.target), &summary);
        let (refused, target) = (p.refused, p.target);
        let result = self.fulfill_inner(p, outcome);
        self.conclude(checkpoint, &result, was_poisoned);
        self.arm_then(result.is_ok());
        self.log_outcome(&what, &result, was_poisoned);
        if refused && result.is_err() && self.holds(ticket) {
            return self.release_refused(ticket, target);
        }
        if matches!(
            result,
            Err(RunnerError::Data { .. } | RunnerError::Shape { .. })
        ) && self.holds(ticket)
        {
            // The failure is in the journal (above); what the host needs now
            // is the commit that takes the target out of `pending`.
            return self.release_failed(ticket, target);
        }
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
        self.store.take_topics();
        let parsed = self
            .data
            .parse_for(p.target, &mut self.store, &p.source, &p.args, outcome);
        // What this reply's turn watched joins the resource's topics whatever
        // the reply says: a turn that yields another request (a long native
        // call) watched them as much as the one that answers.
        if let Target::Resource(i) = p.target {
            for topic in self.store.take_topics() {
                if !self.watching[i].contains(&topic) {
                    self.watching[i].push(topic);
                }
            }
        }
        let value = match parsed.map_err(|error| RunnerError::Data {
            resource: name.clone(),
            error,
        })? {
            Answer::Now(value) => value,
            Answer::Later(request) => {
                // One more round (LLP 1027 D1a): the target keeps its value,
                // a new ticket goes out for the same arguments, and this
                // commit changes nothing but the pending set.
                self.log(super::lines::one_more(&name));
                self.enqueue(p.target, p.source, p.args, request, false);
                return self.update();
            }
        };
        if !self.conforms(&value, ty) {
            return Err(RunnerError::Shape { resource: name });
        }
        match p.target {
            Target::Resource(i) => {
                self.stale[i] = false;
                self.keep_answer(i, &p.args, &value);
                self.resources[i] = Some(ResourceState {
                    args: p.args,
                    value: crate::held::Held::new(value),
                    store_revision: self.store.revision(),
                    placeholder: false,
                });
            }
            Target::Mutation(m) => {
                let slot = self.mutation_slot(m)?;
                self.slots[slot] = Value::some(value);
                self.landed.push(m);
                // The reply landed: what the mutation changed is asked again
                // in this commit (LLP 1054.000.000 D1).
                for r in self.declared_refreshes(m) {
                    self.force_refresh(r);
                }
            }
        }
        self.router_change().and_then(|_| self.settle(false))?;
        self.update()
    }
}
