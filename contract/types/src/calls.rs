//! Calls of actions (LLP 1089): a call's arguments against its callee's
//! parameters, cut where the call's own begin (D8); a call is a statement
//! and gives no value, and an action is never an argument (D1, D10).

use super::{checks, err, infer, routes, ComponentTypes, Ref, Scope, Shapes, Ty, TypeError, Types};
use contract_syntax::inline::calls::{shown, MARK};
use contract_syntax::{Component, Expr, Span, Stmt, TemplatePart};

/// The prefix of a lifted action's capture parameters.
const CAPTURE: &str = "@capture:";

/// The action `e` calls for its value, if it is a call of an action, an
/// action prop or an injected action: what `infer` would type as an action
/// reference (`fn`s and the router's verbs first).
fn action_head<'e>(e: &'e Expr, scope: &Scope, shapes: &Shapes) -> Option<(&'e str, Span)> {
    let Expr::Call(name, args, span) = e else {
        return None;
    };
    if shapes.fns.contains_key(name) || routes::value_call(name, args, scope, shapes) {
        return None;
    }
    matches!(
        scope.lookup(name),
        Some((Ref::Action(_) | Ref::Prop(_), Ty::Action(_)))
    )
    .then_some((name.as_str(), *span))
}

/// The first call of an action inside `e`, at any depth.
fn inside<'e>(e: &'e Expr, scope: &Scope, shapes: &Shapes) -> Option<(&'e str, Span)> {
    if let Some(found) = action_head(e, scope, shapes) {
        return Some(found);
    }
    let mut found = None;
    let mut look = |x: &'e Expr| {
        if found.is_none() {
            found = inside(x, scope, shapes);
        }
    };
    match e {
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => {}
        Expr::List(items, _) => items.iter().for_each(&mut look),
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    look(x);
                }
            }
        }
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => look(x),
        Expr::Call(_, args, _) => args.iter().for_each(look),
        Expr::Binary(_, a, b, _) => {
            look(a);
            look(b);
        }
        Expr::Ternary(a, b, c, _) => {
            look(a);
            look(b);
            look(c);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            look(subject);
            look(some);
            look(none);
        }
        Expr::Let { value, body, .. } => {
            look(value);
            look(body);
        }
        Expr::Arrow { body, .. } => look(body),
    }
    found
}

/// An action called for its value in one of `stmt`'s expressions: `let x =
/// save()`, or a call inside an expression (`type-call-value`). A call's
/// own arguments are [`action_arg`]'s.
pub(super) fn called_for_value(stmt: &Stmt, scope: &Scope, shapes: &Shapes) -> Option<TypeError> {
    let exprs: Vec<&Expr> = match stmt {
        Stmt::Let { expr, .. } | Stmt::Assign { expr, .. } => vec![expr],
        Stmt::If { cond: e, .. } | Stmt::Match { subject: e, .. } => vec![e],
        // A call of an action prop holds its arguments to `action_arg`.
        Stmt::Command { name, .. }
            if matches!(scope.lookup(name), Some((Ref::Prop(_), Ty::Action(_)))) =>
        {
            Vec::new()
        }
        Stmt::Send { args, .. } | Stmt::Command { args, .. } => args.iter().collect(),
        Stmt::Call { .. } | Stmt::Refresh { .. } => Vec::new(),
    };
    exprs.into_iter().find_map(|e| {
        let (name, span) = inside(e, scope, shapes)?;
        Some(TypeError {
            id: "type-call-value",
            message: format!(
                "`{name}(…)` is a call of an action: a call is a statement and returns nothing; compute values with `fn`"
            ),
            span,
        })
    })
}

/// An argument that is an action: a call's arguments are values, and an
/// action is passed only where a view binds it (`type-call-action-arg`).
fn action_arg(
    callee: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
) -> Result<(), TypeError> {
    for arg in args {
        let named = match arg {
            Expr::Ident(n, _) => matches!(
                scope.lookup(n),
                Some((Ref::Action(_) | Ref::Prop(_), Ty::Action(_)))
            ),
            other => action_head(other, scope, shapes).is_some(),
        };
        if named {
            return err(
                "type-call-action-arg",
                format!(
                    "an argument of `{callee}` is an action: a call passes values; call the action itself, or bind it where the view passes it (`go=act`)"
                ),
                arg.span(),
            );
        }
    }
    Ok(())
}

/// Each argument against its parameter's type.
fn arg_types(
    callee: &str,
    args: &[Expr],
    params: &[Ty],
    scope: &Scope,
    shapes: &Shapes,
) -> Result<(), TypeError> {
    for (arg, pt) in args.iter().zip(params) {
        let t = infer(arg, scope, shapes)?;
        if !checks::can_unify(&t, pt) {
            return err(
                "type-argument",
                format!("`{callee}` expects `{pt}`, given `{t}`"),
                arg.span(),
            );
        }
    }
    Ok(())
}

/// A call of an action prop or injected action in its component's scope,
/// before lifting resolves it (D7): its arguments are values of the types
/// its declaration gives, all of them when it gives any.
pub(super) fn prop_args(
    name: &str,
    params: &[Ty],
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    action_arg(name, args, scope, shapes)?;
    for arg in args {
        infer(arg, scope, shapes)?;
    }
    if !params.is_empty() && args.len() != params.len() {
        return err(
            "type-call-arity",
            format!(
                "`{name}` takes {} argument(s), given {}",
                params.len(),
                args.len()
            ),
            span,
        );
    }
    arg_types(name, args, params, scope, shapes)
}

/// A call's own arguments against its callee's own parameters: a lifted
/// callee's captures skipped, then the arguments curried where the action
/// was passed (D8). The callee's body is the callee's, checked as its own
/// action; a call of the same component in a lifted action was checked in
/// its component.
pub(super) fn check(
    stmt: &Stmt,
    lifted: bool,
    c: &Component,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<(), TypeError> {
    let Stmt::Call {
        action,
        args,
        authored,
        curried,
        binding,
        span,
        ..
    } = stmt
    else {
        return Ok(());
    };
    // Expansion could not resolve it, and said why.
    if action.starts_with(MARK) || (lifted && binding.is_none()) {
        return Ok(());
    }
    let (Some(callee), Some((Ref::Action(_), Ty::Action(types)))) = (
        c.actions.iter().find(|a| &a.name == action),
        scope.lookup(action),
    ) else {
        return err(
            "syntax-call-target",
            format!("`{}` is not an action", shown(action)),
            *span,
        );
    };
    let name = shown(action);
    let given = &args[args.len().saturating_sub(*authored)..];
    action_arg(name, given, scope, shapes)?;
    let captures = callee
        .params
        .iter()
        .take_while(|p| p.name.starts_with(CAPTURE))
        .count();
    let own = &types[captures.min(types.len())..];
    let open = own.len().saturating_sub(*curried);
    if own.len() != curried + authored {
        let message = match binding {
            Some(at) if *curried > 0 => format!(
                "`{name}` takes {open} argument(s) after the {curried} curried at line {}, given {authored}",
                at.line
            ),
            _ => format!("`{name}` takes {open} argument(s), given {authored}"),
        };
        return err("type-call-arity", message, *span);
    }
    arg_types(name, given, &own[*curried..], scope, shapes)
}

/// Each call in `body` (not in a call's body: that is its callee's own),
/// with the scope where it stands: what [`refine`] types parameters from.
fn each_call<'b>(
    body: &'b [Stmt],
    scope: &Scope,
    shapes: &Shapes,
    out: &mut Vec<(&'b Stmt, Scope)>,
) {
    let mut scope = scope.clone();
    for stmt in body {
        match stmt {
            Stmt::Let { name, expr, .. } => {
                let ty = infer(expr, &scope, shapes).unwrap_or(Ty::Unknown);
                scope.push(vec![(name.clone(), Ref::Local(0), ty)]);
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                each_call(then, &scope, shapes, out);
                each_call(otherwise, &scope, shapes, out);
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                let inner = match infer(subject, &scope, shapes) {
                    Ok(Ty::Option(t)) => *t,
                    _ => Ty::Unknown,
                };
                let mut arm = scope.clone();
                arm.push(vec![(some.0.clone(), Ref::Local(0), inner)]);
                each_call(&some.1, &arm, shapes, out);
                each_call(none, &scope, shapes, out);
            }
            Stmt::Call { .. } => out.push((stmt, scope.clone())),
            _ => {}
        }
    }
}

/// The calls in `c`'s action bodies give an untyped parameter of the
/// callee its type, as a handler's arguments do (D8): `action move(dr, dc)`
/// called only as `move(1, 0)`. Once, in declaration order; a type learned
/// later does not flow back (LLP 1088 §9.2).
pub(super) fn refine(c: &Component, ct: &mut ComponentTypes, types: &Types) {
    let shapes = &types.shapes;
    for ai in 0..c.actions.len() {
        let mut scope = types.component_scope(c, ct);
        scope.push(
            c.actions[ai]
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        p.name.clone(),
                        Ref::Param(i as u32),
                        ct.actions[ai][i].clone(),
                    )
                })
                .collect(),
        );
        scope.enter_action();
        let mut calls = Vec::new();
        each_call(&c.actions[ai].body, &scope, shapes, &mut calls);
        for (stmt, scope) in calls {
            let Stmt::Call {
                action,
                args,
                authored,
                curried,
                ..
            } = stmt
            else {
                continue;
            };
            let Some(ci) = c.actions.iter().position(|a| &a.name == action) else {
                continue;
            };
            // The whole list when it fits (a lifted callee's captures are
            // passed too); else the call's own, at the cut.
            let captures = c.actions[ci]
                .params
                .iter()
                .take_while(|p| p.name.starts_with(CAPTURE))
                .count();
            let (given, from) = if args.len() == ct.actions[ci].len() {
                (&args[..], 0)
            } else {
                (
                    &args[args.len().saturating_sub(*authored)..],
                    captures + curried,
                )
            };
            for (i, arg) in given.iter().enumerate() {
                let at = from + i;
                if at >= ct.actions[ci].len() {
                    break;
                }
                if let Ok(t) = infer(arg, &scope, shapes) {
                    if let Some(u) = ct.actions[ci][at].unify(&t) {
                        ct.actions[ci][at] = u;
                    }
                }
            }
        }
    }
}
