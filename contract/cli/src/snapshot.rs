//! Snapshot admission (LLP 1126 D2–D7). Only the compiler reads the schema's
//! column; absence refuses a row before an archive can silently omit it.

mod activities;
mod entry;
mod inline;
mod paragraphs;
mod refresh;
mod values;
pub use refresh::{bake_app, check_refreshes, entries, source_plan};

/// Find the props slot by this name; a route URL may add a preceding router slot.
pub const PROPS_SLOT: &str = "snapshot_props";
/// Find an activity's typed state slot by this name.
pub const STATE_SLOT: &str = "snapshot_state";
pub use activities::{compile_activity, payload_bytes, shape_digest, state_schema};
#[cfg(test)]
mod activity_tests;
#[cfg(test)]
mod tests;

/// The reserved inject a component reads the family's size from
/// (`inject` with `snapshot_viewport: <shape>`): the entry's wrapper answers
/// it with `exactViewport()`, so the render stays pure (LLP 1126 D3).
pub const VIEWPORT_SLOT: &str = "snapshot_viewport";
pub use entry::{compile, compile_entries, write_entries, Declaration, Entry};

use crate::CompileError;
use contract_lower::tags::{attr_valued, AttrTarget};
use contract_syntax::{Attr, Expr, File, Node, Span, TemplatePart};
use exact_kernel::{PropId, StyleId};
use std::{collections::HashSet, sync::OnceLock};

fn column(table: &str, name: &str) -> HashSet<String> {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../kernel/tables/schema.json"))
            .expect("schema JSON");
    schema[table]
        .as_array()
        .expect("schema rows")
        .iter()
        .filter_map(|row| {
            let mark = row["snapshot"].as_str()?;
            assert_eq!(mark, "admit", "unknown snapshot admission");
            Some(row[name].as_str().expect("row name").to_string())
        })
        .collect()
}

/// Whether the snapshot driver admits a style row.
pub fn admits(row: StyleId) -> bool {
    static COLUMN: OnceLock<HashSet<String>> = OnceLock::new();
    COLUMN
        .get_or_init(|| column("styles", "field"))
        .contains(row.name())
}

/// Whether the snapshot driver admits a prop row.
pub fn admits_prop(row: PropId) -> bool {
    static COLUMN: OnceLock<HashSet<String>> = OnceLock::new();
    COLUMN
        .get_or_init(|| column("props", "name"))
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

fn expression(e: &Expr, file: &File, depth: usize, errors: &mut Vec<CompileError>) {
    // Bound traversal through helpers; the type checker diagnoses recursion.
    if depth > 64 {
        return;
    }
    if matches!(e, Expr::Call(name, _, _) if name == "performanceNow") {
        errors.push(refusal("snapshot-clock", "a snapshot cannot read the clock; pass an instant from refresh to `time datetime=departure.at` or `progress` over an interval", e.span()));
    }
    let mut recurse = |child: &Expr| expression(child, file, depth + 1, errors);
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

fn attrs(tag: &str, attrs: &[Attr], errors: &mut Vec<CompileError>) {
    for a in attrs {
        let fail = |id, edit: &str| refusal(id, format!("`{}` on `{tag}`: {edit}", a.name), a.span);
        if a.name == "hatch" {
            errors.push(fail("snapshot-interactive", "a snapshot has no live interaction or native hatch; remove this attribute and use a link to open the app"));
            continue;
        }
        let target = if a.name == "title" && tag != "head" {
            Some(AttrTarget::Prop(PropId::Title))
        } else {
            attr_valued(&a.name, &a.value)
        };
        match target {
            Some(AttrTarget::Handler(_)) => errors.push(fail("snapshot-handler", "a snapshot handler cannot navigate or take input; make it a `link href=…` (button and toggle intents are stage 2)")),
            Some(AttrTarget::Styles(_) | AttrTarget::Shorthand) => values::attribute(a, errors),
            Some(AttrTarget::Surface | AttrTarget::MediaMetadata) => errors.push(fail("snapshot-request", "render only supplied props; move this work into the refresh function and use an image")),
            Some(AttrTarget::Prop(p) | AttrTarget::InvertedBoolProp(p))
                if !admits_prop(p) => {
                errors.push(fail("snapshot-prop", "this prop requires a running host; remove it from the entry"));
            }
            _ => {}
        }
    }
}

fn check(file: &File, styles: &HashSet<String>) -> Result<(), Vec<CompileError>> {
    let mut errors = Vec::new();
    for c in &file.components {
        for t in &c.tasks {
            errors.push(refusal("snapshot-timer", "a snapshot has no timer or `every`; return dated entries from the refresh function and use `time` for ticking text", t.span));
        }
        for r in &c.resources {
            // The family's size is the one fact a render reads (LLP 1126 D3):
            // the entry's wrapper declares it and the component injects it.
            if r.source == exact_runner::viewport::SOURCE {
                errors.push(refusal("snapshot-request", format!("`{}`: a snapshot declares no resource; write `inject` with `{}: <shape>` and read the family's width and height from it", r.name, VIEWPORT_SLOT), r.span));
                continue;
            }
            errors.push(refusal("snapshot-request", format!("`{}` requests data during render; move it into the refresh function and pass its answer as props", r.name), r.span));
        }
        if !c.mutations.is_empty() || !c.sequences.is_empty() || !c.persist.is_empty() {
            errors.push(refusal("snapshot-effect", "a snapshot render is read-only; move mutations, sequences and persisted writes into the refresh function", c.span));
        }
        for b in c.states.iter().chain(&c.derives).chain(&c.provides) {
            if b.name == PROPS_SLOT || b.name == STATE_SLOT {
                errors.push(refusal(
                    "snapshot-props-name",
                    format!(
                        "rename `{}`; the snapshot reader reserves that binding for entry data",
                        b.name
                    ),
                    b.span,
                ));
            }
            expression(&b.expr, file, 0, &mut errors);
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
                if tag == "image" {
                    if let Some(src) = a
                        .iter()
                        .find(|a| a.name == "src")
                        .map(|a| &a.value)
                        .or(positional.first())
                    {
                        if !local_image(src) {
                            errors.push(refusal("snapshot-request", "a snapshot image cannot fetch; the refresh must download it into the app group, then use an `app:/` path, a bundle-relative path, or a `symbol:` source", src.span()));
                        }
                    }
                }
                for class in a.iter().filter(|a| a.name == "class") {
                    if let Expr::Ternary(cond, _, _, _) = &class.value {
                        expression(cond, file, 0, &mut errors);
                    }
                }
                for e in positional
                    .iter()
                    .chain(a.iter().filter(|a| a.name != "class").map(|a| &a.value))
                {
                    expression(e, file, 0, &mut errors);
                }
            }
            Node::Use { args, .. } => {
                for a in args {
                    expression(&a.value, file, 0, &mut errors);
                }
            }
            Node::When { cond, .. } => expression(cond, file, 0, &mut errors),
            Node::Each { list, key, .. } => {
                expression(list, file, 0, &mut errors);
                expression(key, file, 0, &mut errors);
            }
            Node::Match { subject, .. } => expression(subject, file, 0, &mut errors),
            _ => {}
        });
    }
    for s in file.styles.iter().filter(|s| styles.contains(&s.name)) {
        attrs(&format!("style {}", s.name), &s.attrs, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn local_image(e: &Expr) -> bool {
    fn path(s: &str) -> bool {
        if let Some(symbol) = s.strip_prefix("symbol:") {
            return !symbol.is_empty();
        }
        let s = s.strip_prefix("app:/").unwrap_or(s);
        !s.is_empty()
            && !s.starts_with('/')
            && !s.contains([':', '\\'])
            && !s.split('/').any(|part| part == "..")
    }
    match e {
        Expr::Str(s, _) => path(s),
        Expr::Ternary(_, yes, no, _) => local_image(yes) && local_image(no),
        // A fixed local prefix cannot turn into a network scheme when props
        // change; no literal part may climb or escape a directory. What a prop
        // carries at run time the driver confines to the bundle or the group.
        Expr::Template(parts, _) => {
            let literal_ok = parts.iter().all(|part| match part {
                TemplatePart::Text(t) => {
                    !t.contains('\\') && !t.split('/').any(|piece| piece == "..")
                }
                _ => true,
            });
            literal_ok
                && matches!(parts.first(), Some(TemplatePart::Text(prefix))
                    if prefix == "symbol:" || prefix.starts_with("symbol:") || (prefix.ends_with('/') && path(prefix)))
        }
        _ => false,
    }
}

fn referenced_styles(file: &File) -> (HashSet<String>, Vec<CompileError>) {
    let mut styles = HashSet::new();
    let mut errors = Vec::new();
    for component in &file.components {
        visit_nodes(&component.view, &mut |node| {
            let Node::Element { attrs, .. } = node else {
                return;
            };
            for attr in attrs.iter().filter(|a| a.name == "class") {
                match &attr.value {
                    Expr::Ident(name, _) => {
                        styles.insert(name.clone());
                    }
                    Expr::Ternary(_, yes, no, _) => match (&**yes, &**no) {
                        (Expr::Ident(a, _), Expr::Ident(b, _)) => {
                            styles.extend([a.clone(), b.clone()]);
                        }
                        _ => errors.push(refusal(
                            "snapshot-class",
                            "choose named styles with `class=(condition ? A : B)`",
                            attr.span,
                        )),
                    },
                    _ => errors.push(refusal(
                        "snapshot-class",
                        "write a style name or `class=(condition ? A : B)`",
                        attr.span,
                    )),
                }
            }
        });
    }
    (styles, errors)
}
