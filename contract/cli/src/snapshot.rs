//! Snapshot admission (LLP 1126 D2–D7). Only the compiler reads the schema's
//! column; absence refuses a row before an archive can silently omit it.

mod entry;
#[cfg(test)]
mod tests;
pub use entry::{compile, compile_widgets, Entry};

use crate::CompileError;
use contract_lower::tags::{attr_valued, AttrTarget};
use contract_syntax::{Attr, Expr, File, Node, Span, TemplatePart};
use exact_kernel::StyleId;
use std::{collections::HashSet, sync::OnceLock};

/// Whether the snapshot driver admits a style row.
pub fn admits(row: StyleId) -> bool {
    static COLUMN: OnceLock<HashSet<String>> = OnceLock::new();
    COLUMN
        .get_or_init(|| {
            let schema: serde_json::Value =
                serde_json::from_str(include_str!("../../../kernel/tables/schema.json"))
                    .expect("schema JSON");
            schema["styles"]
                .as_array()
                .expect("styles")
                .iter()
                .filter_map(|r| {
                    let mark = r["snapshot"].as_str()?;
                    assert_eq!(mark, "admit", "unknown snapshot admission");
                    Some(r["field"].as_str().expect("field").to_string())
                })
                .collect()
        })
        .contains(row.name())
}

fn refusal(id: &str, message: impl Into<String>, span: Span) -> CompileError {
    CompileError {
        pass: "snapshot",
        id: id.into(),
        message: message.into(),
        span,
        file: None,
        related: Box::new([]),
    }
}

fn visit_nodes(nodes: &[Node], f: &mut impl FnMut(&Node)) {
    for n in nodes {
        f(n);
        match n {
            Node::Element { children, .. } | Node::Use { children, .. } => visit_nodes(children, f),
            Node::When {
                then, otherwise, ..
            } => {
                visit_nodes(then, f);
                visit_nodes(otherwise, f);
            }
            Node::Each { body, .. } => visit_nodes(body, f),
            Node::Match { some, none, .. } => {
                visit_nodes(&some.1, f);
                visit_nodes(none, f);
            }
            Node::Children { .. } => {}
        }
    }
}

fn now_like(name: &str) -> bool {
    matches!(
        name.replace('_', "").to_ascii_lowercase().as_str(),
        "now"
            | "nowms"
            | "nowseconds"
            | "nowepoch"
            | "clock"
            | "clockms"
            | "currenttime"
            | "currenttimestamp"
    )
}

fn expression(e: &Expr, time: bool, file: &File, depth: usize, errors: &mut Vec<CompileError>) {
    if depth > 64 {
        return;
    } // The type checker diagnoses recursive functions.
    let clock = match e {
        Expr::Ident(n, _) | Expr::Member(_, n, _) => now_like(n),
        Expr::Call(n, _, _) => matches!(
            n.as_str(),
            "performanceNow" | "exactTime" | "now" | "Date.now"
        ),
        _ => false,
    };
    if clock && !time {
        errors.push(refusal("snapshot-clock", "a snapshot cannot read the clock; replace the calculation with `time datetime=departure.at` or `progress` over an interval", e.span()));
    }
    let mut recurse = |child: &Expr| expression(child, time, file, depth + 1, errors);
    match e {
        Expr::Call(name, args, _) => {
            for a in args {
                recurse(a);
            }
            if let Some(f) = file.fns.iter().find(|f| f.name == *name) {
                recurse(&f.body);
            }
        }
        Expr::List(items, _) => {
            for e in items {
                recurse(e);
            }
        }
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(e) = p {
                    recurse(e);
                }
            }
        }
        Expr::Some(e, _)
        | Expr::Member(e, _, _)
        | Expr::NamedArg(_, e, _)
        | Expr::Unary(_, e, _)
        | Expr::Typed(e, _, _)
        | Expr::Arrow { body: e, .. } => recurse(e),
        Expr::Binary(_, a, b, _)
        | Expr::Let {
            value: a, body: b, ..
        } => {
            recurse(a);
            recurse(b);
        }
        Expr::Ternary(a, b, c, _)
        | Expr::Match {
            subject: a,
            some: b,
            none: c,
            ..
        } => {
            recurse(a);
            recurse(b);
            recurse(c);
        }
        _ => {}
    }
}

fn literal_choices(e: &Expr, allowed: &[&str]) -> bool {
    match e {
        Expr::Str(s, _) => allowed.contains(&s.as_str()),
        Expr::Ternary(_, a, b, _) => literal_choices(a, allowed) && literal_choices(b, allowed),
        _ => false,
    }
}

fn attrs(tag: &str, attrs: &[Attr], errors: &mut Vec<CompileError>) {
    for a in attrs {
        let fail = |id, edit: &str| refusal(id, format!("`{}` on `{tag}`: {edit}", a.name), a.span);
        if a.name.contains(':') || a.name == "hatch" {
            errors.push(fail("snapshot-interactive", "a snapshot has no live interaction or native hatch; remove this attribute and use a link to open the app"));
            continue;
        }
        match attr_valued(&a.name, &a.value) {
            Some(AttrTarget::Handler(_)) => errors.push(fail("snapshot-handler", "a snapshot handler cannot navigate or take input; make it a `link href=…` (button and toggle intents are stage 2)")),
            Some(AttrTarget::Styles(rows)) => {
                for row in rows {
                    if !admits(*row) {
                        errors.push(fail("snapshot-row", &format!("the snapshot driver cannot draw `{}`; remove this row from the entry", row.name().replace('_', "-"))));
                        break;
                    }
                    let allowed = match row {
                        StyleId::PositionType => Some(&["static", "relative", "absolute"][..]),
                        StyleId::OverflowX | StyleId::OverflowY => Some(&["visible", "hidden", "clip"][..]),
                        _ => None,
                    };
                    if let Some(allowed) = allowed {
                        if !literal_choices(&a.value, allowed) {
                            errors.push(fail("snapshot-value", &format!("a snapshot cannot scroll or stick; write one of {} as a literal (or literal branches)", allowed.join(", "))));
                        }
                    }
                }
            }
            Some(AttrTarget::Surface | AttrTarget::MediaMetadata) => errors.push(fail("snapshot-request", "render only supplied props; move this work into the refresh function and use an image")),
            Some(AttrTarget::Prop(p) | AttrTarget::InvertedBoolProp(p)) => {
                // The archive's leaves expose only content, accessibility and a URL.
                if !matches!(p.name(), "text" | "src" | "href" | "alt" | "testId" | "id" | "accessibilityLabel" | "accessibilityRole" | "accessibilityHidden" | "datetime" | "format" | "value" | "max" | "start" | "end") {
                    errors.push(fail("snapshot-prop", "this prop requires a running host; remove it from the entry"));
                }
            }
            _ => {}
        }
    }
}

fn check(file: &File) -> Result<(), Vec<CompileError>> {
    let mut errors = Vec::new();
    for c in &file.components {
        for t in &c.tasks {
            errors.push(refusal("snapshot-timer", "a snapshot has no timer or `every`; return dated entries from the refresh function and use `time` for ticking text", t.span));
        }
        for r in &c.resources {
            errors.push(refusal("snapshot-request", format!("`{}` requests data during render; move it into the refresh function and pass its answer as props", r.name), r.span));
        }
        if !c.mutations.is_empty() || !c.sequences.is_empty() || !c.persist.is_empty() {
            errors.push(refusal("snapshot-effect", "a snapshot render is read-only; move mutations, sequences and persisted writes into the refresh function", c.span));
        }
        for b in c.states.iter().chain(&c.derives).chain(&c.provides) {
            expression(&b.expr, false, file, 0, &mut errors);
        }
        visit_nodes(&c.view, &mut |n| match n {
            Node::Element {
                tag,
                attrs: a,
                positional,
                span,
                ..
            } => {
                if !matches!(
                    tag.as_str(),
                    "view"
                        | "box"
                        | "row"
                        | "column"
                        | "text"
                        | "image"
                        | "link"
                        | "main"
                        | "section"
                        | "header"
                        | "footer"
                        | "article"
                        | "aside"
                        | "hr"
                        | "time"
                        | "progress"
                ) {
                    let id = if contract_lower::is_module_tag(tag) {
                        "snapshot-native"
                    } else {
                        "snapshot-tag"
                    };
                    errors.push(refusal(id, format!("`{tag}` has no snapshot drawing; replace it with boxes, text, an image or a link"), *span));
                }
                attrs(tag, a, &mut errors);
                for e in positional
                    .iter()
                    .chain(a.iter().filter(|a| a.name != "class").map(|a| &a.value))
                {
                    expression(
                        e,
                        matches!(tag.as_str(), "time" | "progress"),
                        file,
                        0,
                        &mut errors,
                    );
                }
            }
            Node::Use { args, .. } => {
                for a in args {
                    expression(&a.value, false, file, 0, &mut errors);
                }
            }
            Node::When { cond, .. } => expression(cond, false, file, 0, &mut errors),
            Node::Each { list, key, .. } => {
                expression(list, false, file, 0, &mut errors);
                expression(key, false, file, 0, &mut errors);
            }
            Node::Match { subject, .. } => expression(subject, false, file, 0, &mut errors),
            _ => {}
        });
    }
    for s in &file.styles {
        attrs(&format!("style {}", s.name), &s.attrs, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
