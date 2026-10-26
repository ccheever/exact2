//! The bake's two refusals for `orientationlock` (LLP 1133.029 D3): a lock
//! written inside a `dialog` or popover, which counts on no host because
//! whether the box is open is the host's state; and a literal type whose
//! orientations share none with the manifest's `orientation`, which no
//! device can hold, since a lock narrows the manifest and never widens it.
//! A lock the walk cannot see (one in a component a dialog uses) is the
//! runner's to journal (`exact_runner::orientation`).

use super::{CompileError, Manifest};
use contract_syntax::{Expr, File, Node, Span};
use exact_runner::orientation::{types, ALL};
use std::path::Path;

/// Every `orientationlock` in `file`: its value as written and whether it
/// lies inside a `dialog` or popover of its own component.
fn locks(file: &File) -> Vec<(&Expr, Span, bool)> {
    fn walk<'a>(nodes: &'a [Node], boxed: bool, out: &mut Vec<(&'a Expr, Span, bool)>) {
        for n in nodes {
            match n {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => {
                    let boxed =
                        boxed || tag == "dialog" || attrs.iter().any(|a| a.name == "popover");
                    for a in attrs.iter().filter(|a| a.name == "orientationlock") {
                        out.push((&a.value, a.span, boxed));
                    }
                    walk(children, boxed, out);
                }
                Node::Use { children, .. } => walk(children, boxed, out),
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, boxed, out);
                    walk(otherwise, boxed, out);
                }
                Node::Each { body, .. } => walk(body, boxed, out),
                Node::Match { some, none, .. } => {
                    walk(&some.1, boxed, out);
                    walk(none, boxed, out);
                }
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    for c in &file.components {
        walk(&c.view, false, &mut out);
    }
    out
}

/// The orientations the manifest's `orientation` allows on every device,
/// or `None` where it does not bound a type the same way on each (`natural`
/// is portrait on a phone and landscape on most tablets) or says nothing.
fn bound(manifest: &Manifest) -> Option<(u8, String)> {
    let word = manifest.json.get("orientation")?.as_str()?;
    types(word).map(|set| (set, word.to_owned()))
}

/// Each `orientationlock` in `file` against the app at `app_root` (only the
/// top-layer check when there is no manifest).
pub(super) fn check(file: &File, app_root: Option<&Path>) -> Result<(), Vec<CompileError>> {
    let refusal = |id: &str, message: String, span: Span| CompileError {
        pass: "bake",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    };
    let found = locks(file);
    if found.is_empty() {
        return Ok(());
    }
    let manifest = match app_root.filter(|root| root.join("app.json").is_file()) {
        Some(root) => Some(
            Manifest::read(root)
                .map_err(|message| vec![refusal("app-manifest", message, Span::default())])?,
        ),
        None => None,
    };
    let bound = manifest.as_ref().and_then(bound);
    let mut errors = Vec::new();
    for (value, span, boxed) in found {
        if boxed {
            errors.push(refusal(
                "orientationlock-in-top-layer",
                "`orientationlock` inside a `dialog` or popover holds nothing on any host: whether the box is open is the host's state, not the tree's. A dialog over a locked screen leaves its lock as it is; put the lock on the screen's content".into(),
                span,
            ));
            continue;
        }
        let (Expr::Str(word, _), Some((allowed, said))) = (value, &bound) else {
            continue;
        };
        if types(word).is_some_and(|set| set != ALL && set & allowed == 0) {
            errors.push(refusal(
                "orientationlock-outside-manifest",
                format!("`orientationlock=\"{word}\"` shares no orientation with the manifest's `\"orientation\": \"{said}\"`, and a lock narrows the manifest, never widens it (iOS refuses a screen outside the app's Info.plist). Widen the manifest's `orientation` (to `\"any\"`) and lock the screens that need {said} with `orientationlock=\"{said}\"`"),
                span,
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
