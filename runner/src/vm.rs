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
use exact_plan::{Items, Opcode, Operand, Plan, Stdlib, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// The longest string, in UTF-8 bytes, an expression may build or an
/// action may take or keep: doubling a string every few instructions
/// otherwise outruns memory within one evaluation.
pub const MAX_STRING: usize = 1 << 26;

/// The most values a list, record or option the VM builds may hold, a shared
/// one counted once per place it appears: that expanded tree is what
/// equality, shape checks and conversions walk. A list doubled through a
/// local (`[x, x]`, then again) is otherwise exponential in instructions.
/// A `some` is not counted (LLP 1090 D2): the checker refuses an option
/// directly inside an option, so a walk meets at most one per counted value.
pub const MAX_VALUE_NODES: u64 = 1 << 24;

/// The most string bytes such a value may hold, counted the same way: a
/// shared string repeated in a doubled list multiplies what encoding writes.
pub const MAX_VALUE_BYTES: u64 = MAX_STRING as u64;

/// The deepest such a value may nest: [`Value::decode`]'s bound, so that
/// whatever the VM builds can cross the data seam, and a value nested once
/// per action cannot grow past what walking or dropping it recurses through.
pub const MAX_VALUE_DEPTH: u32 = 64;

/// The most list steps one evaluation takes (LLP 1017.003 D3): each run of a
/// `map` or `filter` callback body is a step, nested runs each counted, and
/// each item `join` prints is a step. A body is straight-line code with
/// forward jumps, so this bounds how many instructions an evaluation runs.
pub const MAX_LIST_STEPS: u32 = 1 << 16;

/// Only extents at least this large are remembered: smaller ones cost less
/// to walk again than to look up.
const REMEMBERED: u64 = 64;

/// An instance's own slots — the values of the `state` a child component
/// declared, one set per keyed row or shown `when`/`match` arm (LLP 1017
/// P4c) — shared by the instance and every frame that reaches it, so a
/// read during an update and a write applied after an action's commit see
/// one storage.
pub type RowSlots = Rc<RefCell<BTreeMap<u32, Value>>>;

/// One instance scope: what an `each` row or a `match` arm binds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    /// The `each` item, when this scope is a keyed row.
    pub item: Option<Value>,
    /// The row's position in its list, from 0 (LLP 1062 D8).
    pub index: Option<usize>,
    /// The `match` binding, when this scope is a `some(x)` arm.
    pub bound: Option<Value>,
    /// The region this instance belongs to, when this scope is a row or an
    /// arm that owns slots.
    pub region: Option<u32>,
    /// The instance's slots, when this scope is a row or an arm owning some.
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
    /// The plan's string pool, interned once per runner ([`intern`]) as
    /// values: a string literal is inline text or one shared allocation,
    /// never a fresh one, and two evaluations of one literal are the same
    /// object.
    pub strings: &'a [Value],
    /// Checked route table and shapes; present only for a plan with a router.
    /// @ref LLP 1038 D3/D9 — the same table across all calls in this runner.
    pub router: Option<&'a dyn crate::runner::Routing>,
    /// The list engines, when this artifact links them (LLP 1047.000 §9).
    pub lists: Option<&'static crate::instance::ListLinks>,
    /// The `format` capability, when linked (LLP 1054.000.003 D8).
    pub format: crate::runner::FormatLink,
    /// Geometry reads: present only while an action's body runs
    /// (LLP 1051.000 D2).
    pub geometry: Option<&'a crate::geometry::GeometryEnv<'a>>,
    /// State slots by index.
    pub slots: &'a [Value],
    /// Derives by index; `None` while not yet settled this update.
    pub derives: &'a [Option<Value>],
    /// Resource values by index; `None` while not yet settled this update.
    pub resources: &'a [Option<crate::held::Held>],
    /// Action parameters (empty outside an action).
    pub params: &'a [Value],
    /// Enclosing instance scopes, innermost last.
    pub frames: &'a [Frame],
    /// The clock, in milliseconds.
    pub now_ms: f64,
    /// Whether each resource has a request in flight (LLP 1016 D3).
    pub pending_resources: &'a [bool],
    /// Arguments whose latest request failed, by resource (LLP 1054.000.002).
    pub failed_resources: &'a [Option<Vec<Value>>],
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
    /// An expression would make a string longer than [`MAX_STRING`] bytes.
    StringTooLong {
        pc: usize,
    },
    /// A derive or resource read before it settled this update; the runner's
    /// settlement loop retries it later in the same pass.
    Pending {
        pc: usize,
    },
    /// A `List`, `Record` or `Some` would hold more than [`MAX_VALUE_NODES`]
    /// values or [`MAX_VALUE_BYTES`] string bytes, shared ones counted where
    /// they appear.
    ValueTooLarge {
        pc: usize,
    },
    /// A `List`, `Record` or `Some` would nest deeper than [`MAX_VALUE_DEPTH`].
    ValueTooDeep {
        pc: usize,
    },
    /// A `Map`, `Filter` or `join` (at `pc`) would take this evaluation past
    /// [`MAX_LIST_STEPS`] (LLP 1017.003 D3).
    IterationLimit {
        pc: usize,
    },
}

/// A `Map` or `Filter` in progress (LLP 1017.003 D5): its callback body is
/// `start..end`, run once per item with the item and its index bound as
/// locals `locals` and `locals + 1`, on an operand stack of its own (the
/// caller's is kept in `caller`), so a body can neither read nor drop what
/// its caller left, nor drop a local it did not bind.
struct Callback {
    pc: usize,
    filter: bool,
    start: usize,
    end: usize,
    items: Items,
    next: usize,
    out: Vec<Value>,
    extent: Extent,
    locals: usize,
    caller: Vec<Value>,
}

/// Take `n` list steps, or trap at `pc`.
fn step(steps: &mut u32, n: usize, pc: usize) -> Result<(), Trap> {
    *steps = steps.saturating_add(u32::try_from(n).unwrap_or(u32::MAX));
    if *steps > MAX_LIST_STEPS {
        return Err(Trap::IterationLimit { pc });
    }
    Ok(())
}

impl Callback {
    /// Bind item `self.next` and its index, counting the run.
    fn begin(&mut self, locals: &mut Vec<Value>, steps: &mut u32) -> Result<(), Trap> {
        step(steps, 1, self.pc)?;
        locals.push(self.items[self.next].clone());
        locals.push(Value::Number(self.next as f64));
        Ok(())
    }
}

/// A value's expanded extent: its values (a shared one counted once per
/// place), their string bytes, and how deep below it they nest.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Extent {
    nodes: u64,
    bytes: u64,
    depth: u32,
}

impl Extent {
    fn scalar(v: &Value) -> Option<Extent> {
        match v {
            Value::List(_) | Value::Record(_) | Value::Option(Some(_)) => None,
            v if v.is_str() => Some(Extent {
                nodes: 1,
                bytes: v.as_str().unwrap_or_default().len() as u64,
                depth: 0,
            }),
            _ => Some(Extent {
                nodes: 1,
                ..Extent::default()
            }),
        }
    }

    fn check(self, pc: usize) -> Result<Extent, Trap> {
        if self.depth > MAX_VALUE_DEPTH {
            Err(Trap::ValueTooDeep { pc })
        } else if self.nodes > MAX_VALUE_NODES || self.bytes > MAX_VALUE_BYTES {
            Err(Trap::ValueTooLarge { pc })
        } else {
            Ok(self)
        }
    }

    /// Add `part` as one child of a value being built.
    fn with(mut self, part: Extent, pc: usize) -> Result<Extent, Trap> {
        self.nodes += part.nodes;
        self.bytes += part.bytes;
        self.depth = self.depth.max(part.depth + 1);
        self.check(pc)
    }
}

/// The extents of the lists, records and options one evaluation met, by
/// allocation, so building from shared parts costs the parts' count and not
/// their expansion. Each entry pins its value: an address cannot be reused
/// for another while it is remembered.
#[derive(Default)]
struct Extents(exact_kernel::id::IdMap<usize, (Value, Extent)>);

impl Extents {
    fn key(v: &Value) -> Option<usize> {
        match v {
            Value::List(items) | Value::Record(items) => Some(items.addr()),
            Value::Option(Some(inner)) => Some(Rc::as_ptr(inner) as usize),
            _ => None,
        }
    }

    /// The extent of `v`, walked only if this evaluation has not measured
    /// that allocation, and only as far as the bounds.
    fn of(&mut self, v: &Value, pc: usize) -> Result<Extent, Trap> {
        if let Some(e) = Extent::scalar(v) {
            return e.check(pc);
        }
        let key = Self::key(v);
        if let Some((_, e)) = key.and_then(|k| self.0.get(&k)) {
            return Ok(*e);
        }
        let mut total = Extent::default();
        measure(v, 0, &mut total, pc)?;
        self.remember(v, total);
        Ok(total)
    }

    /// The extent of a list or record of `items`.
    fn built(&mut self, items: &[Value], pc: usize) -> Result<Extent, Trap> {
        let mut e = Extent {
            nodes: 1,
            ..Extent::default()
        };
        for item in items {
            e = e.with(self.of(item, pc)?, pc)?;
        }
        Ok(e)
    }

    fn remember(&mut self, v: &Value, e: Extent) {
        if e.nodes >= REMEMBERED {
            if let Some(k) = Self::key(v) {
                self.0.insert(k, (v.clone(), e));
            }
        }
    }
}

/// Count `v`, `depth` below where the walk began, into `total`; stops at the
/// first bound passed, so neither its time nor its recursion outgrows them.
fn measure(v: &Value, depth: u32, total: &mut Extent, pc: usize) -> Result<(), Trap> {
    #[cfg(test)]
    MEASURED.with(|m| m.set(m.get() + 1));
    total.depth = total.depth.max(depth);
    // A `Some` is not a value of its own (LLP 1090 D2), only a level.
    if let Value::Option(Some(inner)) = v {
        total.check(pc)?;
        return measure(inner, depth + 1, total, pc);
    }
    total.nodes += 1;
    match v {
        v if v.is_str() => total.bytes += v.as_str().unwrap_or_default().len() as u64,
        Value::List(items) | Value::Record(items) => {
            total.check(pc)?;
            for item in items.iter() {
                measure(item, depth + 1, total, pc)?;
            }
        }
        _ => {}
    }
    total.check(pc).map(|_| ())
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

/// The plan's string pool as values, for [`Env::strings`].
pub fn intern(plan: &Plan) -> Vec<Value> {
    plan.strings
        .iter()
        .map(|s| Value::str(s.as_str()))
        .collect()
}

/// One decoded instruction: its opcode and operands in declared order,
/// integers widened, an `f64` operand in `number`.
#[derive(Debug, Clone, Copy)]
pub struct Instruction {
    /// Its offset in the body.
    pub pc: usize,
    /// The opcode.
    pub op: Opcode,
    /// Integer operands in declared order.
    pub args: [u64; 3],
    /// The `f64` operand, if the opcode has one.
    pub number: f64,
}

/// Decode the instruction at `r`: the one decoder the VM, the dependency
/// table and the retained-binding grammar share.
pub fn decode(r: &mut Reader<'_>) -> Result<Instruction, Trap> {
    let pc = r.position();
    let malformed = |_| Trap::Malformed { pc };
    let op = Opcode::from_wire(r.u8().map_err(malformed)?).ok_or(Trap::Malformed { pc })?;
    let mut args = [0; 3];
    let mut number = 0.0;
    for (i, operand) in op.operands().iter().enumerate() {
        match operand {
            Operand::U8 | Operand::Enum(_) => args[i] = r.u8().map_err(malformed)? as u64,
            Operand::U16 => args[i] = r.u16().map_err(malformed)? as u64,
            Operand::U32 | Operand::Str | Operand::Idx(_) => {
                args[i] = r.u32().map_err(malformed)? as u64
            }
            Operand::F64 => number = r.f64().map_err(malformed)?,
        }
    }
    Ok(Instruction {
        pc,
        op,
        args,
        number,
    })
}

/// Every instruction of `code` in order, without following jumps; stops
/// after the first malformed one.
pub fn instructions(code: &[u8]) -> impl Iterator<Item = Result<Instruction, Trap>> + '_ {
    let mut r = Reader::new(code);
    let mut failed = false;
    std::iter::from_fn(move || {
        if failed || r.is_empty() {
            return None;
        }
        let next = decode(&mut r);
        failed = next.is_err();
        Some(next)
    })
}

/// Whether `code` is a literal — pushes of values and `Return`, nothing
/// read — so a binding can be told from an expression without running it
/// (LLP 1035.002 D5: a row bound by an expression is `dynamic`). Malformed
/// code is not a literal.
pub fn is_literal(code: &[u8]) -> bool {
    instructions(code).all(|i| {
        i.is_ok_and(|i| {
            matches!(
                i.op,
                Opcode::Number
                    | Opcode::Bool
                    | Opcode::Str
                    | Opcode::None
                    | Opcode::Unit
                    | Opcode::Some
                    | Opcode::Return
            )
        })
    })
}

/// Evaluate `code` in `env`. `allowed_writes` bounds `StoreSlot`; an action
/// passes its `writes` range, an expression passes nothing.
pub fn eval(code: &[u8], env: &Env<'_>, allowed_writes: &[u32]) -> Result<Outcome, Trap> {
    let mut stack: Vec<Value> = Vec::with_capacity(16);
    let mut locals: Vec<Value> = Vec::new();
    let mut out = Outcome::default();
    let mut extents = Extents::default();
    let mut callbacks: Vec<Callback> = Vec::new();
    let mut steps = 0u32;
    let mut r = Reader::new(code);
    let malformed = |pc: usize| Trap::Malformed { pc };
    // A jump inside a callback body stays inside it (the plan checker's
    // rule, checked again here: a plan is never trusted).
    let jump_to = |target: u32, callbacks: &[Callback], pc: usize| {
        if callbacks.last().is_some_and(|c| target as usize > c.end) {
            return Err(Trap::BadJump { pc, target });
        }
        jump(code, target).ok_or(Trap::BadJump { pc, target })
    };
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
    loop {
        // A callback body's end: collect what the run left, then run the
        // next item or finish the list (which may end an outer body too).
        while let Some(c) = callbacks.last_mut().filter(|c| r.position() == c.end) {
            let pc = c.end;
            // One value left, every local it bound dropped.
            if stack.len() != 1 || locals.len() != c.locals + 2 {
                return Err(Trap::Malformed { pc });
            }
            let v = pop!(pc);
            let kept = match (c.filter, v) {
                (true, Value::Bool(keep)) => keep.then(|| c.items[c.next].clone()),
                (true, _) => {
                    return Err(Trap::TypeMismatch {
                        pc: c.pc,
                        op: Opcode::Filter,
                    })
                }
                (false, v) => Some(v),
            };
            if let Some(v) = kept {
                c.extent = c.extent.with(extents.of(&v, c.pc)?, c.pc)?;
                c.out.push(v);
            }
            locals.truncate(c.locals);
            c.next += 1;
            if c.next < c.items.len() {
                c.begin(&mut locals, &mut steps)?;
                r = jump(code, c.start as u32).ok_or(Trap::Malformed { pc })?;
                break;
            }
            let c = callbacks.pop().expect("the last callback");
            stack = c.caller;
            // A filter that kept every item is that list, shared.
            let v = if c.filter && c.out.len() == c.items.len() {
                Value::List(c.items)
            } else {
                let v = Value::list(c.out);
                extents.remember(&v, c.extent);
                v
            };
            stack.push(v);
        }
        if r.is_empty() {
            break;
        }
        let Instruction {
            pc,
            op,
            args,
            number: f64_arg,
        } = decode(&mut r)?;
        match op {
            Opcode::Number => stack.push(Value::Number(f64_arg)),
            Opcode::Bool => stack.push(Value::Bool(args[0] != 0)),
            Opcode::Str => stack.push(
                env.strings
                    .get(args[0] as usize)
                    .ok_or(malformed(pc))?
                    .clone(),
            ),
            Opcode::None => stack.push(Value::NONE),
            Opcode::Unit => stack.push(Value::Unit),
            // @ref LLP 1090 D2 — a `Some` adds depth and nothing else: the
            // JS target erases it, and `type-option-option` keeps at most one
            // above each counted value, so walks stay within twice the count.
            Opcode::Some => {
                let v = pop!(pc);
                let inner = extents.of(&v, pc)?;
                let e = Extent {
                    depth: inner.depth + 1,
                    ..inner
                }
                .check(pc)?;
                let v = Value::some(v);
                extents.remember(&v, e);
                stack.push(v);
            }
            Opcode::LoadSlot => {
                let slot = args[0] as usize;
                let row = env.plan.slots.get(slot).ok_or(malformed(pc))?;
                let v = match env.plan.owner_region(row) {
                    // An owned slot: the value the innermost instance of its
                    // region (a row, an arm) holds.
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
                    .as_ref()
                    .ok_or(Trap::Pending { pc })?
                    .read(env.plan);
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
            Opcode::LoadIndex => {
                let depth = args[0] as usize;
                let index = env
                    .frames
                    .len()
                    .checked_sub(depth + 1)
                    .and_then(|i| env.frames.get(i))
                    .and_then(|f| f.index)
                    .ok_or(Trap::BadScope {
                        pc,
                        depth: depth as u16,
                    })?;
                stack.push(Value::Number(index as f64));
            }
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
                let e = extents.built(&fields, pc)?;
                let v = Value::record(fields);
                extents.remember(&v, e);
                stack.push(v);
            }
            Opcode::List => {
                let n = args[0] as usize;
                if stack.len() < n {
                    return Err(Trap::StackUnderflow { pc });
                }
                let items = stack.split_off(stack.len() - n);
                let e = extents.built(&items, pc)?;
                let v = Value::list(items);
                extents.remember(&v, e);
                stack.push(v);
            }
            // @ref LLP 1024 D1 — the leftover attributes, one replaced object.
            Opcode::NativeProps => {
                let n = args[0] as usize * 2;
                if stack.len() < n {
                    return Err(Trap::StackUnderflow { pc });
                }
                let pairs = stack.split_off(stack.len() - n);
                let json = stdlib::native_props(&pairs).ok_or(Trap::TypeMismatch { pc, op })?;
                if json.len() > MAX_STRING {
                    return Err(Trap::StringTooLong { pc });
                }
                stack.push(Value::str(&json));
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
                let eq = crate::compare::equal(&a, &b).ok_or(Trap::TypeMismatch { pc, op })?;
                stack.push(Value::Bool(if op == Opcode::Eq { eq } else { !eq }));
            }
            Opcode::Not => match pop!(pc) {
                Value::Bool(b) => stack.push(Value::Bool(!b)),
                _ => return Err(Trap::TypeMismatch { pc, op }),
            },
            Opcode::Concat => {
                let b = pop!(pc);
                let a = pop!(pc);
                match (a.as_str(), b.as_str()) {
                    (Some(a), Some(b)) => {
                        if a.len() + b.len() > MAX_STRING {
                            return Err(Trap::StringTooLong { pc });
                        }
                        stack.push(stdlib::assembled(|s| {
                            s.push_str(a);
                            s.push_str(b);
                        }));
                    }
                    _ => return Err(Trap::TypeMismatch { pc, op }),
                }
            }
            Opcode::Jump => r = jump_to(args[0] as u32, &callbacks, pc)?,
            Opcode::JumpIfFalse => {
                let target = args[0] as u32;
                match pop!(pc) {
                    Value::Bool(true) => {}
                    Value::Bool(false) => r = jump_to(target, &callbacks, pc)?,
                    _ => return Err(Trap::TypeMismatch { pc, op }),
                }
            }
            Opcode::JumpIfNone => {
                let target = args[0] as u32;
                match stack.last() {
                    Some(Value::Option(Some(_))) => {}
                    Some(Value::Option(None)) => r = jump_to(target, &callbacks, pc)?,
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
                // The arguments are read where they are, not moved to a
                // vector of their own: a call is the VM's commonest cost.
                let at = stack.len() - n;
                let call_args = &stack[at..];
                let v = if f == Stdlib::Join {
                    if let Some(Value::List(items)) = call_args.first() {
                        step(&mut steps, items.len(), pc)?;
                    }
                    match stdlib::join(call_args, MAX_STRING) {
                        Ok(v) => v,
                        Err(stdlib::JoinError::Type) => return Err(Trap::TypeMismatch { pc, op }),
                        Err(stdlib::JoinError::TooLong) => return Err(Trap::StringTooLong { pc }),
                    }
                } else {
                    stdlib::call(
                        f,
                        call_args,
                        env.now_ms,
                        env.plan,
                        env.router,
                        env.format,
                        env.geometry,
                    )
                    .map_err(|error| match error {
                        stdlib::CallError::TypeMismatch => Trap::TypeMismatch { pc, op },
                        stdlib::CallError::StringTooLong => Trap::StringTooLong { pc },
                    })?
                };
                stack.truncate(at);
                stack.push(v);
            }
            // @ref LLP 1017.003 D5 — a callback body follows inline, to `end`.
            Opcode::Map | Opcode::Filter => {
                let end = args[0] as usize;
                let start = r.position();
                if end < start || end > code.len() {
                    return Err(Trap::BadJump {
                        pc,
                        target: end as u32,
                    });
                }
                let Value::List(items) = pop!(pc) else {
                    return Err(Trap::TypeMismatch { pc, op });
                };
                if items.is_empty() {
                    r = jump_to(end as u32, &callbacks, pc)?;
                    stack.push(Value::List(items));
                    continue;
                }
                let mut c = Callback {
                    pc,
                    filter: op == Opcode::Filter,
                    start,
                    end,
                    out: Vec::with_capacity(if op == Opcode::Map { items.len() } else { 0 }),
                    extent: Extent {
                        nodes: 1,
                        ..Extent::default()
                    },
                    items,
                    next: 0,
                    locals: locals.len(),
                    caller: std::mem::take(&mut stack),
                };
                c.begin(&mut locals, &mut steps)?;
                callbacks.push(c);
            }
            Opcode::StoreSlot => {
                let slot = args[0] as u32;
                if !allowed_writes.contains(&slot) {
                    return Err(Trap::WriteNotDeclared { pc, slot });
                }
                let v = pop!(pc);
                let row = env.plan.slots.get(slot as usize).ok_or(malformed(pc))?;
                match env.plan.owner_region(row) {
                    Some(region) => {
                        // An owned slot: written to the instance in force —
                        // an action run with none (`act`, a timer) has none
                        // to write.
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
            Opcode::PendingResource | Opcode::FailedResource => {
                // Known once the resource settled this pass, like its value.
                let i = args[0] as usize;
                if env.resources.get(i).is_none_or(Option::is_none) {
                    return Err(Trap::Pending { pc });
                }
                let flag = if op == Opcode::FailedResource {
                    env.failed_resources.get(i).is_some_and(Option::is_some)
                } else {
                    env.pending_resources.get(i).copied().unwrap_or(false)
                };
                stack.push(Value::Bool(flag));
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
                if callbacks
                    .last()
                    .is_some_and(|c| locals.len() <= c.locals + 2)
                {
                    return Err(Trap::Malformed { pc });
                }
                locals.pop().ok_or(Trap::StackUnderflow { pc })?;
            }
            Opcode::Return => {
                if !callbacks.is_empty() {
                    return Err(Trap::Malformed { pc });
                }
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

#[cfg(test)]
thread_local! {
    /// Values `measure` walked on this thread: sizing, counted.
    static MEASURED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_plan::asm::Asm;
    use exact_plan::builder::PlanBuilder;

    fn run(body: Asm) -> Result<Outcome, Trap> {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let code = b.code(body);
        let plan = b.finish().unwrap();
        let strings = intern(&plan);
        let env = Env {
            plan: &plan,
            strings: &strings,
            router: None,
            lists: None,
            format: None,
            geometry: None,
            slots: &[],
            derives: &[],
            resources: &[],
            params: &[],
            frames: &[],
            now_ms: 0.0,
            pending_resources: &[],
            failed_resources: &[],
            pending_mutations: &[],
            store_dependent_derives: &[],
            store_dependent_resources: &[],
        };
        eval(plan.code(code), &env, &[])
    }

    /// `x` on the stack becomes `[x, x]`, `times` times, through a local.
    fn doubled(body: &mut Asm, times: usize) -> &mut Asm {
        for _ in 0..times {
            body.bind_local()
                .load_local(0)
                .load_local(0)
                .list(2)
                .drop_local();
        }
        body
    }

    fn measured(f: impl FnOnce()) -> u64 {
        MEASURED.with(|m| m.set(0));
        f();
        MEASURED.with(|m| m.get())
    }

    /// Doubling through a local makes one instruction's value exponential in
    /// instructions: 2^k leaves after k doublings, which equality, shape
    /// checks and encoding then walk leaf by leaf. The VM refuses the
    /// doubling that passes `MAX_VALUE_NODES`, and sizing a shared part
    /// costs one lookup, not its expansion.
    #[test]
    fn a_doubled_list_is_refused_where_it_is_built() {
        // k doublings of a number hold 2^(k+1) - 1 values: 23 fit in 2^24.
        let mut body = Asm::new();
        body.number(1.0);
        doubled(&mut body, 23);
        let mut value = Value::Unit;
        let walked = measured(|| value = run(body).unwrap().value);
        assert!(walked < 1_000, "sizing 23 doublings walked {walked} values");
        let (mut leaves, mut v) = (1u64, &value);
        while let Value::List(items) = v {
            assert_eq!(items.len(), 2);
            leaves *= 2;
            v = &items[0];
        }
        assert_eq!(leaves, 1 << 23);

        let mut body = Asm::new();
        body.number(1.0);
        doubled(&mut body, 64);
        match run(body) {
            // The 24th doubling's `List`: `Number` is 9 bytes, a doubling
            // 13 (bind 1, load 3, load 3, list 5, drop 1).
            Err(Trap::ValueTooLarge { pc }) => assert_eq!(pc, 9 + 23 * 13 + 7),
            other => panic!("{other:?}"),
        }
    }

    /// A shared string counts its bytes wherever it appears: 64 copies of a
    /// 1 MiB string fit `MAX_VALUE_BYTES`, 128 do not.
    #[test]
    fn a_shared_string_counts_its_bytes_in_every_place() {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let x = b.str("x");
        let mut fits = Asm::new();
        fits.str(x);
        for _ in 0..20 {
            fits.bind_local()
                .load_local(0)
                .load_local(0)
                .op(exact_plan::Opcode::Concat, &[])
                .drop_local();
        }
        let mut refused = Asm::new();
        refused.str(x);
        for _ in 0..20 {
            refused
                .bind_local()
                .load_local(0)
                .load_local(0)
                .op(exact_plan::Opcode::Concat, &[])
                .drop_local();
        }
        doubled(&mut fits, 6);
        doubled(&mut refused, 7);
        let fits = b.code(fits);
        let refused = b.code(refused);
        let plan = b.finish().unwrap();
        let strings = intern(&plan);
        let env = Env {
            plan: &plan,
            strings: &strings,
            router: None,
            lists: None,
            format: None,
            geometry: None,
            slots: &[],
            derives: &[],
            resources: &[],
            params: &[],
            frames: &[],
            now_ms: 0.0,
            pending_resources: &[],
            failed_resources: &[],
            pending_mutations: &[],
            store_dependent_derives: &[],
            store_dependent_resources: &[],
        };
        assert!(eval(plan.code(fits), &env, &[]).is_ok());
        assert!(matches!(
            eval(plan.code(refused), &env, &[]),
            Err(Trap::ValueTooLarge { .. })
        ));
    }

    /// A `some` adds a level and no value (LLP 1090 D2): 23 doublings each
    /// wrapped in one fit as the bare doublings do (they would hold
    /// 3 · 2^23 values counted the old way), and the 24th still traps.
    #[test]
    fn a_some_measures_as_its_value() {
        let wrapped = |times: usize| {
            let mut body = Asm::new();
            body.number(1.0);
            for _ in 0..times {
                doubled(&mut body, 1).simple(exact_plan::Opcode::Some);
            }
            run(body)
        };
        assert!(wrapped(23).is_ok());
        assert!(matches!(wrapped(24), Err(Trap::ValueTooLarge { .. })));
    }

    /// The VM nests no deeper than `Value::decode` reads: 64 `some`s around a
    /// number round-trip through the canonical bytes, a 65th is refused.
    #[test]
    fn nesting_stops_where_decoding_does() {
        let nested = |n: usize| {
            let mut body = Asm::new();
            body.number(1.0);
            for _ in 0..n {
                body.simple(exact_plan::Opcode::Some);
            }
            run(body)
        };
        let deepest = nested(64).unwrap().value;
        assert_eq!(Value::from_bytes(&deepest.to_bytes()).unwrap(), deepest);
        assert!(matches!(nested(65), Err(Trap::ValueTooDeep { .. })));
        let mut too_deep = deepest.clone();
        too_deep = Value::some(too_deep);
        assert!(Value::from_bytes(&too_deep.to_bytes()).is_err());
    }

    /// Push a list of `n` ones.
    fn ones(body: &mut Asm, n: u32) -> &mut Asm {
        for _ in 0..n {
            body.number(1.0);
        }
        body.list(n)
    }

    /// `map(outer, x => map(inner, y => 1))`, then, when `extra`, one more
    /// `map` over a one-item list; its value is the nested lists' length.
    fn nested(outer: u32, inner: u32, extra: bool) -> Result<Outcome, Trap> {
        let mut body = Asm::new();
        ones(&mut body, inner).bind_local();
        ones(&mut body, outer);
        let (end, end_inner) = (body.label(), body.label());
        body.each_item(exact_plan::Opcode::Map, end)
            .load_local(0)
            .each_item(exact_plan::Opcode::Map, end_inner)
            .number(1.0)
            .place(end_inner)
            .place(end);
        if extra {
            let last = body.label();
            body.simple(exact_plan::Opcode::Pop);
            ones(&mut body, 1)
                .each_item(exact_plan::Opcode::Map, last)
                .number(1.0)
                .place(last);
        }
        body.drop_local();
        run(body)
    }

    /// LLP 1017.003 D3: 256 outer runs of 255 inner are 65,536 runs, which
    /// fit; one more run anywhere in the evaluation is `IterationLimit`.
    #[test]
    fn callback_runs_are_counted_across_nesting_and_capped() {
        let Value::List(outer) = nested(256, 255, false).unwrap().value else {
            panic!("a list")
        };
        assert_eq!(outer.len(), 256);
        assert!(matches!(&outer[0], Value::List(inner) if inner.len() == 255));
        assert!(matches!(
            nested(256, 255, true),
            Err(Trap::IterationLimit { .. })
        ));
        assert!(nested(255, 255, true).is_ok());
    }

    #[test]
    fn map_and_filter_bind_the_item_and_index_and_filter_keeps_by_bool() {
        // map([1, 1, 1], (x, i) => x + i) == [1, 2, 3]
        let mut body = Asm::new();
        let end = body.label();
        ones(&mut body, 3)
            .each_item(exact_plan::Opcode::Map, end)
            .load_local(0)
            .load_local(1)
            .simple(exact_plan::Opcode::Add)
            .place(end);
        let expected: Vec<Value> = [1.0, 2.0, 3.0].map(Value::Number).into();
        assert_eq!(run(body).unwrap().value, Value::list(expected));
        // filter(xs, (x, i) => i > 0) drops the first; `true` keeps the list itself.
        let mut body = Asm::new();
        let end = body.label();
        ones(&mut body, 3)
            .each_item(exact_plan::Opcode::Filter, end)
            .load_local(1)
            .number(0.0)
            .simple(exact_plan::Opcode::Gt)
            .place(end);
        assert!(matches!(run(body).unwrap().value, Value::List(l) if l.len() == 2));
        let mut body = Asm::new();
        let end = body.label();
        ones(&mut body, 2).bind_local().load_local(0);
        body.each_item(exact_plan::Opcode::Filter, end)
            .bool(true)
            .place(end)
            .load_local(0)
            .op(exact_plan::Opcode::Eq, &[])
            .drop_local();
        assert_eq!(run(body).unwrap().value, Value::Bool(true));
        // A filter body that is not a bool; an empty list runs no body.
        let mut body = Asm::new();
        let end = body.label();
        ones(&mut body, 1)
            .each_item(exact_plan::Opcode::Filter, end)
            .number(1.0)
            .place(end);
        assert!(matches!(
            run(body),
            Err(Trap::TypeMismatch {
                op: exact_plan::Opcode::Filter,
                ..
            })
        ));
        let mut body = Asm::new();
        let end = body.label();
        body.list(0)
            .each_item(exact_plan::Opcode::Map, end)
            .simple(exact_plan::Opcode::Return)
            .place(end);
        assert_eq!(run(body).unwrap().value, Value::list(vec![]));
        // A `Return` inside a body is malformed.
        let mut body = Asm::new();
        let end = body.label();
        ones(&mut body, 1)
            .each_item(exact_plan::Opcode::Map, end)
            .number(1.0)
            .simple(exact_plan::Opcode::Return)
            .place(end);
        assert!(matches!(run(body), Err(Trap::Malformed { .. })));
    }

    /// A body runs on a stack of its own and drops only the locals it bound.
    #[test]
    fn a_callback_body_cannot_touch_what_its_caller_holds() {
        let mut body = Asm::new();
        let end = body.label();
        body.number(7.0);
        ones(&mut body, 1)
            .each_item(exact_plan::Opcode::Map, end)
            .simple(exact_plan::Opcode::Pop)
            .simple(exact_plan::Opcode::Pop)
            .number(1.0)
            .place(end);
        assert!(matches!(run(body), Err(Trap::StackUnderflow { .. })));
        let mut body = Asm::new();
        let end = body.label();
        ones(&mut body, 1)
            .each_item(exact_plan::Opcode::Map, end)
            .drop_local()
            .number(1.0)
            .place(end);
        assert!(matches!(run(body), Err(Trap::Malformed { .. })));
        // Its caller's operands are there again after it.
        let mut body = Asm::new();
        let end = body.label();
        body.number(7.0);
        ones(&mut body, 2)
            .each_item(exact_plan::Opcode::Map, end)
            .number(1.0)
            .place(end)
            .simple(exact_plan::Opcode::Pop);
        assert_eq!(run(body).unwrap().value, Value::Number(7.0));
    }

    /// Each item `join` prints is a step against the same bound.
    #[test]
    fn join_takes_a_step_per_item() {
        let joined = |n: u32| {
            let mut body = Asm::new();
            let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
            let sep = b.str("");
            ones(&mut body, n).str(sep).call(Stdlib::Join);
            let code = b.code(body);
            let plan = b.finish().unwrap();
            let strings = intern(&plan);
            let env = Env {
                plan: &plan,
                strings: &strings,
                router: None,
                lists: None,
                format: None,
                geometry: None,
                slots: &[],
                derives: &[],
                resources: &[],
                params: &[],
                frames: &[],
                now_ms: 0.0,
                pending_resources: &[],
                failed_resources: &[],
                pending_mutations: &[],
                store_dependent_derives: &[],
                store_dependent_resources: &[],
            };
            eval(plan.code(code), &env, &[]).map(|o| o.value)
        };
        assert!(
            matches!(joined(MAX_LIST_STEPS), Ok(s) if s.as_str().map(str::len) == Some(MAX_LIST_STEPS as usize))
        );
        assert!(matches!(
            joined(MAX_LIST_STEPS + 1),
            Err(Trap::IterationLimit { .. })
        ));
    }

    #[test]
    fn join_prints_items_as_to_string_does() {
        let join = |items: Vec<Value>, sep: &str| {
            stdlib::join(&[Value::list(items), Value::str(sep)], MAX_STRING)
        };
        let n = Value::Number;
        assert_eq!(
            join(vec![n(1.5), n(-0.0), n(1e21), n(0.1 + 0.2)], ","),
            Ok(Value::str("1.5,0,1e+21,0.30000000000000004"))
        );
        assert_eq!(
            join(vec![Value::str("a"), Value::Bool(false)], " · "),
            Ok(Value::str("a · false"))
        );
        assert_eq!(join(vec![], ","), Ok(Value::str("")));
        assert_eq!(
            join(vec![Value::record(vec![])], ","),
            Err(stdlib::JoinError::Type)
        );
        assert_eq!(
            stdlib::join(
                &[
                    Value::list(vec![Value::str("ab"), Value::str("cd")]),
                    Value::str("-")
                ],
                4
            ),
            Err(stdlib::JoinError::TooLong)
        );
    }
}
