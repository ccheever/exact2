//! Expression codegen: one assembler, every construct.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{BinOp, Expr, TemplatePart, UnOp};
use contract_types::{infer, Ref, Scope, Ty};
use exact_plan::asm::Asm;
use exact_plan::{Opcode, Stdlib};

/// Emit `e` onto `asm` in `scope`. `locals` counts inline-`match` bindings in
/// force, so nested ones index the VM's locals stack correctly.
pub(crate) fn compile(
    l: &mut Lowerer<'_>,
    asm: &mut Asm,
    e: &Expr,
    scope: &Scope,
    locals: &mut u16,
) -> Result<(), LowerError> {
    match e {
        Expr::Number(n, _) => {
            asm.number(*n);
        }
        Expr::Str(s, _) => {
            let id = l.b.str(s);
            asm.str(id);
        }
        Expr::Bool(b, _) => {
            asm.bool(*b);
        }
        Expr::None(_) => {
            asm.simple(Opcode::None);
        }
        Expr::Some(inner, _) => {
            compile(l, asm, inner, scope, locals)?;
            asm.simple(Opcode::Some);
        }
        Expr::Template(parts, _) => {
            let mut first = true;
            for p in parts {
                match p {
                    TemplatePart::Text(t) => {
                        let id = l.b.str(t);
                        asm.str(id);
                    }
                    TemplatePart::Expr(x) => {
                        compile(l, asm, x, scope, locals)?;
                        if infer(x, scope, &l.types.shapes).ok() != Some(Ty::String) {
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
        }
        Expr::Ident(name, span) => match scope.lookup(name) {
            Some((Ref::Slot(i), _)) => {
                asm.load_slot(l.slots[i as usize]);
            }
            Some((Ref::Derive(i), _)) => {
                asm.load_derive(l.derives[i as usize]);
            }
            Some((Ref::Resource(i), _)) => {
                asm.load_resource(l.resources[i as usize]);
            }
            Some((Ref::Mutation(i), _)) => {
                asm.load_slot(l.mutation_slots[i as usize]);
            }
            Some((Ref::Param(i), _)) => {
                asm.load_param(i as u16);
            }
            Some((Ref::Item(d), _)) => {
                asm.load_item(d as u16);
            }
            Some((Ref::Bound(d), _)) => {
                asm.load_bound(d as u16);
            }
            Some((Ref::Local(i), _)) => {
                asm.load_local(i as u16);
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
            let t = infer(obj, scope, &l.types.shapes).map_err(|e| LowerError {
                id: "lower-type",
                message: e.message,
                span: e.span,
            })?;
            let Ty::Record(shape) = t else {
                return err("lower-not-a-record", format!("`{t}` has no fields"), *span);
            };
            let Some((index, _)) = l.types.shapes.field(&shape, field) else {
                return err(
                    "lower-unknown-field",
                    format!("`{shape}` has no field `{field}`"),
                    *span,
                );
            };
            compile(l, asm, obj, scope, locals)?;
            asm.field(index as u16);
        }
        Expr::Call(name, args, span) => {
            if name == "path" && !l.fns.contains_key(name) {
                let template = l.path_expr(args, *span, scope)?;
                return compile(l, asm, &template, scope, locals);
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
                return Ok(());
            }
            if let Some(f) = l.fns.get(name).cloned() {
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
                let param_tys = l.types.shapes.fns[name].0.clone();
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
                let body = compile(l, asm, &f.body, &inner, locals);
                l.fn_depth -= 1;
                body?;
                for _ in &f.params {
                    *locals -= 1;
                    asm.drop_local();
                }
                return Ok(());
            }
            let Some(f) = Stdlib::from_name(name) else {
                return err(
                    "lower-unknown-function",
                    format!("`{name}` is not in the stdlib roster"),
                    *span,
                );
            };
            for a in args {
                compile(l, asm, a, scope, locals)?;
            }
            asm.call(f);
        }
        Expr::Unary(op, inner, _) => {
            compile(l, asm, inner, scope, locals)?;
            asm.simple(match op {
                UnOp::Neg => Opcode::Neg,
                UnOp::Not => Opcode::Not,
            });
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
                }
                _ => {
                    compile(l, asm, a, scope, locals)?;
                    compile(l, asm, b, scope, locals)?;
                    let ta = infer(a, scope, &l.types.shapes).ok();
                    asm.simple(match op {
                        BinOp::Add if ta == Some(Ty::String) => Opcode::Concat,
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
                }
            }
        }
        Expr::Ternary(c, a, b, _) => {
            compile(l, asm, c, scope, locals)?;
            let otherwise = asm.label();
            let end = asm.label();
            asm.jump_if_false(otherwise);
            compile(l, asm, a, scope, locals)?;
            asm.jump(end);
            asm.place(otherwise);
            compile(l, asm, b, scope, locals)?;
            asm.place(end);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            let bound_ty = match infer(subject, scope, &l.types.shapes) {
                Ok(Ty::Option(t)) => *t,
                _ => Ty::Unknown,
            };
            compile(l, asm, subject, scope, locals)?;
            let is_none = asm.label();
            let end = asm.label();
            asm.jump_if_none(is_none);
            asm.simple(Opcode::Unwrap);
            asm.bind_local();
            let index = *locals;
            *locals += 1;
            let mut inner = scope.clone();
            inner.push(vec![(var.clone(), Ref::Local(index as u32), bound_ty)]);
            compile(l, asm, some, &inner, locals)?;
            *locals -= 1;
            asm.drop_local();
            asm.jump(end);
            asm.place(is_none);
            asm.simple(Opcode::Pop);
            compile(l, asm, none, scope, locals)?;
            asm.place(end);
        }
    }
    Ok(())
}
