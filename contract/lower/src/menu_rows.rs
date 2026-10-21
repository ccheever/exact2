//! `lower-menu-row-popover`, a note: a row of a menu (`role="menu"`) whose
//! `popovertarget` names a popover that is no menu. exact2's menu rule
//! (LLP 1021; theme2 2026-10-21 19:47, 22:05) opens it as if by the menu's
//! own invoker, so the menu closing as its row is chosen doesn't take it
//! along: the note says where it opens, and blames nothing. Only what the
//! source shows: literal ids, roles and targets.

use crate::LowerError;
use contract_syntax::{Attr, Expr, Node};
use std::collections::BTreeMap;

fn has(attrs: &[Attr], name: &str) -> bool {
    attrs.iter().any(|a| a.name == name)
}

/// An attribute's value when it is a string literal (the last one written).
fn literal<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a str> {
    match attrs
        .iter()
        .rev()
        .find(|a| a.name == name)
        .map(|a| &a.value)
    {
        Some(Expr::Str(s, _)) => Some(s.as_str()),
        _ => None,
    }
}

/// Each popover's literal id, and whether it is a menu.
fn popovers<'a>(nodes: &'a [Node], out: &mut BTreeMap<&'a str, bool>) {
    for n in nodes {
        let children: Vec<&Node> = match n {
            Node::Element {
                attrs, children, ..
            } => {
                if let (true, Some(id)) = (has(attrs, "popover"), literal(attrs, "id")) {
                    out.insert(id, literal(attrs, "role") == Some("menu"));
                }
                children.iter().collect()
            }
            Node::When {
                then, otherwise, ..
            } => then.iter().chain(otherwise).collect(),
            Node::Each { body, .. } => body.iter().collect(),
            Node::Match { some, none, .. } => some.1.iter().chain(none).collect(),
            Node::Use { children, .. } => children.iter().collect(),
            Node::Children { .. } => Vec::new(),
        };
        for c in children {
            popovers(std::slice::from_ref(c), out);
        }
    }
}

/// The notes for `nodes`, the root's view.
pub(crate) fn notes(nodes: &[Node], out: &mut Vec<LowerError>) {
    let mut ids = BTreeMap::new();
    popovers(nodes, &mut ids);
    walk(nodes, false, &ids, out);
}

fn walk(nodes: &[Node], in_menu: bool, ids: &BTreeMap<&str, bool>, out: &mut Vec<LowerError>) {
    for n in nodes {
        match n {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                let target = literal(attrs, "popovertarget")
                    .filter(|_| literal(attrs, "popovertargetaction") != Some("hide"));
                if let (true, "button", Some(target)) = (in_menu, tag.as_str(), target) {
                    if ids.get(target) == Some(&false) {
                        out.push(LowerError {
                            id: "lower-menu-row-popover",
                            message: format!("note: this menu row's `{target}` opens from the menu's own invoker (the button that opened the menu, or the node its context menu opened on), not nested in the menu, so it stays open as the menu closes on choose: exact2's menu rule (docs/contract-for-agents.md)"),
                            span: n.span(),
                        });
                    }
                }
                let menu =
                    in_menu || has(attrs, "popover") && literal(attrs, "role") == Some("menu");
                walk(children, menu, ids, out);
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, in_menu, ids, out);
                walk(otherwise, in_menu, ids, out);
            }
            Node::Each { body, .. } => walk(body, in_menu, ids, out),
            Node::Match { some, none, .. } => {
                walk(&some.1, in_menu, ids, out);
                walk(none, in_menu, ids, out);
            }
            Node::Use { children, .. } => walk(children, in_menu, ids, out),
            Node::Children { .. } => {}
        }
    }
}
