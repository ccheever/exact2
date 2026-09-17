//! Subject/key dependencies only. Row-body changes never trigger an all-N key pass.
use super::super::{Env, Frame};
use exact_plan::{bytes::Reader, Opcode, Operand, Plan, RegionsId, Stdlib, Value};
use std::{collections::BTreeSet, rc::Rc};

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
    saved: Option<Vec<Value>>,
}
impl KeyMemo {
    pub(super) fn new(plan: &Plan, region: RegionsId) -> Option<Self> {
        let mut inputs = BTreeSet::new();
        let row = plan.region(region);
        for code in [row.subject, row.key] {
            let mut reader = Reader::new(plan.code(code));
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
                    _ => None,
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
    fn values(&self, env: &Env<'_>, frames: &[Frame]) -> Option<Vec<Value>> {
        let mut out = Vec::with_capacity(self.inputs.len() + frames.len() * 2);
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
        for frame in frames {
            out.extend([
                frame.item.clone().unwrap_or(Value::Unit),
                frame.bound.clone().unwrap_or(Value::Unit),
            ]);
        }
        Some(out)
    }
    pub(super) fn unchanged(&self, env: &Env<'_>, frames: &[Frame]) -> bool {
        let (Some(old), Some(now)) = (&self.saved, self.values(env, frames)) else {
            return false;
        };
        old.len() == now.len() && old.iter().zip(&now).all(|(a, b)| same(a, b))
    }
    pub(super) fn remember(&mut self, env: &Env<'_>, frames: &[Frame]) {
        self.saved = self.values(env, frames);
    }
}
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.to_bits() == b.to_bits(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Unit, Value::Unit) | (Value::Option(None), Value::Option(None)) => true,
        (Value::Str(a), Value::Str(b)) => Rc::ptr_eq(a, b),
        (Value::List(a), Value::List(b)) | (Value::Record(a), Value::Record(b)) => Rc::ptr_eq(a, b),
        (Value::Option(Some(a)), Value::Option(Some(b))) => Rc::ptr_eq(a, b),
        _ => false,
    }
}
