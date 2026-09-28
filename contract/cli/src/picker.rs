//! The picker's bound (LLP 1069.002 D1): every `input type="file"`'s
//! `accept` names media (`image/*`, `video/*`, one image or video subtype)
//! or a MIME type or extension the manifest's `file_handlers` declares
//! (`rules/DEFERRED.md`, widened 2026-09-27 with LLP 1069.010). Lowering
//! already refused a bound or missing `accept`, `capture` and `*/*`; the
//! types themselves are checked here, where the manifest is read. A plan
//! compiled without an app (a text, a test) admits media alone.

use super::{CompileError, Manifest};
use contract_syntax::{Expr, File, Node, Span};
use std::path::Path;

/// The `accept` lists in `file`, with where each is written.
fn accepts(file: &File) -> Vec<(String, Span)> {
    fn walk(nodes: &[Node], out: &mut Vec<(String, Span)>) {
        for n in nodes {
            match n {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => {
                    if contract_syntax::input_control(tag, attrs) == Some("file") {
                        for a in attrs.iter().filter(|a| a.name == "accept") {
                            if let Expr::Str(list, _) = &a.value {
                                out.push((list.clone(), a.span));
                            }
                        }
                    }
                    walk(children, out);
                }
                Node::Use { children, .. } => walk(children, out),
                Node::Provide { body, .. } => walk(body, out),
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, out);
                    walk(otherwise, out);
                }
                Node::Each { body, .. } => walk(body, out),
                Node::Match { some, none, .. } => {
                    walk(&some.1, out);
                    walk(none, out);
                }
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    for c in &file.components {
        walk(&c.view, &mut out);
    }
    out
}

/// The MIME types and extensions `file_handlers` declares, lowercased.
pub fn handled(manifest: &Manifest) -> Vec<String> {
    let mut out = Vec::new();
    for handler in manifest
        .json
        .get("file_handlers")
        .and_then(|h| h.as_array())
        .into_iter()
        .flatten()
    {
        for (mime, extensions) in handler
            .get("accept")
            .and_then(|a| a.as_object())
            .into_iter()
            .flatten()
        {
            out.push(mime.to_ascii_lowercase());
            for e in extensions.as_array().into_iter().flatten() {
                if let Some(e) = e.as_str() {
                    out.push(e.to_ascii_lowercase());
                }
            }
        }
    }
    out
}

/// Every file input's `accept` in `file` against the app at `app_root`
/// (media alone when there is none).
pub(super) fn check(file: &File, app_root: Option<&Path>) -> Result<(), Vec<CompileError>> {
    let used = accepts(file);
    if used.is_empty() {
        return Ok(());
    }
    let refusal = |id: &str, message: String, span: Span| CompileError {
        pass: "bake",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    };
    let declared = match app_root.filter(|root| root.join("app.json").is_file()) {
        Some(root) => Manifest::read(root)
            .map(|m| handled(&m))
            .map_err(|message| vec![refusal("app-manifest", message, Span::default())])?,
        None => Vec::new(),
    };
    let mut errors = Vec::new();
    for (list, span) in used {
        for token in contract_lower::controls::accept_tokens(&list) {
            if contract_lower::controls::media_accept(&token) || declared.contains(&token) {
                continue;
            }
            let listed = if declared.is_empty() {
                "the app's `file_handlers` declares no types".to_owned()
            } else {
                format!("its `file_handlers` declares {}", declared.join(", "))
            };
            errors.push(refusal(
                "bake-picker-accept",
                format!(
                    "`accept` names `{token}`, which is neither an image or video type nor one the app opens ({listed}): {}",
                    contract_lower::controls::PICKER_ADMISSION
                ),
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
