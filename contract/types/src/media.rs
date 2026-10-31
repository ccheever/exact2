//! A media element's commands by HTML's method names (podcast F8, F18):
//! `fastSeek(id, seconds)` seeks every time it runs, where the bound
//! `currentTime` seeks only when its value changes, and `load(id)` loads the
//! source again (a retry after an `error`, or a file that changed).
//! `requestPictureInPicture(id)` floats an `image` and `exitPictureInPicture()`
//! ends it (LLP 1042 §9).
use super::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

pub(super) fn command_args(
    name: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    let (usage, wants): (&str, &[Ty]) = match name {
        "fastSeek" => (
            "`fastSeek(\"element-id\", seconds)`",
            &[Ty::String, Ty::Number],
        ),
        "requestPictureInPicture" => ("`requestPictureInPicture(\"element-id\")`", &[Ty::String]),
        "exitPictureInPicture" => ("`exitPictureInPicture()`", &[]),
        _ => ("`load(\"element-id\")`", &[Ty::String]),
    };
    let wrong = |at| {
        err(
            "type-media-command",
            if wants.is_empty() {
                format!("{usage} takes no arguments: one image floats at a time")
            } else {
                format!("{usage}: the element's `id`, as `focus` takes one")
            },
            at,
        )
    };
    if args.len() != wants.len() || args.iter().any(|a| matches!(a, Expr::NamedArg(..))) {
        return wrong(span);
    }
    for (arg, want) in args.iter().zip(wants) {
        if want.unify(&infer(arg, scope, shapes)?).is_none() {
            return wrong(arg.span());
        }
    }
    Ok(())
}
