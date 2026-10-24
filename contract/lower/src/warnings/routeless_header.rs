//! A screen heading at the top of an app with no navigation root stays
//! in the page on iOS. A keyed root alone is not a route: the repair needs
//! a separately keyed screen whose first child is the header.

use super::nested_header::WRAPPERS;
use super::width::{last, rows};
use super::{computed, has, literal};
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, StyleDecl};
use contract_types::Checked;

pub(super) fn check(checked: &Checked<'_>, out: &mut Vec<LowerError>) {
    let view = &checked.expanded.root.view;
    if has_navigation(view) {
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
        let Some(rows) = rows(attrs, styles) else {
            return;
        };
        if none_or_computed(&rows, "display") {
            return;
        }
        if tag == "header" {
            if !none_or_computed(&rows, "appearance") && headings(children, styles) == Some(1) {
                out.push(LowerError {
                    id: "lower-header-routeless",
                    message: "this app has no `navigationKey`, so on iOS this `header` is drawn in the page, not as the navigation bar. Add a navigation root holding a separately keyed screen (both have `navigationKey`), with this header as the screen's first child, or set `appearance=\"none\"` on the header to keep it drawn as written (docs/recipes/navigation-stack.md)".into(),
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

// Navigation in any branch counts, including one that is initially hidden.
fn has_navigation(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element {
            attrs, children, ..
        } => has(attrs, "navigationKey") || has_navigation(children),
        Node::When {
            then, otherwise, ..
        } => has_navigation(then) || has_navigation(otherwise),
        Node::Match { some, none, .. } => has_navigation(&some.1) || has_navigation(none),
        Node::Each { body, .. } => has_navigation(body),
        Node::Use { children, .. } => has_navigation(children),
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

// A branch with differing counts, a computed role or an unknown style
// could add or remove a heading.
// Dialogs and popovers belong to the top layer, not to the bar.
fn headings(nodes: &[Node], styles: &[StyleDecl]) -> Option<usize> {
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
                count += branch_headings(then, otherwise, styles)?;
                continue;
            }
            Node::Match { some, none, .. } => {
                count += branch_headings(&some.1, none, styles)?;
                continue;
            }
            Node::Each { body, .. } if headings(body, styles) == Some(0) => continue,
            _ => return None,
        };
        if tag == "dialog" || has(attrs, "popover") {
            continue;
        }
        let rows = rows(attrs, styles)?;
        match last(&rows, "display").map(|a| &a.value) {
            Some(Expr::Str(value, _)) if value == "none" => continue,
            Some(Expr::Str(..)) | None => {}
            Some(_) => return None,
        }
        if computed(attrs, "role") {
            return None;
        }
        if tag == "text" && literal(attrs, "role") == Some("heading") {
            count += 1;
        } else if literal(attrs, "role") != Some("tablist") {
            count += headings(children, styles)?;
        }
    }
    Some(count)
}

fn branch_headings(a: &[Node], b: &[Node], styles: &[StyleDecl]) -> Option<usize> {
    let a = headings(a, styles)?;
    (headings(b, styles)? == a).then_some(a)
}
