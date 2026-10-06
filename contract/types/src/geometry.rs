//! Geometry reads (LLP 1051.000 D1/D2): `frame(id)` and `measure(id)`
//! answer a `Geometry`, only inside an action, and `measure`'s id is a string
//! literal, so the runner can answer it before the action's body runs.
//! `elementFromPoint(x, y)` (LLP 1094 D10) reads the same boxes, so it is an
//! action's read too.
//!
//! A derive or a view that read layout would feed layout back into the tree
//! it lays out, the loop LLP 1039 D5 declines. An action reads once, at the
//! event, and decides.

use super::{err, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};
use exact_plan::Stdlib;

/// The record both reads answer, in the order the runner builds it:
/// the border box, then whether it is provisional or unavailable.
pub(super) fn declare(shapes: &mut Shapes) {
    shapes.map.insert(
        "Geometry".into(),
        vec![
            ("x".into(), Ty::Number),
            ("y".into(), Ty::Number),
            ("width".into(), Ty::Number),
            ("height".into(), Ty::Number),
            ("provisional".into(), Ty::Bool),
            ("unavailable".into(), Ty::Bool),
        ],
    );
}

/// Refuse a geometry read outside an action, and a computed id for `measure`.
pub(super) fn check_call(
    f: Stdlib,
    args: &[Expr],
    scope: &Scope,
    span: Span,
) -> Result<(), TypeError> {
    if !matches!(
        f,
        Stdlib::Frame | Stdlib::Measure | Stdlib::ElementFromPoint
    ) {
        return Ok(());
    }
    if !scope.in_action() {
        return err(
            "type-geometry-outside-action",
            format!(
                "`{}` reads layout, which only an action may do: a derive, a view or a `fn` that read it would feed layout back into the tree it lays out (LLP 1051.000 D2, LLP 1039 D5)",
                f.name()
            ),
            span,
        );
    }
    if f == Stdlib::Measure {
        if let Some(arg) = args.first().filter(|a| !matches!(a, Expr::Str(..))) {
            return err(
                "type-measure-literal-id",
                "`measure`'s id is written as a string literal: the runner answers it before the action runs (LLP 1051.000 D2)",
                arg.span(),
            );
        }
    }
    Ok(())
}
