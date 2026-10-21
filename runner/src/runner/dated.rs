//! What holds the launch's date: every state, derive and resource whose
//! value is made from, or decided by, an `exactTime()` resource. The bake
//! leaves out the answer of a resource asked with any of it (LLP
//! 1027.000.000 D2): the bake's date is unknown, zero, and a launch whose
//! source is not ready shows a compiled answer whatever its arguments, so a
//! library stamped at 1970 would stand until the source answered (and a
//! task could keep it).
//! @ref LLP 1027.000.000 D2
use super::{DataSource, Runner};
use crate::instance::Input;
use crate::machine::Instruction;
use exact_plan::Opcode;

/// Which rows hold a value made from the launch's date.
struct Dated {
    slots: Vec<bool>,
    derives: Vec<bool>,
    resources: Vec<bool>,
}

impl Dated {
    fn reads(&self, inputs: &[Input]) -> bool {
        inputs.iter().any(|input| match *input {
            Input::Slot(s) => self.slots.get(s).copied().unwrap_or(false),
            Input::Derive(d) => self.derives.get(d).copied().unwrap_or(false),
            Input::Resource(r) | Input::PendingResource(r) | Input::FailedResource(r) => {
                self.resources.get(r).copied().unwrap_or(false)
            }
            _ => false,
        })
    }

    fn load(&self, i: &Instruction) -> bool {
        let x = i.args[0] as usize;
        let row = match i.op {
            Opcode::LoadSlot => &self.slots,
            Opcode::LoadDerive => &self.derives,
            Opcode::LoadResource
            | Opcode::PendingResource
            | Opcode::FailedResource
            | Opcode::FailureResource => &self.resources,
            _ => return false,
        };
        row.get(x).copied().unwrap_or(false)
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
    /// For each resource, whether an argument of it, `with` arguments
    /// included, is made from the launch's date: an `exactTime()` resource
    /// read directly, or through a derive, another resource asked with it,
    /// or a state it was assigned to or decided (its initial value, or an
    /// action's write of it or under a condition that reads it). The bake
    /// asks once and leaves those answers out of the plan; the launch shows
    /// each one's placeholder until the device's source answers. An `else
    /// source()` row is the bake's for every launch (LLP 1048.003 D6) and is
    /// never one.
    pub fn resources_reading_clock(&self) -> Vec<bool> {
        let plan = &self.plan;
        let deps = self.sites.deps();
        // Each row's inputs and each action's instructions, read once.
        let collect = |reads: &crate::instance::DepReads| deps.inputs(reads).collect::<Vec<_>>();
        let derives: Vec<Vec<Input>> = deps.derives.iter().map(collect).collect();
        let resources: Vec<Vec<Input>> = plan
            .resources
            .iter()
            .map(|row| {
                row.args
                    .iter()
                    .flat_map(|arg| collect(&deps.reads_of(plan, plan.arg(arg).expr)))
                    .collect()
            })
            .collect();
        let slots: Vec<Vec<Input>> = plan
            .slots
            .iter()
            .map(|row| collect(&deps.reads_of(plan, row.init)))
            .collect();
        let actions: Vec<Vec<Instruction>> = plan
            .actions
            .iter()
            .map(|a| {
                crate::vm::instructions(plan.code(a.body))
                    .map_while(Result::ok)
                    .collect()
            })
            .collect();
        let mut dated = Dated {
            slots: vec![false; plan.slots.len()],
            derives: vec![false; plan.derives.len()],
            resources: (0..plan.resources.len())
                .map(|r| plan.str(plan.resources[r].source) == crate::time::SOURCE)
                .collect(),
        };
        // To a fixed point: each round adds what reads the last one's, and
        // the rows bound the rounds.
        loop {
            let mut changed = false;
            for (d, inputs) in derives.iter().enumerate() {
                if !dated.derives[d] && dated.reads(inputs) {
                    changed |= Dated::mark(&mut dated.derives, d);
                }
            }
            for (r, inputs) in resources.iter().enumerate() {
                if !dated.resources[r] && dated.reads(inputs) {
                    changed |= Dated::mark(&mut dated.resources, r);
                }
            }
            for (s, inputs) in slots.iter().enumerate() {
                if !dated.slots[s] && dated.reads(inputs) {
                    changed |= Dated::mark(&mut dated.slots, s);
                }
            }
            for body in &actions {
                changed |= writes_dated(&mut dated, body);
            }
            if !changed {
                break;
            }
        }
        (0..plan.resources.len())
            .map(|r| !self.is_placeholder_row(r) && dated.reads(&resources[r]))
            .collect()
    }
}

/// Mark each state an action's body writes a value made from the date, or
/// writes at all under a condition that reads it. A statement's value is
/// what its instructions load since the last statement ended (a write, a
/// send, a command, a refresh, a dropped value), conditions and both arms
/// of a conditional expression included; a branch whose condition is dated
/// marks every write in its arms, to where they join; a `let` bound from
/// the date, or under such a branch, marks every later read of a local in
/// the body. Parameters are not followed: they are an event's, at the
/// device's date, or a mutation's answer, which comes only once the source
/// is ready.
fn writes_dated(dated: &mut Dated, body: &[Instruction]) -> bool {
    let mut changed = false;
    let (mut statement, mut local) = (false, false);
    // The ends of the dated branches the walk is inside.
    let mut branches: Vec<usize> = Vec::new();
    for (n, i) in body.iter().enumerate() {
        branches.retain(|&end| i.pc < end);
        let decided = !branches.is_empty();
        match i.op {
            Opcode::LoadLocal => statement |= local,
            Opcode::JumpIfFalse | Opcode::JumpIfNone if statement => {
                branches.push(join(body, n, i.args[0] as usize));
            }
            Opcode::BindLocal => {
                local |= statement || decided;
                statement = false;
            }
            Opcode::StoreSlot => {
                if statement || decided {
                    changed |= Dated::mark(&mut dated.slots, i.args[0] as usize);
                }
                statement = false;
            }
            Opcode::Send | Opcode::Command | Opcode::Refresh | Opcode::Pop => statement = false,
            _ => statement |= dated.load(i),
        }
    }
    changed
}

/// Where the arms of the branch at `body[at]`, jumping to `target` when its
/// condition fails, join: past the else arm when the then arm ends by
/// jumping over it, else at `target`.
fn join(body: &[Instruction], at: usize, target: usize) -> usize {
    let first_else = body[at..]
        .iter()
        .position(|i| i.pc >= target)
        .map(|k| at + k);
    match first_else.and_then(|k| k.checked_sub(1)).map(|k| &body[k]) {
        Some(jump) if jump.op == Opcode::Jump && jump.args[0] as usize > target => {
            jump.args[0] as usize
        }
        _ => target,
    }
}
