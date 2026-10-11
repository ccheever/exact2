//! The expression VM's machine: one step of [`crate::vm::eval`]'s loop.
//!
//! Generic over the value ([`Val`]) and over everything a body reads or asks
//! of its runner ([`Host`]), and written in the subset of Rust that Charon and
//! Aeneas translate (no closures, iterators or trait objects; vectors through
//! indices), so that this file, as shipped, is extracted to Lean and proved
//! to step as `semantics/Contract/Vm.lean` does (`semantics/vm-extract`).
//! [`crate::vm`] instantiates it with [`exact_plan::Value`] and the runner's
//! environment; nothing here knows either.
//!
//! What stays outside, behind the two traits, is what the Lean model leaves
//! out or keeps abstract: how a value is stored, `f64` arithmetic and string
//! contents, the roster, where a slot's value lives, and the resource bounds
//! (sizes, list steps).

use exact_plan::Opcode;

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
    /// An expression would make a string longer than
    /// [`crate::vm::MAX_STRING`] bytes.
    StringTooLong {
        pc: usize,
    },
    /// A derive or resource read before it settled this update; the runner's
    /// settlement loop retries it later in the same pass.
    Pending {
        pc: usize,
    },
    /// A `List`, `Record` or `Some` would hold more than
    /// [`crate::vm::MAX_VALUE_NODES`] values or [`crate::vm::MAX_VALUE_BYTES`]
    /// string bytes, shared ones counted where they appear.
    ValueTooLarge {
        pc: usize,
    },
    /// A `List`, `Record` or `Some` would nest deeper than
    /// [`crate::vm::MAX_VALUE_DEPTH`].
    ValueTooDeep {
        pc: usize,
    },
    /// A `Map`, `Filter` or `join` (at `pc`) would take this evaluation past
    /// [`crate::vm::MAX_LIST_STEPS`] (LLP 1017.003 D3).
    IterationLimit {
        pc: usize,
    },
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

/// What a value is, as far as the machine looks into it.
pub enum Kind<'a, V> {
    /// IEEE 754 binary64.
    Number(f64),
    /// A boolean.
    Bool(bool),
    /// Text; its contents are [`Val`]'s.
    Str,
    /// The unit value.
    Unit,
    /// `none`.
    None,
    /// `some(v)`.
    Some(&'a V),
    /// An ordered list.
    List(&'a [V]),
    /// A record's fields by position.
    Record(&'a [V]),
}

/// A binary opcode over two numbers.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Num {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Lt,
    Le,
    Gt,
    Ge,
}

/// A value: its [`Kind`], the scalars the machine makes, and the
/// operations on numbers and strings, which are the value's.
pub trait Val: Clone {
    /// What `self` is.
    fn kind(&self) -> Kind<'_, Self>;
    /// A number.
    fn number(n: f64) -> Self;
    /// A list position as a number (a callback's index local).
    fn index(n: usize) -> Self;
    /// A boolean.
    fn boolean(b: bool) -> Self;
    /// The unit value.
    fn unit() -> Self;
    /// `none`.
    fn none() -> Self;
    /// `op` over two numbers: a number, or a boolean for a comparison.
    fn num(op: Num, a: f64, b: f64) -> Self;
    /// `-a`.
    fn negate(a: f64) -> Self;
    /// IEEE equality.
    fn num_eq(a: f64, b: f64) -> bool;
    /// Whether two strings hold the same text.
    fn str_eq(a: &Self, b: &Self) -> bool;
    /// Two strings joined; `None` past the string bound.
    fn concat(a: &Self, b: &Self) -> Option<Self>;
    /// `op` (a comparison) over two strings by UTF-16 code units (LLP 1088 D1).
    fn str_cmp(op: Num, a: &Self, b: &Self) -> bool;
}

/// The language's structural equality; `None` when the kinds differ before
/// the first difference.
pub fn equal<V: Val>(a: &V, b: &V) -> Option<bool> {
    match (a.kind(), b.kind()) {
        (Kind::Number(x), Kind::Number(y)) => Some(V::num_eq(x, y)),
        (Kind::Bool(x), Kind::Bool(y)) => Some(x == y),
        (Kind::Str, Kind::Str) => Some(V::str_eq(a, b)),
        (Kind::Unit, Kind::Unit) => Some(true),
        (Kind::None, Kind::None) => Some(true),
        (Kind::Some(_), Kind::None) => Some(false),
        (Kind::None, Kind::Some(_)) => Some(false),
        (Kind::Some(x), Kind::Some(y)) => equal(x, y),
        (Kind::List(xs), Kind::List(ys)) => equal_items(xs, ys),
        (Kind::Record(xs), Kind::Record(ys)) => equal_items(xs, ys),
        _ => None,
    }
}

/// Two lists' or records' items, pairwise, stopping at the first unequal
/// pair or kind mismatch; lists of different lengths are unequal.
pub fn equal_items<V: Val>(xs: &[V], ys: &[V]) -> Option<bool> {
    if xs.len() != ys.len() {
        return Some(false);
    }
    let mut i = 0;
    while i < xs.len() {
        match equal(&xs[i], &ys[i]) {
            None => return None,
            Some(false) => return Some(false),
            Some(true) => {}
        }
        i += 1;
    }
    Some(true)
}

/// Everything a body reads or asks for beyond its operand stack and locals:
/// the code, the environment, the roster, its effects, and the resource
/// bounds. Each method traps as the runner does; `pc` names the instruction.
pub trait Host<V> {
    /// What a callback keeps of the size of the list it is building.
    type Extent: Copy;
    /// The instruction at `pos` and the position after it; `None` at the end.
    fn fetch(&mut self, pos: usize) -> Result<Option<(Instruction, usize)>, Trap>;
    /// The code's length: the furthest a jump may go.
    fn code_len(&self) -> usize;
    /// The string pool's entry `i`.
    fn string(&self, i: u64, pc: usize) -> Result<V, Trap>;
    /// Slot `i`.
    fn load_slot(&self, i: u64, pc: usize) -> Result<V, Trap>;
    /// Derive `i`.
    fn load_derive(&mut self, i: u64, pc: usize) -> Result<V, Trap>;
    /// Resource `i`.
    fn load_resource(&mut self, i: u64, pc: usize) -> Result<V, Trap>;
    /// Action parameter `i`.
    fn load_param(&self, i: u64, pc: usize) -> Result<V, Trap>;
    /// The item, index or binding of the frame `depth` from the innermost.
    fn load_frame(&self, op: Opcode, depth: u64, pc: usize) -> Result<V, Trap>;
    /// Whether resource `i` has a request in flight (`PendingResource`) or
    /// failed (`FailedResource`).
    fn resource_flag(&self, op: Opcode, i: u64, pc: usize) -> Result<bool, Trap>;
    /// `none`, or `some` of why resource `i` failed (`FailureResource`).
    fn resource_failure(&mut self, i: u64, pc: usize) -> Result<V, Trap>;
    /// Whether mutation `i` has a request in flight.
    fn pending_mutation(&self, i: u64) -> bool;
    /// `some(v)`.
    fn some(&mut self, v: V, pc: usize) -> Result<V, Trap>;
    /// A list of `items`.
    fn list(&mut self, items: Vec<V>, pc: usize) -> Result<V, Trap>;
    /// How many fields type `ty` has.
    fn record_len(&self, ty: u64, pc: usize) -> Result<usize, Trap>;
    /// A record of type `ty`.
    fn record(&mut self, ty: u64, fields: Vec<V>, pc: usize) -> Result<V, Trap>;
    /// A native module's props from `2 * n` values: key, value, ….
    fn native_props(&mut self, pairs: Vec<V>, pc: usize) -> Result<V, Trap>;
    /// How many arguments roster entry `f` takes.
    fn arity(&self, f: u64, pc: usize) -> Result<usize, Trap>;
    /// Roster entry `f` on `stack[at..]`.
    fn call(&mut self, f: u64, stack: &[V], at: usize, pc: usize) -> Result<V, Trap>;
    /// Whether this body may write slot `slot`.
    fn may_write(&self, slot: u64) -> bool;
    /// Write `v` to slot `slot`.
    fn store(&mut self, slot: u64, v: V, pc: usize) -> Result<(), Trap>;
    /// Issue command `name` (a pool string).
    fn command(&mut self, name: u64, args: Vec<V>);
    /// The slot mutation `m` writes.
    fn mutation_slot(&self, m: u64, pc: usize) -> Result<u64, Trap>;
    /// Send to mutation `m` from `source` (a pool string).
    fn send(&mut self, m: u64, source: u64, args: Vec<V>);
    /// Re-request resource `r`.
    fn refresh(&mut self, r: u64);
    /// Take `n` list steps.
    fn steps(&mut self, n: usize, pc: usize) -> Result<(), Trap>;
    /// The size of an empty list being built.
    fn extent(&self) -> Self::Extent;
    /// `e` with `v` added to it.
    fn kept(&mut self, e: Self::Extent, v: &V, pc: usize) -> Result<Self::Extent, Trap>;
    /// The list a callback built, of size `e`.
    fn collected(&mut self, out: Vec<V>, e: Self::Extent) -> V;
}

/// A `Map` or `Filter` in progress (LLP 1017.003 D5): its callback body is
/// `start..end`, run once per item with the item and its index bound as
/// locals `locals` and `locals + 1`, on an operand stack of its own (the
/// caller's is kept in `caller`), so a body can neither read nor drop what
/// its caller left, nor drop a local it did not bind.
pub struct Callback<V, E> {
    /// The opcode's offset.
    pub pc: usize,
    /// `Filter`, not `Map`.
    pub filter: bool,
    /// The body's first instruction.
    pub start: usize,
    /// Where the body ends.
    pub end: usize,
    /// The list.
    pub list: V,
    /// Its length.
    pub len: usize,
    /// The item being run.
    pub next: usize,
    /// What the runs so far kept.
    pub out: Vec<V>,
    /// `out`'s size.
    pub extent: E,
    /// The locals depth at the opcode.
    pub locals: usize,
    /// The operand stack the opcode set aside.
    pub caller: Vec<V>,
}

/// The machine between steps.
pub struct Machine<V, E> {
    /// The next instruction's position.
    pub pos: usize,
    /// The operand stack, top last.
    pub stack: Vec<V>,
    /// The locals, bottom first.
    pub locals: Vec<V>,
    /// Callbacks in progress, innermost last.
    pub callbacks: Vec<Callback<V, E>>,
}

impl<V, E> Machine<V, E> {
    /// At position 0, nothing on any stack.
    pub fn new() -> Self {
        Machine {
            pos: 0,
            stack: Vec::new(),
            locals: Vec::new(),
            callbacks: Vec::new(),
        }
    }
}

impl<V, E> Default for Machine<V, E> {
    fn default() -> Self {
        Self::new()
    }
}

fn pop<V>(stack: &mut Vec<V>, pc: usize) -> Result<V, Trap> {
    match stack.pop() {
        Some(v) => Ok(v),
        None => Err(Trap::StackUnderflow { pc }),
    }
}

/// The top `n`, in push order; `trap` when there are fewer.
fn pop_n<V>(stack: &mut Vec<V>, n: usize, trap: Trap) -> Result<Vec<V>, Trap> {
    if stack.len() < n {
        return Err(trap);
    }
    let at = stack.len() - n;
    Ok(stack.split_off(at))
}

/// A jump's target, refused past the end of the code or out of the callback
/// body it is in (the plan checker's rule, checked again: a plan is never
/// trusted).
fn jump_to<V, E>(
    len: usize,
    callbacks: &[Callback<V, E>],
    target: u64,
    pc: usize,
) -> Result<usize, Trap> {
    let bad = Trap::BadJump {
        pc,
        target: target as u32,
    };
    let n = callbacks.len();
    if n > 0 && target > callbacks[n - 1].end as u64 {
        return Err(bad);
    }
    if target > len as u64 {
        return Err(bad);
    }
    Ok(target as usize)
}

/// Item `next` of a callback's list.
fn item<V: Val>(list: &V, next: usize, pc: usize) -> Result<V, Trap> {
    match list.kind() {
        Kind::List(items) => {
            if next < items.len() {
                Ok(items[next].clone())
            } else {
                Err(Trap::Malformed { pc })
            }
        }
        _ => Err(Trap::Malformed { pc }),
    }
}

/// Bind the callback's item `next` and its index, counting the run.
fn begin<V: Val, H: Host<V>>(
    host: &mut H,
    c: &Callback<V, H::Extent>,
    locals: &mut Vec<V>,
) -> Result<(), Trap> {
    host.steps(1, c.pc)?;
    let x = item(&c.list, c.next, c.pc)?;
    locals.push(x);
    locals.push(V::index(c.next));
    Ok(())
}

/// The innermost callback's body (callback `k`, the last) ends: collect
/// what the run left, then run the next item or push the list. The callback
/// is updated where it is (it is a large value to move once per item) and
/// popped only when its list is done.
fn body_end<V: Val, H: Host<V>>(
    host: &mut H,
    m: &mut Machine<V, H::Extent>,
    k: usize,
) -> Result<(), Trap> {
    let pc = m.callbacks[k].end;
    // One value left, every local it bound dropped.
    if m.stack.len() != 1 || m.locals.len() != m.callbacks[k].locals + 2 {
        return Err(Trap::Malformed { pc });
    }
    let v = pop(&mut m.stack, pc)?;
    let c = &mut m.callbacks[k];
    let kept = if c.filter {
        match v.kind() {
            Kind::Bool(true) => Some(item(&c.list, c.next, c.pc)?),
            Kind::Bool(false) => None,
            _ => {
                return Err(Trap::TypeMismatch {
                    pc: c.pc,
                    op: Opcode::Filter,
                })
            }
        }
    } else {
        Some(v)
    };
    if let Some(v) = kept {
        c.extent = host.kept(c.extent, &v, c.pc)?;
        c.out.push(v);
    }
    m.locals.truncate(c.locals);
    c.next += 1;
    if c.next < c.len {
        begin(host, c, &mut m.locals)?;
        m.pos = c.start;
        return Ok(());
    }
    let Some(c) = m.callbacks.pop() else {
        return Err(Trap::Malformed { pc });
    };
    m.stack = c.caller;
    // A filter that kept every item is that list, shared.
    let v = if c.filter && c.out.len() == c.len {
        c.list
    } else {
        host.collected(c.out, c.extent)
    };
    m.stack.push(v);
    Ok(())
}

/// One iteration of the loop: a callback body's end, else the instruction
/// at `m.pos`. `Some(v)` when the body returned `v`.
pub fn step<V: Val, H: Host<V>>(
    host: &mut H,
    m: &mut Machine<V, H::Extent>,
) -> Result<Option<V>, Trap> {
    let n = m.callbacks.len();
    if n > 0 && m.pos == m.callbacks[n - 1].end {
        body_end(host, m, n - 1)?;
        return Ok(None);
    }
    match host.fetch(m.pos)? {
        None => Err(Trap::NoResult),
        Some((ins, next)) => {
            m.pos = next;
            exec(host, m, ins)
        }
    }
}

/// Two numbers to one value.
fn num2<V: Val>(stack: &mut Vec<V>, op: Num, opcode: Opcode, pc: usize) -> Result<(), Trap> {
    let b = pop(stack, pc)?;
    let a = pop(stack, pc)?;
    match (a.kind(), b.kind()) {
        (Kind::Number(x), Kind::Number(y)) => {
            stack.push(V::num(op, x, y));
            Ok(())
        }
        _ => Err(Trap::TypeMismatch { pc, op: opcode }),
    }
}

/// A comparison: two numbers by IEEE order (a NaN is unordered: every test
/// false), two strings by UTF-16 code units (LLP 1088 D1).
fn cmp2<V: Val>(stack: &mut Vec<V>, op: Num, opcode: Opcode, pc: usize) -> Result<(), Trap> {
    let b = pop(stack, pc)?;
    let a = pop(stack, pc)?;
    match (a.kind(), b.kind()) {
        (Kind::Number(x), Kind::Number(y)) => {
            stack.push(V::num(op, x, y));
            Ok(())
        }
        (Kind::Str, Kind::Str) => {
            stack.push(V::boolean(V::str_cmp(op, &a, &b)));
            Ok(())
        }
        _ => Err(Trap::TypeMismatch { pc, op: opcode }),
    }
}

/// Run `ins`, with `m.pos` already past it.
pub fn exec<V: Val, H: Host<V>>(
    host: &mut H,
    m: &mut Machine<V, H::Extent>,
    ins: Instruction,
) -> Result<Option<V>, Trap> {
    let pc = ins.pc;
    let op = ins.op;
    let a0 = ins.args[0];
    match op {
        Opcode::Number => m.stack.push(V::number(ins.number)),
        Opcode::Bool => m.stack.push(V::boolean(a0 != 0)),
        Opcode::Str => {
            let v = host.string(a0, pc)?;
            m.stack.push(v);
        }
        Opcode::None => m.stack.push(V::none()),
        Opcode::Unit => m.stack.push(V::unit()),
        Opcode::Some => {
            let v = pop(&mut m.stack, pc)?;
            let v = host.some(v, pc)?;
            m.stack.push(v);
        }
        Opcode::LoadSlot => {
            let v = host.load_slot(a0, pc)?;
            m.stack.push(v);
        }
        Opcode::LoadDerive => {
            let v = host.load_derive(a0, pc)?;
            m.stack.push(v);
        }
        Opcode::LoadResource => {
            let v = host.load_resource(a0, pc)?;
            m.stack.push(v);
        }
        Opcode::LoadParam => {
            let v = host.load_param(a0, pc)?;
            m.stack.push(v);
        }
        Opcode::LoadIndex | Opcode::LoadItem | Opcode::LoadBound => {
            let v = host.load_frame(op, a0, pc)?;
            m.stack.push(v);
        }
        Opcode::Field => {
            let index = a0 as u16;
            let v = pop(&mut m.stack, pc)?;
            let w = match v.kind() {
                Kind::Record(fields) => {
                    if (index as usize) < fields.len() {
                        fields[index as usize].clone()
                    } else {
                        return Err(Trap::BadField { pc, index });
                    }
                }
                _ => return Err(Trap::TypeMismatch { pc, op }),
            };
            m.stack.push(w);
        }
        Opcode::Record => {
            let n = host.record_len(a0, pc)?;
            let fields = pop_n(&mut m.stack, n, Trap::StackUnderflow { pc })?;
            let v = host.record(a0, fields, pc)?;
            m.stack.push(v);
        }
        Opcode::List => {
            let items = pop_n(&mut m.stack, a0 as usize, Trap::StackUnderflow { pc })?;
            let v = host.list(items, pc)?;
            m.stack.push(v);
        }
        // @ref LLP 1024 D1 — the leftover attributes, one replaced object.
        Opcode::NativeProps => {
            let pairs = pop_n(&mut m.stack, a0 as usize * 2, Trap::StackUnderflow { pc })?;
            let v = host.native_props(pairs, pc)?;
            m.stack.push(v);
        }
        Opcode::Add => num2(&mut m.stack, Num::Add, op, pc)?,
        Opcode::Sub => num2(&mut m.stack, Num::Sub, op, pc)?,
        Opcode::Mul => num2(&mut m.stack, Num::Mul, op, pc)?,
        Opcode::Div => num2(&mut m.stack, Num::Div, op, pc)?,
        Opcode::Rem => num2(&mut m.stack, Num::Rem, op, pc)?,
        Opcode::Lt => cmp2(&mut m.stack, Num::Lt, op, pc)?,
        Opcode::Le => cmp2(&mut m.stack, Num::Le, op, pc)?,
        Opcode::Gt => cmp2(&mut m.stack, Num::Gt, op, pc)?,
        Opcode::Ge => cmp2(&mut m.stack, Num::Ge, op, pc)?,
        Opcode::Neg => {
            let v = pop(&mut m.stack, pc)?;
            let w = match v.kind() {
                Kind::Number(x) => V::negate(x),
                _ => return Err(Trap::TypeMismatch { pc, op }),
            };
            m.stack.push(w);
        }
        Opcode::Eq | Opcode::Ne => {
            let b = pop(&mut m.stack, pc)?;
            let a = pop(&mut m.stack, pc)?;
            let eq = match equal(&a, &b) {
                Some(eq) => eq,
                None => return Err(Trap::TypeMismatch { pc, op }),
            };
            let eq = match op {
                Opcode::Eq => eq,
                _ => !eq,
            };
            m.stack.push(V::boolean(eq));
        }
        Opcode::Not => {
            let v = pop(&mut m.stack, pc)?;
            let w = match v.kind() {
                Kind::Bool(b) => V::boolean(!b),
                _ => return Err(Trap::TypeMismatch { pc, op }),
            };
            m.stack.push(w);
        }
        Opcode::Concat => {
            let b = pop(&mut m.stack, pc)?;
            let a = pop(&mut m.stack, pc)?;
            let v = match (a.kind(), b.kind()) {
                (Kind::Str, Kind::Str) => match V::concat(&a, &b) {
                    Some(v) => v,
                    None => return Err(Trap::StringTooLong { pc }),
                },
                _ => return Err(Trap::TypeMismatch { pc, op }),
            };
            m.stack.push(v);
        }
        Opcode::Jump => m.pos = jump_to(host.code_len(), &m.callbacks, a0, pc)?,
        Opcode::JumpIfFalse => {
            let v = pop(&mut m.stack, pc)?;
            match v.kind() {
                Kind::Bool(true) => {}
                Kind::Bool(false) => m.pos = jump_to(host.code_len(), &m.callbacks, a0, pc)?,
                _ => return Err(Trap::TypeMismatch { pc, op }),
            }
        }
        Opcode::JumpIfNone => {
            let n = m.stack.len();
            if n == 0 {
                return Err(Trap::StackUnderflow { pc });
            }
            match m.stack[n - 1].kind() {
                Kind::Some(_) => {}
                Kind::None => m.pos = jump_to(host.code_len(), &m.callbacks, a0, pc)?,
                _ => return Err(Trap::TypeMismatch { pc, op }),
            }
        }
        Opcode::Unwrap => {
            let v = pop(&mut m.stack, pc)?;
            let w = match v.kind() {
                Kind::Some(w) => w.clone(),
                Kind::None => return Err(Trap::UnwrapNone { pc }),
                _ => return Err(Trap::TypeMismatch { pc, op }),
            };
            m.stack.push(w);
        }
        Opcode::Call => {
            let n = host.arity(a0, pc)?;
            if m.stack.len() < n {
                return Err(Trap::Arity { pc, expected: n });
            }
            // The arguments are read where they are, not moved to a vector
            // of their own: a call is the VM's commonest cost.
            let at = m.stack.len() - n;
            let v = host.call(a0, &m.stack, at, pc)?;
            m.stack.truncate(at);
            m.stack.push(v);
        }
        // @ref LLP 1017.003 D5 — a callback body follows inline, to `end`.
        Opcode::Map | Opcode::Filter => {
            let start = m.pos;
            if a0 < start as u64 || a0 > host.code_len() as u64 {
                return Err(Trap::BadJump {
                    pc,
                    target: a0 as u32,
                });
            }
            let end = a0 as usize;
            let list = pop(&mut m.stack, pc)?;
            let len = match list.kind() {
                Kind::List(items) => items.len(),
                _ => return Err(Trap::TypeMismatch { pc, op }),
            };
            if len == 0 {
                m.pos = jump_to(host.code_len(), &m.callbacks, a0, pc)?;
                m.stack.push(list);
                return Ok(None);
            }
            let caller = core::mem::take(&mut m.stack);
            let filter = matches!(op, Opcode::Filter);
            let c = Callback {
                pc,
                filter,
                start,
                end,
                out: if filter {
                    Vec::new()
                } else {
                    Vec::with_capacity(len)
                },
                extent: host.extent(),
                list,
                len,
                next: 0,
                locals: m.locals.len(),
                caller,
            };
            begin(host, &c, &mut m.locals)?;
            m.callbacks.push(c);
        }
        Opcode::StoreSlot => {
            if !host.may_write(a0) {
                return Err(Trap::WriteNotDeclared {
                    pc,
                    slot: a0 as u32,
                });
            }
            let v = pop(&mut m.stack, pc)?;
            host.store(a0, v, pc)?;
        }
        Opcode::Command => {
            let args = pop_n(
                &mut m.stack,
                ins.args[1] as usize,
                Trap::StackUnderflow { pc },
            )?;
            host.command(a0, args);
        }
        Opcode::Send => {
            let slot = host.mutation_slot(a0, pc)?;
            if !host.may_write(slot) {
                return Err(Trap::WriteNotDeclared {
                    pc,
                    slot: slot as u32,
                });
            }
            let args = pop_n(
                &mut m.stack,
                ins.args[2] as usize,
                Trap::StackUnderflow { pc },
            )?;
            host.send(a0, ins.args[1], args);
        }
        Opcode::Refresh => host.refresh(a0),
        Opcode::PendingResource | Opcode::FailedResource => {
            let flag = host.resource_flag(op, a0, pc)?;
            m.stack.push(V::boolean(flag));
        }
        Opcode::FailureResource => {
            let v = host.resource_failure(a0, pc)?;
            m.stack.push(v);
        }
        Opcode::PendingMutation => m.stack.push(V::boolean(host.pending_mutation(a0))),
        Opcode::Pop => {
            pop(&mut m.stack, pc)?;
        }
        Opcode::BindLocal => {
            let v = pop(&mut m.stack, pc)?;
            m.locals.push(v);
        }
        Opcode::LoadLocal => {
            let i = a0 as usize;
            if i < m.locals.len() {
                let v = m.locals[i].clone();
                m.stack.push(v);
            } else {
                return Err(Trap::BadScope {
                    pc,
                    depth: a0 as u16,
                });
            }
        }
        Opcode::DropLocal => {
            let n = m.callbacks.len();
            if n > 0 && m.locals.len() <= m.callbacks[n - 1].locals + 2 {
                return Err(Trap::Malformed { pc });
            }
            if m.locals.pop().is_none() {
                return Err(Trap::StackUnderflow { pc });
            }
        }
        Opcode::Return => {
            if !m.callbacks.is_empty() {
                return Err(Trap::Malformed { pc });
            }
            return Ok(Some(match m.stack.pop() {
                Some(v) => v,
                None => V::unit(),
            }));
        }
    }
    Ok(None)
}

/// Step from `m` until the body returns or traps.
pub fn run<V: Val, H: Host<V>>(host: &mut H, m: &mut Machine<V, H::Extent>) -> Result<V, Trap> {
    loop {
        if let Some(v) = step(host, m)? {
            return Ok(v);
        }
    }
}
