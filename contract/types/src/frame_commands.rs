//! A browser shell's frame commands by the Navigation API's and WebDriver
//! BiDi's names (@ref LLP 1020.002 D3): `navigate(id, url)`,
//! `traverseHistory(id, delta)`, `reload(id)` or `reload(id,
//! ignoreCache=true)`, and `stopLoading(id)`, each by the iframe's `id`.
//! Bare `reload()` stays the development host's reboot. And WebDriver
//! BiDi's `captureScreenshot(id, path, clip=)` (D7), whose `clip` is
//! Geometry's `DOMRect` (or a `DOMRectReadOnly`, whose first four fields
//! are the same) in the guest's CSS pixels. And Electron's `<webview>`
//! `openDevTools(id)` (D11).
use super::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

pub(super) fn is(name: &str) -> bool {
    matches!(
        name,
        "navigate"
            | "traverseHistory"
            | "reload"
            | "stopLoading"
            | "captureScreenshot"
            | "openDevTools"
    )
}

pub(super) fn command_args(
    name: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    if name == "reload" && args.is_empty() {
        return Ok(());
    }
    let usage = match name {
        "navigate" => "`navigate(\"frame-id\", url)`: the iframe's `id` and the URL to load",
        "traverseHistory" => "`traverseHistory(\"frame-id\", delta)`: the iframe's `id` and how far to move through its history, -1 back, 1 forward",
        "reload" => "`reload()` reloads the app; `reload(\"frame-id\")` or `reload(\"frame-id\", ignoreCache=true)` reloads that iframe's page",
        "captureScreenshot" => "`captureScreenshot(\"frame-id\", \"app:/tmp/shot.png\")` or `captureScreenshot(\"frame-id\", path, clip=DOMRect(x=…, y=…, width=…, height=…))`: the iframe's `id`, the `app:/` file to write, and a part of the guest's viewport in CSS pixels",
        "openDevTools" => "`openDevTools(\"frame-id\")`: the iframe's `id`",
        _ => "`stopLoading(\"frame-id\")`: the iframe's `id`",
    };
    let wrong = |at| err("type-frame-command", usage, at);
    let (positional, named): (Vec<&Expr>, Vec<&Expr>) =
        args.iter().partition(|a| !matches!(a, Expr::NamedArg(..)));
    let wants: &[Ty] = match name {
        "navigate" | "captureScreenshot" => &[Ty::String, Ty::String],
        "traverseHistory" => &[Ty::String, Ty::Number],
        _ => &[Ty::String],
    };
    if positional.len() != wants.len() {
        return wrong(span);
    }
    for (arg, want) in positional.iter().zip(wants) {
        if want.unify(&infer(arg, scope, shapes)?).is_none() {
            return wrong(arg.span());
        }
    }
    for arg in named {
        let Expr::NamedArg(n, value, at) = arg else {
            continue;
        };
        let ok = match (name, n.as_str()) {
            ("reload", "ignoreCache") => Ty::Bool.unify(&infer(value, scope, shapes)?).is_some(),
            ("captureScreenshot", "clip") => matches!(
                infer(value, scope, shapes)?,
                Ty::Record(shape) if shape == "DOMRect" || shape == "DOMRectReadOnly"
            ),
            _ => false,
        };
        if !ok {
            return wrong(*at);
        }
    }
    Ok(())
}
