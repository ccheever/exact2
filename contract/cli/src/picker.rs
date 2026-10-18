//! The picker's bound (LLP 1069.002 D1): a specific `input type="file"`'s
//! `accept` names media (`image/*`, `video/*`, one image or video subtype)
//! or a MIME type or extension the manifest's `file_handlers` declares
//! (`rules/DEFERRED.md`, widened 2026-09-27 with LLP 1069.010). Lowering
//! refuses a bound `accept` and `capture`; missing, empty and `*/*` accept
//! any file. Specific types are checked here, where the manifest is read. A plan
//! compiled without an app (a text, a test) admits media and unrestricted input.

use super::{CompileError, Manifest};
use contract_syntax::{Expr, File, Node, Span, Stmt};
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
    let mut errors = missing_targets(file);
    for (list, span) in used {
        for token in contract_lower::controls::accept_tokens(&list) {
            if token == "*/*"
                || contract_lower::controls::media_accept(&token)
                || declared.contains(&token)
            {
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

// A literal target is provably absent only when no file input has a dynamic
// id. Conditional inputs can exist later; this checks declarations, not life.
fn missing_targets(file: &File) -> Vec<CompileError> {
    fn ids(nodes: &[Node], found: &mut Vec<String>, dynamic: &mut bool) {
        for node in nodes {
            match node {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => {
                    if contract_syntax::input_control(tag, attrs) == Some("file") {
                        if let Some(id) = attrs.iter().find(|a| a.name == "id") {
                            match &id.value {
                                Expr::Str(id, _) => found.push(id.clone()),
                                _ => *dynamic = true,
                            }
                        }
                    }
                    ids(children, found, dynamic);
                }
                Node::Use { children, .. } => ids(children, found, dynamic),
                Node::When {
                    then, otherwise, ..
                } => {
                    ids(then, found, dynamic);
                    ids(otherwise, found, dynamic);
                }
                Node::Each { body, .. } => ids(body, found, dynamic),
                Node::Match { some, none, .. } => {
                    ids(&some.1, found, dynamic);
                    ids(none, found, dynamic);
                }
                Node::Children { .. } => {}
            }
        }
    }
    fn targets(body: &[Stmt], found: &[String], errors: &mut Vec<CompileError>) {
        for statement in body {
            match statement {
                Stmt::Command { name, args, span } if name == "showPicker" => {
                    if let Some(Expr::Str(id, _)) = args.first() {
                        if !found.contains(id) {
                            errors.push(CompileError { pass: "bake", id: "bake-picker-id".into(),
                                message: format!("showPicker(\"{id}\") names no file input id: give input type=\"file\" id=\"{id}\"; testId is a separate identifier used by tests"),
                                span: *span, file: None, related: Box::new([]) });
                        }
                    }
                }
                Stmt::If {
                    then, otherwise, ..
                } => {
                    targets(then, found, errors);
                    targets(otherwise, found, errors);
                }
                Stmt::Match { some, none, .. } => {
                    targets(&some.1, found, errors);
                    targets(none, found, errors);
                }
                Stmt::Call { body, .. } => targets(body, found, errors),
                _ => {}
            }
        }
    }
    let (mut found, mut dynamic, mut errors) = (Vec::new(), false, Vec::new());
    for component in &file.components {
        ids(&component.view, &mut found, &mut dynamic);
    }
    if !dynamic {
        for component in &file.components {
            for action in &component.actions {
                targets(&action.body, &found, &mut errors);
            }
        }
    }
    errors
}
