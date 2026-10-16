//! `failed(x)` and `failure(x)` (LLP 1054.000.002, LLP 1109 D3): whether
//! resource `x`'s latest request failed without an answer, and why. Like
//! `pending`, each reads a resource's status, not its value, so its argument
//! is a name.

use super::{err, Ref, Scope, Shapes, Ty, TypeError};
use contract_syntax::Expr;
use contract_syntax::Span;

/// The record `failure(x)` answers, in the order the runner builds it: a
/// code from the closed vocabulary the grammar lists, the message the
/// agent's `state.failed` shows, then the reason the source's error named (a
/// Snapback refusal's code, else empty; the author's ruling of 2026-10-12).
pub(super) fn declare(shapes: &mut Shapes) {
    shapes.map.insert(
        "Failure".into(),
        vec![
            ("code".into(), Ty::String),
            ("message".into(), Ty::String),
            ("reason".into(), Ty::String),
        ],
    );
}

/// The type of `failed(x)` (`bool`) or `failure(x)` (`option<Failure>`), or
/// `None` when `name` is neither.
pub(super) fn check(
    name: &str,
    args: &[Expr],
    span: Span,
    scope: &Scope,
) -> Option<Result<Ty, TypeError>> {
    let (id, answer) = match name {
        "failed" => ("type-failed-argument", Ty::Bool),
        "failure" => (
            "type-failure-argument",
            Ty::Option(Box::new(Ty::Record("Failure".into()))),
        ),
        _ => return None,
    };
    let [Expr::Ident(target, tspan)] = args else {
        return Some(err(
            id,
            format!("`{name}(x)` names one resource or mutation"),
            span,
        ));
    };
    Some(match scope.lookup(target) {
        Some((Ref::Resource(_) | Ref::Mutation(_), _)) => Ok(answer),
        _ => err(
            id,
            format!("`{target}` is not a resource or mutation"),
            *tspan,
        ),
    })
}
