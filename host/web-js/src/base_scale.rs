//! @ref LLP 1121 stage 11 B (B2) — the base scale on the JS target, which
//! keeps no kernel at run time. Its one look is the neutral one (the build
//! says so, `roles::set_column`), so its column is the kernel's in this
//! build (`metrics::column()`):
//! - a static row naming a base name is that column's length, as the kernel
//!   reads the literal while the sheet is projected (`theme::measure`);
//! - the sheet declares each base name the plan may read as a length
//!   ([`named`]), and the design does not define, once as a custom property
//!   on `:root`, `--exact-<name>`
//!   (`--exact-space-md`), from the kernel's table, never a copied number;
//! - a bound row's value naming one reads that property (`measures::map`),
//!   an SVG attribute its length.
//!
//! A design that defines a base name makes it its own token, which the
//! token path writes (`modes`, `measures`). The web has no iOS or Mac look
//! class to declare a second column under; one would add its rules here.

use exact_kernel::style::{metrics, scale};
use exact_kernel::StyleId;
use exact_plan::{BindingKind, Opcode, Plan};
use std::collections::BTreeSet;

/// The base names `plan` may read as a length and its design does not
/// define, in the table's order (B2 round 1, Astra 3; theme2's (B)): the
/// plan's length reads, as D7's retention keeps a token, never its string
/// pool (a text that says `space.md` declares nothing):
/// - each base name a length row's value may be, through `?:`, `match` and
///   `let` (its bytecode's string constants, [`returns`]);
/// - each a length token aliases (`card.pad: length = space.md`);
/// - every one when a length row's value is computed (`"space." + size`, a
///   state, data), as lowering keeps every length token then
///   (`computed_measure`): declaring all nine is safe, missing one is not.
pub fn named(plan: &Plan) -> Vec<&'static str> {
    let mut every = false;
    let mut said: BTreeSet<&str> = BTreeSet::new();
    for row in &plan.theme {
        if exact_kernel::style::theme::Kind::of(row.kind)
            != Some(exact_kernel::style::theme::Kind::Length)
        {
            continue;
        }
        let slices = plan.theme_slices[row.slices.start as usize..][..row.slices.len as usize]
            .iter()
            .map(|s| s.value);
        said.extend(
            std::iter::once(row.value)
                .chain(slices)
                .map(|v| plan.str(v).trim()),
        );
    }
    for b in &plan.bindings {
        let length = b.kind == BindingKind::Style
            && StyleId::from_bit(u32::from(b.id)).is_some_and(|id| {
                exact_kernel::style::theme::admits(exact_kernel::style::theme::Kind::Length, id)
            });
        if !length {
            continue;
        }
        match returns(plan, plan.code(b.expr)) {
            Some(strings) => said.extend(
                strings
                    .into_iter()
                    .map(|id| plan.str(exact_plan::StrId(id)).trim()),
            ),
            None => every = true,
        }
    }
    scale::names()
        .filter(|n| every || said.contains(n))
        .filter(|n| !plan.theme.iter().any(|r| plan.str(r.name) == *n))
        .collect()
}

/// An abstract value of [`returns`]: the string constants it may be, or
/// any string (`unknown`: read or computed), and its plan type where a read
/// says it (a record's, for `Field`). No string and not unknown is a value
/// that is no base name: a number, a bool, a record, or a string with a part
/// no base name holds (`10px`).
#[derive(Clone, Default, PartialEq)]
struct Value {
    strings: BTreeSet<u32>,
    unknown: bool,
    ty: Option<exact_plan::TypesId>,
}

impl Value {
    fn unknown() -> Value {
        Value {
            unknown: true,
            ..Value::default()
        }
    }
    /// A read of plan type `ty`: a string (or an option of one) may be any.
    fn typed(plan: &Plan, ty: exact_plan::TypesId) -> Value {
        use exact_plan::TypeKind;
        let row = &plan.types[ty.0 as usize];
        let mut v = match row.kind {
            TypeKind::String => Value::unknown(),
            TypeKind::Option => match row.elem {
                Some(elem) => Value::typed(plan, elem),
                None => Value::unknown(),
            },
            _ => Value::default(),
        };
        v.ty = Some(ty);
        v
    }
    fn join(&mut self, other: &Value) {
        self.strings.extend(&other.strings);
        self.unknown |= other.unknown;
        if self.ty != other.ty {
            self.ty = None;
        }
    }
    /// Whether every string it may be holds a part no base name holds, so
    /// no concatenation with it is one (a digit, `px`, `%`).
    fn no_base(&self, plan: &Plan) -> bool {
        !self.unknown
            && self.strings.iter().all(|id| {
                let text = plan.str(exact_plan::StrId(*id));
                !scale::names().any(|n| n.contains(text))
            })
    }
}

/// The string constants (pool ids) `code` may return, when its result is a
/// literal through its branches (`?:`, `match`, `let`; a condition may read
/// anything); `None` when it may be any string: read (a state, a param,
/// data), computed (`Concat`, a call), or code this reading does not follow.
/// A literal's leaves as lowering's `literal` reads them, on the bytecode.
fn returns(plan: &Plan, code: &[u8]) -> Option<BTreeSet<u32>> {
    let program: Vec<_> = exact_runner::vm::instructions(code)
        .collect::<Result<_, _>>()
        .ok()?;
    let at = |pc: usize| program.iter().position(|i| i.pc == pc);
    let mut seen: Vec<Option<(Vec<Value>, Vec<Value>)>> = vec![None; program.len()];
    let mut work = vec![0usize];
    seen[0] = Some((Vec::new(), Vec::new()));
    let mut out = Value::default();
    // Each instruction's state joined from every way in; one that grows is
    // walked again (a finite lattice: the body's constants).
    while let Some(i) = work.pop() {
        let (mut stack, mut locals) = seen[i].clone()?;
        let ins = program[i];
        let next = i + 1;
        let mut to: Vec<usize> = vec![next];
        let pop = |stack: &mut Vec<Value>| stack.pop();
        match ins.op {
            Opcode::Number | Opcode::Bool | Opcode::None | Opcode::Unit => {
                stack.push(Value::default())
            }
            Opcode::LoadIndex
            | Opcode::PendingResource
            | Opcode::PendingMutation
            | Opcode::FailedResource
            | Opcode::FailedMutation => stack.push(Value::default()),
            Opcode::Str => stack.push(Value {
                strings: [ins.args[0] as u32].into(),
                ..Value::default()
            }),
            Opcode::Some => {}
            Opcode::Unwrap => {
                let v = pop(&mut stack)?;
                let elem = v.ty.map(|t| &plan.types[t.0 as usize]).and_then(|t| {
                    (t.kind == exact_plan::TypeKind::Option)
                        .then_some(t.elem)
                        .flatten()
                });
                stack.push(match elem {
                    Some(elem) => Value::typed(plan, elem),
                    None => Value { ty: None, ..v },
                });
            }
            // A read says its type: a string one may be any (a state an
            // action sets to `"space." + size`), another is no base name.
            Opcode::LoadSlot => {
                stack.push(Value::typed(plan, plan.slots.get(ins.args[0] as usize)?.ty))
            }
            Opcode::LoadDerive => stack.push(Value::typed(
                plan,
                plan.derives.get(ins.args[0] as usize)?.ty,
            )),
            Opcode::LoadResource => stack.push(Value::typed(
                plan,
                plan.resources.get(ins.args[0] as usize)?.ty,
            )),
            Opcode::LoadParam
            | Opcode::LoadItem
            | Opcode::LoadBound
            | Opcode::FailureResource
            | Opcode::FailureMutation => stack.push(Value::unknown()),
            Opcode::Field => {
                let v = pop(&mut stack)?;
                let record =
                    v.ty.map(|t| &plan.types[t.0 as usize])
                        .filter(|t| t.kind == exact_plan::TypeKind::Record);
                stack.push(match record {
                    Some(t) if (ins.args[0] as u32) < t.fields.len => Value::typed(
                        plan,
                        plan.fields[(t.fields.start + ins.args[0] as u32) as usize].ty,
                    ),
                    _ => Value::unknown(),
                });
            }
            Opcode::Call => {
                let f = exact_plan::Stdlib::from_wire(ins.args[0] as u8)?;
                for _ in 0..f.arity() {
                    pop(&mut stack)?;
                }
                let returns = f.returns();
                stack.push(
                    if returns == "string"
                        || returns == "any"
                        || returns.starts_with("option<string")
                    {
                        Value::unknown()
                    } else {
                        Value::default()
                    },
                );
            }
            Opcode::Neg | Opcode::Not => {
                pop(&mut stack)?;
                stack.push(Value::default());
            }
            Opcode::Add
            | Opcode::Sub
            | Opcode::Mul
            | Opcode::Div
            | Opcode::Rem
            | Opcode::Eq
            | Opcode::Ne
            | Opcode::Lt
            | Opcode::Le
            | Opcode::Gt
            | Opcode::Ge => {
                pop(&mut stack)?;
                pop(&mut stack)?;
                stack.push(Value::default());
            }
            // A string with a part no base name holds (`size + "px"`) is none.
            Opcode::Concat => {
                let (b, a) = (pop(&mut stack)?, pop(&mut stack)?);
                stack.push(if a.no_base(plan) || b.no_base(plan) {
                    Value::default()
                } else {
                    Value::unknown()
                });
            }
            Opcode::Pop => {
                pop(&mut stack)?;
            }
            Opcode::BindLocal => locals.push(pop(&mut stack)?),
            Opcode::LoadLocal => stack.push(locals.get(ins.args[0] as usize)?.clone()),
            Opcode::DropLocal => {
                locals.pop()?;
            }
            Opcode::Jump => to = vec![at(ins.args[0] as usize)?],
            Opcode::JumpIfFalse => {
                pop(&mut stack)?;
                to.push(at(ins.args[0] as usize)?);
            }
            Opcode::JumpIfNone => to.push(at(ins.args[0] as usize)?),
            Opcode::Return => {
                out.join(stack.last()?);
                to.clear();
            }
            _ => return None,
        }
        for t in to {
            if t >= program.len() {
                return None;
            }
            match &mut seen[t] {
                Some((s, l)) => {
                    if s.len() != stack.len() || l.len() != locals.len() {
                        return None;
                    }
                    let before = (s.clone(), l.clone());
                    s.iter_mut().zip(&stack).for_each(|(a, b)| a.join(b));
                    l.iter_mut().zip(&locals).for_each(|(a, b)| a.join(b));
                    if (s.clone(), l.clone()) != before {
                        work.push(t);
                    }
                }
                slot => {
                    *slot = Some((stack.clone(), locals.clone()));
                    work.push(t);
                }
            }
        }
    }
    (!out.unknown).then_some(out.strings)
}

/// A base name's custom property: `--exact-space-md`.
pub fn property(name: &str) -> String {
    format!("--exact-{}", name.replace('.', "-"))
}

/// Base name `name`'s CSS in this build's column (the kernel's table).
pub fn value(name: &str) -> String {
    scale::css(scale::of(name, metrics::column()).expect("a base name"))
}

/// The sheet's declarations: `:root{--exact-space-md:16px;…}` for each base
/// name the plan names, nothing when it names none.
pub fn css(plan: &Plan) -> String {
    let names = named(plan);
    if names.is_empty() {
        return String::new();
    }
    let declarations: String = names
        .iter()
        .map(|n| format!("{}:{};", property(n), value(n)))
        .collect();
    format!(":root{{{declarations}}}")
}

/// A bound row's map entries for the base names the plan names: each its
/// property's `var()`, or its length where the row is an SVG attribute,
/// which reads no property.
pub fn pairs(plan: &Plan, attribute: bool) -> Vec<String> {
    named(plan)
        .into_iter()
        .map(|n| {
            let css = if attribute {
                value(n)
            } else {
                format!("var({})", property(n))
            };
            format!(
                "{}:{}",
                serde_json::to_string(n).unwrap(),
                serde_json::to_string(&css).unwrap()
            )
        })
        .collect()
}
