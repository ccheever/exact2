//! `lower-screen-by-state` (LLP 1116.005 D1.1): an `if`/`else`, complementary
//! sibling `if`s or a `match` that swaps the page between screens by state,
//! at the top of the view or inside a route. On the web such a screen has no
//! URL: the browser's Back leaves the app, a reload loses the place and
//! nothing can be linked (rounds 7 and 34 of LLP 1116).
//!
//! A screen is an arm with a heading and two controls or more. A state
//! compared with three string literals or more across one choice, in an
//! `else if` chain or in sibling `if`s (`page == "detail"`), names pages:
//! their arms need no heading, and the state may persist.
//!
//! Ordinary state stays quiet: tabs (a state a `tab` or `radio` reads), a
//! loading, error or empty view (a condition on a resource or a mutation,
//! or an arm of one control or none), a kept setting (a `persist` state),
//! disclosure and an arm with no heading, anything inside a row, a button,
//! a tab panel (other than a route's), a dialog or a popover. A viewport read
//! that two sibling `if`s share as the same `or` term is the split's, not a
//! loading view's: it is set aside and the state parts are judged. A swap
//! counts only when a press in one of its arms moves it: the detail's Back,
//! the list's row, a form's Cancel. The outermost swap is named once.

use super::{computed, has, literal};
use crate::LowerError;
use contract_syntax::{BinOp, Component, Expr, Node, Stmt, TemplatePart, UnOp};
use contract_types::Checked;
use std::collections::BTreeSet;

/// The tags that are controls a person operates.
const CONTROLS: &[&str] = &["button", "link", "input", "select", "textarea"];

/// Every warning: at most one a swap, and none inside one that warned.
pub(super) fn screen_by_state(checked: &Checked<'_>) -> Vec<LowerError> {
    let root = &checked.expanded.root;
    // A game's screens are its art.
    if root.resources.iter().any(|r| r.source == "exactSurface") {
        return Vec::new();
    }
    let app = App::new(root);
    let mut out = Vec::new();
    app.walk(&root.view, false, &mut out);
    out
}

/// What the expanded root's names are.
struct App<'a> {
    root: &'a Component,
    /// The root's actions by name, the first of each.
    actions: std::collections::HashMap<&'a str, &'a contract_syntax::Action>,
    /// States a `tab` or a `radio` reads its selection from.
    picked: BTreeSet<String>,
}

impl<'a> App<'a> {
    fn new(root: &'a Component) -> Self {
        let mut actions = std::collections::HashMap::new();
        for a in &root.actions {
            actions.entry(a.name.as_str()).or_insert(a);
        }
        let mut app = App {
            root,
            actions,
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
            } else if let Some(resource) = r.resources.iter().find(|x| x.name == name) {
                if resource.source == "exactViewport" {
                    reads.viewport = true;
                } else {
                    reads.data = true;
                }
            } else if r.mutations.iter().any(|m| m.name == name) {
                reads.data = true;
            } else {
                reads.unknown = true;
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
        if let Some(a) = self.actions.get(name.as_str()) {
            assigned(&a.body, &mut out);
        }
        out
    }

    /// Looks for swaps in `nodes`, outside every row, control, tab panel,
    /// dialog and popover, and inside a route's screens: `routed` is inside
    /// a loop over the router's stacks, whose tab panels are routes'.
    fn walk(&self, nodes: &[Node], routed: bool, out: &mut Vec<LowerError>) {
        let mut paired = BTreeSet::new();
        for (i, node) in nodes.iter().enumerate() {
            if paired.contains(&i) {
                continue;
            }
            match node {
                Node::Element { tag, attrs, .. } => {
                    let closed = CONTROLS.contains(&tag.as_str())
                        || has(attrs, "press")
                        || tag == "dialog"
                        || has(attrs, "popover")
                        || (!routed && literal(attrs, "role") == Some("tabpanel"))
                        || literal(attrs, "display") == Some("none")
                        || computed(attrs, "role");
                    if !closed {
                        lists(node)
                            .into_iter()
                            .for_each(|l| self.walk(l, routed, out));
                    }
                }
                Node::When { .. } | Node::Match { .. } => {
                    let mut arms = choice(node);
                    let named = single_when(node).and_then(|(cond, then)| {
                        let (state, page, test) = self.page_test(cond)?;
                        let mut group = vec![(Some(test), then)];
                        let mut pages = BTreeSet::from([page]);
                        let mut members = Vec::new();
                        for (j, sibling) in nodes.iter().enumerate().skip(i + 1) {
                            let Some((other, body)) = single_when(sibling) else {
                                continue;
                            };
                            if paired.contains(&j) {
                                continue;
                            }
                            match self.page_test(other) {
                                Some((s, p, t)) if s == state && pages.insert(p) => {
                                    group.push((Some(t), body));
                                    members.push(j);
                                }
                                _ => {}
                            }
                        }
                        (group.len() >= 2 && self.swaps(&group)).then_some((group, members))
                    });
                    if let Some((group, members)) = named {
                        arms = group;
                        paired.extend(members);
                    } else if let Some((cond, then)) = single_when(node) {
                        for (j, sibling) in nodes.iter().enumerate().skip(i + 1) {
                            if paired.contains(&j) {
                                continue;
                            }
                            if let Some((other, body)) = single_when(sibling) {
                                if let Some((a, b)) = self.complementary(cond, other) {
                                    let pair = vec![(Some(a), then), (Some(b), body)];
                                    if self.swaps(&pair) {
                                        arms = pair;
                                        paired.insert(j);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if self.swaps(&arms) {
                        let what = if matches!(node, Node::Match { .. }) {
                            "match"
                        } else {
                            "if"
                        };
                        out.push(LowerError {
                            id: "lower-screen-by-state",
                            message: format!("this `{what}` swaps the page between screens by state: on the web the browser's Back leaves the app, a reload loses the place and no screen can be linked, and on iOS no screen gets a Back button or the edge swipe. Make each screen a route: `routes` with a path for each, `push` to open one (a list's row pushes its detail) and `back` to return; a wide window then shows the list beside its detail (`contract vocab screen`, docs/recipes/navigation-stack.md). Top-level sections are tabs, each a route (docs/recipes/tabs.md)"),
                            span: node.span(),
                        });
                        continue;
                    }
                    for (_, arm) in arms {
                        self.walk(arm, routed, out);
                    }
                }
                // A route's screens are walked; a row's swap is the row's.
                Node::Each { list, body, .. } if route_stack(list) => self.walk(body, true, out),
                Node::Each { .. } | Node::Children { .. } => {}
                Node::Use { children, .. } => self.walk(children, routed, out),
            }
        }
    }

    /// The page a condition names, `page == "detail"` on a state of the
    /// root, alone or among `and`/`or` terms: the state, the page and the
    /// comparison.
    fn page_test<'e>(&self, e: &'e Expr) -> Option<(&'e str, &'e str, &'e Expr)> {
        match e {
            Expr::Binary(BinOp::Eq, a, b, _) => match (&**a, &**b) {
                (Expr::Ident(state, _), Expr::Str(page, _))
                | (Expr::Str(page, _), Expr::Ident(state, _))
                    if self.root.states.iter().any(|s| &s.name == state) =>
                {
                    Some((state, page, e))
                }
                _ => None,
            },
            Expr::Binary(BinOp::And | BinOp::Or, a, b, _) => {
                self.page_test(a).or_else(|| self.page_test(b))
            }
            _ => None,
        }
    }

    /// Whether a choice's arms compare one state with three pages or more.
    fn names_pages(&self, arms: &[(Option<&Expr>, &[Node])]) -> bool {
        let mut pages: std::collections::HashMap<&str, BTreeSet<&str>> =
            std::collections::HashMap::new();
        for (state, page, _) in arms.iter().filter_map(|(c, _)| self.page_test((*c)?)) {
            pages.entry(state).or_default().insert(page);
        }
        pages.values().any(|p| p.len() >= 3)
    }

    fn viewport_only(&self, cond: &Expr) -> bool {
        let reads = self.reads(cond);
        reads.viewport && !reads.data && !reads.unknown && reads.states.is_empty()
    }

    fn complementary<'e>(&self, a: &'e Expr, b: &'e Expr) -> Option<(&'e Expr, &'e Expr)> {
        if complements(a, b) {
            return Some((a, b));
        }
        if let (Expr::Binary(BinOp::Or, av, ap, _), Expr::Binary(BinOp::Or, bv, bp, _)) = (a, b) {
            // Both panes coexist under the same viewport predicate. Different
            // predicates do not establish a split, even if the states oppose.
            for (av, ap) in [(av, ap), (ap, av)] {
                for (bv, bp) in [(bv, bp), (bp, bv)] {
                    if same(av, bv) && self.viewport_only(av) && complements(ap, bp) {
                        return Some((ap, bp));
                    }
                }
            }
        }
        None
    }

    /// Whether a choice between `arms` swaps the page's screens: two arms
    /// or more are screens, keyed on states a press inside one moves.
    fn swaps(&self, arms: &[(Option<&Expr>, &[Node])]) -> bool {
        let named = self.names_pages(arms);
        // An arm keyed on a resource or a mutation, and a last `else` after
        // one, is a loading, error or empty view. Viewport reads count here too:
        // only a sibling split has its shared viewport predicate removed first.
        let (mut states, mut data, mut screens) = (BTreeSet::new(), false, Vec::new());
        for &(cond, arm) in arms {
            let keyed = match cond {
                Some(cond) => {
                    let reads = self.reads(cond);
                    let keyed = reads.data || reads.viewport;
                    data |= keyed;
                    if !keyed {
                        states.extend(reads.states);
                    }
                    keyed
                }
                None => data,
            };
            let mut seen = Seen::default();
            look(arm, &mut seen);
            if !keyed && (seen.heading || named) && seen.controls >= 2 {
                screens.push(arm);
            }
        }
        if screens.len() < 2
            || states.is_empty()
            || states
                .iter()
                .any(|s| (!named && self.root.persists(s)) || self.picked.contains(s))
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
    /// A resource other than the viewport, or a mutation: loading, error or empty.
    data: bool,
    viewport: bool,
    unknown: bool,
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

/// Whether a `for` walks the router's screens: `stack(nav)`, a tab's
/// `t.stack` or the tabs, `nav.tabs`.
fn route_stack(list: &Expr) -> bool {
    match list {
        Expr::Call(name, _, _) => name == "stack",
        Expr::Member(_, field, _) => field == "stack" || field == "tabs",
        _ => false,
    }
}

fn single_when(node: &Node) -> Option<(&Expr, &[Node])> {
    match node {
        Node::When {
            cond,
            then,
            otherwise,
            ..
        } if otherwise.is_empty() => Some((cond, then)),
        _ => None,
    }
}

fn complements(a: &Expr, b: &Expr) -> bool {
    if matches!(a, Expr::Unary(UnOp::Not, x, _) if same(x, b))
        || matches!(b, Expr::Unary(UnOp::Not, x, _) if same(a, x))
    {
        return true;
    }
    match (a, b) {
        (Expr::Binary(aop, al, ar, _), Expr::Binary(bop, bl, br, _))
            if matches!((aop, bop), (BinOp::Eq, BinOp::Ne) | (BinOp::Ne, BinOp::Eq)) =>
        {
            same(al, bl) && same(ar, br)
        }
        _ => false,
    }
}

/// Compare simple predicates without their source spans. Unrecognised forms
/// stay quiet; this does not try to prove arbitrary expressions equivalent.
fn same(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Ident(a, _), Expr::Ident(b, _)) | (Expr::Str(a, _), Expr::Str(b, _)) => a == b,
        (Expr::Number(a, _), Expr::Number(b, _)) => a == b,
        (Expr::Bool(a, _), Expr::Bool(b, _)) => a == b,
        (Expr::None(_), Expr::None(_)) => true,
        (Expr::Some(a, _), Expr::Some(b, _)) => same(a, b),
        (Expr::List(a, _), Expr::List(b, _)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (Expr::Ternary(ac, at, af, _), Expr::Ternary(bc, bt, bf, _)) => {
            same(ac, bc) && same(at, bt) && same(af, bf)
        }
        (Expr::Member(a, af, _), Expr::Member(b, bf, _)) => af == bf && same(a, b),
        (Expr::Unary(ao, a, _), Expr::Unary(bo, b, _)) => ao == bo && same(a, b),
        (Expr::Binary(ao, al, ar, _), Expr::Binary(bo, bl, br, _)) => {
            ao == bo && same(al, bl) && same(ar, br)
        }
        (Expr::Call(an, aa, _), Expr::Call(bn, ba, _)) => {
            an == bn && aa.len() == ba.len() && aa.iter().zip(ba).all(|(a, b)| same(a, b))
        }
        _ => false,
    }
}

/// The arms of an `if`, each with its condition (an `else if` chain
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
pub(super) fn idents(e: &Expr, out: &mut Vec<String>) {
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
