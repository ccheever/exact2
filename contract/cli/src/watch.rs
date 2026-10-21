//! Watch admission (LLP 1126.002 D9): what the watch's live driver draws and
//! takes as input. The schema's `watch` column admits a prop or style row,
//! as `snapshot` does for a widget; absence refuses it at bake, with the edit
//! to make, so the watch never meets a node it cannot place.

use crate::CompileError;
use contract_lower::tags::{attr_valued, AttrTarget};
use contract_syntax::{Attr, Expr, File, Node, Span, TemplatePart};
use exact_kernel::{PropId, StyleId};
use std::{collections::HashSet, sync::OnceLock};

/// The elements the driver draws: the snapshot driver's, and what a live
/// session adds (a scroller, a button, a text field).
const TAGS: &[&str] = &[
    "view", "box", "row", "column", "text", "image", "link", "main", "section", "header", "footer",
    "article", "aside", "hr", "time", "progress", "scroll", "button", "input",
];

/// The events the driver delivers: a tap, a field's text, a scroller's
/// offset, and the platform's Back to a navigation root.
const HANDLERS: &[&str] = &["press", "input", "scroll", "navigate"];

fn column(table: &str, name: &str) -> HashSet<String> {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../kernel/tables/schema.json"))
            .expect("schema JSON");
    schema[table]
        .as_array()
        .expect("schema rows")
        .iter()
        .filter_map(|row| {
            let mark = row["watch"].as_str()?;
            assert_eq!(mark, "admit", "unknown watch admission");
            Some(row[name].as_str().expect("row name").to_string())
        })
        .collect()
}

/// Whether the watch driver admits a style row.
pub fn admits(row: StyleId) -> bool {
    static COLUMN: OnceLock<HashSet<String>> = OnceLock::new();
    COLUMN
        .get_or_init(|| column("styles", "field"))
        .contains(row.name())
}

/// Whether the watch driver admits a prop row.
pub fn admits_prop(row: PropId) -> bool {
    static COLUMN: OnceLock<HashSet<String>> = OnceLock::new();
    COLUMN
        .get_or_init(|| column("props", "name"))
        .contains(row.name())
}

fn refusal(id: &str, message: String, span: Span) -> CompileError {
    CompileError {
        pass: "watch",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    }
}

fn visit(nodes: &[Node], f: &mut impl FnMut(&Node)) {
    for n in nodes {
        f(n);
        match n {
            Node::Element { children, .. } | Node::Use { children, .. } => visit(children, f),
            Node::When {
                then, otherwise, ..
            } => {
                visit(then, f);
                visit(otherwise, f);
            }
            Node::Each { body, .. } => visit(body, f),
            Node::Match { some, none, .. } => {
                visit(&some.1, f);
                visit(none, f);
            }
            Node::Children { .. } => {}
        }
    }
}

/// A length the driver's style reader cannot resolve: `calc()` or `env()`,
/// which the kernel lays out but the drawn box does not take.
fn calculated(e: &Expr) -> bool {
    match e {
        Expr::Str(s, _) => s.contains("calc(") || s.contains("env("),
        Expr::Template(parts, _) => parts.iter().any(
            |p| matches!(p, TemplatePart::Text(s) if s.contains("calc(") || s.contains("env(")),
        ),
        Expr::Ternary(_, a, b, _) => calculated(a) || calculated(b),
        _ => false,
    }
}

fn attrs(tag: &str, attrs: &[Attr], errors: &mut Vec<CompileError>) {
    for a in attrs {
        let fail = |id, edit: &str| refusal(id, format!("`{}` on `{tag}`: {edit}", a.name), a.span);
        if a.name == "hatch" {
            errors.push(fail(
                "watch-hatch",
                "the watch has no native hatch; remove it from the watch entry",
            ));
            continue;
        }
        match attr_valued(&a.name, &a.value) {
            Some(AttrTarget::Handler(event)) if !HANDLERS.contains(&event) => errors.push(fail(
                "watch-handler",
                "the watch delivers press, input, scroll and the platform's Back; remove this handler from the watch entry",
            )),
            Some(AttrTarget::Surface | AttrTarget::MediaMetadata) => errors.push(fail(
                "watch-surface",
                "the watch has no GPU surface or media player; remove it from the watch entry",
            )),
            Some(AttrTarget::Prop(p) | AttrTarget::InvertedBoolProp(p)) if !admits_prop(p) => {
                errors.push(fail("watch-prop", "the watch driver does not draw this prop; remove it from the watch entry"));
            }
            Some(AttrTarget::Styles(_) | AttrTarget::Shorthand | AttrTarget::Flex) => {
                if let Ok(rows) = contract_lower::style_values(&a.name, &a.value) {
                    if rows.iter().any(|(row, _)| !admits(*row)) {
                        errors.push(fail("watch-row", "the watch driver cannot draw this row; remove it from the watch entry"));
                    }
                }
                if calculated(&a.value) {
                    errors.push(fail("watch-calc", "the watch driver draws boxes at plain lengths; write points or a percentage, not calc() or env()"));
                }
            }
            _ => {}
        }
    }
}

/// Every refusal in a watch entry's file, each with the edit that admits it.
pub fn check(file: &File) -> Result<(), Vec<CompileError>> {
    let mut errors = Vec::new();
    for c in &file.components {
        visit(&c.view, &mut |n| {
            if let Node::Element {
                tag,
                attrs: a,
                span,
                ..
            } = n
            {
                if !TAGS.contains(&tag.as_str()) {
                    errors.push(refusal(
                        "watch-tag",
                        format!("`{tag}` has no watch drawing; replace it with boxes, text, an image, a button, a field or a scroller"),
                        *span,
                    ));
                }
                attrs(tag, a, &mut errors);
            }
        });
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(src: &str) -> Vec<String> {
        let file = contract_syntax::parse(src).expect("parses");
        check(&file)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect()
    }

    #[test]
    fn caltrains_watch_entry_is_admitted() {
        let src = include_str!("../../../apps/caltrain/watch.contract");
        assert_eq!(ids(src), Vec::<String>::new());
    }

    #[test]
    fn what_the_driver_cannot_draw_or_deliver_is_refused() {
        let ok = "component A\n  action go\n    nothing()\n  view\n    scroll width=\"100%\" height=\"100%\"\n      button press=go padding=8 border-radius=10\n        text \"Go\"\n";
        assert_eq!(ids(ok), Vec::<String>::new());
        let bad = "component A\n  action go\n    nothing()\n  view\n    column width=\"calc(100% - 8px)\" hover=go\n      canvas width=10\n      text \"hi\" box-shadow=\"0 1px #000\"\n";
        assert_eq!(
            ids(bad),
            ["watch-calc", "watch-handler", "watch-tag", "watch-row"]
        );
    }
}
