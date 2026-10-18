//! `lower-scroll-grows`: a scroller that fills a column which grows with its
//! content. A flex item's automatic minimum height is its content's height,
//! and only a scroll container's is zero (CSS Flexbox §4.5,
//! <https://www.w3.org/TR/css-flexbox-1/#min-size-auto>). So in
//! `column height=400 > column flex=1 > scroll flex=1`, the middle column is
//! at least as tall as the scroller's content, the scroller fills it, and
//! nothing scrolls. `scroll flex=1` directly in the bounded column scrolls;
//! the wrapper is the footgun. The euxy port lost six minutes to one in a
//! sheet, a component whose root column had `flex=1` and no `min-height`.
//!
//! Only what is certain is named. The scroller is a `scroll`, or a box with
//! `overflow` or `overflow-y` of `auto` or `scroll`, that grows (`flex` or
//! `flex-grow` above zero) from a basis that is not a length, with no
//! `height` or `max-height`. Its wrappers are the unbroken run of flex
//! columns above it that pass their content's height up: neither a scroll
//! container, and none writes `height`, `max-height`, `aspect-ratio`,
//! `overflow`, `position` or `navigationKey` (a host sizes a route), nor caps
//! what it passes with a basis it cannot grow from under a zero minimum (CSS
//! Flexbox §9.9.3). A box with `min-height=0` still passes its content's
//! height (a box's min-content height is its content's whatever its
//! minimum), so a run needs the repair on each.
//!
//! A wrapper is named when its parent is a flex column, it writes no
//! `min-height` and it can shrink, and when the box above the run bounds its
//! height: a length `height` or `max-height`, a percentage of a definite
//! height, insets in a definite box, a route (the host sizes it to the
//! screen or the sheet), or a flex item with a zero minimum in a bounded
//! column. With no such box the page grows as a whole, and `min-height=0`
//! would change nothing. A computed value, or a `class` the walk cannot see
//! through, is the benefit of the doubt.

use super::width::{last, rows};
use super::BOXES;
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, Span, StyleDecl};
use std::collections::BTreeSet;

/// One wrapper of the run: where it is, its tag, and whether it is named.
type Wrapper<'a> = (Span, &'a str, bool);

/// What the walk knows about the element whose children it visits.
#[derive(Clone)]
struct Parent<'a> {
    /// It is certainly a flex column and not a scroll container.
    column: bool,
    /// Its height is certainly definite: a percentage of it resolves.
    definite: bool,
    /// Its height is certainly bounded: definite, or capped by a maximum.
    bounded: bool,
    /// When it passes its content's height up: it and the run of wrappers
    /// above it that do too, outermost first. Empty otherwise.
    wrappers: Vec<Wrapper<'a>>,
    /// Whether the box above that run bounds its height.
    run_bounded: bool,
}

impl Parent<'_> {
    /// What the walk knows of nothing: a use, a slot, a class it cannot read.
    fn unknown() -> Self {
        Parent {
            column: false,
            definite: false,
            bounded: false,
            wrappers: Vec::new(),
            run_bounded: false,
        }
    }
}

/// `lower-scroll-grows` over the whole view.
pub(super) fn scroll_grows(view: &[Node], styles: &[StyleDecl], out: &mut Vec<LowerError>) {
    let mut seen = BTreeSet::new();
    // The view's roots sit in the window, whose height is definite.
    let window = Parent {
        definite: true,
        bounded: true,
        ..Parent::unknown()
    };
    walk(view, &window, styles, &mut seen, out);
}

fn walk<'a>(
    nodes: &'a [Node],
    parent: &Parent<'a>,
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
                    walk(children, &Parent::unknown(), styles, seen, out);
                    continue;
                };
                let scrolls = scroll_container(tag, &all);
                if scrolls == Some(true) && grows_unbounded(&all) && parent.run_bounded {
                    for &(wrapper, wrapper_tag, named) in &parent.wrappers {
                        if named {
                            flag((wrapper, wrapper_tag), (*span, tag), seen, out);
                        }
                    }
                }
                let flex = flex_of(&all);
                let min_auto = min_auto(&all);
                let column = scrolls == Some(false) && flex_column(tag, &all);
                let zero_min = !min_auto || scrolls == Some(true);
                let definite = definite(&all, parent, &flex, zero_min);
                let bounded = definite || capped(&all, parent);
                // A basis it cannot grow from, under a zero minimum, caps
                // what it passes up (CSS Flexbox §9.9.3).
                let caps =
                    !min_auto && !flex.grow.is_some_and(|g| g > 0.0) && flex.content != Some(true);
                let (wrappers, run_bounded) = if column && passes_content(&all) && !caps {
                    let mut run = parent.wrappers.clone();
                    let named = parent.column && min_auto && flex.shrink.is_some_and(|s| s > 0.0);
                    run.push((*span, tag.as_str(), named));
                    // A run's first wrapper is bounded only as a flex item:
                    // in a block it is as tall as its content.
                    let above = if parent.wrappers.is_empty() {
                        parent.column && parent.bounded
                    } else {
                        parent.run_bounded
                    };
                    (run, above)
                } else {
                    (Vec::new(), false)
                };
                let next = Parent {
                    column,
                    definite,
                    bounded,
                    wrappers,
                    run_bounded,
                };
                walk(children, &next, styles, seen, out);
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, parent, styles, seen, out);
                walk(otherwise, parent, styles, seen, out);
            }
            Node::Each { body, .. } => walk(body, parent, styles, seen, out),
            Node::Match { some, none, .. } => {
                walk(&some.1, parent, styles, seen, out);
                walk(none, parent, styles, seen, out);
            }
            Node::Use { children, .. } => walk(children, &Parent::unknown(), styles, seen, out),
            Node::Children { .. } => {}
        }
    }
}

/// A height or maximum as written.
#[derive(PartialEq)]
enum Size {
    Length,
    Percent,
    /// Unwritten, `auto`, `none`, computed, or a function.
    Other,
}

fn size(rows: &[&Attr], name: &str) -> Size {
    match last(rows, name).map(|a| &a.value) {
        Some(Expr::Number(..)) => Size::Length,
        Some(Expr::Str(v, _)) => {
            let v = v.trim();
            let number = |s: &str| s.trim().parse::<f64>().is_ok();
            if v.strip_suffix('%').is_some_and(number) {
                Size::Percent
            } else if ["px", "rem", "em"]
                .iter()
                .any(|u| v.strip_suffix(u).is_some_and(number))
            {
                Size::Length
            } else {
                Size::Other
            }
        }
        _ => Size::Other,
    }
}

/// Whether the element's height is certainly definite. `zero_min` says its
/// minimum height is not its content's (written, or it scrolls).
fn definite(rows: &[&Attr], parent: &Parent<'_>, flex: &Flex, zero_min: bool) -> bool {
    let has = |name: &str| last(rows, name).is_some();
    let position = word(rows, "position");
    let positioned = matches!(position, Some(Some("absolute" | "fixed")));
    let in_flow = matches!(
        position,
        Some(None | Some("static" | "relative" | "sticky"))
    );
    has("navigationKey")
        || match size(rows, "height") {
            Size::Length => true,
            Size::Percent => parent.definite,
            Size::Other => false,
        }
        || (positioned && parent.definite && (has("inset") || (has("top") && has("bottom"))))
        // A flex item's size after flexing in a bounded column, which a
        // zero minimum keeps within it (CSS Flexbox §9.8).
        || (parent.column
            && parent.bounded
            && in_flow
            && zero_min
            && flex.shrink.is_some_and(|s| s > 0.0))
}

/// Whether a maximum caps the element's height.
fn capped(rows: &[&Attr], parent: &Parent<'_>) -> bool {
    match size(rows, "max-height") {
        Size::Length => true,
        Size::Percent => parent.definite,
        Size::Other => false,
    }
}

/// The literal text of the last row named `name`: `Some(None)` when it is
/// not written, `None` when it is computed.
fn word<'a>(rows: &[&'a Attr], name: &str) -> Option<Option<&'a str>> {
    match last(rows, name) {
        None => Some(None),
        Some(Attr {
            value: Expr::Str(v, _),
            ..
        }) => Some(Some(v.trim())),
        Some(_) => None,
    }
}

/// Whether the element scrolls on y: `Some(true)` certainly, `Some(false)`
/// certainly not (no `overflow` row of any kind), `None` unknown.
fn scroll_container(tag: &str, rows: &[&Attr]) -> Option<bool> {
    let y = match (word(rows, "overflow-y")?, word(rows, "overflow")?) {
        (Some(_), Some(_)) => return None,
        // `overflow: <x> <y>`.
        (y, both) => y.or(both.and_then(|v| v.split_whitespace().last())),
    };
    let x_visible = matches!(word(rows, "overflow-x")?, None | Some("visible"));
    match y {
        Some("auto" | "scroll") => Some(true),
        Some("hidden" | "clip") => Some(false),
        None if tag == "scroll" => Some(true),
        // A visible y beside a scrolling x computes to `auto` (CSS Overflow 3 §3).
        None | Some("visible") if tag != "scroll" && x_visible => Some(false),
        _ => None,
    }
}

/// Whether the element is certainly a plain box laid out as a flex column.
fn flex_column(tag: &str, rows: &[&Attr]) -> bool {
    let (Some(display), Some(direction)) = (word(rows, "display"), word(rows, "flex-direction"))
    else {
        return false;
    };
    if !BOXES.contains(&tag) {
        return false;
    }
    let flex = match display {
        None => tag == "column",
        Some(d) => matches!(d, "flex" | "inline-flex"),
    };
    let column = match direction {
        None => tag == "column",
        Some(d) => d.starts_with("column"),
    };
    flex && column
}

/// Whether a wrapper's content height is certainly its own min-content
/// height: a box in flow that writes nothing that bounds it.
fn passes_content(rows: &[&Attr]) -> bool {
    let bounds = rows.iter().any(|a| match a.name.as_str() {
        "height" => !matches!(&a.value, Expr::Str(v, _) if v.trim() == "auto"),
        "max-height" => !matches!(&a.value, Expr::Str(v, _) if v.trim() == "none"),
        "aspect-ratio" | "navigationKey" | "popover" | "overflow" | "overflow-x"
        | "overflow-y" => true,
        "position" => {
            !matches!(&a.value, Expr::Str(v, _) if matches!(v.trim(), "static" | "relative" | "sticky"))
        }
        _ => false,
    });
    !bounds
}

/// Whether a flex item's minimum height is certainly its content's: it
/// writes no `min-height`, or `auto`.
fn min_auto(rows: &[&Attr]) -> bool {
    match last(rows, "min-height") {
        None => true,
        Some(a) => matches!(&a.value, Expr::Str(v, _) if v.trim() == "auto"),
    }
}

/// Whether a scroller grows into its parent's free space from a basis that
/// is its content (`auto`, `content`, or a percentage of a height its parent
/// does not have), with nothing else bounding its height.
fn grows_unbounded(rows: &[&Attr]) -> bool {
    let bounded = rows.iter().any(|a| match a.name.as_str() {
        "height" => !matches!(&a.value, Expr::Str(v, _) if v.trim() == "auto"),
        "max-height" => !matches!(&a.value, Expr::Str(v, _) if v.trim() == "none"),
        "position" => {
            !matches!(&a.value, Expr::Str(v, _) if matches!(v.trim(), "static" | "relative" | "sticky"))
        }
        _ => false,
    });
    let flex = flex_of(rows);
    !bounded && flex.grow.is_some_and(|g| g > 0.0) && flex.content == Some(true)
}

/// A flex item's factors, each `None` when computed.
struct Flex {
    grow: Option<f64>,
    shrink: Option<f64>,
    /// Whether its basis is its content.
    content: Option<bool>,
}

/// A flex item's factors from its `flex`, `flex-grow`, `flex-shrink` and
/// `flex-basis`, in the order written.
fn flex_of(rows: &[&Attr]) -> Flex {
    let mut f = Flex {
        grow: Some(0.0),
        shrink: Some(1.0),
        content: Some(true),
    };
    for a in rows {
        match a.name.as_str() {
            "flex" => match crate::values::flex_longhands(&a.value) {
                Some((g, s, basis)) => {
                    f = Flex {
                        grow: Some(g),
                        shrink: Some(s),
                        content: Some(content_basis(&basis)),
                    }
                }
                None => {
                    f = Flex {
                        grow: None,
                        shrink: None,
                        content: None,
                    }
                }
            },
            "flex-grow" => f.grow = crate::values::numeric_literal(&a.value),
            "flex-shrink" => f.shrink = crate::values::numeric_literal(&a.value),
            "flex-basis" => {
                f.content = match &a.value {
                    Expr::Str(v, _) => Some(content_basis(v)),
                    _ => None,
                }
            }
            _ => {}
        }
    }
    f
}

/// A flex basis that resolves to the item's content in a parent of no
/// definite height (CSS Flexbox §9.2 3: a percentage there is `content`).
fn content_basis(basis: &str) -> bool {
    let basis = basis.trim();
    matches!(basis, "auto" | "content") || basis.ends_with('%')
}

fn flag(
    (wrapper, wrapper_tag): (Span, &str),
    (scroller, tag): (Span, &str),
    seen: &mut BTreeSet<(u32, u32, u32, u32)>,
    out: &mut Vec<LowerError>,
) {
    // A component used twice is inlined twice: one line for its source.
    if !seen.insert((
        wrapper.source_id,
        wrapper.line,
        wrapper.col,
        wrapper.end_col,
    )) {
        return;
    }
    let which = if scroller.source_id == wrapper.source_id {
        format!("the `{tag}` at line {}", scroller.line)
    } else {
        format!("the `{tag}`")
    };
    out.push(LowerError {
        id: "lower-scroll-grows",
        message: format!(
            "this `{wrapper_tag}` is at least as tall as its content, so {which} inside it is too and never scrolls: a flex item's minimum height is its content's unless it scrolls itself (CSS `min-height: auto`). Write `min-height=0` on this `{wrapper_tag}`"
        ),
        span: wrapper,
    });
}
