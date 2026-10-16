//! `lower-width-padding` (LLP 1116.009 D5): a box written `width="100%"`
//! with horizontal padding or a border, under the default
//! `box-sizing: content-box`. The percentage is of the content, so the box
//! is as wide as its parent plus its padding and runs past it, and past the
//! phone's right side when the parent is the screen. Focus-timer and
//! recipe-finder, round 11, wrote `column padding=24 width="100%"` for a
//! page's body, which clipped the right side of a phone while `verify` said
//! nothing. The repair is one of two words: drop the width (a block or a
//! stretched flex item fills its parent), or say `box-sizing="border-box"`.
//!
//! Only what is certain is named. The width is exactly `100%` (a smaller
//! percentage may fit); the box is a container; its parent takes the
//! percentage of its own content (a block, or a column: a row's items
//! shrink to fit, a grid's resolve against a cell); a padding or border
//! that is written with a positive size. A computed value, a `class` the
//! compiler would have to look through for an unnamed style, a positioned
//! box, or a `box-sizing` written either way is the benefit of the doubt.

use super::BOXES;
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, Span, StyleDecl};
use std::collections::BTreeSet;

/// How a node lays out the children the walk gives it.
#[derive(Clone, Copy, PartialEq)]
enum Parent {
    /// A block, or a column that stretches its items: a child's percentage
    /// width is of this box's content.
    Fills,
    /// Anything else, or nothing known: a row, a grid, a component's slot.
    Other,
}

/// The tags whose children a percentage width is of; `scroll` is a block
/// with overflow, `form` a block.
fn container(tag: &str) -> bool {
    BOXES.contains(&tag) || matches!(tag, "scroll" | "form")
}

/// The rows a node is drawn with: its `class`'s style rows, then its own,
/// which win by coming last. `None` when the class is a choice or names no
/// style the file declares, which the walk cannot see through.
fn rows<'a>(attrs: &'a [Attr], styles: &'a [StyleDecl]) -> Option<Vec<&'a Attr>> {
    let mut all: Vec<&Attr> = Vec::new();
    for class in attrs.iter().filter(|a| a.name == "class") {
        let Expr::Ident(name, _) = &class.value else {
            return None;
        };
        all.extend(&styles.iter().find(|s| &s.name == name)?.attrs);
    }
    all.extend(attrs.iter().filter(|a| a.name != "class"));
    Some(all)
}

/// The last row named `name` among `rows`.
fn last<'a>(rows: &[&'a Attr], name: &str) -> Option<&'a Attr> {
    rows.iter().rev().find(|a| a.name == name).copied()
}

/// How a parent lays out its children: from its tag, and a `display` or
/// `flex-direction` it writes. A `list`'s sections are the grouped list's,
/// whose rows lowering places itself.
fn parent_of(tag: &str, rows: &[&Attr], in_list: bool) -> Parent {
    if !container(tag) || (tag == "section" && in_list) {
        return Parent::Other;
    }
    let text = |name: &str| match last(rows, name) {
        Some(Attr {
            value: Expr::Str(v, _),
            ..
        }) => Some(Some(v.as_str())),
        Some(_) => None,
        None => Some(None),
    };
    let (Some(display), Some(direction)) = (text("display"), text("flex-direction")) else {
        return Parent::Other;
    };
    let column = match direction {
        Some(d) => d.starts_with("column"),
        None => tag == "column",
    };
    match display {
        None if matches!(tag, "row" | "column") => {
            if column {
                Parent::Fills
            } else {
                Parent::Other
            }
        }
        None | Some("block") | Some("flow-root") => Parent::Fills,
        Some("flex") | Some("inline-flex") if column => Parent::Fills,
        _ => Parent::Other,
    }
}

/// What a side's padding or border is, as far as the walk can tell.
#[derive(Clone, Copy, PartialEq)]
enum Is {
    /// Written as nothing, or not written.
    Nothing,
    /// Written with a size above zero.
    Some,
    /// Written, but not as a length this reads (a computed value, `env()`).
    Unknown,
}

/// A CSS length written plainly: a number, or a number with `px`, `rem`,
/// `em` or `%`.
fn length(token: &str) -> Is {
    let digits = token
        .strip_suffix("px")
        .or_else(|| token.strip_suffix("rem"))
        .or_else(|| token.strip_suffix("em"))
        .or_else(|| token.strip_suffix('%'))
        .unwrap_or(token);
    match digits.parse::<f64>() {
        Ok(n) if n > 0.0 => Is::Some,
        Ok(_) => Is::Nothing,
        Err(_) => Is::Unknown,
    }
}

/// A border's width: a length, drawn only with a line style beside it
/// (`1px solid …`); `none` or no style draws nothing.
fn border(tokens: &[&str]) -> Is {
    if tokens.iter().any(|t| matches!(*t, "none" | "hidden")) {
        return Is::Nothing;
    }
    let styled = tokens.iter().any(|t| {
        matches!(
            *t,
            "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge"
        )
    });
    match tokens.first().map(|t| length(t)) {
        Some(Is::Some) if styled => Is::Some,
        Some(Is::Nothing) | None => Is::Nothing,
        _ if styled => Is::Unknown,
        _ => Is::Nothing,
    }
}

/// Whether the rows write a horizontal padding or border that is certainly
/// more than nothing, in the order they are written: a later `padding-left=0`
/// takes back an earlier `padding=24`. A side written with a value this does
/// not read stays unknown, which is not certain.
fn inset(rows: &[&Attr]) -> bool {
    // Left padding, right padding, left border, right border.
    let mut sides = [Is::Nothing; 4];
    for a in rows {
        let tokens: Vec<&str> = match &a.value {
            Expr::Number(n, _) => {
                let is = if *n > 0.0 { Is::Some } else { Is::Nothing };
                match a.name.as_str() {
                    "padding" | "padding-inline" => sides[..2].fill(is),
                    "padding-left" | "padding-inline-start" => sides[0] = is,
                    "padding-right" | "padding-inline-end" => sides[1] = is,
                    _ => {}
                }
                continue;
            }
            Expr::Str(v, _) => v.split_whitespace().collect(),
            _ => {
                // A computed value: unknown for every side the row names.
                match a.name.as_str() {
                    "padding" | "padding-inline" => sides[..2].fill(Is::Unknown),
                    "padding-left" | "padding-inline-start" => sides[0] = Is::Unknown,
                    "padding-right" | "padding-inline-end" => sides[1] = Is::Unknown,
                    "border" | "border-inline" => sides[2..].fill(Is::Unknown),
                    "border-left" => sides[2] = Is::Unknown,
                    "border-right" => sides[3] = Is::Unknown,
                    _ => {}
                }
                continue;
            }
        };
        let at = |i: usize| tokens.get(i).map_or(Is::Unknown, |t| length(t));
        match (a.name.as_str(), tokens.len()) {
            // `padding: all`, `v h`, `t h b`, `t r b l`.
            ("padding", 1) => sides[..2].fill(at(0)),
            ("padding", 2 | 3) => sides[..2].fill(at(1)),
            ("padding", 4) => (sides[1], sides[0]) = (at(1), at(3)),
            ("padding", _) => sides[..2].fill(Is::Unknown),
            ("padding-inline", 1) => sides[..2].fill(at(0)),
            ("padding-inline", 2) => (sides[0], sides[1]) = (at(0), at(1)),
            ("padding-left" | "padding-inline-start", 1) => sides[0] = at(0),
            ("padding-right" | "padding-inline-end", 1) => sides[1] = at(0),
            ("border" | "border-inline", _) => sides[2..].fill(border(&tokens)),
            ("border-left", _) => sides[2] = border(&tokens),
            ("border-right", _) => sides[3] = border(&tokens),
            _ => {}
        }
    }
    sides.contains(&Is::Some)
}

/// `lower-width-padding` over the whole view.
pub(super) fn width_padding(view: &[Node], styles: &[StyleDecl], out: &mut Vec<LowerError>) {
    let mut seen = BTreeSet::new();
    walk(view, Parent::Other, false, styles, &mut seen, out);
}

fn walk(
    nodes: &[Node],
    parent: Parent,
    in_list: bool,
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
                let Some(all) = rows(attrs, styles) else {
                    walk(children, Parent::Other, in_list, styles, seen, out);
                    continue;
                };
                if parent == Parent::Fills && container(tag) && wide(&all) {
                    flag(*span, seen, out);
                }
                walk(
                    children,
                    parent_of(tag, &all, in_list),
                    in_list || tag == "list",
                    styles,
                    seen,
                    out,
                );
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, parent, in_list, styles, seen, out);
                walk(otherwise, parent, in_list, styles, seen, out);
            }
            Node::Each { body, .. } => walk(body, parent, in_list, styles, seen, out),
            Node::Match { some, none, .. } => {
                walk(&some.1, parent, in_list, styles, seen, out);
                walk(none, parent, in_list, styles, seen, out);
            }
            Node::Use { children, .. } => walk(children, Parent::Other, in_list, styles, seen, out),
            Node::Children { .. } => {}
        }
    }
}

/// Whether the rows make a content-box container exactly as wide as its
/// parent's content, with padding or a border on top. What the compiler
/// added to an element is not the author's: only the written rows say the
/// width and the insets.
fn wide(rows: &[&Attr]) -> bool {
    let written: Vec<&Attr> = rows.iter().copied().filter(|a| a.written).collect();
    let full = matches!(
        last(&written, "width"),
        Some(Attr { value: Expr::Str(v, _), .. }) if v.trim().strip_suffix('%').and_then(|n| n.trim().parse::<f64>().ok()) == Some(100.0)
    );
    let flowing = match last(rows, "position") {
        None => true,
        Some(Attr {
            value: Expr::Str(v, _),
            ..
        }) => matches!(v.as_str(), "static" | "relative"),
        Some(_) => false,
    };
    // A `box-sizing` written either way is the author's own word; a box
    // placed out of the flow is not of its parent's content.
    full && last(rows, "box-sizing").is_none() && flowing && shown(rows) && inset(&written)
}

/// `display="none"` shows nothing to overflow.
fn shown(rows: &[&Attr]) -> bool {
    !matches!(
        last(rows, "display"),
        Some(Attr { value: Expr::Str(v, _), .. }) if v == "none"
    )
}

fn flag(span: Span, seen: &mut BTreeSet<(u32, u32, u32, u32)>, out: &mut Vec<LowerError>) {
    // A component used twice is inlined twice: one line for its source.
    if !seen.insert((span.source_id, span.line, span.col, span.end_col)) {
        return;
    }
    out.push(LowerError {
        id: "lower-width-padding",
        message: "`width=\"100%\"` with padding is wider than its parent by the padding (the width is of the content, as in CSS's `content-box`), so it runs past its parent and, on a phone, past the screen's right side: drop the width (a block or a stretched flex item fills its parent; give a `flex=1` field `min-width=0`) or say `box-sizing=\"border-box\"`".into(),
        span,
    });
}
