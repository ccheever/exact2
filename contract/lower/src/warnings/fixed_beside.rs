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

use super::{computed, has, literal};
use crate::LowerError;
use contract_syntax::{Attr, Node};

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
    let mut level: Vec<&Node> = children.iter().collect();
    loop {
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
                        "this box stays put {} the list that scrolls this route, and holds more than controls (a text, an image or a fill), so on iOS the rows stop at it in a hard line where a hand-built screen's list scrolls under what is pinned over it, blurred by the scroll edge effect. Put what it says in the list (a section's `header` or `footer`, or the first row), or keep only buttons, fields and a tablist in it, unfilled: those the list runs under (docs/recipes/list-rows.md)",
                        if below { "below" } else { "above" }
                    ),
                    span,
                });
            }
        }
        level = match scroller {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } if BOXES.contains(&tag.as_str()) && !scrolls(tag, attrs) && !across(tag, attrs) => {
                children.iter().collect()
            }
            Node::Element { .. } => return,
            // Of an `if` or a `match`, only the branch with the scroller: the others are shown instead.
            Node::When {
                then, otherwise, ..
            } => branch([then, otherwise]),
            Node::Each { body, .. } => body.iter().collect(),
            Node::Match { some, none, .. } => branch([&some.1, none]),
            Node::Use { .. } | Node::Children { .. } => return,
        };
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

fn branch(arms: [&Vec<Node>; 2]) -> Vec<&Node> {
    let arm = arms.into_iter().find(|a| a.iter().any(holds_scroller));
    arm.map(|a| a.iter().collect()).unwrap_or_default()
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
