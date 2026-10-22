//! `lower-menu-row-popover`, a note: a row of a menu (`role="menu"`) whose
//! `popovertarget` names a popover that is no menu. exact2's menu rule
//! (LLP 1021; theme2 2026-10-21 19:47, 22:05) opens it as if by the menu's
//! own invoker, so the menu closing as its row is chosen doesn't take it
//! along: the note says where it opens, and blames nothing. Only rows the
//! source settles: literal ids, roles, targets and actions (`commandfor` as
//! well as `popovertarget`); a confirmation is the platform's own.

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

/// What a popover is, as far as its source says.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// A literal `role="menu"`, or a confirmation (`alertdialog`, a `dialog`): the platform's own.
    Menu,
    /// No role, or a literal one of another kind: the rule's.
    Other,
    /// A role the source doesn't settle: no note.
    Unknown,
}

/// Each popover's literal id, and what it is.
fn popovers<'a>(nodes: &'a [Node], out: &mut BTreeMap<&'a str, Kind>) {
    for n in nodes {
        let children: Vec<&Node> = match n {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                if let (true, Some(id)) = (
                    has(attrs, "popover") || tag == "dialog",
                    literal(attrs, "id"),
                ) {
                    let kind = match (tag.as_str(), has(attrs, "role"), literal(attrs, "role")) {
                        ("dialog", ..) | (_, _, Some("menu" | "alertdialog")) => Kind::Menu,
                        (_, false, _) | (_, true, Some(_)) => Kind::Other,
                        (_, true, None) => Kind::Unknown,
                    };
                    // Two arms that say different things leave it unsettled.
                    let settled = *out.entry(id).or_insert(kind);
                    if settled != kind {
                        out.insert(id, Kind::Unknown);
                    }
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

/// The literal id a button opens, as HTML resolves its invocation:
/// `commandfor` with a showing `command` before `popovertarget` without
/// `hide`; none for a hide, a value the source doesn't settle, or neither.
fn opens(attrs: &[Attr]) -> Option<&str> {
    // An empty `commandfor` names nothing, and HTML falls back to `popovertarget`.
    if has(attrs, "commandfor") && literal(attrs, "commandfor") != Some("") {
        let command = literal(attrs, "command")?.to_ascii_lowercase();
        return matches!(command.as_str(), "show-popover" | "toggle-popover")
            .then(|| literal(attrs, "commandfor"))
            .flatten();
    }
    if has(attrs, "popovertargetaction")
        && literal(attrs, "popovertargetaction")?.eq_ignore_ascii_case("hide")
    {
        return None;
    }
    literal(attrs, "popovertarget")
}

/// The notes for `nodes`, the root's view.
pub(crate) fn notes(nodes: &[Node], out: &mut Vec<LowerError>) {
    let mut ids = BTreeMap::new();
    popovers(nodes, &mut ids);
    walk(nodes, false, &ids, out);
}

fn walk(nodes: &[Node], in_menu: bool, ids: &BTreeMap<&str, Kind>, out: &mut Vec<LowerError>) {
    for n in nodes {
        match n {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                if let (true, "button", Some(target)) = (in_menu, tag.as_str(), opens(attrs)) {
                    if ids.get(target) == Some(&Kind::Other) {
                        out.push(LowerError {
                            id: "lower-menu-row-popover",
                            message: format!("note: this menu row's `{target}` opens from the menu's own invoker (the button that opened the menu, or the node its context menu opened on), not nested in the menu, so it stays open as the menu closes on choose, unless it is written inside the menu: exact2's menu rule (docs/contract-for-agents.md)"),
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
