//! `lower-screen-by-state` (LLP 1116.005 D1.1): a `when`/`else` or a
//! `match` that swaps the page between screens, in an app that declares no
//! `routes`. recipe-finder and event-rsvp (round 7) did: a list and its
//! detail, or a detail and a create form, chosen by a state, each holding a
//! heading as a screen does, so the browser's Back left the app, a reload
//! lost the place and nothing could be linked.
//!
//! Ordinary state stays quiet: tabs (a state a `tab` or `radio` reads), a
//! loading, error or empty view (a condition on a resource or a mutation,
//! or an arm of one control or none), a kept setting (a `persist` state),
//! disclosure and an arm with no heading, anything inside a row, a button,
//! a tab panel, a dialog or a popover. A swap counts only when a press in
//! one of its arms moves it: the detail's Back, the list's row, a form's
//! Cancel. The outermost swap is named once.

use super::{computed, has, literal};
use crate::LowerError;
use contract_syntax::{Component, Expr, Node, Stmt, TemplatePart};
use contract_types::Checked;
use std::collections::BTreeSet;

/// The tags that are controls a person operates.
const CONTROLS: &[&str] = &["button", "link", "input", "select", "textarea"];

/// Every warning: at most one a swap, and none inside one that warned.
pub(super) fn screen_by_state(checked: &Checked<'_>) -> Vec<LowerError> {
    let root = &checked.expanded.root;
    // A game's screens are its art; a router's are its routes.
    if checked.file.routes.is_some() || root.resources.iter().any(|r| r.source == "exactSurface") {
        return Vec::new();
    }
    let app = App::new(root);
    let mut out = Vec::new();
    app.walk(&root.view, &mut out);
    out
}

/// What the expanded root's names are.
struct App<'a> {
    root: &'a Component,
    /// States a `tab` or a `radio` reads its selection from.
    picked: BTreeSet<String>,
}

impl<'a> App<'a> {
    fn new(root: &'a Component) -> Self {
        let mut app = App {
            root,
            picked: BTreeSet::new(),
        };
        let mut picked = BTreeSet::new();
        let mut stack: Vec<&Node> = root.view.iter().collect();
        while let Some(node) = stack.pop() {
            if let Node::Element { attrs, .. } = node {
                if matches!(literal(attrs, "role"), Some("tab" | "radio")) {
                    for a in attrs
                        .iter()
                        .filter(|a| a.name == "aria-selected" || a.name == "aria-checked")
                    {
                        picked.extend(app.reads(&a.value).states);
                    }
                }
            }
            stack.extend(lists(node).into_iter().flatten());
        }
        app.picked = picked;
        app
    }

    /// The root's names an expression reads, through derives.
    fn reads(&self, e: &Expr) -> Reads {
        let mut names = Vec::new();
        idents(e, &mut names);
        let mut seen = BTreeSet::new();
        let mut reads = Reads::default();
        while let Some(name) = names.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            let r = self.root;
            if let Some(d) = r.derives.iter().find(|d| d.name == name) {
                idents(&d.expr, &mut names);
            } else if r.states.iter().any(|s| s.name == name) {
                reads.states.insert(name);
            } else if r.resources.iter().any(|x| x.name == name)
                || r.mutations.iter().any(|m| m.name == name)
            {
                reads.data = true;
            }
        }
        reads
    }

    /// The states the action a `press` names assigns.
    fn assigns(&self, handler: &Expr) -> BTreeSet<String> {
        let name = match handler {
            Expr::Ident(name, _) | Expr::Call(name, _, _) => name,
            _ => return BTreeSet::new(),
        };
        let mut out = BTreeSet::new();
        if let Some(a) = self.root.actions.iter().find(|a| &a.name == name) {
            assigned(&a.body, &mut out);
        }
        out
    }

    /// Looks for swaps in `nodes`, outside every row, control, tab panel,
    /// dialog and popover.
    fn walk(&self, nodes: &[Node], out: &mut Vec<LowerError>) {
        for node in nodes {
            match node {
                Node::Element { tag, attrs, .. } => {
                    let closed = CONTROLS.contains(&tag.as_str())
                        || has(attrs, "press")
                        || tag == "dialog"
                        || has(attrs, "popover")
                        || literal(attrs, "role") == Some("tabpanel")
                        || literal(attrs, "display") == Some("none")
                        || computed(attrs, "role");
                    if !closed {
                        lists(node).into_iter().for_each(|l| self.walk(l, out));
                    }
                }
                Node::When { .. } | Node::Match { .. } => {
                    let arms = choice(node);
                    if self.swaps(&arms) {
                        let what = if matches!(node, Node::Match { .. }) {
                            "match"
                        } else {
                            "when"
                        };
                        out.push(LowerError {
                            id: "lower-screen-by-state",
                            message: format!("this `{what}` swaps the page between screens by state, in an app with no `routes`: on the web the browser's Back leaves the app, a reload loses the place and no screen can be linked, and on iOS no screen gets a Back button or the edge swipe. Make each screen a route: `routes` with a path for each, `push` to open one (a list's row pushes its detail) and `back` to return; a wide window then shows the list beside its detail (`contract vocab screen`, docs/recipes/navigation-stack.md)"),
                            span: node.span(),
                        });
                        continue;
                    }
                    for (_, arm) in arms {
                        self.walk(arm, out);
                    }
                }
                // A row's swap is the row's.
                Node::Each { .. } | Node::Children { .. } => {}
                Node::Use { children, .. } => self.walk(children, out),
            }
        }
    }

    /// Whether a choice between `arms` swaps the page's screens: two arms
    /// or more are screens, keyed on states a press inside one moves.
    fn swaps(&self, arms: &[(Option<&Expr>, &[Node])]) -> bool {
        // An arm keyed on a resource or a mutation, and a last `else` after
        // one, is a loading, error or empty view.
        let (mut states, mut data, mut screens) = (BTreeSet::new(), false, Vec::new());
        for &(cond, arm) in arms {
            let keyed = match cond {
                Some(cond) => {
                    let reads = self.reads(cond);
                    data |= reads.data;
                    if !reads.data {
                        states.extend(reads.states);
                    }
                    reads.data
                }
                None => data,
            };
            let mut seen = Seen::default();
            look(arm, &mut seen);
            if !keyed && seen.heading && seen.controls >= 2 {
                screens.push(arm);
            }
        }
        if screens.len() < 2
            || states.is_empty()
            || states
                .iter()
                .any(|s| self.root.persists(s) || self.picked.contains(s))
        {
            return false;
        }
        // A press inside a screen moves the swap: the detail's Back, a row.
        screens.iter().any(|arm| {
            let mut presses = Vec::new();
            pressed(arm, &mut presses);
            presses
                .iter()
                .any(|p| !self.assigns(p).is_disjoint(&states))
        })
    }
}

#[derive(Default)]
struct Reads {
    states: BTreeSet<String>,
    /// A resource or mutation: a loading, error or empty view.
    data: bool,
}

/// What one arm holds.
#[derive(Default)]
struct Seen {
    heading: bool,
    controls: usize,
}

/// A screen's heading (a `text role="heading"` or with `aria-level`) and
/// its controls, outside dialogs, popovers and what never shows. A heading
/// inside a control is the control's.
fn look(nodes: &[Node], seen: &mut Seen) {
    for node in nodes {
        if let Node::Element { tag, attrs, .. } = node {
            if tag == "dialog" || has(attrs, "popover") || literal(attrs, "display") == Some("none")
            {
                continue;
            }
            if CONTROLS.contains(&tag.as_str()) || has(attrs, "press") {
                seen.controls += 1;
                continue;
            }
            if tag == "text"
                && (literal(attrs, "role") == Some("heading") || has(attrs, "aria-level"))
            {
                seen.heading = true;
            }
        }
        lists(node).into_iter().for_each(|l| look(l, seen));
    }
}

/// The `press` and `submit` handlers in `nodes`, outside dialogs and popovers.
fn pressed<'n>(nodes: &'n [Node], out: &mut Vec<&'n Expr>) {
    for node in nodes {
        if let Node::Element { tag, attrs, .. } = node {
            if tag == "dialog" || has(attrs, "popover") {
                continue;
            }
            out.extend(
                attrs
                    .iter()
                    .filter(|a| a.name == "press" || a.name == "submit")
                    .map(|a| &a.value),
            );
        }
        lists(node).into_iter().for_each(|l| pressed(l, out));
    }
}

/// The arms of a `when`, each with its condition (an `else when` chain
/// flattened; with no last `else` it chooses among its arms only), or of a
/// `match`, keyed on its subject.
fn choice(node: &Node) -> Vec<(Option<&Expr>, &[Node])> {
    match node {
        Node::When {
            cond,
            then,
            otherwise,
            ..
        } => {
            let mut arms = vec![(Some(cond), then.as_slice())];
            match otherwise.as_slice() {
                [chained @ Node::When { .. }] => arms.extend(choice(chained)),
                [] => {}
                rest => arms.push((None, rest)),
            }
            arms
        }
        Node::Match {
            subject,
            some,
            none,
            ..
        } => vec![
            (Some(subject), some.1.as_slice()),
            (Some(subject), none.as_slice()),
        ],
        _ => Vec::new(),
    }
}

/// Every child list of a node: an element's children, a choice's arms, a
/// row's body, a use's fill.
fn lists(node: &Node) -> Vec<&[Node]> {
    match node {
        Node::Element { children, .. } | Node::Use { children, .. } => vec![children],
        Node::When {
            then, otherwise, ..
        } => vec![then, otherwise],
        Node::Match { some, none, .. } => vec![&some.1, none],
        Node::Each { body, .. } => vec![body],
        Node::Children { .. } => Vec::new(),
    }
}

/// Every state a statement list assigns, through branches and calls.
fn assigned(body: &[Stmt], out: &mut BTreeSet<String>) {
    for s in body {
        match s {
            Stmt::Assign { target, .. } => {
                out.insert(target.clone());
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                assigned(then, out);
                assigned(otherwise, out);
            }
            Stmt::Match { some, none, .. } => {
                assigned(&some.1, out);
                assigned(none, out);
            }
            Stmt::Call { body, .. } => assigned(body, out),
            Stmt::Let { .. } | Stmt::Command { .. } | Stmt::Send { .. } | Stmt::Refresh { .. } => {}
        }
    }
}

/// Every name an expression reads (a `fn`'s body reads only its
/// parameters, so a call reads its arguments).
fn idents(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Ident(n, _) => out.push(n.clone()),
        Expr::Call(_, args, _) | Expr::List(args, _) => args.iter().for_each(|a| idents(a, out)),
        Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Some(x, _)
        | Expr::Unary(_, x, _)
        | Expr::Arrow { body: x, .. } => idents(x, out),
        Expr::Binary(_, a, b, _) => {
            idents(a, out);
            idents(b, out);
        }
        Expr::Ternary(a, b, c, _) => {
            idents(a, out);
            idents(b, out);
            idents(c, out);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            idents(subject, out);
            idents(some, out);
            idents(none, out);
        }
        Expr::Let { value, body, .. } => {
            idents(value, out);
            idents(body, out);
        }
        Expr::Template(parts, _) => parts.iter().for_each(|p| {
            if let TemplatePart::Expr(x) = p {
                idents(x, out);
            }
        }),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
    }
}
