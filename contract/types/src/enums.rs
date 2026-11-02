//! Finite strings: assignment admission differs from expression unification.
use super::{checks, infer, Scope, Shapes, Ty};
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
    let child = |want: &Ty, expr: &Expr| {
        infer(expr, scope, shapes).is_ok_and(|have| accepts_expr(want, expr, &have, scope, shapes))
    };
    match (want, expr) {
        (Ty::Enum(values), Expr::Str(value, _)) => values.contains(value),
        (Ty::Option(inner), Expr::Some(value, _)) => child(inner, value),
        (Ty::List(inner), Expr::List(values, _)) => values.iter().all(|v| child(inner, v)),
        (_, Expr::Ternary(_, a, b, _)) if contains(want) => child(want, a) && child(want, b),
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
            _ => None,
        }
    }
    let mut message = format!("`{target}` is `{want}`, cannot assign `{have}`");
    if let Some(value) = rejected(want, expr) {
        message.push_str(&format!("; {value:?} is not a declared member"));
    }
    message
}
