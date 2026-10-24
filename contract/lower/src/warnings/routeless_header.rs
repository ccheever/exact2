//! A screen heading at the top of an app with no navigation route stays
//! in the page on iOS. A keyed root alone is not a route: the repair needs
//! a separately keyed screen whose first child is the header.

use super::nested_header::WRAPPERS;
use super::width::{last, rows};
use super::{computed, has, literal};
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node};
use contract_types::Checked;

pub(super) fn check(checked: &Checked<'_>, out: &mut Vec<LowerError>) {
    let view = &checked.expanded.root.view;
    if has_route(view, false) {
        return;
    }
    let styles = &checked.file.styles;
    let mut at = view.as_slice();
    while let Some(
        node @ Node::Element {
            tag,
            attrs,
            children,
            ..
        },
    ) = at.first()
    {
        if has(attrs, "popover") {
            return;
        }
        let Some(rows) = rows(attrs, styles) else {
            return;
        };
        if none_or_computed(&rows, "display") {
            return;
        }
        if tag == "header" {
            if !none_or_computed(&rows, "appearance") && headings(children) == Some(1) {
                out.push(LowerError {
                    id: "lower-header-routeless",
                    message: "this app has no navigation route (a keyed screen inside a keyed navigation root), so on iOS this `header` is drawn in the page, not as the navigation bar. Add a navigation root holding a separately keyed screen (both have `navigationKey`), with this header as the screen's first child, or set `appearance=\"none\"` on the header to keep it drawn as written (docs/recipes/navigation-stack.md)".into(),
                    span: node.span(),
                });
            }
            return;
        }
        if !WRAPPERS.contains(&tag.as_str()) {
            return;
        }
        at = children;
    }
}

// A route in any branch counts, including one that is initially hidden.
fn has_route(nodes: &[Node], in_root: bool) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element {
            attrs, children, ..
        } => {
            let keyed = has(attrs, "navigationKey");
            // A host may treat a keyed child as a route even with `navigationBack`.
            (in_root && keyed) || has_route(children, in_root || keyed)
        }
        Node::When {
            then, otherwise, ..
        } => has_route(then, in_root) || has_route(otherwise, in_root),
        Node::Match { some, none, .. } => has_route(&some.1, in_root) || has_route(none, in_root),
        Node::Each { body, .. } => has_route(body, in_root),
        Node::Use { children, .. } => has_route(children, in_root),
        Node::Children { .. } => false,
    })
}

fn none_or_computed(rows: &[&Attr], name: &str) -> bool {
    match last(rows, name).map(|a| &a.value) {
        Some(Expr::Str(value, _)) => value == "none",
        Some(_) => true,
        None => false,
    }
}

// A branch with differing counts or a computed display or role can make
// the heading count uncertain. Classes do not change the bar's heading count.
// Dialogs and popovers belong to the top layer, not to the bar.
fn headings(nodes: &[Node]) -> Option<usize> {
    let mut count = 0;
    for node in nodes {
        let (tag, attrs, children) = match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => (tag, attrs, children),
            Node::When {
                then, otherwise, ..
            } => {
                count += branch_headings(then, otherwise)?;
                continue;
            }
            Node::Match { some, none, .. } => {
                count += branch_headings(&some.1, none)?;
                continue;
            }
            Node::Each { body, .. } if headings(body) == Some(0) => continue,
            _ => return None,
        };
        if literal(attrs, "display") == Some("none") || tag == "dialog" || has(attrs, "popover") {
            continue;
        }
        let found = if tag == "text" && computed(attrs, "role") {
            None
        } else if tag == "text" && literal(attrs, "role") == Some("heading") {
            Some(1)
        } else if literal(attrs, "role") == Some("tablist") {
            Some(0)
        } else {
            headings(children)
        };
        // Styles cannot supply a role. Unknown presentation cannot change
        // the heading count of a subtree that cannot contain a heading.
        if found == Some(0) {
            continue;
        }
        if computed(attrs, "display") || computed(attrs, "role") {
            return None;
        }
        count += found?;
    }
    Some(count)
}

fn branch_headings(a: &[Node], b: &[Node]) -> Option<usize> {
    let a = headings(a)?;
    (headings(b)? == a).then_some(a)
}
