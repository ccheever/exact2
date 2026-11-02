//! A media element's commands by HTML's method names (podcast F8, F18):
//! `fastSeek(id, seconds)` seeks every time it runs, where the bound
//! `currentTime` seeks only when its value changes, and `load(id)` loads the
//! source again (a retry after an `error`, or a file that changed).
//! `requestPictureInPicture(id)` floats an `image` and `exitPictureInPicture()`
//! ends it (LLP 1042 §9); `presentation="window"` or `"panel"` asks for a
//! floating window Exact draws in place of the platform's, with its `title`,
//! `width` and `height` (LLP 1042 §9.1).
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
    let positional: Vec<_> = args
        .iter()
        .filter(|a| !matches!(a, Expr::NamedArg(..)))
        .collect();
    let named = args.len() - positional.len();
    if positional.len() != wants.len() || (named > 0 && name != "requestPictureInPicture") {
        return wrong(span);
    }
    if named > 0 {
        window_options(args, scope, shapes)?;
    }
    for (arg, want) in positional.into_iter().zip(wants) {
        if want.unify(&infer(arg, scope, shapes)?).is_none() {
            return wrong(arg.span());
        }
    }
    Ok(())
}

/// `requestPictureInPicture`'s options by name, each once: `presentation`
/// is the literal `"window"` or `"panel"`, and `title`, `width` and
/// `height` go with it.
fn window_options(args: &[Expr], scope: &Scope, shapes: &Shapes) -> Result<(), TypeError> {
    const USAGE: &str = "`requestPictureInPicture(\"element-id\", presentation=\"window\", title=\"…\", width=480, height=320)`";
    let mut seen = std::collections::BTreeSet::new();
    let mut window = false;
    for arg in args {
        let Expr::NamedArg(name, value, at) = arg else {
            continue;
        };
        if !seen.insert(name.as_str()) {
            return err(
                "type-media-command",
                format!("`{name}` is given twice"),
                *at,
            );
        }
        let ok = match name.as_str() {
            "presentation" => {
                window = matches!(value.as_ref(), Expr::Str(s, _) if s == "window" || s == "panel");
                window
            }
            "title" => infer(value, scope, shapes)?.is_string(),
            "width" | "height" => infer(value, scope, shapes)? == Ty::Number,
            _ => {
                return err(
                    "type-media-command",
                    format!("`requestPictureInPicture` has no option `{name}`: {USAGE}"),
                    *at,
                )
            }
        };
        if !ok {
            return err(
                "type-media-command",
                format!("`presentation` is the literal \"window\" or \"panel\", `title` a string, `width` and `height` numbers of points: {USAGE}"),
                *at,
            );
        }
    }
    if !window {
        let at = args.iter().find_map(|a| match a {
            Expr::NamedArg(_, _, at) => Some(*at),
            _ => None,
        });
        return err(
            "type-media-command",
            format!("`title`, `width` and `height` describe Exact's window: they go with `presentation=\"window\"` or `\"panel\"`: {USAGE}"),
            at.unwrap_or_default(),
        );
    }
    Ok(())
}
