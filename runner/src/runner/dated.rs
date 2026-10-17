//! What holds the launch's date: every state, derive and resource whose
//! value is made from an `exactTime()` resource. The bake leaves out the
//! answer of a resource asked with any of it (LLP 1027.000.000 D2): the
//! bake's date is unknown, zero, and a launch whose source is not ready
//! shows a compiled answer whatever its arguments, so a library stamped at
//! 1970 would stand until the source answered (and a task could keep it).
//! @ref LLP 1027.000.000 D2
use super::{DataSource, Runner};
use crate::instance::{DepReads as Reads, Input};
use exact_plan::{Code, Opcode};

/// Which rows hold a value made from the launch's date.
struct Dated {
    slots: Vec<bool>,
    derives: Vec<bool>,
    resources: Vec<bool>,
}

impl Dated {
    fn reads(&self, inputs: impl Iterator<Item = Input>) -> bool {
        inputs.into_iter().any(|input| match input {
            Input::Slot(s) => self.slots.get(s).copied().unwrap_or(false),
            Input::Derive(d) => self.derives[d],
            Input::Resource(r) | Input::PendingResource(r) | Input::FailedResource(r) => {
                self.resources[r]
            }
            _ => false,
        })
    }

    fn mark(row: &mut [bool], i: usize) -> bool {
        match row.get_mut(i) {
            Some(held) if !*held => {
                *held = true;
                true
            }
            _ => false,
        }
    }
}

impl<D: DataSource> Runner<D> {
    /// Whether an argument of resource `name`, `with` arguments included,
    /// is made from the launch's date: an `exactTime()` resource read
    /// directly, or through a derive, another resource asked with it, or a
    /// state it was assigned to (its initial value, or any action's write).
    /// The bake leaves its answer out of the plan, and the launch shows its
    /// placeholder until the device's source answers. An `else source()`
    /// row is the bake's for every launch (LLP 1048.003 D6) and is never
    /// this.
    pub fn resource_reads_clock(&self, name: &str) -> bool {
        let Some(i) = (0..self.plan.resources.len())
            .find(|&i| self.plan.str(self.plan.resources[i].name) == name)
        else {
            return false;
        };
        let dated = self.dated();
        !self.is_placeholder_row(i) && self.args_dated(&dated, i)
    }

    fn args_dated(&self, dated: &Dated, i: usize) -> bool {
        let deps = self.sites.deps();
        self.plan.resources[i].args.iter().any(|arg| {
            let reads = deps.reads_of(&self.plan, self.plan.arg(arg).expr);
            dated.reads(deps.inputs(&reads))
        })
    }

    /// The rows that hold the launch's date, to a fixed point: each round
    /// adds what reads the last one's, and the rows bound the rounds.
    fn dated(&self) -> Dated {
        let plan = &self.plan;
        let deps = self.sites.deps();
        let mut dated = Dated {
            slots: vec![false; plan.slots.len()],
            derives: vec![false; plan.derives.len()],
            resources: (0..plan.resources.len())
                .map(|r| plan.str(plan.resources[r].source) == crate::time::SOURCE)
                .collect(),
        };
        let reads = |dated: &Dated, r: &Reads| dated.reads(deps.inputs(r));
        loop {
            let mut changed = false;
            for d in 0..plan.derives.len() {
                if !dated.derives[d] && reads(&dated, &deps.derives[d]) {
                    changed |= Dated::mark(&mut dated.derives, d);
                }
            }
            for r in 0..plan.resources.len() {
                if !dated.resources[r] && self.args_dated(&dated, r) {
                    changed |= Dated::mark(&mut dated.resources, r);
                }
            }
            for s in 0..plan.slots.len() {
                if !dated.slots[s] && reads(&dated, &deps.reads_of(plan, plan.slots[s].init)) {
                    changed |= Dated::mark(&mut dated.slots, s);
                }
            }
            for a in &plan.actions {
                changed |= self.writes_dated(&mut dated, a.body);
            }
            if !changed {
                return dated;
            }
        }
    }

    /// Mark each state an action's body assigns a value made from the date.
    /// A statement's value is what its instructions load since the last
    /// statement ended (a write, a send, a command, a refresh, a dropped
    /// value, a branch's condition, which decides a write without being
    /// written); a `let` that holds the date marks every later read of a
    /// local in the body. Parameters are not followed: they are an event's,
    /// at the device's date, or a mutation's answer, which comes only once
    /// the source is ready.
    fn writes_dated(&self, dated: &mut Dated, body: Code) -> bool {
        let mut changed = false;
        let (mut statement, mut local) = (false, false);
        for instruction in crate::vm::instructions(self.plan.code(body)) {
            let Ok(i) = instruction else { break };
            let x = i.args[0] as usize;
            match i.op {
                Opcode::LoadSlot => statement |= dated.slots.get(x).copied().unwrap_or(false),
                Opcode::LoadDerive => statement |= dated.derives.get(x).copied().unwrap_or(false),
                Opcode::LoadResource
                | Opcode::PendingResource
                | Opcode::FailedResource
                | Opcode::FailureResource => {
                    statement |= dated.resources.get(x).copied().unwrap_or(false)
                }
                Opcode::LoadLocal => statement |= local,
                Opcode::BindLocal => {
                    local |= statement;
                    statement = false;
                }
                Opcode::StoreSlot => {
                    if statement {
                        changed |= Dated::mark(&mut dated.slots, x);
                    }
                    statement = false;
                }
                Opcode::JumpIfFalse
                | Opcode::Send
                | Opcode::Command
                | Opcode::Refresh
                | Opcode::Pop => statement = false,
                _ => {}
            }
        }
        changed
    }
}
