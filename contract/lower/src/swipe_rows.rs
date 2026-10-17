//! @ref LLP 1116.020 D1 — `delete=action` on a row of a `list` (a grouped
//! list's section row, or a row of a list whose rows are cells) is the
//! row's swipe to delete, written once. The compiler expands it to the
//! swipe form `docs/recipes/list-rows.md` once taught by hand: a sideways
//! snap `scroll` naming the row as its `swipeContent` and a generated
//! destructive button titled "Delete" as its `swipeTrailing`, which a full
//! swipe performs. On iOS that is UIKit's own swipe (the grouped list's
//! cell, or a cell's table: `SwipeActionsIOS.swift`); the web and the Mac
//! scroll it. The gesture runs the action; a confirmation is its handler's.
//!
//! `disabled` beside `delete=` is the delete's: a locked row still opens,
//! and offers no Delete. The button's `testId` is the row's with
//! `-delete` after it.
//!
//! In a grouped list the row is styled as one (`grouped::row`) and the
//! scroll takes only the row's box in its card: its separator and the
//! margins around it. The inset is inside the row, so the cell UIKit swipes
//! and its long-press preview hold it, as a hand-built list's cell does, and
//! the row slides from the card's edge; the Mac's form, whose rows stand in
//! from both edges, keeps it on the scroll (`grouped-swipe-margin`). So the
//! web's separator under such a row starts at the card's edge, where UIKit
//! draws its own at the inset.

use crate::metrics::grouped as metric;
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, Span, TemplatePart};

/// The names the expansion gives the content and the action, where the
/// row has no `id` of its own. Every row repeats them: a swipe row's names
/// resolve inside it, on every host.
const CONTENT: &str = "exact-swipe-row";
const ACTION: &str = "exact-swipe-delete";

/// The sheet rows of a grouped row that are its box in the card, which
/// the scroll takes: its separator and the margins around it.
const BOX: &[&str] = &[
    "margin-right",
    "margin-bottom",
    "border-bottom-width",
    "border-bottom-style",
    "border-bottom-color",
];

fn last(attrs: &[Attr], name: &str) -> Option<Attr> {
    attrs.iter().rev().find(|a| a.name == name).cloned()
}

fn implied(name: &str, value: Expr, span: Span) -> Attr {
    Attr {
        name: name.into(),
        value,
        span,
        written: crate::IMPLIED,
    }
}

fn s(name: &str, value: &str, span: Span) -> Attr {
    implied(name, Expr::Str(value.into(), span), span)
}

fn n(name: &str, value: f64, span: Span) -> Attr {
    implied(name, Expr::Number(value, span), span)
}

/// `testId` with `-delete` after it.
fn suffixed(id: &Expr) -> Expr {
    let tail = TemplatePart::Text("-delete".into());
    match id {
        Expr::Str(v, span) => Expr::Str(format!("{v}-delete"), *span),
        Expr::Template(parts, span) => {
            let mut parts = parts.clone();
            parts.push(tail);
            Expr::Template(parts, *span)
        }
        other => Expr::Template(vec![TemplatePart::Expr(other.clone()), tail], other.span()),
    }
}

/// A row that says `delete=`, read: the row without it, the name the
/// scroll's `swipeContent` gives it, and its Delete button.
struct Parts {
    content: Node,
    name: Expr,
    action: Node,
}

/// `node`'s parts when it says `delete=`. `sheet` makes a structural row
/// of the content's a sheet row, under the author's (`grouped::split`), or
/// leaves it as it is.
fn parts(node: &Node, sheet: &dyn Fn(Attr) -> Attr) -> Option<Parts> {
    let Node::Element {
        tag,
        positional,
        attrs,
        children,
        span,
        instance,
    } = node
    else {
        return None;
    };
    let delete = last(attrs, "delete")?;
    let at = delete.span;
    let disabled = last(attrs, "disabled");
    let mut own: Vec<Attr> = attrs
        .iter()
        .filter(|a| a.name != "delete" && a.name != "disabled")
        .cloned()
        .collect();
    let name = match last(attrs, "id") {
        Some(id) => id.value,
        None => {
            own.push(s("id", CONTENT, *span));
            Expr::Str(CONTENT.into(), *span)
        }
    };
    let mut rows: Vec<Attr> = [
        s("width", "100%", *span),
        n("flex-shrink", 0.0, *span),
        s("scroll-snap-align", "start", *span),
        s("box-sizing", "border-box", *span),
    ]
    .into_iter()
    .map(sheet)
    .collect();
    rows.extend(own);
    let content = Node::Element {
        tag: tag.clone(),
        positional: positional.clone(),
        attrs: rows,
        children: children.clone(),
        span: *span,
        instance: *instance,
    };
    let mut button = vec![
        s("id", ACTION, at),
        implied("destructive", Expr::Bool(true, at), at),
        implied("press", delete.value.clone(), at),
        s("aria-label", "Delete", at),
        n("flex-shrink", 0.0, at),
        s("scroll-snap-align", "start", at),
        s("display", "flex", at),
        s("align-items", "center", at),
        n("padding-left", 20.0, at),
        n("padding-right", 20.0, at),
        s("background-color", "-exact-system-red", at),
        s("color", "white", at),
    ];
    button.extend(disabled.map(|d| Attr { span: at, ..d }));
    if let Some(id) = last(attrs, "testId") {
        button.push(implied("testId", suffixed(&id.value), at));
    }
    let action = Node::Element {
        tag: "button".into(),
        positional: Vec::new(),
        attrs: button,
        children: vec![Node::Element {
            tag: "text".into(),
            positional: vec![Expr::Str("Delete".into(), at)],
            attrs: Vec::new(),
            children: Vec::new(),
            span: at,
            instance: *instance,
        }],
        span: at,
        instance: *instance,
    };
    Some(Parts {
        content,
        name,
        action,
    })
}

/// The scroll holding `content` and `action`, with `rows` under its own.
fn scroll(parts: Parts, rows: Vec<Attr>, span: Span, instance: u32) -> Node {
    let mut attrs = rows;
    attrs.extend([
        s("display", "flex", span),
        s("flex-direction", "row", span),
        implied("swipeContent", parts.name, span),
        s("swipeTrailing", ACTION, span),
        s("overflow-x", "scroll", span),
        s("overflow-y", "hidden", span),
        s("scrollbar-width", "none", span),
        s("scroll-snap-type", "x mandatory", span),
    ]);
    Node::Element {
        tag: "scroll".into(),
        positional: Vec::new(),
        attrs,
        children: vec![parts.content, parts.action],
        span,
        instance,
    }
}

/// A grouped list's row: `row` styles it, and a row that says `delete=` is
/// styled so and put in its swipe.
pub(crate) fn grouped(node: &Node, row: &dyn Fn(&Node) -> Node) -> Node {
    let mark = |name: &str| format!("{}{name}", crate::grouped::MARK);
    let sheet = |a: Attr| Attr {
        name: mark(&a.name),
        ..a
    };
    let Some(parts) = parts(node, &sheet) else {
        return row(node);
    };
    let (span, instance) = match node {
        Node::Element { span, instance, .. } => (*span, *instance),
        _ => unreachable!("parts reads an element"),
    };
    let Node::Element {
        tag,
        positional,
        attrs,
        children,
        span: at,
        instance: own,
    } = row(&parts.content)
    else {
        unreachable!("row keeps an element")
    };
    let boxed = |a: &Attr| BOX.iter().any(|b| a.name == mark(b));
    let inset = mark("margin-left");
    let mut outer: Vec<Attr> = attrs.iter().filter(|a| boxed(a)).cloned().collect();
    outer.push(s(&inset, &metric("swipe-margin"), span));
    let mut inner = vec![s(&mark("padding-left"), &metric("swipe-padding"), span)];
    inner.extend(attrs.into_iter().filter(|a| !boxed(a) && a.name != inset));
    // A leading symbol stands at the row's inset as in any row, with no
    // margin to pull it back into.
    let pull = metric("symbol-pull");
    let children = children
        .into_iter()
        .map(|c| match c {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
                instance,
            } => Node::Element {
                attrs: attrs
                    .into_iter()
                    .filter(|a| !matches!(&a.value, Expr::Str(v, _) if *v == pull))
                    .collect(),
                tag,
                positional,
                children,
                span,
                instance,
            },
            other => other,
        })
        .collect();
    let content = Node::Element {
        tag,
        positional,
        attrs: inner,
        children,
        span: at,
        instance: own,
    };
    scroll(Parts { content, ..parts }, outer, span, instance)
}

/// The rows of a list whose rows are cells (`cells.rs`), each that says
/// `delete=` put in its swipe; `None` when none does.
pub(crate) fn cells(children: &[Node]) -> Option<Vec<Node>> {
    fn says(node: &Node) -> bool {
        match node {
            Node::Element { attrs, .. } => attrs.iter().any(|a| a.name == "delete"),
            Node::When {
                then, otherwise, ..
            } => then.iter().chain(otherwise).any(says),
            Node::Each { body, .. } => body.iter().any(says),
            Node::Match { some, none, .. } => some.1.iter().chain(none).any(says),
            Node::Use { .. } | Node::Children { .. } => false,
        }
    }
    fn expand(node: &Node) -> Node {
        match node {
            Node::Element { span, instance, .. } => match parts(node, &|a| a) {
                Some(parts) => scroll(parts, Vec::new(), *span, *instance),
                None => node.clone(),
            },
            Node::When {
                tag,
                cond,
                then,
                otherwise,
                span,
            } => Node::When {
                tag: *tag,
                cond: cond.clone(),
                then: then.iter().map(expand).collect(),
                otherwise: otherwise.iter().map(expand).collect(),
                span: *span,
            },
            Node::Each {
                tag,
                var,
                index,
                list,
                key,
                body,
                span,
            } => Node::Each {
                tag: *tag,
                var: var.clone(),
                index: index.clone(),
                list: list.clone(),
                key: key.clone(),
                body: body.iter().map(expand).collect(),
                span: *span,
            },
            Node::Match {
                tag,
                subject,
                some,
                none,
                span,
            } => Node::Match {
                tag: *tag,
                subject: subject.clone(),
                some: (some.0.clone(), some.1.iter().map(expand).collect()),
                none: none.iter().map(expand).collect(),
                span: *span,
            },
            other => other.clone(),
        }
    }
    children
        .iter()
        .any(says)
        .then(|| children.iter().map(expand).collect())
}

/// Where an element stands, for `lower-delete-row`: a list's row, a
/// grouped list's section, or anywhere else.
#[derive(Clone, Copy, PartialEq)]
enum At {
    Row,
    Sections,
    Section,
    Other,
}

/// Where the walk stands within a row a `for` repeats.
#[derive(Clone, Copy)]
struct Walk {
    at: At,
    /// A `for` stands between the list or section and here, with only `if`
    /// and `match` between: an element here is a repeated row.
    repeated: bool,
    /// Inside a repeated row that says no `delete=`, outside anything that
    /// holds its own actions (a dialog, a popover, a menu, a swipe's scroll).
    advise: bool,
}

/// `lower-delete-row`: a `delete=` on an element that is no list row, which
/// lowering drops. Read as the expansion reads rows: a grouped list's
/// section rows, a cell list's children, through `if`, `for` and `match`.
///
/// @ref LLP 1116.021 D5 — `lower-row-delete-button`: a `button
/// destructive=true` inside the row a `for` repeats, which says no
/// `delete=`. Advice, keyed on structure alone: `destructive` can mean
/// revoke, cancel or reset, so it asks rather than tells. A detail screen's
/// section is no repeated row, and an action in a row's dialog, popover,
/// menu or swipe is that surface's. Round 19's census found it on no row
/// but a per-row delete across the repo's contracts and the docs' blocks.
pub(crate) fn unplaced(view: &[Node]) -> Vec<LowerError> {
    fn walk(nodes: &[Node], w: Walk, out: &mut Vec<LowerError>) {
        for node in nodes {
            match node {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    span,
                    ..
                } => {
                    let at = w.at;
                    let label = at == At::Section && (tag == "header" || tag == "footer");
                    let row = matches!(at, At::Row) || (at == At::Section && !label);
                    let delete = attrs.iter().find(|a| a.name == "delete");
                    if let Some(a) = delete.filter(|_| !row) {
                        out.push(LowerError {
                            id: "lower-delete-row",
                            message: "`delete=` is a list row's swipe to delete, and this element is no row of a `list`, so it does nothing here: put it on a row of a grouped list's `section`, or on a row of a `list` whose rows go down the page (docs/recipes/list-rows.md)".into(),
                            span: a.span,
                        });
                    }
                    let literal = |name: &str, value: &str| {
                        attrs
                            .iter()
                            .rev()
                            .find(|a| a.name == name)
                            .is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == value))
                    };
                    let holds_actions = tag == "dialog"
                        || attrs
                            .iter()
                            .any(|a| a.name == "popover" || a.name == "swipeTrailing")
                        || literal("role", "menu");
                    let advise = if row {
                        w.repeated && delete.is_none() && !holds_actions
                    } else {
                        w.advise && !holds_actions
                    };
                    let destructive = attrs
                        .iter()
                        .rev()
                        .find(|a| a.name == "destructive")
                        .is_some_and(|a| matches!(a.value, Expr::Bool(true, _)));
                    if w.advise && !row && tag == "button" && destructive {
                        out.push(LowerError {
                            id: "lower-row-delete-button",
                            message: "if this button deletes the row, write `delete=<its press>` on the row and iOS offers it as a swipe (docs/recipes/list-rows.md)".into(),
                            span: *span,
                        });
                    }
                    let inside = match (tag.as_str(), at) {
                        ("list", _) => match crate::grouped::style(tag, attrs) {
                            Ok(Some(_)) => At::Sections,
                            Ok(None) if crate::cells::rows(tag, attrs, children, false) => At::Row,
                            _ => At::Other,
                        },
                        ("section", At::Sections) => At::Section,
                        _ => At::Other,
                    };
                    let w = Walk {
                        at: inside,
                        repeated: false,
                        advise,
                    };
                    walk(children, w, out);
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, w, out);
                    walk(otherwise, w, out);
                }
                Node::Each { body, .. } => {
                    let repeated = w.repeated || matches!(w.at, At::Row | At::Section);
                    walk(body, Walk { repeated, ..w }, out)
                }
                Node::Match { some, none, .. } => {
                    walk(&some.1, w, out);
                    walk(none, w, out);
                }
                Node::Use { children, .. } => walk(
                    children,
                    Walk {
                        at: At::Other,
                        repeated: false,
                        advise: false,
                    },
                    out,
                ),
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    let top = Walk {
        at: At::Other,
        repeated: false,
        advise: false,
    };
    walk(view, top, &mut out);
    out
}

/// An element's attributes without a `delete=` that is no list row's,
/// which `lower-delete-row` names; `None` when it has none.
pub(crate) fn stray(attrs: &[Attr]) -> Option<Vec<Attr>> {
    attrs.iter().any(|a| a.name == "delete").then(|| {
        attrs
            .iter()
            .filter(|a| a.name != "delete")
            .cloned()
            .collect()
    })
}
