//! Subject/key dependencies only. Row-body changes never trigger an all-N key pass.
use super::super::{Env, Frame};
use crate::compare::same;
use exact_plan::{bytes::Reader, Code, Opcode, Operand, Plan, RegionsId, Stdlib, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Input {
    Slot(u32),
    Derive(u32),
    Resource(u32),
    PendingResource(u32),
    PendingMutation(u32),
    Clock,
}
#[derive(Debug)]
pub(super) struct KeyMemo {
    inputs: Vec<Input>,
    saved: Option<Snapshot>,
}
#[derive(Debug)]
struct Scope {
    item: Option<Value>,
    bound: Option<Value>,
    region: Option<u32>,
    has_row: bool,
}
#[derive(Debug)]
struct Snapshot {
    inputs: Vec<Value>,
    frames: Vec<Scope>,
}
impl KeyMemo {
    pub(super) fn new(plan: &Plan, region: RegionsId) -> Option<Self> {
        let row = plan.region(region);
        Self::for_codes(plan, &[row.subject, row.key])
    }
    /// The subject's fresh outer allocation says nothing about key inputs.
    /// The current item's identity is checked separately, at its old position.
    pub(super) fn key_only(plan: &Plan, region: RegionsId) -> Option<Self> {
        Self::for_codes(plan, &[plan.region(region).key])
    }
    fn for_codes(plan: &Plan, codes: &[Code]) -> Option<Self> {
        let mut inputs = BTreeSet::new();
        for code in codes {
            let mut reader = Reader::new(plan.code(*code));
            while !reader.is_empty() {
                let op = Opcode::from_wire(reader.u8().ok()?)?;
                let mut first = 0;
                for (i, operand) in op.operands().iter().enumerate() {
                    let n = match operand {
                        Operand::U8 | Operand::Enum(_) => reader.u8().ok()? as u32,
                        Operand::U16 => reader.u16().ok()? as u32,
                        Operand::U32 | Operand::Str | Operand::Idx(_) => reader.u32().ok()?,
                        Operand::F64 => {
                            reader.f64().ok()?;
                            0
                        }
                    };
                    if i == 0 {
                        first = n;
                    }
                }
                let input = match op {
                    Opcode::LoadSlot => Some(Input::Slot(first)),
                    Opcode::LoadDerive => Some(Input::Derive(first)),
                    Opcode::LoadResource => Some(Input::Resource(first)),
                    Opcode::PendingResource => Some(Input::PendingResource(first)),
                    Opcode::PendingMutation => Some(Input::PendingMutation(first)),
                    Opcode::Call => match Stdlib::from_wire(first as u8)? {
                        Stdlib::Now => Some(Input::Clock),
                        Stdlib::FormatClockTime
                        | Stdlib::FormatCountdownMinutes
                        | Stdlib::FormatDistance
                        | Stdlib::FormatWalk
                        | Stdlib::Length
                        | Stdlib::IsEmpty
                        | Stdlib::ToString
                        | Stdlib::Floor
                        | Stdlib::Max
                        | Stdlib::Min
                        | Stdlib::EncodeURIComponent
                        | Stdlib::EncodeRouteSegment => None,
                        _ => return None,
                    },
                    Opcode::LoadParam
                    | Opcode::StoreSlot
                    | Opcode::Command
                    | Opcode::Send
                    | Opcode::Refresh => return None,
                    Opcode::Number
                    | Opcode::Bool
                    | Opcode::Str
                    | Opcode::None
                    | Opcode::Unit
                    | Opcode::Some
                    | Opcode::LoadItem
                    | Opcode::LoadBound
                    | Opcode::Field
                    | Opcode::Record
                    | Opcode::List
                    | Opcode::Add
                    | Opcode::Sub
                    | Opcode::Mul
                    | Opcode::Div
                    | Opcode::Rem
                    | Opcode::Neg
                    | Opcode::Eq
                    | Opcode::Ne
                    | Opcode::Lt
                    | Opcode::Le
                    | Opcode::Gt
                    | Opcode::Ge
                    | Opcode::Not
                    | Opcode::Concat
                    | Opcode::Jump
                    | Opcode::JumpIfFalse
                    | Opcode::JumpIfNone
                    | Opcode::Unwrap
                    | Opcode::Pop
                    | Opcode::BindLocal
                    | Opcode::LoadLocal
                    | Opcode::DropLocal
                    | Opcode::Return => None,
                };
                if let Some(input) = input {
                    inputs.insert(input);
                }
            }
        }
        Some(Self {
            inputs: inputs.into_iter().collect(),
            saved: None,
        })
    }
    fn values(&self, env: &Env<'_>, frames: &[Frame]) -> Option<Snapshot> {
        let mut out = Vec::with_capacity(self.inputs.len());
        for input in &self.inputs {
            out.push(match *input {
                Input::Slot(i) => match env.plan.slots.get(i as usize)?.owner {
                    Some(region) => Frame::row_of(frames, region.0)?.borrow().get(&i)?.clone(),
                    None => env.slots.get(i as usize)?.clone(),
                },
                Input::Derive(i) => env.derives.get(i as usize)?.as_ref()?.clone(),
                Input::Resource(i) => env.resources.get(i as usize)?.as_ref()?.clone(),
                Input::PendingResource(i) => Value::Bool(*env.pending_resources.get(i as usize)?),
                Input::PendingMutation(i) => Value::Bool(*env.pending_mutations.get(i as usize)?),
                Input::Clock => Value::Number(env.now_ms),
            });
        }
        Some(Snapshot {
            inputs: out,
            frames: frames
                .iter()
                .map(|frame| Scope {
                    item: frame.item.clone(),
                    bound: frame.bound.clone(),
                    region: frame.region,
                    has_row: frame.row.is_some(),
                })
                .collect(),
        })
    }
    pub(super) fn unchanged(&self, env: &Env<'_>, frames: &[Frame]) -> bool {
        let (Some(old), Some(now)) = (&self.saved, self.values(env, frames)) else {
            return false;
        };
        old.inputs.len() == now.inputs.len()
            && old.inputs.iter().zip(&now.inputs).all(|(a, b)| same(a, b))
            && old.frames.len() == now.frames.len()
            && old.frames.iter().zip(&now.frames).all(|(a, b)| {
                a.region == b.region
                    && a.has_row == b.has_row
                    && same_opt(&a.item, &b.item)
                    && same_opt(&a.bound, &b.bound)
            })
    }
    pub(super) fn remember(&mut self, env: &Env<'_>, frames: &[Frame]) {
        self.saved = self.values(env, frames);
    }
}
fn same_opt(a: &Option<Value>, b: &Option<Value>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => same(a, b),
        (None, None) => true,
        _ => false,
    }
}
