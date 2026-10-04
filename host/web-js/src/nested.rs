//! Options of options, which the JS target refuses by name.
//!
//! The runtime holds `none` as `null` and `some(x)` as `x` (code.rs), so it
//! cannot tell `some(none)` from `none`. A plan that can make one is refused
//! at build time, as the target's other refusals are, rather than run with a
//! different meaning: a declared `option<option<…>>` anywhere in its types,
//! or a body that wraps a value it knows is an option in `some`, or takes
//! `first`/`at` of a list whose items it knows are options. What a body
//! knows is followed over the bytecode from the plan's declared types (slots,
//! derives, resources, an action's parameters, records' fields); a value of a
//! type it cannot follow (a region's item, a `map`'s result, most roster
//! results) is not refused.

use exact_plan::{Opcode, Plan, Stdlib, TypeKind, TypesId};
use exact_runner::vm::Instruction;

/// Why the plan is refused, if it declares an option of an option.
pub fn declared(plan: &Plan) -> Result<(), String> {
    let nested = plan.types.iter().any(|t| {
        t.kind == TypeKind::Option
            && t.elem
                .is_some_and(|e| plan.types[e.0 as usize].kind == TypeKind::Option)
    });
    if nested {
        return Err(REFUSAL.into());
    }
    Ok(())
}

const REFUSAL: &str = "an option of an option (`some(none)`): the JS target holds `some(x)` as `x` and cannot tell it from `none`";

/// What a body knows of a value's type.
#[derive(Clone)]
enum St {
    Unknown,
    Plan(TypesId),
    Opt(Box<St>),
    List(Box<St>),
}

impl St {
    fn kind<'p>(&self, plan: &'p Plan) -> Option<&'p exact_plan::TypesRow> {
        match self {
            St::Plan(t) => Some(&plan.types[t.0 as usize]),
            _ => None,
        }
    }
    fn is_option(&self, plan: &Plan) -> bool {
        matches!(self, St::Opt(_)) || self.kind(plan).is_some_and(|t| t.kind == TypeKind::Option)
    }
    /// The element of an option or a list.
    fn elem(&self, plan: &Plan) -> St {
        match self {
            St::Opt(e) | St::List(e) => (**e).clone(),
            St::Plan(_) => self
                .kind(plan)
                .and_then(|t| t.elem)
                .map_or(St::Unknown, St::Plan),
            St::Unknown => St::Unknown,
        }
    }
}

/// Refuse `ins[from..to]` if it makes an option of an option.
pub fn check(plan: &Plan, ins: &[Instruction], params: &[TypesId]) -> Result<(), String> {
    range(plan, ins, 0, ins.len(), params, &mut Vec::new())
}

fn range(
    plan: &Plan,
    ins: &[Instruction],
    from: usize,
    to: usize,
    params: &[TypesId],
    locals: &mut Vec<St>,
) -> Result<(), String> {
    let mut stack: Vec<St> = Vec::new();
    // The stack a forward jump leaves at its target.
    let mut at: Vec<(usize, Vec<St>, usize)> = Vec::new();
    let mut dead = false;
    let pop = |s: &mut Vec<St>| s.pop().unwrap_or(St::Unknown);
    let popn = |s: &mut Vec<St>, n: usize| {
        let k = s.len().saturating_sub(n);
        s.split_off(k)
    };
    let mut i = from;
    while i < to {
        let x = &ins[i];
        if let Some((_, s, l)) = at.iter().find(|(t, _, _)| *t == x.pc) {
            if dead {
                stack = s.clone();
                locals.truncate(*l);
            }
            dead = false;
        }
        match x.op {
            Opcode::Number | Opcode::Bool | Opcode::Str | Opcode::Unit => stack.push(St::Unknown),
            Opcode::None => stack.push(St::Opt(Box::new(St::Unknown))),
            Opcode::Some => {
                let v = pop(&mut stack);
                if v.is_option(plan) {
                    return Err(REFUSAL.into());
                }
                stack.push(St::Opt(Box::new(v)));
            }
            Opcode::Unwrap => {
                let v = pop(&mut stack);
                stack.push(v.elem(plan));
            }
            Opcode::LoadSlot => stack.push(St::Plan(plan.slots[x.args[0] as usize].ty)),
            Opcode::LoadDerive => stack.push(St::Plan(plan.derives[x.args[0] as usize].ty)),
            Opcode::LoadResource => stack.push(St::Plan(plan.resources[x.args[0] as usize].ty)),
            Opcode::LoadParam => stack.push(
                params
                    .get(x.args[0] as usize)
                    .map_or(St::Unknown, |t| St::Plan(*t)),
            ),
            Opcode::LoadItem
            | Opcode::LoadIndex
            | Opcode::LoadBound
            | Opcode::PendingResource
            | Opcode::FailedResource
            | Opcode::PendingMutation => stack.push(St::Unknown),
            Opcode::Field => {
                let v = pop(&mut stack);
                let field = v
                    .kind(plan)
                    .filter(|t| t.kind == TypeKind::Record)
                    .and_then(|t| t.fields.iter().nth(x.args[0] as usize))
                    .map_or(St::Unknown, |f| St::Plan(plan.field(f).ty));
                stack.push(field);
            }
            Opcode::Record => {
                let n = plan.types[x.args[0] as usize].fields.len as usize;
                popn(&mut stack, n);
                stack.push(St::Plan(TypesId(x.args[0] as u32)));
            }
            Opcode::List => {
                let items = popn(&mut stack, x.args[0] as usize);
                let e = items.into_iter().next().unwrap_or(St::Unknown);
                stack.push(St::List(Box::new(e)));
            }
            Opcode::Add
            | Opcode::Concat
            | Opcode::Sub
            | Opcode::Mul
            | Opcode::Div
            | Opcode::Rem
            | Opcode::Lt
            | Opcode::Le
            | Opcode::Gt
            | Opcode::Ge
            | Opcode::Eq
            | Opcode::Ne => {
                popn(&mut stack, 2);
                stack.push(St::Unknown);
            }
            Opcode::Not | Opcode::Neg => {
                pop(&mut stack);
                stack.push(St::Unknown);
            }
            Opcode::Call => {
                let f = Stdlib::from_wire(x.args[0] as u8).ok_or("unknown stdlib entry")?;
                let args = popn(&mut stack, f.arity());
                if matches!(f, Stdlib::First | Stdlib::At) {
                    let e = args.first().map_or(St::Unknown, |l| l.elem(plan));
                    if e.is_option(plan) {
                        return Err(REFUSAL.into());
                    }
                    stack.push(St::Opt(Box::new(e)));
                } else {
                    stack.push(St::Unknown);
                }
            }
            Opcode::Jump => {
                at.push((x.args[0] as usize, stack.clone(), locals.len()));
                dead = true;
            }
            Opcode::JumpIfFalse => {
                pop(&mut stack);
                at.push((x.args[0] as usize, stack.clone(), locals.len()));
            }
            Opcode::JumpIfNone => at.push((x.args[0] as usize, stack.clone(), locals.len())),
            Opcode::Pop | Opcode::StoreSlot => {
                pop(&mut stack);
            }
            Opcode::BindLocal => {
                let v = pop(&mut stack);
                locals.push(v);
            }
            Opcode::LoadLocal => stack.push(
                locals
                    .get(x.args[0] as usize)
                    .cloned()
                    .unwrap_or(St::Unknown),
            ),
            Opcode::DropLocal => {
                locals.pop();
            }
            Opcode::Map | Opcode::Filter => {
                let end = x.args[0] as usize;
                let list = pop(&mut stack);
                let mut j = i + 1;
                while j < to && ins[j].pc < end {
                    j += 1;
                }
                let base = locals.len();
                locals.push(list.elem(plan));
                locals.push(St::Unknown);
                range(plan, ins, i + 1, j, params, locals)?;
                locals.truncate(base);
                stack.push(if x.op == Opcode::Filter {
                    list
                } else {
                    St::List(Box::new(St::Unknown))
                });
                i = j;
                continue;
            }
            Opcode::Command => {
                popn(&mut stack, x.args[1] as usize);
            }
            Opcode::Send => {
                popn(&mut stack, x.args[2] as usize);
            }
            Opcode::Refresh => {}
            Opcode::NativeProps => {
                popn(&mut stack, x.args[0] as usize * 2);
                stack.push(St::Unknown);
            }
            Opcode::Return => dead = true,
        }
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    fn emitted(src: &str) -> Result<(), String> {
        let plan = contract::compile(src).map_err(|e| e.to_string())?;
        crate::emit::emit(&plan, false, false).map(drop)
    }

    fn refused(src: &str) -> bool {
        emitted(src).is_err_and(|e| e.contains("an option of an option"))
    }

    #[test]
    fn a_declared_option_of_an_option_is_refused() {
        assert!(refused(
            "component App\n  state o = some(none)\n  action fill\n    o = some(some(5))\n  view\n    button \"f\" press=fill\n"
        ));
    }

    #[test]
    fn some_of_a_known_option_is_refused() {
        // A mutation's slot, an action's parameter, `at` of a list of options.
        assert!(refused("component App\n  mutation m as shape number\n  derive d = match some(m) { case some(x) => true, case none => false }\n  view\n    text `${d}`\n"));
        assert!(refused("component App\n  state n = 0\n  action f(p: option<number>)\n    n = match some(p) { case some(x) => 1, case none => 0 }\n  view\n    button \"f\" press=f(none)\n"));
        assert!(refused("shape B\n  v: option<number>\n\ncomponent App\n  resource xs = load() as shape list<B>\n  derive vs = map(xs, x => x.v)\n  derive g = match at(vs, 0) { case some(v) => 1, case none => 0 }\n  view\n    text `${g}`\n"));
    }

    #[test]
    fn plain_options_build() {
        assert_eq!(emitted("component App\n  mutation m as shape number\n  derive d = match m { case some(x) => some(x + 1), case none => none }\n  view\n    text \"x\"\n"), Ok(()));
    }
}
