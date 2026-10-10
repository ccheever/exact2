//! The expression VM.
//!
//! @ref LLP 1004 D2 (`opcodes.json` is the one authority) / LLP 0508 §4.3
//! (closed expressions; research)
//!
//! A stack machine over [`Value`]s. Type inference in the compiler guarantees
//! stack discipline for compiled plans; the VM still checks everything and
//! traps with a typed reason, because a plan is data from outside and never
//! trusted. There is no closure, no heap of the VM's own, no ambient read that
//! is not an operand: `performanceNow()` reads the clock the runner passes in.

use crate::machine::{self, Host, Kind, Machine, Num, Val};
pub use crate::machine::{Instruction, Trap};
use crate::stdlib;
use exact_plan::bytes::Reader;
use exact_plan::{Opcode, Operand, Plan, Stdlib, Value};
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
    /// Why, while `failed_resources` holds (LLP 1109 D3); may be shorter.
    pub failed_why: &'a [Option<crate::failure::Failure>],
    /// Whether each mutation has a request in flight.
    pub pending_mutations: &'a [bool],
    /// Store dependence of settled derives, for bake provenance propagation.
    pub store_dependent_derives: &'a [bool],
    /// Store dependence of settled resources, for bake provenance propagation.
    pub store_dependent_resources: &'a [bool],
}

/// Take `n` list steps, or trap at `pc`.
fn step(steps: &mut u32, n: usize, pc: usize) -> Result<(), Trap> {
    *steps = steps.saturating_add(u32::try_from(n).unwrap_or(u32::MAX));
    if *steps > MAX_LIST_STEPS {
        return Err(Trap::IterationLimit { pc });
    }
    Ok(())
}

/// `concat`, `split`, and `slice`, `includes` and `indexOf` over a list
/// (LLP 1088 §9.1), on this evaluation's budget as `join` is: the list steps
/// first — one per item `concat`, `slice` and `split` keep, one per item
/// `includes` and `indexOf` scan up to the match — then a list they build
/// measured as `Opcode::List` measures one (LLP 1090 D3). `None` for
/// `slice`, `includes` and `indexOf` over text.
fn list_call(
    f: Stdlib,
    args: &[Value],
    steps: &mut u32,
    extents: &mut Extents,
    pc: usize,
) -> Result<Option<Value>, Trap> {
    let mismatch = Trap::TypeMismatch {
        pc,
        op: Opcode::Call,
    };
    if f == Stdlib::Split {
        let (Some(s), Some(sep)) = (args[0].as_str(), args[1].as_str()) else {
            return Err(mismatch);
        };
        step(steps, crate::strings::pieces(s, sep), pc)?;
        let built: Vec<Value> = (crate::strings::split(s, sep).iter())
            .map(|p| Value::str(p))
            .collect();
        let e = extents.built(&built, pc)?;
        let v = Value::list(built);
        extents.remember(&v, e);
        return Ok(Some(v));
    }
    let items = match (f, args.first()) {
        (
            Stdlib::Concat | Stdlib::Slice | Stdlib::Includes | Stdlib::IndexOf,
            Some(Value::List(items)),
        ) => items,
        (Stdlib::Concat, _) => return Err(mismatch),
        _ => return Ok(None),
    };
    let built = match (f, args.get(1), args.get(2)) {
        (Stdlib::Concat, Some(Value::List(more)), _) => {
            step(steps, items.len() + more.len(), pc)?;
            crate::lists::concat(items, more)
        }
        (Stdlib::Slice, Some(Value::Number(a)), Some(Value::Number(b))) => {
            let kept = crate::lists::slice(items, *a, *b);
            step(steps, kept.len(), pc)?;
            kept.to_vec()
        }
        (Stdlib::Includes, Some(x), _) => {
            let at = crate::lists::position(items, x);
            step(steps, at.map_or(items.len(), |i| i + 1), pc)?;
            return Ok(Some(Value::Bool(at.is_some())));
        }
        (Stdlib::IndexOf, Some(x), _) => {
            let at = crate::lists::index_of(items, x);
            step(steps, at.map_or(items.len(), |i| i + 1), pc)?;
            return Ok(Some(Value::Number(at.map_or(-1.0, |i| i as f64))));
        }
        _ => return Err(mismatch),
    };
    let e = extents.built(&built, pc)?;
    let v = Value::list(built);
    extents.remember(&v, e);
    Ok(Some(v))
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

/// Decode the instruction at `r`: the one decoder the VM, the dependency
/// table and the retained-binding grammar share.
#[inline]
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
///
/// The loop is [`machine::run`], over [`Value`] and this environment.
pub fn eval(code: &[u8], env: &Env<'_>, allowed_writes: &[u32]) -> Result<Outcome, Trap> {
    let mut run = Run {
        env,
        code,
        allowed_writes,
        cursor: Reader::new(code),
        out: Outcome::default(),
        extents: Extents::default(),
        steps: 0,
    };
    let mut m = Machine::new();
    m.stack = Vec::with_capacity(16);
    run.out.value = machine::run(&mut run, &mut m)?;
    Ok(run.out)
}

/// The runner's value, as the machine sees it.
impl Val for Value {
    #[inline]
    fn kind(&self) -> Kind<'_, Value> {
        match self {
            Value::Number(n) => Kind::Number(*n),
            Value::Bool(b) => Kind::Bool(*b),
            exact_plan::str_value!() => Kind::Str,
            Value::Unit => Kind::Unit,
            Value::Option(None) => Kind::None,
            Value::Option(Some(v)) => Kind::Some(v),
            Value::List(items) => Kind::List(items),
            Value::Record(items) => Kind::Record(items),
        }
    }

    #[inline]
    fn number(n: f64) -> Value {
        Value::Number(n)
    }

    #[inline]
    fn index(n: usize) -> Value {
        Value::Number(n as f64)
    }

    #[inline]
    fn boolean(b: bool) -> Value {
        Value::Bool(b)
    }

    #[inline]
    fn unit() -> Value {
        Value::Unit
    }

    #[inline]
    fn none() -> Value {
        Value::NONE
    }

    #[inline]
    fn num(op: Num, a: f64, b: f64) -> Value {
        match op {
            Num::Add => Value::Number(a + b),
            Num::Sub => Value::Number(a - b),
            Num::Mul => Value::Number(a * b),
            Num::Div => Value::Number(a / b),
            Num::Rem => Value::Number(a % b),
            Num::Lt => Value::Bool(a < b),
            Num::Le => Value::Bool(a <= b),
            Num::Gt => Value::Bool(a > b),
            Num::Ge => Value::Bool(a >= b),
        }
    }

    #[inline]
    fn negate(a: f64) -> Value {
        Value::Number(-a)
    }

    #[inline]
    fn num_eq(a: f64, b: f64) -> bool {
        a == b
    }

    #[inline]
    fn str_eq(a: &Value, b: &Value) -> bool {
        Value::same_str(a, b) || a.as_str() == b.as_str()
    }

    fn str_cmp(op: Num, a: &Value, b: &Value) -> bool {
        let o = crate::strings::order(
            a.as_str().unwrap_or_default(),
            b.as_str().unwrap_or_default(),
        );
        match op {
            Num::Lt => o.is_lt(),
            Num::Le => o.is_le(),
            Num::Gt => o.is_gt(),
            _ => o.is_ge(),
        }
    }

    #[inline]
    fn concat(a: &Value, b: &Value) -> Option<Value> {
        let (a, b) = (a.as_str()?, b.as_str()?);
        if a.len() + b.len() > MAX_STRING {
            return None;
        }
        Some(stdlib::assembled(|s| {
            s.push_str(a);
            s.push_str(b);
        }))
    }
}

/// One evaluation's environment, effects and bounds: the machine's host.
struct Run<'e, 'a> {
    env: &'e Env<'a>,
    code: &'e [u8],
    allowed_writes: &'e [u32],
    /// Where the last instruction was read up to.
    cursor: Reader<'e>,
    out: Outcome,
    extents: Extents,
    steps: u32,
}

impl Host<Value> for Run<'_, '_> {
    type Extent = Extent;

    #[inline]
    fn fetch(&mut self, pos: usize) -> Result<Option<(Instruction, usize)>, Trap> {
        if pos >= self.code.len() {
            return Ok(None);
        }
        // Straight-line code reads on from the last instruction; a jump or a
        // callback's next item moves the cursor.
        if self.cursor.position() != pos {
            self.cursor = Reader::new(self.code);
            self.cursor
                .bytes(pos)
                .map_err(|_| Trap::Malformed { pc: pos })?;
        }
        let ins = decode(&mut self.cursor)?;
        Ok(Some((ins, self.cursor.position())))
    }

    #[inline]
    fn code_len(&self) -> usize {
        self.code.len()
    }

    fn string(&self, i: u64, pc: usize) -> Result<Value, Trap> {
        self.env
            .strings
            .get(i as usize)
            .cloned()
            .ok_or(Trap::Malformed { pc })
    }

    fn load_slot(&self, i: u64, pc: usize) -> Result<Value, Trap> {
        let slot = i as usize;
        let env = self.env;
        let row = env.plan.slots.get(slot).ok_or(Trap::Malformed { pc })?;
        match env.plan.owner_region(row) {
            // An owned slot: the value the innermost instance of its region
            // (a row, an arm) holds.
            Some(region) => Frame::row_of(env.frames, region.0)
                .and_then(|r| r.borrow().get(&(slot as u32)).cloned())
                .ok_or(Trap::BadScope { pc, depth: 0 }),
            None => env.slots.get(slot).cloned().ok_or(Trap::Malformed { pc }),
        }
    }

    fn load_derive(&mut self, i: u64, pc: usize) -> Result<Value, Trap> {
        let derive = i as usize;
        let env = self.env;
        let value = env
            .derives
            .get(derive)
            .ok_or(Trap::Malformed { pc })?
            .clone()
            .ok_or(Trap::Pending { pc })?;
        self.out.store_dependent |= env
            .store_dependent_derives
            .get(derive)
            .copied()
            .unwrap_or(false);
        Ok(value)
    }

    fn load_resource(&mut self, i: u64, pc: usize) -> Result<Value, Trap> {
        let resource = i as usize;
        let env = self.env;
        let value = env
            .resources
            .get(resource)
            .ok_or(Trap::Malformed { pc })?
            .as_ref()
            .ok_or(Trap::Pending { pc })?
            .read(env.plan);
        self.out.store_dependent |= env
            .store_dependent_resources
            .get(resource)
            .copied()
            .unwrap_or(false);
        Ok(value)
    }

    fn load_param(&self, i: u64, pc: usize) -> Result<Value, Trap> {
        self.env
            .params
            .get(i as usize)
            .cloned()
            .ok_or(Trap::BadParam {
                pc,
                index: i as u16,
            })
    }

    fn load_frame(&self, op: Opcode, depth: u64, pc: usize) -> Result<Value, Trap> {
        let depth = depth as usize;
        let frames = self.env.frames;
        let bad = Trap::BadScope {
            pc,
            depth: depth as u16,
        };
        let frame = frames
            .len()
            .checked_sub(depth + 1)
            .and_then(|i| frames.get(i))
            .ok_or(bad.clone())?;
        match op {
            Opcode::LoadIndex => frame.index.map(|index| Value::Number(index as f64)),
            Opcode::LoadItem => frame.item.clone(),
            _ => frame.bound.clone(),
        }
        .ok_or(bad)
    }

    fn resource_flag(&self, op: Opcode, i: u64, pc: usize) -> Result<bool, Trap> {
        // Known once the resource settled this pass, like its value.
        let i = i as usize;
        let env = self.env;
        if env.resources.get(i).is_none_or(Option::is_none) {
            return Err(Trap::Pending { pc });
        }
        Ok(if op == Opcode::FailedResource {
            env.failed_resources.get(i).is_some_and(Option::is_some)
        } else {
            env.pending_resources.get(i).copied().unwrap_or(false)
        })
    }

    // @ref LLP 1109 D3 — `failure(x)`: known once settled, as `failed(x)`;
    // the `Failure` record is `{ code, message }` by position.
    fn resource_failure(&mut self, i: u64, pc: usize) -> Result<Value, Trap> {
        if !self.resource_flag(Opcode::FailedResource, i, pc)? {
            return Ok(Value::Option(None));
        }
        let why = self.env.failed_why.get(i as usize).and_then(Option::as_ref);
        let (code, message) = why.map_or(("error", "it failed"), |w| (w.code.name(), &w.message));
        let record = Value::Record(
            [Value::str(code), Value::str(message)]
                .into_iter()
                .collect(),
        );
        self.some(record, pc)
    }

    #[inline]
    fn pending_mutation(&self, i: u64) -> bool {
        self.env
            .pending_mutations
            .get(i as usize)
            .copied()
            .unwrap_or(false)
    }

    // @ref LLP 1090 D2 — a `Some` adds depth and nothing else: the JS
    // target erases it, and `type-option-option` keeps at most one above
    // each counted value, so walks stay within twice the count.
    fn some(&mut self, v: Value, pc: usize) -> Result<Value, Trap> {
        let inner = self.extents.of(&v, pc)?;
        let e = Extent {
            depth: inner.depth + 1,
            ..inner
        }
        .check(pc)?;
        let v = Value::some(v);
        self.extents.remember(&v, e);
        Ok(v)
    }

    fn list(&mut self, items: Vec<Value>, pc: usize) -> Result<Value, Trap> {
        let e = self.extents.built(&items, pc)?;
        let v = Value::list(items);
        self.extents.remember(&v, e);
        Ok(v)
    }

    fn record_len(&self, ty: u64, _pc: usize) -> Result<usize, Trap> {
        let ty = exact_plan::TypesId(ty as u32);
        Ok(self.env.plan.type_(ty).fields.len as usize)
    }

    fn record(&mut self, _ty: u64, fields: Vec<Value>, pc: usize) -> Result<Value, Trap> {
        let e = self.extents.built(&fields, pc)?;
        let v = Value::record(fields);
        self.extents.remember(&v, e);
        Ok(v)
    }

    fn native_props(&mut self, pairs: Vec<Value>, pc: usize) -> Result<Value, Trap> {
        let json = stdlib::native_props(&pairs).ok_or(Trap::TypeMismatch {
            pc,
            op: Opcode::NativeProps,
        })?;
        if json.len() > MAX_STRING {
            return Err(Trap::StringTooLong { pc });
        }
        Ok(Value::str(&json))
    }

    fn arity(&self, f: u64, pc: usize) -> Result<usize, Trap> {
        let f = Stdlib::from_wire(f as u8).ok_or(Trap::Malformed { pc })?;
        Ok(f.arity())
    }

    fn call(&mut self, f: u64, stack: &[Value], at: usize, pc: usize) -> Result<Value, Trap> {
        let f = Stdlib::from_wire(f as u8).ok_or(Trap::Malformed { pc })?;
        let op = Opcode::Call;
        let call_args = &stack[at..];
        if f == Stdlib::Join {
            if let Some(Value::List(items)) = call_args.first() {
                step(&mut self.steps, items.len(), pc)?;
            }
            return match stdlib::join(call_args, MAX_STRING) {
                Ok(v) => Ok(v),
                Err(stdlib::JoinError::Type) => Err(Trap::TypeMismatch { pc, op }),
                Err(stdlib::JoinError::TooLong) => Err(Trap::StringTooLong { pc }),
            };
        }
        if let Some(v) = list_call(f, call_args, &mut self.steps, &mut self.extents, pc)? {
            return Ok(v);
        }
        let env = self.env;
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
        })
    }

    fn may_write(&self, slot: u64) -> bool {
        self.allowed_writes.contains(&(slot as u32))
    }

    fn store(&mut self, slot: u64, v: Value, pc: usize) -> Result<(), Trap> {
        let slot = slot as u32;
        let plan = self.env.plan;
        let row = plan
            .slots
            .get(slot as usize)
            .ok_or(Trap::Malformed { pc })?;
        match plan.owner_region(row) {
            Some(region) => {
                // An owned slot: written to the instance in force — an action
                // run with none (`act`, a timer) has none to write.
                let row = Frame::row_of(self.env.frames, region.0)
                    .ok_or(Trap::BadScope { pc, depth: 0 })?;
                self.out.row_writes.push((slot, v, row.clone()));
            }
            None => self.out.writes.push((slot, v)),
        }
        Ok(())
    }

    fn command(&mut self, name: u64, args: Vec<Value>) {
        let name = self
            .env
            .plan
            .str(exact_plan::StrId(name as u32))
            .to_string();
        self.out.commands.push((name, args));
    }

    fn mutation_slot(&self, m: u64, _pc: usize) -> Result<u64, Trap> {
        let id = exact_plan::MutationsId(m as u32);
        Ok(u64::from(self.env.plan.mutation(id).slot.0))
    }

    fn send(&mut self, m: u64, source: u64, args: Vec<Value>) {
        let source = self
            .env
            .plan
            .str(exact_plan::StrId(source as u32))
            .to_string();
        self.out.sends.push((m as u32, source, args));
    }

    fn refresh(&mut self, r: u64) {
        self.out.refreshes.push(r as u32);
    }

    #[inline]
    fn steps(&mut self, n: usize, pc: usize) -> Result<(), Trap> {
        step(&mut self.steps, n, pc)
    }

    #[inline]
    fn extent(&self) -> Extent {
        Extent {
            nodes: 1,
            ..Extent::default()
        }
    }

    #[inline]
    fn kept(&mut self, e: Extent, v: &Value, pc: usize) -> Result<Extent, Trap> {
        e.with(self.extents.of(v, pc)?, pc)
    }

    fn collected(&mut self, out: Vec<Value>, e: Extent) -> Value {
        let v = Value::list(out);
        self.extents.remember(&v, e);
        v
    }
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
            failed_why: &[],
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
            failed_why: &[],
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
                failed_why: &[],
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
