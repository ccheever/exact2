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

/// The events the driver delivers: a tap, a field's text, and the
/// platform's Back to a navigation root. A scroller's offset reaches the
/// runner, but its `scroll` and `scrollend` are not dispatched yet.
const HANDLERS: &[&str] = &["press", "input", "navigate"];

/// A horizontal scroller: the driver scrolls vertically only. A value it
/// cannot read at bake may be one.
fn scrolls_across(row: StyleId, value: &Expr) -> bool {
    row.name() == "overflow_x"
        && match value {
            Expr::Str(s, _) => matches!(s.trim(), "scroll" | "auto"),
            _ => true,
        }
}

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
                "the watch delivers press, input and the platform's Back; remove this handler from the watch entry",
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
                    } else if rows.iter().any(|(row, value)| scrolls_across(*row, value)) {
                        errors.push(fail("watch-scroll-x", "the watch scrolls vertically only; write overflow-y, or overflow-x=\"hidden\""));
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
    // The driver stacks one tab's routes (LLP 1126.002 §4: tabs are stage 4's).
    if let Some(routes) = &file.routes {
        if routes.rows.iter().filter(|r| r.tab).count() > 1 {
            errors.push(refusal(
                "watch-tabs",
                "the watch shows one tab's routes; declare one `tab` in the watch entry's routes"
                    .into(),
                routes.span,
            ));
        }
    }
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
        let scrolled = "component A\n  action go\n    nothing()\n  view\n    column scroll=go overflow-x=\"scroll\"\n      text \"hi\"\n";
        assert_eq!(ids(scrolled), ["watch-handler", "watch-scroll-x"]);
        let clipped = "component A\n  view\n    column overflow=\"hidden\" data-privacy=\"private\"\n      link href=\"/stations\"\n        text \"Stations\"\n";
        assert_eq!(ids(clipped), Vec::<String>::new());
        let tabs = "routes nav\n  tab home \"/\"\n  tab more \"/more\"\ncomponent A\n  view\n    text \"hi\"\n";
        assert_eq!(ids(tabs), ["watch-tabs"]);
    }

    /// An app directory holding `files`, removed on drop.
    struct App(std::path::PathBuf);
    impl App {
        fn new(files: &[(&str, &str)]) -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("exact-watch-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("app.json"),
                r#"{"name":"W","app":{"id":"dev.exact.w","name":"W"}}"#,
            )
            .unwrap();
            for (name, src) in files {
                std::fs::write(dir.join(name), src).unwrap();
            }
            Self(dir)
        }
        fn compile(&self, component: &str) -> Vec<String> {
            let entry = crate::manifest::WatchEntry {
                source: "watch.contract".into(),
                component: component.into(),
            };
            crate::compile_watch(&self.0, &entry)
                .err()
                .unwrap_or_default()
                .into_iter()
                .map(|e| e.id)
                .collect()
        }
    }
    impl Drop for App {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_named_component_must_be_the_files_first() {
        let app = App::new(&[(
            "watch.contract",
            "component Root\n  view\n    text \"hi\"\n",
        )]);
        assert_eq!(app.compile("Root"), Vec::<String>::new());
        assert_eq!(app.compile("Other"), ["watch-component"]);
    }

    #[test]
    fn a_used_files_components_are_admitted_too() {
        let app = App::new(&[
            (
                "watch.contract",
                "use Card from \"./card.contract\"\ncomponent Root\n  view\n    Card()\n",
            ),
            (
                "card.contract",
                "component Card\n  view\n    canvas width=10\n",
            ),
        ]);
        assert_eq!(app.compile("Root"), ["watch-tag"]);
    }
}
