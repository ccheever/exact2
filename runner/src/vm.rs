//! The expression VM.
//!
//! @ref LLP 1004 D2 (`opcodes.json` is the one authority) / LLP 0508 §4.3
//! (closed expressions; research)
//!
//! A stack machine over [`Value`]s. Type inference in the compiler guarantees
//! stack discipline for compiled plans; the VM still checks everything and
//! traps with a typed reason, because a plan is data from outside and never
//! trusted. There is no closure, no heap of the VM's own, no ambient read that
//! is not an operand: `now()` reads the clock the runner passes in.

use crate::stdlib;
use exact_plan::bytes::Reader;
use exact_plan::{Opcode, Operand, Plan, Stdlib, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A keyed row's own slots — the values of the `state` a child component
/// declared, one set per row (LLP 1017 P4c) — shared by the row and every
/// frame that reaches it, so a read during an update and a write applied
/// after an action's commit see one storage.
pub type RowSlots = Rc<RefCell<BTreeMap<u32, Value>>>;

/// One instance scope: what an `each` row or a `match` arm binds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    /// The `each` item, when this scope is a keyed row.
    pub item: Option<Value>,
    /// The `match` binding, when this scope is a `some(x)` arm.
    pub bound: Option<Value>,
    /// The `each` region this row belongs to, when this scope is a row.
    pub region: Option<u32>,
    /// The row's slots, when this scope is a row.
    pub row: Option<RowSlots>,
}

impl Frame {
    /// The row slots of the innermost frame belonging to `region`.
    pub fn row_of(frames: &[Frame], region: u32) -> Option<&RowSlots> {
        frames
            .iter()
            .rev()
            .find(|f| f.region == Some(region))
            .and_then(|f| f.row.as_ref())
    }
}

/// Everything an expression may read.
pub struct Env<'a> {
    /// The plan the code belongs to.
    pub plan: &'a Plan,
    /// State slots by index.
    pub slots: &'a [Value],
    /// Derives by index; `None` while not yet settled this update.
    pub derives: &'a [Option<Value>],
    /// Resource values by index; `None` while not yet settled this update.
    pub resources: &'a [Option<Value>],
    /// Action parameters (empty outside an action).
    pub params: &'a [Value],
    /// Enclosing instance scopes, innermost last.
    pub frames: &'a [Frame],
    /// The clock, in milliseconds.
    pub now_ms: f64,
    /// Whether each resource has a request in flight (LLP 1016 D3).
    pub pending_resources: &'a [bool],
    /// Whether each mutation has a request in flight.
    pub pending_mutations: &'a [bool],
    /// Store dependence of settled derives, for bake provenance propagation.
    pub store_dependent_derives: &'a [bool],
    /// Store dependence of settled resources, for bake provenance propagation.
    pub store_dependent_resources: &'a [bool],
}

/// A typed evaluation failure. The plan was validated, so a trap is a
/// semantic defect the compiler let through, named by pc.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum Trap {
    StackUnderflow {
        pc: usize,
    },
    TypeMismatch {
        pc: usize,
        op: Opcode,
    },
    UnwrapNone {
        pc: usize,
    },
    BadField {
        pc: usize,
        index: u16,
    },
    BadScope {
        pc: usize,
        depth: u16,
    },
    BadParam {
        pc: usize,
        index: u16,
    },
    Arity {
        pc: usize,
        expected: usize,
    },
    WriteNotDeclared {
        pc: usize,
        slot: u32,
    },
    BadJump {
        pc: usize,
        target: u32,
    },
    Malformed {
        pc: usize,
    },
    NoResult,
    /// A derive or resource read before it settled this update; the runner's
    /// settlement loop retries it later in the same pass.
    Pending {
        pc: usize,
    },
}

/// What a body produced: its value, slot writes in order, and commands.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    /// The value left on the stack (`Unit` for statement bodies).
    pub value: Value,
    /// `(slot, value)` writes in execution order.
    pub writes: Vec<(u32, Value)>,
    /// `(slot, value, row)` writes to row slots, with the row they belong to
    /// (LLP 1017 P4c).
    pub row_writes: Vec<(u32, Value, RowSlots)>,
    /// `(name, args)` commands in execution order.
    pub commands: Vec<(String, Vec<Value>)>,
    /// `(mutation, source, args)` sends in execution order (LLP 1016).
    pub sends: Vec<(u32, String, Vec<Value>)>,
    /// Resources to re-request with their current arguments.
    pub refreshes: Vec<u32>,
    /// The evaluated path read a value derived from the durable store.
    pub store_dependent: bool,
}

/// Evaluate `code` in `env`. `allowed_writes` bounds `StoreSlot`; an action
/// passes its `writes` range, an expression passes nothing.
pub fn eval(code: &[u8], env: &Env<'_>, allowed_writes: &[u32]) -> Result<Outcome, Trap> {
    let mut stack: Vec<Value> = Vec::with_capacity(16);
    let mut locals: Vec<Value> = Vec::new();
    let mut out = Outcome::default();
    let mut r = Reader::new(code);
    let malformed = |pc: usize| Trap::Malformed { pc };
    macro_rules! pop {
        ($pc:expr) => {
            stack.pop().ok_or(Trap::StackUnderflow { pc: $pc })?
        };
    }
    macro_rules! num2 {
        ($pc:expr, $op:expr, $f:expr) => {{
            let b = pop!($pc);
            let a = pop!($pc);
            match (a, b) {
                (Value::Number(a), Value::Number(b)) => stack.push($f(a, b)),
                _ => return Err(Trap::TypeMismatch { pc: $pc, op: $op }),
            }
        }};
    }
    while !r.is_empty() {
        let pc = r.position();
        let op = Opcode::from_wire(r.u8().map_err(|_| malformed(pc))?).ok_or(malformed(pc))?;
        // Operands, in declared order.
        let mut args: [u64; 3] = [0, 0, 0];
        let mut f64_arg = 0.0;
        for (i, operand) in op.operands().iter().enumerate() {
            match operand {
                Operand::U8 | Operand::Enum(_) => {
                    args[i] = r.u8().map_err(|_| malformed(pc))? as u64
                }
                Operand::U16 => args[i] = r.u16().map_err(|_| malformed(pc))? as u64,
                Operand::U32 | Operand::Str | Operand::Idx(_) => {
                    args[i] = r.u32().map_err(|_| malformed(pc))? as u64
                }
                Operand::F64 => f64_arg = r.f64().map_err(|_| malformed(pc))?,
            }
        }
        match op {
            Opcode::Number => stack.push(Value::Number(f64_arg)),
            Opcode::Bool => stack.push(Value::Bool(args[0] != 0)),
            Opcode::Str => stack.push(Value::Str(Rc::from(
                env.plan.str(exact_plan::StrId(args[0] as u32)),
            ))),
            Opcode::None => stack.push(Value::NONE),
            Opcode::Unit => stack.push(Value::Unit),
            Opcode::Some => {
                let v = pop!(pc);
                stack.push(Value::some(v));
            }
            Opcode::LoadSlot => {
                let slot = args[0] as usize;
                let row = env.plan.slots.get(slot).ok_or(malformed(pc))?;
                let v = match row.owner {
                    // A row slot: the value the innermost row of its region holds.
                    Some(region) => Frame::row_of(env.frames, region.0)
                        .and_then(|r| r.borrow().get(&(slot as u32)).cloned())
                        .ok_or(Trap::BadScope { pc, depth: 0 })?,
                    None => env.slots.get(slot).cloned().ok_or(malformed(pc))?,
                };
                stack.push(v);
            }
            Opcode::LoadDerive => {
                let derive = args[0] as usize;
                let value = env
                    .derives
                    .get(derive)
                    .ok_or(malformed(pc))?
                    .clone()
                    .ok_or(Trap::Pending { pc })?;
                out.store_dependent |= env
                    .store_dependent_derives
                    .get(derive)
                    .copied()
                    .unwrap_or(false);
                stack.push(value);
            }
            Opcode::LoadResource => {
                let resource = args[0] as usize;
                let value = env
                    .resources
                    .get(resource)
                    .ok_or(malformed(pc))?
                    .clone()
                    .ok_or(Trap::Pending { pc })?;
                out.store_dependent |= env
                    .store_dependent_resources
                    .get(resource)
                    .copied()
                    .unwrap_or(false);
                stack.push(value);
            }
            Opcode::LoadParam => stack.push(env.params.get(args[0] as usize).cloned().ok_or(
                Trap::BadParam {
                    pc,
                    index: args[0] as u16,
                },
            )?),
            Opcode::LoadItem | Opcode::LoadBound => {
                let depth = args[0] as usize;
                let frame = env
                    .frames
                    .len()
                    .checked_sub(depth + 1)
                    .and_then(|i| env.frames.get(i))
                    .ok_or(Trap::BadScope {
                        pc,
                        depth: depth as u16,
                    })?;
                let v = if op == Opcode::LoadItem {
                    &frame.item
                } else {
                    &frame.bound
                };
                stack.push(v.clone().ok_or(Trap::BadScope {
                    pc,
                    depth: depth as u16,
                })?);
            }
            Opcode::Field => {
                let index = args[0] as u16;
                match pop!(pc) {
                    Value::Record(fields) => stack.push(
                        fields
                            .get(index as usize)
                            .cloned()
                            .ok_or(Trap::BadField { pc, index })?,
                    ),
                    _ => return Err(Trap::TypeMismatch { pc, op }),
                }
            }
            Opcode::Record => {
                let ty = exact_plan::TypesId(args[0] as u32);
                let n = env.plan.type_(ty).fields.len as usize;
                if stack.len() < n {
                    return Err(Trap::StackUnderflow { pc });
                }
                let fields = stack.split_off(stack.len() - n);
                stack.push(Value::record(fields));
            }
            Opcode::List => {
                let n = args[0] as usize;
                if stack.len() < n {
                    return Err(Trap::StackUnderflow { pc });
                }
                let items = stack.split_off(stack.len() - n);
                stack.push(Value::list(items));
            }
            Opcode::Add => num2!(pc, op, |a, b| Value::Number(a + b)),
            Opcode::Sub => num2!(pc, op, |a, b| Value::Number(a - b)),
            Opcode::Mul => num2!(pc, op, |a, b| Value::Number(a * b)),
            Opcode::Div => num2!(pc, op, |a, b| Value::Number(a / b)),
            Opcode::Rem => num2!(pc, op, |a, b| Value::Number(a % b)),
            Opcode::Lt => num2!(pc, op, |a, b| Value::Bool(a < b)),
            Opcode::Le => num2!(pc, op, |a, b| Value::Bool(a <= b)),
            Opcode::Gt => num2!(pc, op, |a, b| Value::Bool(a > b)),
            Opcode::Ge => num2!(pc, op, |a, b| Value::Bool(a >= b)),
            Opcode::Neg => match pop!(pc) {
                Value::Number(n) => stack.push(Value::Number(-n)),
                _ => return Err(Trap::TypeMismatch { pc, op }),
            },
            Opcode::Eq | Opcode::Ne => {
                let b = pop!(pc);
                let a = pop!(pc);
                let eq = equal(&a, &b).ok_or(Trap::TypeMismatch { pc, op })?;
                stack.push(Value::Bool(if op == Opcode::Eq { eq } else { !eq }));
            }
            Opcode::Not => match pop!(pc) {
                Value::Bool(b) => stack.push(Value::Bool(!b)),
                _ => return Err(Trap::TypeMismatch { pc, op }),
            },
            Opcode::Concat => {
                let b = pop!(pc);
                let a = pop!(pc);
                match (a, b) {
                    (Value::Str(a), Value::Str(b)) => {
                        let mut s = String::with_capacity(a.len() + b.len());
                        s.push_str(&a);
                        s.push_str(&b);
                        stack.push(Value::Str(Rc::from(s)));
                    }
                    _ => return Err(Trap::TypeMismatch { pc, op }),
                }
            }
            Opcode::Jump => {
                let target = args[0] as u32;
                r = jump(code, target).ok_or(Trap::BadJump { pc, target })?;
            }
            Opcode::JumpIfFalse => {
                let target = args[0] as u32;
                match pop!(pc) {
                    Value::Bool(true) => {}
                    Value::Bool(false) => {
                        r = jump(code, target).ok_or(Trap::BadJump { pc, target })?
                    }
                    _ => return Err(Trap::TypeMismatch { pc, op }),
                }
            }
            Opcode::JumpIfNone => {
                let target = args[0] as u32;
                match stack.last() {
                    Some(Value::Option(Some(_))) => {}
                    Some(Value::Option(None)) => {
                        r = jump(code, target).ok_or(Trap::BadJump { pc, target })?
                    }
                    Some(_) => return Err(Trap::TypeMismatch { pc, op }),
                    None => return Err(Trap::StackUnderflow { pc }),
                }
            }
            Opcode::Unwrap => match pop!(pc) {
                Value::Option(Some(v)) => stack.push((*v).clone()),
                Value::Option(None) => return Err(Trap::UnwrapNone { pc }),
                _ => return Err(Trap::TypeMismatch { pc, op }),
            },
            Opcode::Call => {
                let f = Stdlib::from_wire(args[0] as u8).ok_or(malformed(pc))?;
                let n = f.arity();
                if stack.len() < n {
                    return Err(Trap::Arity { pc, expected: n });
                }
                let call_args = stack.split_off(stack.len() - n);
                let v =
                    stdlib::call(f, &call_args, env.now_ms).ok_or(Trap::TypeMismatch { pc, op })?;
                stack.push(v);
            }
            Opcode::StoreSlot => {
                let slot = args[0] as u32;
                if !allowed_writes.contains(&slot) {
                    return Err(Trap::WriteNotDeclared { pc, slot });
                }
                let v = pop!(pc);
                match env
                    .plan
                    .slots
                    .get(slot as usize)
                    .ok_or(malformed(pc))?
                    .owner
                {
                    Some(region) => {
                        // A row slot: written to the row in force — an action
                        // run with no row (`act`, a timer) has none to write.
                        let row = Frame::row_of(env.frames, region.0)
                            .ok_or(Trap::BadScope { pc, depth: 0 })?;
                        out.row_writes.push((slot, v, row.clone()));
                    }
                    None => out.writes.push((slot, v)),
                }
            }
            Opcode::Command => {
                let name = env.plan.str(exact_plan::StrId(args[0] as u32)).to_string();
                let n = args[1] as usize;
                if stack.len() < n {
                    return Err(Trap::StackUnderflow { pc });
                }
                let cargs = stack.split_off(stack.len() - n);
                out.commands.push((name, cargs));
            }
            Opcode::Send => {
                let m = args[0] as u32;
                let slot = env.plan.mutation(exact_plan::MutationsId(m)).slot.0;
                if !allowed_writes.contains(&slot) {
                    return Err(Trap::WriteNotDeclared { pc, slot });
                }
                let source = env.plan.str(exact_plan::StrId(args[1] as u32)).to_string();
                let n = args[2] as usize;
                if stack.len() < n {
                    return Err(Trap::StackUnderflow { pc });
                }
                let sargs = stack.split_off(stack.len() - n);
                out.sends.push((m, source, sargs));
            }
            Opcode::Refresh => out.refreshes.push(args[0] as u32),
            Opcode::PendingResource => {
                // Known once the resource settled this pass, like its value.
                let i = args[0] as usize;
                if env.resources.get(i).is_none_or(Option::is_none) {
                    return Err(Trap::Pending { pc });
                }
                stack.push(Value::Bool(
                    env.pending_resources.get(i).copied().unwrap_or(false),
                ));
            }
            Opcode::PendingMutation => stack.push(Value::Bool(
                env.pending_mutations
                    .get(args[0] as usize)
                    .copied()
                    .unwrap_or(false),
            )),
            Opcode::Pop => {
                pop!(pc);
            }
            Opcode::BindLocal => {
                let v = pop!(pc);
                locals.push(v);
            }
            Opcode::LoadLocal => stack.push(locals.get(args[0] as usize).cloned().ok_or(
                Trap::BadScope {
                    pc,
                    depth: args[0] as u16,
                },
            )?),
            Opcode::DropLocal => {
                locals.pop().ok_or(Trap::StackUnderflow { pc })?;
            }
            Opcode::Return => {
                out.value = stack.pop().unwrap_or(Value::Unit);
                return Ok(out);
            }
        }
    }
    Err(Trap::NoResult)
}

fn jump(code: &[u8], target: u32) -> Option<Reader<'_>> {
    if target as usize > code.len() {
        return None;
    }
    let mut r = Reader::new(code);
    r.bytes(target as usize).ok()?;
    Some(r)
}

/// Structural equality for closed values; `None` when the kinds differ
/// (a compile-time rejection that a hostile plan may still attempt).
pub fn equal(a: &Value, b: &Value) -> Option<bool> {
    Some(match (a, b) {
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Unit, Value::Unit) => true,
        (Value::Option(None), Value::Option(None)) => true,
        (Value::Option(Some(_)), Value::Option(None))
        | (Value::Option(None), Value::Option(Some(_))) => false,
        (Value::Option(Some(a)), Value::Option(Some(b))) => equal(a, b)?,
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .try_fold(true, |acc, (x, y)| equal(x, y).map(|e| acc && e))?
        }
        (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .try_fold(true, |acc, (x, y)| equal(x, y).map(|e| acc && e))?
        }
        _ => return None,
    })
}
