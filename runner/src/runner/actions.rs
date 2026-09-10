//! Action writes and owned mutation effects.
use super::*;

impl<D: DataSource> Runner<D> {
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
        let since = self.store.writes().len();
        let kept = self.checkpoint(frames);
        let result = self.run_action_inner(action, args, frames);
        match &result {
            Ok(_) => self.log_store_writes(since),
            Err(_) => self.restore(kept),
        }
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

    fn run_action_inner(
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
        // Ask and validate every send before publishing any slot write.
        let mut sends = Vec::new();
        let mut answered = Vec::new();
        for (m, source, sargs, lifetime) in &outcome.sends {
            let m = *m as usize;
            let target = lifetime.map_or(Target::Mutation(m), |t| Target::OwnedMutation(m, t));
            let row = self.plan.mutations[m].clone();
            let name = self.plan.str(row.name).to_string();
            let (answer, context) =
                self.answer_call(source, sargs)
                    .map_err(|error| RunnerError::Data {
                        resource: name.clone(),
                        error,
                    })?;
            if let Answer::Now(value) = &answer {
                if !value.conforms(&self.plan, row.ty) {
                    return Err(RunnerError::Shape { resource: name });
                }
                answered.push((row.slot.0, Value::some(value.clone())));
            }
            sends.push((target, source.clone(), sargs.clone(), answer, context));
        }
        for (slot, value) in outcome
            .writes
            .iter()
            .map(|(s, v)| (s, v))
            .chain(outcome.row_writes.iter().map(|(s, v, _)| (s, v)))
        {
            if !value.conforms(&self.plan, self.plan.slots[*slot as usize].ty) {
                return Err(RunnerError::SlotType {
                    slot: self
                        .plan
                        .str(self.plan.slots[*slot as usize].name)
                        .to_string(),
                });
            }
        }
        let written: Vec<u32> = outcome
            .writes
            .iter()
            .map(|(s, _)| *s)
            .chain(outcome.row_writes.iter().map(|(s, _, _)| *s))
            .collect();
        for (slot, value) in answered {
            if let Some(owner) = self.plan.slots[slot as usize].owner {
                Frame::row_of(frames, owner.0)
                    .ok_or(Trap::BadScope { pc: 0, depth: 0 })?
                    .borrow_mut()
                    .insert(slot, value);
            } else {
                self.slots[slot as usize] = value;
            }
        }
        for (slot, value) in outcome.writes {
            self.slots[slot as usize] = value;
        }
        for (slot, value, rows) in outcome.row_writes {
            rows.borrow_mut().insert(slot, value);
        }
        // Ticket replacements are tentative until settlement and the walk
        // stand. The outer checkpoint restores even undrained requests.
        let sent: Vec<_> = sends
            .iter()
            .filter(|(_, _, _, answer, _)| matches!(answer, Answer::Later(_)))
            .map(|(target, ..)| *target)
            .collect();
        for (target, source, args, answer, context) in sends {
            match answer {
                Answer::Now(_) => self.forget(target),
                Answer::Later(request) => {
                    self.enqueue(target, source, args, request, false, context)
                }
            }
        }
        for m in 0..self.plan.mutations.len() {
            let slot = self.plan.mutations[m].slot.0;
            if !written.contains(&slot) {
                continue;
            }
            let target = match self.plan.slots[slot as usize].owner {
                None => Target::Mutation(m),
                Some(owner) => Target::OwnedMutation(
                    m,
                    Frame::scope_of(frames, owner.0)
                        .ok_or(Trap::BadScope { pc: 0, depth: 0 })?
                        .borrow()
                        .lifetime,
                ),
            };
            if sent.contains(&target) {
                self.discard_reply(target);
            } else {
                self.forget(target);
            }
        }
        for (i, lifetime) in outcome.refreshes {
            match lifetime {
                None => self.refresh_next.push(i as usize),
                Some(t) => {
                    let cell = self
                        .target_scope(Target::OwnedResource(i as usize, t))
                        .ok_or(Trap::BadScope { pc: 0, depth: 0 })?;
                    if let Some(res) = cell.borrow_mut().resources.get_mut(&(i as usize)) {
                        res.refresh = true;
                    };
                }
            }
        }
        let commands: Vec<_> = outcome
            .commands
            .into_iter()
            .map(|(name, args)| Command { name, args })
            .collect();
        self.settle(false)?;
        let receipt = self.update()?;
        for command in commands {
            let mut line = format!("command {}(", command.name);
            for (i, arg) in command.args.iter().enumerate() {
                if i > 0 {
                    line.push_str(", ");
                }
                crate::agent::untyped_json(arg, &mut line);
            }
            line.push(')');
            self.log(line);
            self.commands.push(command);
        }
        Ok(receipt)
    }
}
