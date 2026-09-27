//! Expression codegen: one assembler, every construct. Each construct
//! returns the type of the value it leaves, built from the checked types of
//! the names it reads, so no expression is inferred again while lowering
//! (a re-inference at every node made deep expressions quadratic).

use crate::{err, LowerError, Lowerer};
use contract_syntax::{BinOp, Expr, TemplatePart, UnOp};
use contract_types::{Ref, Scope, Ty};
use exact_plan::asm::Asm;
use exact_plan::{Opcode, Stdlib};

/// Emit `e` onto `asm` in `scope` and return its type. `locals` counts
/// inline-`match` bindings in force, so nested ones index the VM's locals
/// stack correctly.
pub(crate) fn compile(
    l: &mut Lowerer<'_>,
    asm: &mut Asm,
    e: &Expr,
    scope: &Scope,
    locals: &mut u16,
) -> Result<Ty, LowerError> {
    Ok(match e {
        Expr::Number(n, _) => {
            asm.number(*n);
            Ty::Number
        }
        Expr::Str(s, _) => {
            let id = l.b.str(s);
            asm.str(id);
            Ty::String
        }
        Expr::Bool(b, _) => {
            asm.bool(*b);
            Ty::Bool
        }
        Expr::NamedArg(_, _, span) => {
            return err(
                "lower-named-argument",
                "named arguments belong to a canvas surface binding",
                *span,
            )
        }
        Expr::None(_) => {
            asm.simple(Opcode::None);
            Ty::Option(Box::new(Ty::Unknown))
        }
        Expr::Some(inner, _) => {
            let t = compile(l, asm, inner, scope, locals)?;
            asm.simple(Opcode::Some);
            Ty::Option(Box::new(t))
        }
        Expr::Template(parts, _) => {
            // Component expansion can make every interpolated string literal.
            // Reuse should then cost the same as spelling the final text once.
            if parts.iter().all(|part| {
                matches!(
                    part,
                    TemplatePart::Text(_) | TemplatePart::Expr(Expr::Str(..))
                )
            }) {
                let text: String = parts
                    .iter()
                    .map(|part| match part {
                        TemplatePart::Text(text) | TemplatePart::Expr(Expr::Str(text, _)) => {
                            text.as_str()
                        }
                        _ => unreachable!(),
                    })
                    .collect();
                asm.str(l.b.str(&text));
                return Ok(Ty::String);
            }
            let mut first = true;
            for p in parts {
                match p {
                    TemplatePart::Text(t) => {
                        let id = l.b.str(t);
                        asm.str(id);
                    }
                    TemplatePart::Expr(x) => {
                        if compile(l, asm, x, scope, locals)? != Ty::String {
                            asm.call(Stdlib::ToString);
                        }
                    }
                }
                if !first {
                    asm.simple(Opcode::Concat);
                }
                first = false;
            }
            if parts.is_empty() {
                let id = l.b.str("");
                asm.str(id);
            }
            Ty::String
        }
        Expr::Ident(name, span) => match scope.lookup(name) {
            Some((Ref::Slot(i), t)) => {
                asm.load_slot(l.slots[i as usize]);
                t.clone()
            }
            Some((Ref::Derive(i), t)) => {
                asm.load_derive(l.derives[i as usize]);
                t.clone()
            }
            Some((Ref::Resource(i), t)) => {
                asm.load_resource(l.resources[i as usize]);
                t.clone()
            }
            Some((Ref::Mutation(i), t)) => {
                asm.load_slot(l.mutation_slots[i as usize]);
                t.clone()
            }
            Some((Ref::Param(i), t)) => {
                asm.load_param(i as u16);
                t.clone()
            }
            Some((Ref::Item(d), t)) => {
                asm.load_item(d as u16);
                t.clone()
            }
            Some((Ref::Index(d), t)) => {
                asm.load_index(d as u16);
                t.clone()
            }
            Some((Ref::Bound(d), t)) => {
                asm.load_bound(d as u16);
                t.clone()
            }
            Some((Ref::Local(i), t)) => {
                asm.load_local(i as u16);
                t.clone()
            }
            Some((Ref::Prop(_), _)) => {
                return err(
                    "lower-prop-in-root",
                    format!("`{name}` is a prop; props are inlined away"),
                    *span,
                )
            }
            Some((Ref::Action(_), _)) => {
                return err(
                    "lower-action-as-value",
                    format!("`{name}` is an action, not a value"),
                    *span,
                )
            }
            None => {
                return err(
                    "lower-unknown-name",
                    format!("unknown name `{name}`"),
                    *span,
                )
            }
        },
        Expr::Member(obj, field, span) => {
            let t = compile(l, asm, obj, scope, locals)?;
            let Ty::Record(shape) = t else {
                return err("lower-not-a-record", format!("`{t}` has no fields"), *span);
            };
            let Some((index, ty)) = l.types.shapes.field(&shape, field) else {
                return err(
                    "lower-unknown-field",
                    format!("`{shape}` has no field `{field}`"),
                    *span,
                );
            };
            let ty = ty.clone();
            asm.field(index as u16);
            ty
        }
        Expr::Call(name, args, span) => {
            if name == "path" && !l.fns.contains_key(name.as_str()) {
                let template = l.path_expr(args, *span, scope)?;
                return compile(l, asm, &template, scope, locals);
            }
            if contract_types::strings::is_text_call(name, scope) {
                return l.text_call(asm, args, *span, scope, locals);
            }
            if name == "failed" {
                let [Expr::Ident(target, _)] = args.as_slice() else {
                    return err("lower-failed", "`failed(x)` names one resource", *span);
                };
                let Some((Ref::Resource(i), _)) = scope.lookup(target) else {
                    return err(
                        "lower-failed",
                        format!("`{target}` is not a resource"),
                        *span,
                    );
                };
                asm.failed_resource(l.resources[i as usize]);
                return Ok(Ty::Bool);
            }
            if name == "pending" {
                // Typed already: one name, a resource or a mutation.
                let Some(Expr::Ident(target, _)) = args.first() else {
                    return err(
                        "lower-pending",
                        "`pending(x)` names one resource or mutation",
                        *span,
                    );
                };
                match scope.lookup(target) {
                    Some((Ref::Resource(i), _)) => asm.pending_resource(l.resources[i as usize]),
                    Some((Ref::Mutation(i), _)) => asm.pending_mutation(l.mutations[i as usize]),
                    _ => {
                        return err(
                            "lower-pending",
                            format!("`{target}` is not a resource or a mutation"),
                            *span,
                        )
                    }
                };
                return Ok(Ty::Bool);
            }
            if let Some((f, shared)) = l.fns.get(name.as_str()).copied() {
                // A `fn` (LLP 1017 P5), expanded here: each argument bound
                // as a local, the body compiled in a scope of the parameters
                // only, the locals dropped after — no new opcode, no table,
                // and (the type pass having refused a cycle) no recursion.
                if l.fn_depth > 32 {
                    return err(
                        "lower-fn-depth",
                        format!("`{name}` expands too deeply"),
                        *span,
                    );
                }
                let base = *locals;
                for a in args {
                    compile(l, asm, a, scope, locals)?;
                    asm.bind_local();
                    *locals += 1;
                }
                let param_tys = &l.types.shapes.fns[name].0;
                let mut inner = Scope::default();
                inner.push(
                    f.params
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            (
                                p.name.clone(),
                                Ref::Local((base + i as u16) as u32),
                                param_tys[i].clone(),
                            )
                        })
                        .collect(),
                );
                l.fn_depth += 1;
                let body = compile(l, asm, shared, &inner, locals);
                l.fn_depth -= 1;
                body?;
                for _ in &f.params {
                    *locals -= 1;
                    asm.drop_local();
                }
                return Ok(l.types.shapes.fns[name].1.clone());
            }
            if let (Some(f @ (Stdlib::Map | Stdlib::Filter)), [list, callback]) =
                (Stdlib::from_name(name), args.as_slice())
            {
                return callback_call(l, asm, f, list, callback, scope, locals);
            }
            let Some(f) = Stdlib::from_name(name) else {
                return err(
                    "lower-unknown-function",
                    format!("`{name}` is not in the stdlib roster"),
                    *span,
                );
            };
            let mut given = Vec::with_capacity(args.len());
            for a in args {
                given.push(compile(l, asm, a, scope, locals)?);
            }
            asm.call(f);
            match (f, given.first()) {
                // `first(list<T>)` is `option<T>` (LLP 1054.000 C4).
                (Stdlib::First, Some(Ty::List(item))) => Ty::Option(item.clone()),
                _ => Ty::from_roster(f.returns()),
            }
        }
        Expr::Unary(op, inner, _) => {
            compile(l, asm, inner, scope, locals)?;
            match op {
                UnOp::Neg => {
                    asm.simple(Opcode::Neg);
                    Ty::Number
                }
                UnOp::Not => {
                    asm.simple(Opcode::Not);
                    Ty::Bool
                }
            }
        }
        Expr::Binary(op, a, b, _) => {
            match op {
                BinOp::And | BinOp::Or => {
                    // Short-circuit: evaluate `a`; if it decides, keep it.
                    compile(l, asm, a, scope, locals)?;
                    let end = asm.label();
                    let other = asm.label();
                    // Stack: [a]. Duplicate by re-evaluating is wrong (effects are none, but cost);
                    // instead branch on a copy via BindLocal/LoadLocal.
                    asm.bind_local();
                    asm.load_local(*locals);
                    if *op == BinOp::And {
                        asm.jump_if_false(other);
                    } else {
                        asm.simple(Opcode::Not);
                        asm.jump_if_false(other);
                    }
                    asm.drop_local();
                    compile(l, asm, b, scope, locals)?;
                    asm.jump(end);
                    asm.place(other);
                    asm.load_local(*locals);
                    asm.drop_local();
                    asm.place(end);
                    Ty::Bool
                }
                _ => {
                    let ta = compile(l, asm, a, scope, locals)?;
                    let tb = compile(l, asm, b, scope, locals)?;
                    asm.simple(match op {
                        BinOp::Add if ta == Ty::String => Opcode::Concat,
                        BinOp::Add => Opcode::Add,
                        BinOp::Sub => Opcode::Sub,
                        BinOp::Mul => Opcode::Mul,
                        BinOp::Div => Opcode::Div,
                        BinOp::Rem => Opcode::Rem,
                        BinOp::Eq => Opcode::Eq,
                        BinOp::Ne => Opcode::Ne,
                        BinOp::Lt => Opcode::Lt,
                        BinOp::Le => Opcode::Le,
                        BinOp::Gt => Opcode::Gt,
                        BinOp::Ge => Opcode::Ge,
                        BinOp::And | BinOp::Or => unreachable!(),
                    });
                    match op {
                        BinOp::Add if ta == Ty::String && tb == Ty::String => Ty::String,
                        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem
                            if ta == Ty::Number && tb == Ty::Number =>
                        {
                            Ty::Number
                        }
                        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                            Ty::Unknown
                        }
                        _ => Ty::Bool,
                    }
                }
            }
        }
        Expr::Ternary(c, a, b, _) => {
            compile(l, asm, c, scope, locals)?;
            let otherwise = asm.label();
            let end = asm.label();
            asm.jump_if_false(otherwise);
            let ta = compile(l, asm, a, scope, locals)?;
            asm.jump(end);
            asm.place(otherwise);
            let tb = compile(l, asm, b, scope, locals)?;
            asm.place(end);
            ta.unify(&tb).unwrap_or(Ty::Unknown)
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            let bound_ty = match compile(l, asm, subject, scope, locals)? {
                Ty::Option(t) => *t,
                _ => Ty::Unknown,
            };
            let is_none = asm.label();
            let end = asm.label();
            asm.jump_if_none(is_none);
            asm.simple(Opcode::Unwrap);
            asm.bind_local();
            let index = *locals;
            *locals += 1;
            let mut inner = scope.clone();
            inner.push(vec![(var.clone(), Ref::Local(index as u32), bound_ty)]);
            let ta = compile(l, asm, some, &inner, locals)?;
            *locals -= 1;
            asm.drop_local();
            asm.jump(end);
            asm.place(is_none);
            asm.simple(Opcode::Pop);
            let tb = compile(l, asm, none, scope, locals)?;
            asm.place(end);
            ta.unify(&tb).unwrap_or(Ty::Unknown)
        }
        Expr::Let {
            name, value, body, ..
        } => {
            // Evaluated once, then read from the locals stack, as an inline
            // `match` binds its value.
            let ty = compile(l, asm, value, scope, locals)?;
            asm.bind_local();
            let index = *locals;
            *locals += 1;
            let mut inner = scope.clone();
            inner.push(vec![(name.clone(), Ref::Local(index as u32), ty)]);
            let ty = compile(l, asm, body, &inner, locals)?;
            *locals -= 1;
            asm.drop_local();
            ty
        }
        Expr::Arrow { span, .. } => {
            return err(
                "lower-arrow-position",
                "an arrow function is only the second argument of `map` or `filter`",
                *span,
            )
        }
    })
}

/// `map(list, callback)` or `filter(list, callback)` (LLP 1017.003 D5):
/// the list, then `Map`/`Filter` with the callback's body inline, the item
/// and its index the next two locals, which the VM binds for each run.
fn callback_call(
    l: &mut Lowerer<'_>,
    asm: &mut Asm,
    f: Stdlib,
    list: &Expr,
    callback: &Expr,
    scope: &Scope,
    locals: &mut u16,
) -> Result<Ty, LowerError> {
    let listed = compile(l, asm, list, scope, locals)?;
    let Expr::Arrow { params, body, span } = callback else {
        return err(
            "lower-arrow-position",
            format!("the second argument of `{}` is an arrow function", f.name()),
            callback.span(),
        );
    };
    let item = match &listed {
        Ty::List(item) => (**item).clone(),
        _ => Ty::Unknown,
    };
    if params.len() > 2 {
        return err(
            "lower-arrow-parameters",
            "a callback takes the item and its index",
            *span,
        );
    }
    let end = asm.label();
    asm.each_item(
        if f == Stdlib::Map {
            Opcode::Map
        } else {
            Opcode::Filter
        },
        end,
    );
    let base = *locals;
    *locals += 2;
    let mut inner = scope.clone();
    inner.push(
        params
            .iter()
            .zip([item, Ty::Number])
            .enumerate()
            .map(|(i, (p, t))| (p.clone(), Ref::Local((base + i as u16) as u32), t))
            .collect(),
    );
    let result = compile(l, asm, body, &inner, locals);
    *locals -= 2;
    let result = result?;
    asm.place(end);
    Ok(if f == Stdlib::Map {
        Ty::List(Box::new(result))
    } else {
        listed
    })
}
