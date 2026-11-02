//! `openURL(url[, id])`: the URL, and the element whose `change` hears
//! whether it opened (LLP 1133.000 §2).

use super::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

/// One or two strings, positional.
pub(super) fn args(
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    const USAGE: &str = "`openURL` takes the URL, then optionally the `id` of the element whose `change` hears `opened` or `failed`: `openURL(url)`, `openURL(url, \"opened\")`";
    if args.is_empty() || args.len() > 2 || args.iter().any(|a| matches!(a, Expr::NamedArg(..))) {
        return err("type-open-url-argument", USAGE, span);
    }
    for arg in args {
        if Ty::String.unify(&infer(arg, scope, shapes)?).is_none() {
            return err("type-open-url-argument", USAGE, arg.span());
        }
    }
    Ok(())
}
