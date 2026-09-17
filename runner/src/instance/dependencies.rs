//! Conservative memo for outer keyed regions. The cache compares only referenced
//! immutable globals, never a deep list walk. Row-local state and contextual
//! expressions fall back to normal evaluation. No app annotation is required.
use super::{sites, Site};
use crate::vm::Env;
use exact_plan::{bytes::Reader, Code, Opcode, Operand, Plan, RegionsId, Stdlib, Value};
use std::{collections::BTreeSet, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Input {
    Slot(usize),
    Derive(usize),
    Resource(usize),
    PendingResource(usize),
    PendingMutation(usize),
    Clock,
}

#[derive(Debug)]
pub(super) struct Memo {
    inputs: Vec<Input>,
    saved: Option<Vec<Value>>,
}
impl Memo {
    pub(super) fn for_region(plan: &Plan, region: RegionsId, body_only: bool) -> Option<Self> {
        let mut inputs = BTreeSet::new();
        let mut stack = vec![Site::Region(region)];
        while let Some(site) = stack.pop() {
            match site {
                Site::Node(n) => {
                    let node = plan.node(n);
                    for binding in node.bindings.iter() {
                        scan(plan, plan.binding(binding).expr, &mut inputs)?;
                    }
                    if let Some(surface) = node.surface {
                        for arg in plan.surface(surface).args.iter() {
                            scan(plan, plan.arg(arg).expr, &mut inputs)?;
                        }
                    }
                    stack.extend(sites(plan, Some(n), node.arm).into_iter().map(|(_, s)| s));
                }
                Site::Region(r) => {
                    let row = plan.region(r);
                    if !body_only || r != region {
                        scan(plan, row.subject, &mut inputs)?;
                        scan(plan, row.key, &mut inputs)?;
                    }
                    for arm in row.arms.iter() {
                        stack.extend(sites(plan, None, Some(arm)).into_iter().map(|(_, s)| s));
                    }
                }
            }
        }
        Some(Self {
            inputs: inputs.into_iter().collect(),
            saved: None,
        })
    }
    fn values(&self, env: &Env<'_>) -> Option<Vec<Value>> {
        self.inputs
            .iter()
            .map(|input| {
                Some(match *input {
                    Input::Slot(i) => env.slots.get(i)?.clone(),
                    Input::Derive(i) => env.derives.get(i)?.as_ref()?.clone(),
                    Input::Resource(i) => env.resources.get(i)?.as_ref()?.clone(),
                    Input::PendingResource(i) => Value::Bool(*env.pending_resources.get(i)?),
                    Input::PendingMutation(i) => Value::Bool(*env.pending_mutations.get(i)?),
                    Input::Clock => Value::Number(env.now_ms),
                })
            })
            .collect()
    }
    pub(super) fn unchanged(&self, env: &Env<'_>) -> bool {
        let (Some(old), Some(now)) = (&self.saved, self.values(env)) else {
            return false;
        };
        old.len() == now.len() && old.iter().zip(&now).all(|(a, b)| same(a, b))
    }
    pub(super) fn remember(&mut self, env: &Env<'_>) {
        self.saved = self.values(env);
    }
}

// Immutable values are shared across updates. A different allocation is a
// conservative miss even if equal: checking it recursively would itself be O(N).
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
fn scan(plan: &Plan, code: Code, inputs: &mut BTreeSet<Input>) -> Option<()> {
    let mut reader = Reader::new(plan.code(code));
    while !reader.is_empty() {
        let op = Opcode::from_wire(reader.u8().ok()?)?;
        let mut first = 0;
        for (i, operand) in op.operands().iter().enumerate() {
            let value = match operand {
                Operand::U8 | Operand::Enum(_) => reader.u8().ok()? as u32,
                Operand::U16 => reader.u16().ok()? as u32,
                Operand::U32 | Operand::Str | Operand::Idx(_) => reader.u32().ok()?,
                Operand::F64 => {
                    reader.f64().ok()?;
                    0
                }
            };
            if i == 0 {
                first = value;
            }
        }
        match op {
            Opcode::LoadSlot => {
                if plan.slots.get(first as usize)?.owner.is_some() {
                    return None;
                }
                inputs.insert(Input::Slot(first as usize));
            }
            Opcode::LoadDerive => {
                inputs.insert(Input::Derive(first as usize));
            }
            Opcode::LoadResource => {
                inputs.insert(Input::Resource(first as usize));
            }
            Opcode::PendingResource => {
                inputs.insert(Input::PendingResource(first as usize));
            }
            Opcode::PendingMutation => {
                inputs.insert(Input::PendingMutation(first as usize));
            }
            Opcode::Call => match Stdlib::from_wire(first as u8)? {
                Stdlib::Now => {
                    inputs.insert(Input::Clock);
                }
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
                | Stdlib::EncodeRouteSegment => {}
                _ => return None,
            },
            Opcode::LoadParam
            | Opcode::StoreSlot
            | Opcode::Command
            | Opcode::Send
            | Opcode::Refresh => return None,
            // Bindings can refer to the rows/arms *inside* this outer region;
            // their identity is entirely determined by the captured globals.
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
            | Opcode::Return => {}
        }
    }
    Some(())
}

// Row bodies may distinguish signed zero (for example `1 / n > 0`). Value's
// language equality deliberately does not. Cache equivalence must preserve bits.
pub(super) fn same_item(a: &Option<Value>, b: &Option<Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => equivalent(a, b),
        _ => false,
    }
}
fn equivalent(a: &Value, b: &Value) -> bool {
    if same(a, b) {
        return true;
    }
    match (a, b) {
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::List(a), Value::List(b)) | (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| equivalent(a, b))
        }
        (Value::Option(Some(a)), Value::Option(Some(b))) => equivalent(a, b),
        _ => false,
    }
}
