//! `failed(x)` and `failure(x)` (LLP 1054.000.002, LLP 1109 D3): whether
//! resource `x`'s latest request failed without an answer, and why. Like
//! `pending`, each reads a resource's status, not its value, so its argument
//! is a name.

use super::{err, Ref, Scope, Shapes, Ty, TypeError};
use contract_syntax::Expr;
use contract_syntax::Span;

/// The record `failure(x)` answers, in the order the runner builds it: a
/// code from the closed vocabulary the grammar lists, then the message the
/// agent's `state.failed` shows.
pub(super) fn declare(shapes: &mut Shapes) {
    shapes.map.insert(
        "Failure".into(),
        vec![("code".into(), Ty::String), ("message".into(), Ty::String)],
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
        return Some(err(id, format!("`{name}(x)` names one resource"), span));
    };
    Some(match scope.lookup(target) {
        Some((Ref::Resource(_), _)) => Ok(answer),
        Some((Ref::Mutation(_), _)) => err(
            id,
            format!("`{target}` is a mutation, and `{name}` takes a resource: a mutation whose request fails without an answer keeps its previous value and its `then` does not run, so answer a domain result (`{{ ok: false, message }}`) to show the failure"),
            *tspan,
        ),
        _ => err(id, format!("`{target}` is not a resource"), *tspan),
    })
}
