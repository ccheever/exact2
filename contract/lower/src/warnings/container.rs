//! `lower-flex-on-block`: a flex or grid container's property written on a
//! box that is neither. `view` is `display: block`, as a bare `<div>` is,
//! and CSS gives `align-items`, `justify-content`, `flex-direction`, `gap`
//! and the rest no meaning there (CSS Flexbox 1 §5, §8; CSS Grid 2 §7; CSS
//! Box Alignment 3 §5), so the box lays its children out as blocks and
//! nothing says why. The euxy port wrote `view … align-items="center"` for
//! each step's cell, and every light sat at the cell's left edge.
//!
//! Only the certain case is named: the box's `display` is `block`, written
//! or its tag's. A computed `display`, a `class` the walk cannot see
//! through, and `none` are left alone. Box Alignment gives `align-content`
//! and `justify-items` a meaning on a block container (the children's
//! default `justify-self`, which Chrome draws), so neither is named; nor are
//! `justify-content` and the gaps on a multi-column box, which lay out its
//! columns (CSS Multi-column 2, Box Alignment 3).

use super::width::{last, rows};
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, Span, StyleDecl};
use std::collections::BTreeSet;

/// The properties of a flex container, then a grid container's, then those
/// both share.
const FLEX: &[&str] = &["flex-direction", "flex-wrap"];
const GRID: &[&str] = &[
    "grid-template-columns",
    "grid-template-rows",
    "grid-auto-flow",
];
const BOTH: &[&str] = &[
    "align-items",
    "justify-content",
    "gap",
    "row-gap",
    "column-gap",
];

/// The tags that are a block box unless they say otherwise: the plain
/// boxes and `scroll`.
fn block_tag(tag: &str) -> bool {
    super::BOXES.contains(&tag) && !matches!(tag, "row" | "column") || tag == "scroll"
}

pub(super) fn flex_on_block(view: &[Node], styles: &[StyleDecl], out: &mut Vec<LowerError>) {
    let mut seen = BTreeSet::new();
    walk(view, styles, &mut seen, out);
}

fn walk(
    nodes: &[Node],
    styles: &[StyleDecl],
    seen: &mut BTreeSet<(u32, u32, u32, u32)>,
    out: &mut Vec<LowerError>,
) {
    for node in nodes {
        match node {
            Node::Element {
                tag,
                attrs,
                children,
                span,
                ..
            } => {
                if let Some(all) = rows(attrs, styles) {
                    check(tag, &all, *span, seen, out);
                }
                walk(children, styles, seen, out);
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, styles, seen, out);
                walk(otherwise, styles, seen, out);
            }
            Node::Each { body, .. } => walk(body, styles, seen, out),
            Node::Match { some, none, .. } => {
                walk(&some.1, styles, seen, out);
                walk(none, styles, seen, out);
            }
            Node::Use { children, .. } => walk(children, styles, seen, out),
            Node::Children { .. } => {}
        }
    }
}

fn check(
    tag: &str,
    rows: &[&Attr],
    span: Span,
    seen: &mut BTreeSet<(u32, u32, u32, u32)>,
    out: &mut Vec<LowerError>,
) {
    if !block_tag(tag) {
        return;
    }
    let display = match last(rows, "display") {
        None => None,
        Some(Attr {
            value: Expr::Str(v, _),
            ..
        }) => Some(v.trim()),
        Some(_) => return,
    };
    if !matches!(display, None | Some("block")) {
        return;
    }
    let columns = rows
        .iter()
        .any(|a| matches!(a.name.as_str(), "columns" | "column-count" | "column-width"));
    let mut named: Vec<&str> = Vec::new();
    for a in rows.iter().filter(|a| a.written) {
        let name = a.name.as_str();
        let container = FLEX.contains(&name) || GRID.contains(&name) || BOTH.contains(&name);
        let multicol =
            columns && matches!(name, "gap" | "column-gap" | "row-gap" | "justify-content");
        if container && !multicol && !named.contains(&name) {
            named.push(name);
        }
    }
    if named.is_empty() || !seen.insert((span.source_id, span.line, span.col, span.end_col)) {
        return;
    }
    out.push(LowerError {
        id: "lower-flex-on-block",
        message: message(tag, display, &named),
        span,
    });
}

fn message(tag: &str, display: Option<&str>, named: &[&str]) -> String {
    let list: Vec<String> = named.iter().map(|n| format!("`{n}`")).collect();
    let (list, verb) = match list.split_last() {
        Some((last, [])) => (last.clone(), "does"),
        Some((last, rest)) => (format!("{} and {last}", rest.join(", ")), "do"),
        None => unreachable!(),
    };
    let grid = named.iter().all(|n| GRID.contains(n));
    let fix = if grid {
        "write `display=\"grid\"`".to_string()
    } else if display.is_none() && matches!(tag, "view" | "box") {
        format!("write `column` (children stacked) or `row` in place of `{tag}`, or `display=\"flex\"` (a row unless `flex-direction` says otherwise)")
    } else {
        "write `display=\"flex\"` (a row unless `flex-direction` says otherwise) or `display=\"grid\"`".to_string()
    };
    let display = display.unwrap_or("block");
    format!("{list} {verb} nothing on this `{tag}`: it is `display: {display}`, as in CSS, and they lay out the children of a flex or grid container only; {fix}")
}
