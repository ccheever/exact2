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
    refresh_next: Vec<usize>,
    reread_next: Vec<usize>,
    pending: Vec<PendingReq>,
    queues: super::queue::Saved,
    timers: Vec<super::Timer>,
    published: Option<Box<Published>>,
    commands: usize,
}

/// What a settlement publishes that a refusal after it — the gate step
/// (LLP 1092 D8) — must put back; taken only for a plan with a gated task,
/// the one refusal that follows a settlement that stood.
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
            failed_args: self.failed_args.clone(),
            refresh_next: self.refresh_next.clone(),
            reread_next: self.reread_next.clone(),
            pending: self.pending.clone(),
            queues: self.queues.save(),
            timers: self.timers.clone(),
            published: self
                .plan
                .timers
                .iter()
                .any(|t| t.gated || t.keyed)
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
                self.failed_args = c.failed_args;
                self.refresh_next = c.refresh_next;
                self.reread_next = c.reread_next;
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
    /// timer's due time, and the refusal rides along. Frame tasks fire their
    /// virtual frames here unless the host presents frames
    /// ([`Runner::present_frames`]; LLP 1073 D3).
    pub fn advance_timed(&mut self, now_ms: f64) -> Advanced {
        self.advance_within(now_ms, false, true)
    }

    /// Whether the host presents frames (LLP 1073 D4): while it does, frame
    /// tasks fire only at [`Runner::frame`]; while it doesn't (the default:
    /// tests, and the agent's seekable clock), every advance fires their
    /// virtual frames. A host turns it on when its display drives the clock
    /// and off when the agent's clock takes over.
    pub fn present_frames(&mut self, on: bool) {
        self.presenting = on;
    }

    /// A presented frame at `now_ms` (LLP 1073 D2, D4): timers due by then
    /// fire as [`Runner::advance_timed`] fires them, then every frame task
    /// once, at `now_ms`, in plan order; a frame missed is never caught up.
    /// Each task's virtual frames restart from `now_ms` (`super::virtual_frame`).
    pub fn frame(&mut self, now_ms: f64) -> Advanced {
        let presenting = std::mem::replace(&mut self.presenting, true);
        let mut a = self.advance_timed(now_ms);
        self.presenting = presenting;
        // A refused `next` is journaled and the frame goes on: its frame
        // tasks fire, and the `next` waits for a later wake (LLP 1092 D3).
        let next_refused = std::mem::take(&mut self.queues.stopped_on_next);
        if (a.error.is_some() && !next_refused) || !self.wants_frames() {
            return a;
        }
        let at = self.now_ms;
        for i in 0..self.plan.timers.len() {
            // An idle gated frame task is skipped (LLP 1092 D10).
            if !self.plan.timers[i].frame || !self.timers[i].armed {
                continue;
            }
            self.timers[i].base = at;
            self.timers[i].k = 1;
            self.timers[i].next_ms = super::virtual_frame(at, 1);
            let action = self.plan.timers[i].action;
            let was_poisoned = self.poisoned;
            match self.run_action(action, Vec::new(), &[]) {
                Ok(receipt) => a.receipts.push(Timed { at_ms: at, receipt }),
                Err(e) => {
                    let what = format!(
                        "frame {} ({})",
                        i,
                        self.plan.str(self.plan.action(action).name)
                    );
                    let failed = Err(e);
                    self.log_outcome(&what, &failed, was_poisoned);
                    a.error = failed.err();
                    return a;
                }
            }
        }
        a.now_ms = self.now_ms;
        a
    }

    /// [`Runner::advance_timed`], stopping after the first timer whose
    /// commit hands the host a request: the clock stays at that timer's due
    /// time, for the host to land the reply before advancing again. The
    /// runner keeps one request per target (LLP 1016 D5), so the next tick's
    /// send would drop it. An agent's clock jump advances this way (LLP
    /// 1012); on the wall clock, replies land between ticks by themselves.
    pub fn advance_until_request(&mut self, now_ms: f64) -> Advanced {
        self.advance_within(now_ms, true, true)
    }

    /// The `then` of every answer already landed, each its own commit, with
    /// the clock where it is and no timer fired: how an agent's input ends
    /// (LLP 1012 §2), so the next operation sees what the input settled —
    /// a wall-clock host runs them at once by itself (`timer_due_ms`).
    /// trivia F3, kanban F19.
    pub fn land_then(&mut self) -> Advanced {
        self.advance_within(self.now_ms, false, false)
    }

    fn advance_within(&mut self, now_ms: f64, until_request: bool, timers: bool) -> Advanced {
        // A host that presents frames fires frame tasks at `frame`; else
        // their virtual frames are timers (LLP 1073 D3).
        let frames = !self.presenting;
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
        self.queues.stopped_on_next = false;
        loop {
            // The earliest due timer, deterministic by index on ties.
            let due = self
                .timers
                .iter()
                .enumerate()
                .filter(|_| timers)
                .filter(|(i, t)| t.next_ms <= now_ms && (frames || !self.plan.timers[*i].frame))
                .min_by(|(ia, a), (ib, b)| {
                    a.next_ms.partial_cmp(&b.next_ms).unwrap().then(ia.cmp(ib))
                })
                .map(|(i, t)| (i, t.next_ms));
            // An answer's `then` goes before a timer due at the same time: the
            // answer landed first. A queue's `next` goes after the `then`s and
            // before the timers (LLP 1092 D3).
            let then = self
                .then_due
                .iter()
                .enumerate()
                .filter(|(_, at)| **at <= now_ms)
                .min_by(|(ia, a), (ib, b)| a.partial_cmp(b).unwrap().then(ia.cmp(ib)))
                .map(|(m, at)| (m, *at));
            let next = self
                .due_next(now_ms)
                .filter(|(_, at)| due.is_none_or(|(_, timer)| *at <= timer))
                .filter(|(_, at)| then.is_none_or(|(_, then)| *at < then));
            let then = then.filter(|(_, at)| due.is_none_or(|(_, timer)| *at <= timer));
            let then = then.filter(|_| next.is_none());
            if let Some((m, at)) = next {
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
                let ticket = self.next_ticket;
                match self.run_next(m) {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        self.queues.stopped_on_next = true;
                        return Advanced {
                            receipts,
                            now_ms: self.now_ms,
                            error: Some(e),
                        };
                    }
                }
                if until_request && self.next_ticket != ticket {
                    landed = self.now_ms;
                    break;
                }
                continue;
            }
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
            // A drop's hold ends at its deadline (LLP 1094 D8), before a
            // timer due at the same time: the drop came first.
            if let Some(at) = self
                .reorder_deadline()
                .filter(|at| timers && *at <= now_ms && due.is_none_or(|(_, t)| *at <= t))
            {
                self.now_ms = self.now_ms.max(at);
                match self.reorder_timeout() {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        return Advanced {
                            receipts,
                            now_ms: self.now_ms,
                            error: Some(e),
                        }
                    }
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
            } else if row.frame {
                self.timers[i].k = self.timers[i].k.saturating_add(1);
                super::virtual_frame(self.timers[i].base, self.timers[i].k)
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
            let queue = self.plan.mutations[m].queue;
            let early = !self.data.ready() && !exact_plan::runner_owned_source(source);
            if early && !queue {
                unsent.retain(|u| u.0 != m);
                unsent.push((m, source.clone(), sargs.clone()));
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
                }
                Ok(super::queue::Asked::Later(request)) => {
                    later.push((m, source.clone(), sargs.clone(), request))
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
        for m in &assigned {
            self.pending_mut[*m] = false;
        }
        for (m, _, _, _) in &later {
            if !assigned.contains(m) {
                self.pending_mut[*m] = true;
            }
        }
        // Waiting from here, so the settlement sees them pending; a refusal
        // puts the waiting sends back as they were.
        let unsent_before = (!unsent.is_empty()).then(|| self.unsent.clone());
        for (m, source, args) in &unsent {
            self.unsent.retain(|u| u.0 != *m);
            if !assigned.contains(m) {
                self.unsent.push((*m, source.clone(), args.clone()));
                self.pending_mut[*m] = true;
            }
        }
        // What the action refreshes, added to what is already waiting
        // (LLP 1054.000.000 D2); and what each mutation it sent to declares
        // it changes, read again now that every send has asked its source.
        // A mutation answered at once has landed: what it changes is forced,
        // as a reply's landing forces it (`fulfill`). A re-read would drop a
        // source that answers later, and no reply would come to ask again
        // (an async read stayed at its old value, found by the data6 lane).
        for r in outcome.refreshes.iter() {
            self.force_refresh(*r as usize);
        }
        for m in &landed_now {
            for r in self.declared_refreshes(*m) {
                self.force_refresh(r);
            }
        }
        self.reread_next = asked
            .iter()
            .filter(|m| !landed_now.contains(m))
            .flat_map(|m| self.declared_refreshes(*m))
            .collect();
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
            if let Some(before) = unsent_before {
                self.unsent = before;
            }
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
        for (m, _, _) in unsent.iter().filter(|u| !assigned.contains(&u.0)) {
            self.log(super::lines::unsent(
                self.plan.str(self.plan.mutations[*m].name),
            ));
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
        // row form's six arguments) is the host's, as `focus` is.
        let stated: Vec<Command> = self.commands.drain(first_command..).collect();
        for command in stated {
            if command.name != "scrollIntoView" || command.args.len() != 6 {
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
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.forgot = true;
            self.log(super::lines::forgot(t, &self.target_name(target)));
        }
        if let Target::Mutation(m) = target {
            self.unsent.retain(|u| u.0 != m);
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
            stream: request.stream.then(StreamCount::default),
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
                .any(|p| p.in_flight() && !self.device_holds.iter().any(|h| h.ticket == p.ticket))
    }

    /// The requests in flight, as `has_pending` counts them: the agent's
    /// `state.pending` (a held request is listed there under `device`).
    /// The module's background round is listed last, as `background`.
    pub fn in_flight(&self) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self
            .pending
            .iter()
            .filter(|p| p.in_flight() && !self.device_holds.iter().any(|h| h.ticket == p.ticket))
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
        }
        let p = p.clone();
        self.sync_pending_flags();
        let (ticket, target) = (p.ticket, p.target);
        let what = super::lines::fulfilling(ticket, &self.target_name(target), summary);
        let result = self.fulfill_inner(p, outcome);
        self.conclude(checkpoint, &result, was_poisoned);
        self.arm_then(result.is_ok());
        self.arm_next(result.is_ok());
        self.log_outcome(&what, &result, was_poisoned);
        if result.is_err() && self.holds(ticket) {
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
        let message = p.stream.is_some() && matches!(outcome, Outcome::Message(_));
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
                let shown = self.resources[i].take();
                self.revalidated(i, shown.as_ref(), &value);
                self.stale[i] = false;
                self.failed_args[i] = None;
                self.keep_answer(i, &p.args, &value);
                self.resources[i] = Some(ResourceState {
                    args: p.args,
                    value: crate::held::Held::new(value),
                    store_revision: self.store.revision(),
                    placeholder: false,
                    kept_seed: false,
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
        self.router_change()
            .and_then(|_| self.settle(false))
            .and_then(|_| self.gate_step())?;
        self.update()
    }
}
