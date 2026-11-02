//! Finite strings: assignment admission differs from expression unification.
use super::{checks, infer, Ref, Scope, Shapes, Ty};
use contract_syntax::Expr;

pub(super) fn contains(ty: &Ty) -> bool {
    match ty {
        Ty::Enum(_) => true,
        Ty::Option(t) | Ty::List(t) => contains(t),
        Ty::Action(params) => params.iter().any(contains),
        _ => false,
    }
}

/// Check literal membership recursively, retaining declared constraints.
/// Each child is inferred in its own context: mixed enum/literal lists may
/// have widened during expression inference without becoming unsafe assignments.
pub(super) fn accepts_expr(
    want: &Ty,
    expr: &Expr,
    have: &Ty,
    scope: &Scope,
    shapes: &Shapes,
) -> bool {
    let child = |want: &Ty, expr: &Expr, scope: &Scope| {
        infer(expr, scope, shapes).is_ok_and(|have| accepts_expr(want, expr, &have, scope, shapes))
    };
    match (want, expr) {
        (Ty::Enum(values), Expr::Str(value, _)) => values.contains(value),
        (Ty::Option(inner), Expr::Some(value, _)) => child(inner, value, scope),
        (Ty::List(inner), Expr::List(values, _)) => values.iter().all(|v| child(inner, v, scope)),
        (_, Expr::Ternary(_, a, b, _)) if contains(want) => {
            child(want, a, scope) && child(want, b, scope)
        }
        (
            _,
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            },
        ) if contains(want) => {
            let Ok(Ty::Option(inner)) = infer(subject, scope, shapes) else {
                return false;
            };
            let mut inner_scope = scope.clone();
            inner_scope.push(vec![(var.clone(), Ref::Local(0), *inner)]);
            child(want, some, &inner_scope) && child(want, none, scope)
        }
        _ => checks::can_unify(want, have),
    }
}

/// An assignment diagnostic keeps the rejected literal visible through wrappers.
pub(super) fn assignment_error(target: &str, want: &Ty, expr: &Expr, have: &Ty) -> String {
    fn rejected<'a>(want: &Ty, expr: &'a Expr) -> Option<&'a str> {
        match (want, expr) {
            (Ty::Enum(values), Expr::Str(value, _)) if !values.contains(value) => Some(value),
            (Ty::Option(inner), Expr::Some(value, _)) => rejected(inner, value),
            (Ty::List(inner), Expr::List(values, _)) => {
                values.iter().find_map(|v| rejected(inner, v))
            }
            (_, Expr::Ternary(_, a, b, _)) => rejected(want, a).or_else(|| rejected(want, b)),
            (_, Expr::Match { some, none, .. }) => {
                rejected(want, some).or_else(|| rejected(want, none))
            }
            _ => None,
        }
    }
    let mut message = format!("`{target}` is `{want}`, cannot assign `{have}`");
    if let Some(value) = rejected(want, expr) {
        message.push_str(&format!("; {value:?} is not a declared member"));
    }
    message
}
