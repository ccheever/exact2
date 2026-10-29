//! `lower-list-beside-fixed`: a box that stays put above or below the
//! list that scrolls its route, holding more than controls (a caption, a
//! count, an image, a fill). On iOS 26 a hand-built screen's list runs under
//! what is pinned over its edges and the scroll edge effect blurs it there;
//! Exact draws a list under its neighbours only when they are bars of
//! controls (LLP 1131 D1), so beside this box the rows stop in a hard line.
//! Inbox's "320 messages" footer did this in bake-off r24.
//!
//! An `if`, `match`, use or slot beside the list, and a computed attribute,
//! are given the benefit of the doubt.
//!
//! The list in a row beside another box (contacts' own section index, bake-off
//! r28) is named too: the route's list is then no longer its own, so its
//! large title stays open and its rows stop over the bottom bar.

use super::{computed, has, literal};
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node};

/// The plain boxes the walk looks inside, down to the scroller.
const BOXES: &[&str] = &[
    "view", "box", "column", "row", "nav", "section", "footer", "article", "aside", "main",
];

/// The tags taken whole as controls (LLP 1131 D1).
const CONTROLS: &[&str] = &["button", "link", "input", "textarea", "select"];

/// The route's scroller and the boxes above and below it on the way down,
/// warned. A `header` is left to the route's bar and its own warnings.
pub(super) fn fixed_beside(route: (&str, &[Attr]), children: &[Node], out: &mut Vec<LowerError>) {
    if across(route.0, route.1) {
        return;
    }
    // A route whose first child is no `header` has no bar to run the list under.
    if matches!(children.first(), Some(Node::Element { tag, .. }) if tag != "header") {
        return;
    }
    walk(children.iter().collect(), out);
}

/// One level on the way down to the scroller: its neighbours there warned,
/// then the box or each branch holding it.
fn walk(level: Vec<&Node>, out: &mut Vec<LowerError>) {
    let Some(i) = level.iter().position(|n| holds_scroller(n)) else {
        return;
    };
    let scroller = level[i];
    for (j, n) in level.iter().enumerate().filter(|(j, _)| *j != i) {
        if let Some(span) = fixed(n).filter(|s| {
            !out.iter()
                .any(|e| e.id == "lower-list-beside-fixed" && e.span == *s)
        }) {
            let below = j > i;
            out.push(LowerError {
                id: "lower-list-beside-fixed",
                message: format!(
                    "this box stays put {} the list that scrolls this route, and holds more than controls (a text, an image or a fill), so on iOS the rows stop at it in a hard line where a hand-built screen's list scrolls under what is pinned over it, blurred by the scroll edge effect. Put what it says in the list (a section's `header` or `footer`, or the first row), or keep only buttons, fields and a tablist in it, unfilled: those the list runs under, faded and blurred where it passes as iOS does under a hand-built composer, so a field's text over it stays legible (docs/recipes/list-rows.md)",
                    if below { "below" } else { "above" }
                ),
                span,
            });
        }
    }
    match scroller {
        Node::Element {
            tag,
            attrs,
            children,
            ..
        } if BOXES.contains(&tag.as_str()) && !scrolls(tag, attrs) && !across(tag, attrs) => {
            walk(children.iter().collect(), out)
        }
        Node::Element {
            tag,
            attrs,
            children,
            span,
            ..
        } if BOXES.contains(&tag.as_str()) && !scrolls(tag, attrs) => {
            if children
                .iter()
                .any(|n| !holds_scroller(n) && beside(n, false))
            {
                out.push(LowerError {
                    id: "lower-list-beside-fixed",
                    message: "the list that scrolls this route sits in a row beside another box, so on iOS it is not the route's own: its large title never collapses, and its rows stop above the bottom bar (a search field there) where a hand-built screen's list collapses the title and runs under the bar. Make the list the route's child after its `header`, and leave what stands beside it to the platform: a list whose sections are each headed by one letter has UIKit's A–Z index, and sections above the letters (Favorites) are left out of it".into(),
                    span: *span,
                });
            }
        }
        Node::Element { .. } | Node::Use { .. } | Node::Children { .. } => {}
        // Of an `if` or a `match`, each branch with a scroller (an empty
        // state's own is one): the others are shown instead.
        Node::When {
            then, otherwise, ..
        } => branches([then, otherwise], out),
        Node::Each { body, .. } => walk(body.iter().collect(), out),
        Node::Match { some, none, .. } => branches([&some.1, none], out),
    }
}

/// Whether a box lays its children side by side: its neighbours then
/// are beside the scroller, not over its ends.
fn across(tag: &str, attrs: &[Attr]) -> bool {
    match literal(attrs, "flex-direction") {
        Some(d) => d.starts_with("row"),
        None => tag == "row" || computed(attrs, "flex-direction"),
    }
}

/// Whether a node in the row stands beside the list: an element in flow,
/// or a use. One shown only sometimes counts when it is a strip (an index, at
/// most 48 wide): a wide one is often a pane a wide window adds.
fn beside(node: &Node, sometimes: bool) -> bool {
    match node {
        Node::Element { tag, attrs, .. } => {
            let out_of_flow = matches!(literal(attrs, "position"), Some("absolute" | "fixed"))
                || literal(attrs, "display") == Some("none")
                || has(attrs, "popover")
                || tag == "dialog";
            let strip = || match attrs
                .iter()
                .rev()
                .find(|a| a.name == "width")
                .map(|a| &a.value)
            {
                Some(Expr::Number(w, _)) => *w <= 48.0,
                Some(Expr::Str(w, _)) => w
                    .trim_end_matches("px")
                    .parse::<f64>()
                    .is_ok_and(|w| w <= 48.0),
                _ => false,
            };
            !out_of_flow && (!sometimes || strip())
        }
        Node::When {
            then, otherwise, ..
        } => then.iter().chain(otherwise).any(|n| beside(n, true)),
        Node::Each { body, .. } => body.iter().any(|n| beside(n, sometimes)),
        Node::Match { some, none, .. } => some.1.iter().chain(none).any(|n| beside(n, true)),
        Node::Use { .. } => !sometimes,
        Node::Children { .. } => false,
    }
}

fn branches(arms: [&Vec<Node>; 2], out: &mut Vec<LowerError>) {
    for arm in arms {
        walk(arm.iter().collect(), out);
    }
}

/// A vertical scroller that takes the room it is given (a `list`, or a
/// `scroll` not held sideways, with a `flex`), or a box or branch holding one.
fn holds_scroller(node: &Node) -> bool {
    match node {
        Node::Element {
            tag,
            attrs,
            children,
            ..
        } => {
            if scrolls(tag, attrs) {
                return has(attrs, "flex") || has(attrs, "flex-grow");
            }
            BOXES.contains(&tag.as_str()) && children.iter().any(holds_scroller)
        }
        Node::When {
            then, otherwise, ..
        } => then.iter().chain(otherwise).any(holds_scroller),
        Node::Each { body, .. } => body.iter().any(holds_scroller),
        Node::Match { some, none, .. } => some.1.iter().chain(none).any(holds_scroller),
        Node::Use { .. } | Node::Children { .. } => false,
    }
}

fn scrolls(tag: &str, attrs: &[Attr]) -> bool {
    tag == "list" || (tag == "scroll" && literal(attrs, "overflow-y") != Some("hidden"))
}

/// Where a neighbour in flow holds more than controls: its own span, or a
/// repeated one's. None for a bar of controls, a box out of flow or
/// hidden, a conditional one (often another route's, or a banner that
/// comes and goes) and what the compiler cannot see into.
fn fixed(node: &Node) -> Option<contract_syntax::Span> {
    match node {
        Node::Element { tag, attrs, .. } => {
            let out_of_flow = matches!(literal(attrs, "position"), Some("absolute" | "fixed"))
                || literal(attrs, "display") == Some("none")
                || has(attrs, "popover")
                || has(attrs, "navigationSplit")
                || tag == "dialog"
                || tag == "header";
            if out_of_flow || computed(attrs, "position") || computed(attrs, "display") {
                return None;
            }
            (!controls_only(node)?).then(|| node.span())
        }
        Node::Each { body, .. } => body.iter().find_map(fixed),
        Node::When { .. } | Node::Match { .. } | Node::Use { .. } | Node::Children { .. } => None,
    }
}

/// Whether a node holds only controls and unfilled boxes of them (LLP 1131
/// D1); None where the compiler cannot tell.
fn controls_only(node: &Node) -> Option<bool> {
    match node {
        Node::Element {
            tag,
            attrs,
            children,
            ..
        } => {
            if CONTROLS.contains(&tag.as_str()) || literal(attrs, "role") == Some("tablist") {
                return Some(true);
            }
            if !BOXES.contains(&tag.as_str())
                || has(attrs, "background-color")
                || has(attrs, "background")
            {
                return Some(false);
            }
            all(children)
        }
        // What shows only sometimes (a reply's preview over a composer) is given the benefit.
        Node::When {
            then, otherwise, ..
        } => all_of(then.iter().chain(otherwise)).filter(|&only| only),
        Node::Each { body, .. } => all(body),
        Node::Match { some, none, .. } => all_of(some.1.iter().chain(none)).filter(|&only| only),
        Node::Use { .. } | Node::Children { .. } => None,
    }
}

fn all(nodes: &[Node]) -> Option<bool> {
    all_of(nodes.iter())
}

fn all_of<'a>(nodes: impl Iterator<Item = &'a Node>) -> Option<bool> {
    let mut answer = Some(true);
    for n in nodes {
        match controls_only(n) {
            Some(false) => return Some(false),
            None => answer = None,
            Some(true) => {}
        }
    }
    answer
}
