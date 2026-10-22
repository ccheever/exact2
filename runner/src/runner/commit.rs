//! Commits: an action, a timer's action, a request's reply — each one
//! transaction that settles, updates the tree and applies one batch, or
//! refuses and leaves the kernel as it was.

use super::*;

/// A string argument or write past [`crate::vm::MAX_STRING`] bytes.
fn too_long(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| s.len() > crate::vm::MAX_STRING)
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
    failed_args: Vec<Option<Vec<Value>>>,
    failed_why: Vec<Option<crate::failure::Failure>>,
    failed_mut: Vec<Option<crate::failure::Failure>>,
    refresh_next: Vec<usize>,
    writes: super::writes::Writes,
    unsent: Vec<(usize, String, Vec<Value>)>,
    reconciling: Vec<usize>,
    pending: Vec<PendingReq>,
    queues: super::queue::Saved,
    timers: Vec<super::Timer>,
    published: Option<Box<Published>>,
    commands: usize,
    /// Socket batches of earlier commits, not yet taken (LLP 1016.000 D7):
    /// a refusal takes back the batches staged after them.
    sockets: usize,
    /// The root font size, and the window's app tuple (`setMode`).
    root_font: (super::root_font::RootFont, f32, u32),
    /// Kept answers refused before this commit (`kept::refuse`).
    refused: usize,
}

/// What a settlement publishes that a refusal after it — the gate step
/// (LLP 1092 D8) — must put back; taken only for a plan with a gated task
/// or a sequence (LLP 1125 D2), whose gate steps are the one refusal that
/// follows a settlement that stood.
pub(super) struct Published {
    settled: Option<super::settlement::Settled>,
    derives: Vec<Option<Value>>,
    derive_store_dependent: Vec<bool>,
    resource_values: Vec<Option<crate::held::Held>>,
    resources: Vec<Option<ResourceState>>,
    awaiting: Vec<bool>,
    requests: usize,
}

impl<D: DataSource> Runner<D> {
    /// The presenter's on-demand geometry for shaped inline runs.
    pub fn set_inline_geometry(
        &mut self,
        read: Option<std::rc::Rc<dyn crate::geometry::InlineGeometry>>,
    ) {
        self.scrolled.inline = read;
    }

    /// Before a commit. `resources`: the commit edits the settled caches
    /// before settling (a reply does).
    pub(super) fn checkpoint(&mut self, resources: bool) -> Checkpoint {
        self.start_commit();
        self.derives_evaluated = 0;
        self.copied_at_checkpoint = self.store.copied_bytes();
        Checkpoint {
            store: self.store.checkpoint(),
            slots: self.slots.clone(),
            resources: resources.then(|| self.resources.clone()),
            stale: self.stale.clone(),
            store_readers: self.store_readers.clone(),
            watching: self.watching.clone(),
            failed_args: self.failed_args.clone(),
            failed_why: self.failed_why.clone(),
            failed_mut: self.failed_mut.clone(),
            refresh_next: self.refresh_next.clone(),
            writes: self.writes.clone(),
            unsent: self.unsent.clone(),
            reconciling: self.reconciling.clone(),
            pending: self.pending.clone(),
            queues: self.queues.save(),
            timers: self.timers.clone(),
            published: (self.plan.timers.iter().any(|t| t.gated || t.keyed)
                || !self.plan.sequences.is_empty())
            .then(|| {
                Box::new(Published {
                    settled: self.settled.clone(),
                    derives: self.derives.clone(),
                    derive_store_dependent: self.derive_store_dependent.clone(),
                    resource_values: self.resource_values.clone(),
                    resources: self.resources.clone(),
                    awaiting: self.awaiting.clone(),
                    requests: self.requests.len(),
                })
            }),
            commands: self.commands.len(),
            sockets: self.sockets.len(),
            root_font: (
                self.root_font,
                self.kernel.root_font_size(),
                self.ids.theme.window,
            ),
            refused: self.keeping.refused.len(),
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
            Ok(_) => {
                self.persist_slots();
                self.journal_refused();
                self.log_store_writes(c.store.writes);
                self.transport_stood();
            }
            Err(e) if self.poisoned && !was_poisoned => {
                self.store.restore(c.store);
                self.refuse_batches(c.sockets, &crate::failure::Failure::of(e).message);
            }
            Err(e) => {
                self.refuse_batches(c.sockets, &crate::failure::Failure::of(e).message);
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
                self.failed_args = c.failed_args;
                self.failed_why = c.failed_why;
                self.failed_mut = c.failed_mut;
                self.refresh_next = c.refresh_next;
                // Ids stay unique: a rolled-back send's id is never reused.
                let seq = self.writes.seq;
                self.writes = c.writes;
                self.writes.seq = seq;
                self.overlays.iter_mut().for_each(|o| *o = None);
                self.unsent = c.unsent;
                self.reconciling = c.reconciling;
                // A send its source refused when it was sent stays ended.
                for (m, id) in std::mem::take(&mut self.refused_unsent) {
                    self.unsent.retain(|u| u.0 != m);
                    self.writes.records.retain(|r| Some(r.id) != id);
                    // And what it showed in is still read again.
                    self.reconcile_after(m);
                }
                self.pending = c.pending;
                self.queues.restore(c.queues);
                self.timers = c.timers;
                if let Some(p) = c.published {
                    self.settled = p.settled;
                    self.derives = p.derives;
                    self.derive_store_dependent = p.derive_store_dependent;
                    self.resource_values = p.resource_values;
                    self.resources = p.resources;
                    self.awaiting = p.awaiting;
                    // A resource request the settlement handed out goes
                    // with it: never taken, its continuation is let go.
                    let dropped: Vec<RequestOut> = self.requests.drain(p.requests..).collect();
                    for r in &dropped {
                        self.discard_request(&r.request);
                    }
                }
                self.sync_pending_flags();
                self.commands.truncate(c.commands);
                // A refusal the restore undid is not said.
                self.keeping.refused.truncate(c.refused);
                self.root_font = c.root_font.0;
                let _ = self.kernel.set_root_font_size(c.root_font.1);
                self.ids.theme.window = c.root_font.2;
            }
        }
        self.refused_unsent.clear();
        // What a refused commit's gate step read is let go (LLP 1125 D2).
        self.transport.pending = None;
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
            self.data.forgotten(&self.store, &in_flight);
        }
        self.sample();
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
        self.arm_next(result.is_ok());
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
                self.then_due[m] = self.armed_at();
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
            // A hidden parameter (`@capture:…`, a child's captured prop; `@`
            // begins no authored name) is the compiler's argument, not the
            // host's: it has its type, but its numbers need not be finite
            // nor its strings short, as a prop read in place needs neither.
            if self.plan.str(param.name).starts_with('@') {
                if !args[i].typed(&self.plan, param.ty) {
                    return Err(RunnerError::ArgumentType {
                        action: self.plan.str(row.name).to_string(),
                        param: self.plan.str(param.name).to_string(),
                    });
                }
                continue;
            }
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
        // Geometry (LLP 1051.000 D2): every `measure` in the body is
        // answered now, with the kernel's engine tree to lay out; `frame`
        // reads through the borrowed kernel while the body runs.
        let measured;
        let geometry = match self.links.geometry {
            Some(links) => {
                measured = links.ahead(
                    &self.plan,
                    self.plan.code(row.body),
                    &mut self.kernel,
                    &self.scrolled,
                );
                Some(crate::geometry::GeometryEnv::new(
                    &self.kernel,
                    &self.scrolled,
                    links,
                    &measured,
                ))
            }
            None => None,
        };
        let outcome = {
            // The frames in force at the view the event hit (LLP 1017 P4c):
            // a row action reads and writes its row through them.
            let mut env = self.env(&args, frames);
            env.geometry = geometry.as_ref();
            vm::eval(self.plan.code(row.body), &env, &allowed)?
        };
        // Sends (LLP 1016 §4): each asks the source now. An answer lands in
        // the mutation's slot inside this commit; a request goes to the host
        // once the commit stands. A queue mutation's send is asked only when
        // the mutation is free and it is the commit's first; else it waits
        // its turn (LLP 1092 D2).
        let mut later: Vec<(usize, String, Vec<Value>, Request)> = Vec::new();
        let mut answered: Vec<(u32, Value)> = Vec::new();
        let mut asked: Vec<usize> = Vec::new();
        let mut landed_now: Vec<usize> = Vec::new();
        let mut failed_now = Vec::new();
        // Non-queue mutations whose send replaces one in flight without a
        // request of its own: an answer at once, or one waiting for the source.
        let mut replaced: Vec<usize> = Vec::new();
        // A send before the source can answer — a TypeScript module a native
        // host loads after first pixel (LLP 1027 D4) — waits for it, pending,
        // as a resource's ask does and as the web's send to a module still
        // loading does: `data_ready` sends it (notes diary: a document
        // opened at launch was refused, and lost). A queue's waits in its
        // queue instead, every send in order, its head asked once the source
        // is ready (LLP 1092 D2).
        let mut unsent: Vec<(usize, String, Vec<Value>)> = Vec::new();
        for (m, source, sargs) in &outcome.sends {
            let m = *m as usize;
            self.accept_write(m, source, sargs);
            let queue = self.plan.mutations[m].queue;
            let early = !self.data.ready() && !exact_plan::runner_owned_source(source);
            if early && !queue {
                unsent.retain(|u| u.0 != m);
                unsent.push((m, source.clone(), sargs.clone()));
                // It replaces a send still in flight (newest wins).
                replaced.push(m);
                continue;
            }
            if queue && (early || asked.contains(&m) || !self.asks_now(m)) {
                if let Err(e) = self.wait_turn(m, source, sargs) {
                    self.discard_later(&later);
                    return Err(e);
                }
                continue;
            }
            asked.push(m);
            match self.ask_send(m, source, sargs) {
                Ok(super::queue::Asked::Now(slot, v)) => {
                    answered.push((slot as u32, v));
                    self.landed.push(m);
                    landed_now.push(m);
                    // It replaces an earlier send of this action still to go
                    // out (newest wins): that request never goes.
                    if !queue {
                        let earlier: Vec<_> = later
                            .iter()
                            .filter(|l| l.0 == m)
                            .map(|l| l.3.clone())
                            .collect();
                        for request in &earlier {
                            self.discard_request(request);
                        }
                        later.retain(|l| l.0 != m);
                    }
                }
                Ok(super::queue::Asked::Later(request)) => {
                    later.push((m, source.clone(), sargs.clone(), request))
                }
                Ok(super::queue::Asked::Failed) => {
                    let (older, rest) = later.into_iter().partition(|l| l.0 == m);
                    later = rest;
                    self.discard_later(&older);
                    self.pending_mut[m] = false;
                    failed_now.push(m);
                }
                Err(e) => {
                    self.discard_later(&later);
                    return Err(e);
                }
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
            self.commands.push(Command {
                name,
                args,
                source: self.input_source,
            });
        }
        // An assignment to a mutation's slot tentatively makes it not
        // pending. The pending map is changed only after settlement stands:
        // a refused assignment did not change what reply the view wants.
        // A queue's assignment forgets nothing: every reply still lands
        // (LLP 1092 D4).
        let assigned: Vec<usize> = (0..self.plan.mutations.len())
            .filter(|m| {
                written.contains(&self.plan.mutations[*m].slot.0) && !self.plan.mutations[*m].queue
            })
            .collect();
        for m in 0..self.plan.mutations.len() {
            if written.contains(&self.plan.mutations[m].slot.0) {
                self.failed_mut[m] = None;
            }
        }
        for m in asked.iter().chain(assigned.iter()) {
            if !failed_now.contains(m) {
                self.failed_mut[*m] = None;
            }
        }
        for m in &assigned {
            self.pending_mut[*m] = false;
            self.end_pending(*m, None);
        }
        // A send answered at once replaces one in flight: not pending.
        for m in &landed_now {
            if !self.plan.mutations[*m].queue {
                self.pending_mut[*m] = false;
            }
        }
        for (m, _, _, _) in &later {
            if !assigned.contains(m) {
                self.pending_mut[*m] = true;
            }
        }
        // A send that goes now replaces one still waiting for the source
        // (newest wins)
        for m in asked.iter().filter(|m| !self.plan.mutations[**m].queue) {
            self.unsent.retain(|u| u.0 != *m);
        }
        // Waiting from here, so the settlement sees them pending; a refusal
        // puts the waiting sends back as they were (the checkpoint).
        for (m, source, args) in &unsent {
            self.unsent.retain(|u| u.0 != *m);
            if !assigned.contains(m) {
                self.unsent.push((*m, source.clone(), args.clone()));
                self.pending_mut[*m] = true;
            }
        }
        // What the action refreshes, added to what is already waiting
        // (LLP 1054.000.000 D2). A mutation answered at once has landed:
        // what it changes is asked, as a reply's landing asks it
        // (`fulfill`). A send still in flight asks nothing: its write shows
        // through the overlay.
        for r in outcome.refreshes.iter() {
            self.force_refresh(*r as usize);
        }
        for m in &landed_now {
            for r in self.declared_refreshes(*m) {
                self.force_refresh(r);
            }
        }
        // A refusal from here is put back by the checkpoint (run_action);
        // row slots live in the tree, so they are undone here. The gate
        // step joins this path (LLP 1092 D8): before `enqueue` and
        // `into_view`, which a refusal there must not leave applied.
        if let Err(e) = self
            .router_change()
            .and_then(|_| self.settle(false))
            .and_then(|_| {
                // Settlement's flags follow the tickets, and this commit's
                // requests are handed out after the step: a gate reads them
                // pending, as the settled derives did.
                for (m, _, _, _) in &later {
                    self.pending_mut[*m] |= !assigned.contains(m);
                }
                self.gate_step()
            })
        {
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
        for m in 0..self.plan.mutations.len() {
            if written.contains(&self.plan.mutations[m].slot.0) {
                self.failed_mut[m] = None;
            }
        }
        for m in asked.iter().chain(assigned.iter()) {
            if !failed_now.contains(m) {
                self.failed_mut[*m] = None;
            }
        }
        for m in &assigned {
            self.forget(Target::Mutation(*m));
        }
        // A send answered at once, or waiting for the source, replaces one
        // still in flight (newest wins): the older reply is not wanted.
        for m in landed_now.into_iter().chain(replaced).chain(failed_now) {
            if !self.plan.mutations[m].queue {
                self.forget_ticket(Target::Mutation(m));
            }
        }
        self.sync_pending_flags();
        for (m, source, args, request) in later {
            self.enqueue(Target::Mutation(m), source, args, request, false);
            if assigned.contains(&m) {
                self.forget(Target::Mutation(m));
            }
        }
        for (m, _, _) in unsent.iter().filter(|u| !assigned.contains(&u.0)) {
            self.log(super::lines::unsent(
                self.plan.str(self.plan.mutations[*m].name),
            ));
        }
        self.take_set_modes(first_command);
        // The scheme and root size a development host holds: kept as asked, not applied (LLP 1122 T7).
        self.take_held(first_command);
        // `setRootFontSize` is the runner's too: the size lands in this
        // commit's layout, and the command stays for a host that mirrors it;
        // a refused one is journaled as refused, never as a command.
        let mut i = first_command;
        while i < self.commands.len() {
            if self.commands[i].name == "setRootFontSize" {
                let args = self.commands[i].args.clone();
                if !self.app_root_font_size(&args) {
                    self.commands.remove(i);
                    continue;
                }
            }
            i += 1;
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
        // `scrollIntoView` of a list's row is the runner's own (LLP
        // 1070.000): it runs in this commit, after the update, and never
        // reaches a host. An element's (its id and three options, not the
        // row form's seven arguments) is the host's, as `focus` is.
        let stated: Vec<Command> = self.commands.drain(first_command..).collect();
        for command in stated {
            if command.name != "scrollIntoView" || command.args.len() != 7 {
                self.commands.push(command);
                continue;
            }
            match crate::instance::collection::IntoView::from_command(&command.args) {
                Ok(request) => self.into_view.push(request),
                Err(why) => self.log(format!("scrollIntoView refused: {why}")),
            }
        }
        let result = self.update();
        // Commands are journaled only once the update committed: a failure
        // there poisons the runner and clears them.
        if result.is_ok() {
            for line in commands {
                self.log(line);
            }
            self.apply_sounds(first_command);
        }
        result
    }

    /// Whether `value` conforms to `ty`, checking only the list items not
    /// already found conforming ([`crate::conform::Conformed`]).
    pub(super) fn conforms(&self, value: &Value, ty: exact_plan::TypesId) -> bool {
        self.conformed.borrow_mut().conforms(&self.plan, value, ty)
    }

    /// The refusal of `value`, which did not conform to `ty`.
    pub(super) fn shape(
        &self,
        resource: String,
        value: &Value,
        ty: exact_plan::TypesId,
    ) -> RunnerError {
        let why = crate::conform::mismatch(&self.plan, value, ty);
        RunnerError::Shape { resource, why }
    }

    pub(super) fn poison(&mut self) {
        self.poisoned = true;
        self.notes.clear();
        self.commands.clear();
        self.requests.clear();
        self.drop_background();
        self.forgot |= !self.pending.is_empty();
        self.pending.clear();
        self.unsent.clear();
        self.forget_waiting();
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
        self.forget_ticket(target);
        if let Target::Mutation(m) = target {
            self.unsent.retain(|u| u.0 != m);
        }
        self.sync_pending_flags();
    }

    /// Let go of `target`'s request in flight, keeping a send that waits
    /// for the source.
    pub(super) fn forget_ticket(&mut self, target: Target) {
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.forgot = true;
            self.log(super::lines::forgot(t, &self.target_name(target)));
        }
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
            stream: sockets::stream_count(&request),
            grant: request
                .stream
                .then(|| {
                    crate::grants::admitting(
                        self.data.grants(),
                        request.grants.as_deref(),
                        &request.url,
                    )
                })
                .flatten()
                .map(str::to_string),
            ask_again: false,
            origin: self.writes.tick(),
            write: match target {
                Target::Mutation(m) => self.asked_write(m),
                Target::Resource(_) => None,
            },
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
        // A stream is pending until its first message (LLP 1016.000 D1).
        for p in self.pending.iter().filter(|p| p.in_flight()) {
            match p.target {
                Target::Resource(i) => self.pending_res[i] = true,
                Target::Mutation(m) => self.pending_mut[m] = true,
            }
        }
        // A queue's waiting send is pending too (LLP 1092 D4).
        for m in 0..self.pending_mut.len() {
            self.pending_mut[m] |= self.queues.waits(m);
        }
        // A placeholder shown until the source can answer is pending too,
        // and so is a send waiting for it.
        for (i, awaiting) in self.awaiting.iter().enumerate() {
            self.pending_res[i] |= *awaiting;
        }
        for (m, _, _) in &self.unsent {
            self.pending_mut[*m] = true;
        }
    }

    /// The requests the host is to run since the last take (LLP 1016 D2).
    /// The module's background round goes out here when it has one (LLP
    /// 1097 D5), and its journal lines come in.
    pub fn take_requests(&mut self) -> Vec<RequestOut> {
        self.take_data_logs();
        self.arm_background();
        std::mem::take(&mut self.requests)
    }

    /// The work behind continuation `token` (LLP 1027.002 D3): a host asks
    /// on this thread, after the commit that handed the request out, so a
    /// worker's snapshot is the store as committed then.
    pub fn dispatch_work(&mut self, token: u64) -> Dispatch {
        // A module request the runner holds for the agent (LLP 1133.002 §6).
        if let Some(dispatch) = self.module_dispatch(token) {
            return dispatch;
        }
        self.data.dispatch(token, &self.store)
    }

    /// Where this app's long native calls go: the source's own slot, or,
    /// for a source with none (a Rust source), the runner's. The host
    /// installs the app module in it (LLP 1067.000 Q6, Q9).
    pub fn native_slot(&self) -> crate::Native {
        self.data.native().unwrap_or_else(|| self.native.clone())
    }

    /// The work behind a long native call ([`Request::is_native`]): hand its
    /// body to the source's native handler with the reply, on the host's
    /// worker, which returns at once — the module's own thread answers.
    pub fn native_work(&mut self, request: &Request) -> Dispatch {
        // A module's name called from data code: the runner's admission, then
        // the module (LLP 1133.002 D14).
        if let Some(dispatch) = self.module_call(&request.body) {
            return dispatch;
        }
        let handler = self.native_slot().handler();
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
        let mut out = self.data.release(&self.store);
        out.extend(self.module_release());
        out
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
        let mut out: Vec<(String, u64)> = self
            .pending
            .iter()
            .filter(|p| !self.device_holds.iter().any(|h| h.ticket == p.ticket))
            .map(|p| (self.target_name(p.target), p.ticket))
            .collect();
        out.extend(
            self.background_ticket()
                .map(|t| (super::background::NAME.to_string(), t)),
        );
        out
    }

    /// Whether any request is in flight (the agent's `settle` waits on it).
    /// An open stream counts only until its first message (LLP 1016.000
    /// D5): after that it is open, not in flight, or `settle` never ends.
    /// A request held for the agent (LLP 1069.007 D3) is not I/O either: no
    /// clock waits on it. The module's background round is (LLP 1097 D9).
    pub fn has_pending(&self) -> bool {
        self.background_ticket().is_some()
            || self
                .pending
                .iter()
                .any(|p| p.in_flight() && !self.held_for_agent(p))
    }

    /// A request held for the agent: a device's, or a module's (LLP 1133.002 §6).
    fn held_for_agent(&self, p: &PendingReq) -> bool {
        self.device_holds.iter().any(|h| h.ticket == p.ticket)
            || self.extensions.holds_token(p.continuation)
    }

    /// The requests in flight, as `has_pending` counts them: the agent's
    /// `state.pending` (a held request is listed there under `device`).
    /// The module's background round is listed last, as `background`.
    pub fn in_flight(&self) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self
            .pending
            .iter()
            .filter(|p| p.in_flight() && !self.held_for_agent(p))
            .map(|p| (self.target_name(p.target), p.ticket))
            .collect();
        out.extend(
            self.background_ticket()
                .map(|t| (super::background::NAME.to_string(), t)),
        );
        out
    }

    /// Every open stream: its resource, ticket, and counts (the agent's
    /// `state.streams`, LLP 1016.000 D5).
    pub fn streams(&self) -> Vec<(String, u64, StreamCount)> {
        self.pending
            .iter()
            .filter_map(|p| Some((self.target_name(p.target), p.ticket, p.stream?)))
            .collect()
    }

    /// The grant line that admitted stream `ticket`, when one of the app's
    /// does (`state.streams`).
    pub fn stream_grant(&self, ticket: u64) -> Option<&str> {
        let p = self.pending.iter().find(|p| p.ticket == ticket)?;
        p.grant.as_deref()
    }

    /// Whether resource `i` has a stream open that has delivered: it is not
    /// pending, but a newer answer must still let its ticket go.
    pub(super) fn streaming(&self, i: usize) -> bool {
        self.pending
            .iter()
            .any(|p| p.target == Target::Resource(i) && !p.in_flight())
    }

    /// Whether request `ticket` is still wanted. A host asks after each
    /// commit and lets go of the work for any it holds that isn't (LLP 1016
    /// D5): a superseded or forgotten reply would only be dropped here.
    pub fn holds(&self, ticket: u64) -> bool {
        self.is_background(ticket) || self.pending.iter().any(|p| p.ticket == ticket)
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
        self.fulfill_measured(ticket, outcome, None)
    }

    /// Fulfill with elapsed wall milliseconds measured by the executor, never
    /// by the seekable agent clock. Missing measurements stay unnamed.
    pub fn fulfill_measured(
        &mut self,
        ticket: u64,
        outcome: Outcome,
        elapsed_ms: Option<u64>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let answering = std::mem::replace(&mut self.churn.answering, true);
        let out = self.arrived(|r| r.fulfill_now(ticket, outcome, elapsed_ms));
        self.churn.answering = answering;
        out
    }

    fn fulfill_now(
        &mut self,
        ticket: u64,
        outcome: Outcome,
        elapsed_ms: Option<u64>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if self.is_background(ticket) {
            // Its own path (LLP 1097 D5): no commit, no `then`, no update.
            self.background_landed(outcome);
            return Ok(None);
        }
        let mut summary = outcome.summary();
        if let Some(ms) = elapsed_ms {
            exact_num::push_text!(&mut summary, "; wall {} ms", ms);
        }
        let Some(pos) = self.pending.iter().position(|p| p.ticket == ticket) else {
            self.log(super::lines::dropped(ticket, &summary));
            return Ok(None);
        };
        if let (Some(_), Outcome::Message(_)) = (self.pending[pos].stream, &outcome) {
            return self.message(pos, outcome, &summary);
        }
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(true);
        let p = self.pending.remove(pos);
        self.sync_pending_flags();
        let what = super::lines::fulfilling(ticket, &self.target_name(p.target), &summary);
        let (refused, target) = (p.refused, p.target);
        let result = self.fulfill_inner(p, outcome);
        self.conclude(checkpoint, &result, was_poisoned);
        self.arm_then(result.is_ok());
        self.arm_next(result.is_ok());
        self.log_outcome(&what, &result, was_poisoned);
        if let Err(e) = &result {
            // A mutation's reply is spent whatever refused its commit: left
            // in flight, its write would show forever.
            if (refused
                || matches!(e, RunnerError::Data { .. } | RunnerError::Shape { .. })
                || matches!(target, Target::Mutation(_)))
                && self.holds(ticket)
            {
                // The failure is in the journal (above); what the host needs now
                // is the commit that takes the target out of `pending`.
                // A refused request has no later reply, even when its shaped
                // failure traps during settlement. Admission refusal alone
                // cannot restart a source whose earlier turns may have written.
                return self.release_failed(ticket, target, crate::failure::Failure::of(e));
            }
        }
        result.map(Some)
    }

    /// One message of the open stream at `pos` (LLP 1016.000 D1): parsed
    /// like a single reply and committed as its own settlement, with the
    /// ticket kept open. A message the source cannot take ends the stream,
    /// as a failed reply ends a request.
    fn message(
        &mut self,
        pos: usize,
        outcome: Outcome,
        summary: &str,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(true);
        let p = &mut self.pending[pos];
        if let (Some(count), Outcome::Message(m)) = (p.stream.as_mut(), &outcome) {
            count.messages += 1;
            count.coalesced += u64::from(m.coalesced);
            sockets::socket_message(count, m);
        }
        let p = p.clone();
        self.sync_pending_flags();
        let (ticket, target, name) = (p.ticket, p.target, self.target_name(p.target));
        let what = super::lines::fulfilling(ticket, &name, summary);
        let before = self.sockets.len();
        crate::stream_timing::entered(ticket);
        let result = self.fulfill_inner(p, outcome);
        // What the mapper staged answering this frame (LLP 1016.000 D7):
        // addressed for the commit, or left staged by a frame that failed.
        // The commit's answers may have sent too (its amendment of
        // 2026-10-18); their batches are not the mapper's.
        let left = !self.store.take_sockets().is_empty();
        let mapper = |b: &crate::SocketSend| matches!(b.origin, crate::SocketOrigin::Mapper(_));
        let replied = left || self.sockets[before..].iter().any(mapper);
        self.conclude(checkpoint, &result, was_poisoned);
        crate::stream_timing::committed(result.is_ok());
        self.arm_then(result.is_ok());
        self.arm_next(result.is_ok());
        self.log_outcome(&what, &result, was_poisoned);
        // Or by one whose answer reopened the stream, which sends none.
        if replied && (result.is_err() || left) {
            let why = (result.as_ref().err())
                .map_or("its answer reopened the stream".into(), |e| {
                    crate::failure::Failure::of(e).message
                });
            self.log(sockets::refused(&crate::SocketOrigin::Mapper(name), &why));
        }
        if let (Err(e), true) = (&result, self.holds(ticket)) {
            return self.release_failed(ticket, target, crate::failure::Failure::of(e));
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
        let message = p.stream.is_some() && matches!(outcome, Outcome::Message(_));
        let ty = match p.target {
            Target::Resource(i) => self.plan.resources[i].ty,
            Target::Mutation(m) => self.plan.mutations[m].ty,
        };
        self.store.take_topics();
        self.snapshot_sockets();
        let parsed = if Self::activity_send(&p.source) {
            Self::activity_reply(&self.plan, ty, outcome)
        } else if self.plan.module_named(&p.source).is_some() {
            self.module_parse(ty, &p.source, outcome)
        } else {
            self.data
                .parse_for(p.target, &mut self.store, &p.source, &p.args, outcome)
        };
        // Only a socket's mapper answering a frame stages for this commit
        // (LLP 1016.000 D7); a turn of an answer sends whatever it answered
        // (the amendment of 2026-10-18).
        if !message {
            self.store.take_sockets();
        }
        self.stage_answer(p.target);
        for line in self.data.take_logs() {
            self.log(line);
        }
        // What this reply's turn watched joins the resource's topics whatever
        // the reply says: a turn that yields another request (a long native
        // call) watched them as much as the one that answers.
        if let Target::Resource(i) = p.target {
            // A module's source that data code read: the answer is asked
            // again when that source's answer changes (LLP 1133.002 D14).
            let module = (p.keepable.as_ref()).and_then(|r| self.module_topic(&r.body));
            for topic in self.store.take_topics().into_iter().chain(module) {
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
            Answer::Later(request) if message && !request.stream => {
                // A message commits as its own settlement; a request that is
                // not a stream would replace the stream's ticket with one
                // answer (LLP 1016.000 D1).
                return Err(RunnerError::Data {
                    resource: name,
                    error: DataError::Unavailable(
                        "a stream's message answers now, or reopens the stream".into(),
                    ),
                });
            }
            Answer::Later(request) => {
                // One more round (LLP 1027 D1a): the target keeps its value,
                // a new ticket goes out for the same arguments, and this
                // commit changes nothing but the pending set.
                // A round of an ask made before a watched topic changed
                // carries the ask again to its last (LLP 1016.002 D4).
                self.log(super::lines::one_more(&name));
                self.enqueue(p.target, p.source, p.args, request, false);
                if let Some(next) = self.pending.last_mut() {
                    next.ask_again |= p.ask_again;
                    next.origin = p.origin;
                    next.write = p.write;
                }
                return self.update();
            }
        };
        if !self.conforms(&value, ty) {
            return Err(self.shape(name, &value, ty));
        }
        // Its replies go out once this frame's commit stands (D7).
        if message {
            if let Err(why) = self.stage_replies(&p) {
                let error = DataError::Unavailable(why);
                return Err(RunnerError::Data {
                    resource: name,
                    error,
                });
            }
        }
        match p.target {
            Target::Resource(i) => {
                let shown = self.resources[i].take();
                self.revalidated(i, shown.as_ref(), &value);
                self.stale[i] = false;
                self.failed_args[i] = None;
                self.keep_answer(i, &p.args, &value);
                if p.ask_again {
                    self.log(super::lines::asked_again(p.ticket, &name));
                    self.force_refresh(i);
                }
                self.resources[i] = Some(ResourceState {
                    args: p.args,
                    value: crate::held::Held::new(value),
                    store_revision: self.store.revision(),
                    placeholder: false,
                    kept_seed: false,
                    answered_for: None,
                    origin: p.origin,
                });
            }
            Target::Mutation(m) => {
                self.failed_mut[m] = None;
                let slot = self.mutation_slot(m)?;
                self.land_write(m, p.write, &value);
                self.slots[slot] = Value::some(value);
                self.landed.push(m);
                // The reply landed: what the mutation changed is asked again
                // in this commit (LLP 1054.000.000 D1); its write shows until
                // that answer lands.
                for r in self.declared_refreshes(m) {
                    self.force_refresh(r);
                }
            }
        }
        self.router_change()
            .and_then(|_| self.settle(false))
            .and_then(|_| self.gate_step())?;
        self.update()
    }
}
