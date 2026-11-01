//! A browser shell's `iframe` (@ref LLP 1020.002 D1, D6, §9): the words only
//! an `iframe` takes, and what a `partition` refuses beside it. A
//! partitioned frame is a top-level page with its own storage, so nothing
//! may reach the app from it: no `sandbox` (WebKit cannot sandbox a
//! top-level page, so the host would have to fail open), no message bridge,
//! and no bundled or `app:/` source (the `exact-app` scheme a remote page
//! must never reach).
use crate::{err, LowerError};
use contract_syntax::{Attr, Expr};

/// What only an `iframe` takes.
const FRAME_ONLY: [&str; 12] = [
    "partition",
    "zoom",
    "currententrychange",
    "navigatesuccess",
    "navigateerror",
    "newwindow",
    "download",
    "capture",
    "userpromptopened",
    "apple.allowsBackForwardNavigationGestures",
    "apple.allowsMagnification",
    "apple.inactiveSchedulingPolicy",
];

/// `WKPreferences.InactiveSchedulingPolicy`'s cases.
const SCHEDULING: [&str; 3] = ["suspend", "throttle", "none"];

/// Whether `src` is a source a partitioned frame shows: an http(s) URL or
/// `about:blank`.
pub(crate) fn web_page(src: &str) -> bool {
    let lower = src.to_ascii_lowercase();
    lower == "about:blank"
        || ["http://", "https://"]
            .iter()
            .any(|scheme| lower.starts_with(scheme) && lower.len() > scheme.len())
        || lower.starts_with("//")
}

pub(crate) fn check(tag: &str, positional: &[Expr], attrs: &[Attr]) -> Result<(), LowerError> {
    if tag != "iframe" {
        return match attrs.iter().find(|a| FRAME_ONLY.contains(&a.name.as_str())) {
            Some(a) => err(
                "lower-attr-tag",
                format!("`{}` belongs to `iframe`, not `{tag}`", a.name),
                a.span,
            ),
            None => Ok(()),
        };
    }
    if let Some(a) = attrs
        .iter()
        .find(|a| a.name == "apple.inactiveSchedulingPolicy")
    {
        if let Expr::Str(v, _) = &a.value {
            if !SCHEDULING.contains(&v.as_str()) {
                return err(
                    "lower-attr-value",
                    format!("`apple.inactiveSchedulingPolicy` is `WKPreferences.InactiveSchedulingPolicy`'s: \"suspend\", \"throttle\" or \"none\", not {v:?}"),
                    a.span,
                );
            }
        }
    }
    if !attrs.iter().any(|a| a.name == "partition") {
        return Ok(());
    }
    if let Some(a) = attrs.iter().find(|a| a.name == "sandbox") {
        return err(
            "lower-iframe-partition-sandbox",
            "an `iframe` with a `partition` takes no `sandbox`: its guest is a top-level page, which WebKit cannot sandbox; drop one of the two",
            a.span,
        );
    }
    if let Some(a) = attrs.iter().find(|a| a.name == "message") {
        return err(
            "lower-iframe-partition-message",
            "an `iframe` with a `partition` has no `message`: a partitioned guest has no way to the app that a browser tab lacks",
            a.span,
        );
    }
    let src = attrs
        .iter()
        .find(|a| a.name == "src")
        .map(|a| &a.value)
        .or(positional.first());
    if let Some(Expr::Str(s, span)) = src {
        if !web_page(s) {
            return err(
                "lower-iframe-partition-src",
                format!("an `iframe` with a `partition` shows web pages: {s:?} is not an http, https or about:blank URL (a bundled or `app:/` page is the app's own; frame it without `partition`)"),
                *span,
            );
        }
    }
    Ok(())
}

/// The view's iframes, by literal id: whether each is partitioned. `None`
/// when one's id is computed, or a node is not expanded: then any id may
/// name a frame, and the host refuses a missing one into its log.
fn frame_ids(view: &[contract_syntax::Node]) -> Option<std::collections::BTreeMap<&str, bool>> {
    use contract_syntax::Node;
    let mut ids = std::collections::BTreeMap::new();
    let mut stack: Vec<&Node> = view.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                if tag == "iframe" {
                    let partitioned = attrs.iter().any(|a| a.name == "partition");
                    match attrs
                        .iter()
                        .rev()
                        .find(|a| a.name == "id")
                        .map(|a| &a.value)
                    {
                        Some(Expr::Str(id, _)) => {
                            ids.insert(id.as_str(), partitioned);
                        }
                        Some(_) => return None,
                        None => {}
                    }
                }
                stack.extend(children);
            }
            Node::When {
                then, otherwise, ..
            } => stack.extend(then.iter().chain(otherwise)),
            Node::Each { body, .. } => stack.extend(body),
            Node::Match { some, none, .. } => stack.extend(some.1.iter().chain(none)),
            Node::Use { .. } | Node::Children { .. } => return None,
        }
    }
    Some(ids)
}

/// The frame commands (@ref LLP 1020.002 D3), whose first argument is an
/// iframe's `id`; `reload` only with one (bare, it is the app's reload).
pub(crate) fn is_frame_command(name: &str, args: &[Expr]) -> bool {
    matches!(
        name,
        "navigate" | "traverseHistory" | "stopLoading" | "captureScreenshot" | "openDevTools"
    ) || name == "reload" && !args.is_empty()
}

/// Whether `path` names a file under one of the app's own roots, which a
/// capture writes (@ref LLP 1069.002 D7): `app:/data/…`, `app:/cache/…`
/// or `app:/tmp/…`, with no `.` or `..` part.
fn app_file(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("app:/") else {
        return false;
    };
    let mut parts = rest.split('/');
    matches!(parts.next(), Some("data" | "cache" | "tmp"))
        && rest.contains('/')
        && parts.all(|p| !p.is_empty() && p != "." && p != "..")
}

/// A literal id a frame command names must be an iframe's, and a
/// `postMessage` must not name a partitioned one's.
pub(crate) fn check_command(
    view: &[contract_syntax::Node],
    name: &str,
    args: &[Expr],
) -> Result<(), LowerError> {
    if name == "captureScreenshot" {
        if let Some(Expr::Str(path, at)) = args.get(1) {
            if !app_file(path) {
                return err(
                    "lower-frame-capture-path",
                    format!("`captureScreenshot` writes the app's own files: an `app:/data/…`, `app:/cache/…` or `app:/tmp/…` path, not {path:?}"),
                    *at,
                );
            }
        }
    }
    let target = match name {
        "postMessage" => args.get(1),
        _ if is_frame_command(name, args) => args.first(),
        _ => return Ok(()),
    };
    let Some(Expr::Str(id, at)) = target else {
        return Ok(());
    };
    let Some(ids) = frame_ids(view) else {
        return Ok(());
    };
    match (name, ids.get(id.as_str())) {
        ("postMessage", Some(true)) => err(
            "lower-iframe-partition-message",
            format!("`postMessage` to \"{id}\": that `iframe` has a `partition`, and a partitioned guest has no message bridge"),
            *at,
        ),
        ("postMessage", _) => Ok(()),
        (_, None) => err(
            "lower-frame-command",
            format!("`{name}(\"{id}\", …)` names no iframe: nothing in the view is an `iframe` with `id=\"{id}\"`"),
            *at,
        ),
        _ => Ok(()),
    }
}
