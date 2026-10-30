//! Calls of actions (LLP 1089): a call's arguments against its callee's
//! parameters, cut where the call's own begin (D8); a call is a statement
//! and gives no value, and an action is never an argument (D1, D10).

use super::{checks, err, infer, routes, ComponentTypes, Ref, Scope, Shapes, Ty, TypeError, Types};
use contract_syntax::inline::calls::{shown, MARK};
use contract_syntax::{one_spelling_edit, Component, Expr, Span, Stmt, TemplatePart, UnOp};

/// The prefix of a lifted action's capture parameters.
pub(crate) const CAPTURE: &str = "@capture:";

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
        if !checks::accepts_expr(pt, arg, &t, scope, shapes) {
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

/// Whether this action can refine a callee, following the same statement
/// positions as `each_call`, never a call's body (the callee's own).
fn has_call(body: &[Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        Stmt::Call { .. } => true,
        Stmt::If {
            then, otherwise, ..
        } => has_call(then) || has_call(otherwise),
        Stmt::Match { some, none, .. } => has_call(&some.1) || has_call(none),
        Stmt::Let { .. }
        | Stmt::Assign { .. }
        | Stmt::Command { .. }
        | Stmt::Send { .. }
        | Stmt::Refresh { .. } => false,
    })
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
    let mut declarations: Option<Scope> = None;
    for ai in 0..c.actions.len() {
        if !has_call(&c.actions[ai].body) {
            continue;
        }
        let mut scope = declarations
            .get_or_insert_with(|| types.component_scope(c, ct))
            .clone();
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
                    if ct.actions[ci][at].contains_enum() {
                        continue;
                    }
                    if let Some(u) = ct.actions[ci][at].unify(&t) {
                        if ct.actions[ci][at] != u {
                            ct.actions[ci][at] = u;
                            // The next caller sees newly learned types; this
                            // caller keeps the scopes it started with.
                            declarations = None;
                        }
                    }
                }
            }
        }
    }
}

/// `-exact-label` unquoted, which reads as a minus and an unknown name: a
/// platform token is a string. Not when a name in scope ends at a later
/// hyphen (`-exact-a-exact-b`): that reads as a subtraction missing its
/// spaces, which the unknown name's own refusal says. The span runs from
/// the minus through the name.
pub(super) fn unquoted_token(
    op: UnOp,
    inner: &Expr,
    scope: &Scope,
    minus: Span,
) -> Option<TypeError> {
    let (UnOp::Neg, Expr::Ident(name, at)) = (op, inner) else {
        return None;
    };
    let subtraction = name
        .match_indices('-')
        .any(|(i, _)| i > "exact".len() && scope.lookup(&name[..i]).is_some());
    if !name.starts_with("exact-") || scope.lookup(name).is_some() || subtraction {
        return None;
    }
    let span = match at.line == minus.line {
        true => Span {
            end_col: at.end_col,
            ..minus
        },
        false => *at,
    };
    Some(TypeError {
        id: "type-unknown-name",
        message: format!("unknown name `{name}`: `-{name}` reads as a minus and a name; if it is a platform token, it is a string: write `\"-{name}\"`"),
        span,
    })
}

// Suggestions change only a refusal's text. Global functions have authored
// names here; lifted child actions do not, so scoped actions only disambiguate.
pub(super) fn unknown_function(
    name: &str,
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> TypeError {
    if name == "now" {
        return TypeError {
            id: "type-now-renamed",
            message: contract_syntax::idioms::NOW_REFUSED.into(),
            span,
        };
    }
    // The web's list operations and number formatters Contract refuses
    // (LLP 1017.003 §Diagnostics) say what to do instead.
    if let Some(why) = contract_syntax::idioms::refusal(name) {
        return TypeError {
            id: "type-refused-idiom",
            message: why,
            span,
        };
    }
    // @ref LLP 1116.014 D1 — an `empty()` outside a resource's `else` is a list's `[]`.
    if name == crate::placeholder::EMPTY {
        return TypeError {
            id: "type-unknown-function",
            message: "write `[]` for an empty list; `empty()` is only a resource's `else`".into(),
            span,
        };
    }
    let mut message = format!(
        "`{name}` is not in the stdlib roster and is not an action; data comes from a `resource`"
    );
    // The runner's own sources (the `SOURCE` of runner/src/time.rs, viewport.rs, page.rs,
    // delivery.rs, surface_record.rs, transport.rs, midi.rs), called like functions (r34 t7:
    // `exactTime().epochAtZero`).
    if matches!(
        name,
        "exactTime"
            | "exactViewport"
            | "exactPage"
            | "exactDelivery"
            | "exactSurface"
            | "exactTransport"
            | "exactMIDI"
            | "exactActivities"
    ) {
        let call = if name == "exactSurface" {
            "exactSurface(\"world\")".to_string()
        } else {
            format!("{name}()")
        };
        message.push_str(&format!(
            "; `{name}` is a source the runner answers: declare `resource r = {call} as shape S` in the \
             root component, with a shape S of the fields you read, and read `r.field`"
        ));
    } else if let Some(candidate) = similar_function(name, scope, shapes) {
        // A method's name is offered as the method (LLP 1116.015 D2).
        let candidate = match candidate {
            "length" => "x.length".to_string(),
            c if contract_syntax::idioms::is_method(c) => format!("x.{c}(…)"),
            c => c.to_string(),
        };
        message.push_str(&format!("; did you mean `{candidate}`?"));
    }
    TypeError {
        id: "type-unknown-function",
        message,
        span,
    }
}

fn similar_function<'a>(name: &str, scope: &'a Scope, shapes: &'a Shapes) -> Option<&'a str> {
    if !name.is_ascii() || !(3..=64).contains(&name.len()) {
        return None;
    }
    let global = |candidate: &str| {
        matches!(candidate, "pending" | "failed" | "failure")
            || (candidate == "path" && shapes.routes.is_some())
            || shapes.fns.contains_key(candidate)
            || super::Stdlib::from_name(candidate)
                .is_some_and(|f| super::routes::require_table(f, shapes, Span::default()).is_ok())
    };
    let names = shapes
        .fns
        .keys()
        .map(String::as_str)
        .chain(super::Stdlib::ALL.iter().map(|f| f.name()))
        .chain(["pending", "failed", "failure", "path"]);
    let mut found = None;
    for candidate in names {
        if !one_spelling_edit(name.as_bytes(), candidate.as_bytes()) || !global(candidate) {
            continue;
        }
        if found.is_some_and(|previous| previous != candidate) {
            return None;
        }
        found = Some(candidate);
    }
    let candidate = found?;
    for frame in &scope.frames {
        for (scoped, _, _) in &frame.names {
            // Expansion can append instance suffixes. The stem is only a
            // conservative ambiguity veto, never an offered correction.
            let authored = scoped.split('#').next().unwrap();
            if authored != candidate
                && one_spelling_edit(name.as_bytes(), authored.as_bytes())
                && matches!(
                    scope.lookup(scoped),
                    Some((Ref::Action(_) | Ref::Prop(_), Ty::Action(_)))
                )
            {
                return None;
            }
        }
    }
    Some(candidate)
}

#[test]
fn call_refinement_preserves_declaration_order() {
    for (actions, expected) in [
        (
            "  action seed\n    relay(7)\n    relay(9)\n  action relay(n)\n    helper(n)\n",
            Ty::Number,
        ),
        (
            "  action relay(n)\n    helper(n)\n  action seed\n    relay(7)\n",
            Ty::Unknown,
        ),
    ] {
        let source = format!(
            "component App\n{actions}  action helper(n)\n    let value = n\n  view\n    text \"ok\"\n"
        );
        let parsed = contract_syntax::parse(&source).unwrap();
        let (file, errors) = contract_syntax::inline::calls::expand_file(&parsed);
        assert!(errors.is_empty(), "{errors:?}");
        let c = &file.components[0];
        let mut ct = ComponentTypes {
            name: c.name.clone(),
            actions: c
                .actions
                .iter()
                .map(|a| vec![Ty::Unknown; a.params.len()])
                .collect(),
            ..ComponentTypes::default()
        };
        crate::calls::refine(c, &mut ct, &Types::default());
        let parameter = |name: &str| {
            let ai = c.actions.iter().position(|a| a.name == name).unwrap();
            ct.actions[ai][0].clone()
        };
        assert_eq!(parameter("relay"), Ty::Number, "{source}");
        assert_eq!(parameter("helper"), expected, "{source}");
    }
}

#[test]
fn call_refinement_keeps_each_callers_original_scope() {
    // Action arguments are refused later. Their inferred types still keep
    // the current caller's snapshot and observe earlier callers' learning.
    let source = r#"component App
  action seed
    relay(7)
    retain(relay)
  action later
    retainLater(relay)
  action relay(n)
    let value = n
  action retain(value)
    let result = value
  action retainLater(value)
    let result = value
  view
    text "ok"
"#;
    let parsed = contract_syntax::parse(source).unwrap();
    let (file, errors) = contract_syntax::inline::calls::expand_file(&parsed);
    assert!(errors.is_empty(), "{errors:?}");
    let c = &file.components[0];
    let mut ct = ComponentTypes {
        name: c.name.clone(),
        actions: c
            .actions
            .iter()
            .map(|a| vec![Ty::Unknown; a.params.len()])
            .collect(),
        ..ComponentTypes::default()
    };
    crate::calls::refine(c, &mut ct, &Types::default());
    assert_eq!(ct.actions[2], vec![Ty::Number]);
    assert_eq!(ct.actions[3], vec![Ty::Action(vec![Ty::Unknown])]);
    assert_eq!(ct.actions[4], vec![Ty::Action(vec![Ty::Number])]);
    assert!(crate::check(&parsed, |_| false)
        .err()
        .expect("action arguments must be refused")
        .message
        .contains("a call passes values"));
}
