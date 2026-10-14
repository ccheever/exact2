//! Geometry reads (LLP 1051.000 D1/D2): `frame(id)` and `measure(id)`
//! answer a `Geometry`, only inside an action, and `measure`'s id is a string
//! literal, so the runner can answer it before the action's body runs.
//! `elementFromPoint(x, y)` and `elementsFromPoint(x, y)` (LLP 1094 D10)
//! read the same boxes; `querySelectorAll` (LLP 1075.003 §9.1) reads the
//! mounted tree. All are action reads.
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
        Stdlib::Frame
            | Stdlib::Measure
            | Stdlib::ElementFromPoint
            | Stdlib::ElementsFromPoint
            | Stdlib::QuerySelectorAll
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
    if f == Stdlib::QuerySelectorAll {
        if let Some(Expr::Str(selector, at)) = args.first() {
            if !data_selector(selector) {
                return err(
                    "type-data-selector",
                    "`querySelectorAll` accepts only `[data-word~=\"value\"]` (LLP 1075.003 §9.1)",
                    *at,
                );
            }
        }
    }
    Ok(())
}

fn data_selector(selector: &str) -> bool {
    let Some(rest) = selector.strip_prefix("[data-") else {
        return false;
    };
    let Some((word, value)) = rest.split_once("~=\"") else {
        return false;
    };
    let Some(value) = value.strip_suffix("\"]") else {
        return false;
    };
    !word.is_empty()
        && word.split('-').all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|c| c.is_ascii_lowercase())
                && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
        && !value.is_empty()
        && !value.chars().any(|c| matches!(c, '"' | '\\' | '\n' | '\r'))
}
