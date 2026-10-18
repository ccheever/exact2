//! `lower-replace-root`: a `replace` that the router refuses on the screen a
//! launch opens. A launch at `/` opens one root route, `T`, as the only
//! visit of its stack, and while `T` is alone the router keeps the stack's
//! root route: a location matching any other route is refused ("replace
//! cannot change the tab's root route", `exact_route::replace`), the value is
//! returned unchanged and only the log says so. Codex Sol's habit app did
//! this (guard-rail evaluation, 2026-10-16): a welcome at `/` whose Continue
//! replaced it with the main list, a button that did nothing.
//!
//! The claim: if the app boots and draws its launch screen, the button,
//! pressed there before anything else changes the router, runs a `replace`
//! that is refused. What happens before that press is outside it: what a
//! person does (another control, a link), what the world outside sends (an
//! incoming URL) and what native code the app adds does (a module's hatch
//! pressing a control), which the compiler does not read. A refusal while
//! the app boots or draws (a state's initializer, a key) stops it before any
//! press and is its own error. Nor is it a claim that the layout shows the
//! button: one laid out off screen still runs the refused `replace`.
//!
//! So that the press is one a person can make, nothing in the Contract may
//! move the router by itself first: an action that a task, a mutation's
//! `then`, a sequence's `ended` or an element's handler runs (any but a
//! `press` a person makes and the navigation root's `navigate`; a native
//! view's `press` may come by itself) writes the router only with a
//! `replace` refused while `T` is alone or a `back`, and binds no other
//! value to its name. The warning then reads one shape, the one the
//! patterns and that app write:
//! - the navigation root is keyed `${top(nav).id}`, is no link and has no
//!   `press`, and draws `for e in stack(nav)`; `if e.name == "T"` (or
//!   `top(nav).name == "T"`, or the `else` of another route's name) chooses
//!   `T`'s route, keyed `${e.id}`;
//! - a `button` in that route, reached through elements only, none of which
//!   is a link or names another action (they run in the same click), presses
//!   an action, with literal arguments if any, whose one router write is
//!   `nav = replace(nav, L)` and whose other statements assign literals or
//!   run commands with literal arguments;
//! - `L` is a literal or a `path()` of literals that the table matches to a
//!   route other than `T`, and no `fn` of the app takes a name that the
//!   router or `path()` calls.
//!
//! Tabs and every other form are the benefit of the doubt, and a hidden or
//! disabled button is left alone as plainly unused.

use crate::LowerError;
use contract_syntax::{Attr, BinOp, Component, Expr, Node, Span, Stmt, TemplatePart};
use contract_types::Checked;
use exact_route::Table;
use std::collections::BTreeSet;

pub(super) fn root_replace(checked: &Checked<'_>) -> Vec<LowerError> {
    let (Some(decl), Some(table)) = (&checked.file.routes, &checked.types.shapes.routes) else {
        return Vec::new();
    };
    // A function of the app's that takes one of these names is called in
    // its place, `path()`'s `encodeRouteSegment` included.
    let fns = &checked.types.shapes.fns;
    if [
        "replace",
        "back",
        "path",
        "encodeRouteSegment",
        "stack",
        "top",
    ]
    .iter()
    .any(|f| fns.contains_key(*f))
    {
        return Vec::new();
    }
    let chain = table.chain("/");
    let [launched] = chain.as_slice() else {
        return Vec::new();
    };
    let app = App {
        root: &checked.expanded.root,
        slot: &decl.slot,
        table,
        launched: &launched.name,
    };
    let Some((navigation, routes)) = app.navigation_root() else {
        return Vec::new();
    };
    if !app.still_at_launch(navigation) {
        return Vec::new();
    }
    let mut presses = Vec::new();
    app.routes(routes, &mut presses);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for handler in presses {
        let Some((to, span)) = app.refused(handler) else {
            continue;
        };
        if seen.insert((span.source_id, span.line, span.col)) {
            out.push(LowerError {
                id: "lower-replace-root",
                message: app.message(&to),
                span,
            });
        }
    }
    out
}

/// What the app's declarations say.
struct App<'a> {
    root: &'a Component,
    /// The router's state, `nav` in `routes nav`.
    slot: &'a str,
    table: &'a Table,
    /// The root route a launch at `/` opens alone.
    launched: &'a str,
}

/// An element that may be hidden, or not pressable, while it holds this
/// attribute with a value the compiler cannot read or one of these.
const HIDING: &[(&str, &[&str])] = &[
    ("display", &["none"]),
    ("visibility", &["hidden", "collapse"]),
    ("pointer-events", &["none"]),
];

impl App<'_> {
    /// The navigation root, a top-level element keyed `${top(nav).id}` that
    /// nothing hides, with its children.
    fn navigation_root(&self) -> Option<(&[Attr], &[Node])> {
        self.root.view.iter().find_map(|node| match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } if attrs.iter().any(|a| a.name == "navigationKey") => {
                let top = matches!(keyed_by(attrs), Some(Expr::Call(top, args, _))
                    if top == "top" && matches!(args.as_slice(), [a] if self.is_slot(a)));
                // A link or `press` on the root runs in the same click.
                let pressed = link(tag, attrs) || attrs.iter().any(|a| a.name == "press");
                (top && !hidden(tag, attrs) && !pressed)
                    .then_some((attrs.as_slice(), children.as_slice()))
            }
            _ => None,
        })
    }

    /// Whether nothing moves the router by itself before the first press:
    /// every action a task, a mutation's `then`, a sequence's `ended` or an
    /// element's handler other than a `press` runs (the navigation root's
    /// `navigate` hears links and the world outside) writes the router only
    /// with a `replace` refused while the launched route is alone, or a
    /// `back`.
    fn still_at_launch(&self, navigation: &[Attr]) -> bool {
        let r = self.root;
        let mut automatic: Vec<&Expr> = Vec::new();
        handlers(&r.view, &mut automatic);
        automatic.retain(|e| {
            !navigation
                .iter()
                .any(|a| a.name == "navigate" && std::ptr::eq(&a.value, *e))
        });
        let named: BTreeSet<&str> = r
            .tasks
            .iter()
            .map(|t| t.timer.1.as_str())
            .chain(
                r.mutations
                    .iter()
                    .filter_map(|m| Some(m.then.as_ref()?.0.as_str())),
            )
            .chain(
                r.sequences
                    .iter()
                    .filter_map(|s| Some(s.ended.as_ref()?.0.as_str())),
            )
            .collect();
        r.actions
            .iter()
            .filter(|a| {
                named.contains(a.name.as_str())
                    || automatic.iter().any(|e| mentions(e, &|n| n == a.name))
            })
            .all(|a| !self.binds_router(a) && self.only_refused(&a.body))
    }

    /// Whether an action binds the router's name to another value: a
    /// parameter, a `let` or a `match`'s `case some(…)`, which `nav` in it
    /// then reads.
    fn binds_router(&self, action: &contract_syntax::Action) -> bool {
        let mut all = Vec::new();
        every(&action.body, &mut all);
        action.params.iter().any(|p| p.name == self.slot)
            || all.iter().any(|s| match s {
                Stmt::Let { name, .. } => name == self.slot,
                Stmt::Match { some, .. } => some.0 == self.slot,
                _ => false,
            })
    }

    /// Whether every router write in an action is a `replace` the router
    /// refuses while the launched route is alone, or a `back`.
    fn only_refused(&self, body: &[Stmt]) -> bool {
        let mut all = Vec::new();
        every(body, &mut all);
        all.iter().all(|s| match s {
            Stmt::Assign { target, expr, .. } if target == self.slot => match expr {
                Expr::Call(f, args, _) => match (f.as_str(), args.as_slice()) {
                    ("replace", [r, l]) => self.is_slot(r) && self.elsewhere(l).is_some(),
                    ("back", [r]) => self.is_slot(r),
                    _ => false,
                },
                _ => false,
            },
            _ => true,
        })
    }

    fn is_slot(&self, e: &Expr) -> bool {
        matches!(e, Expr::Ident(name, _) if name == self.slot)
    }

    /// Whether an expression names one of the root's actions.
    fn names_action(&self, e: &Expr) -> bool {
        mentions(e, &|n| self.root.actions.iter().any(|a| a.name == n))
    }

    /// The `press` handlers of the launched route's buttons, among the
    /// navigation root's children.
    fn routes<'n>(&self, nodes: &'n [Node], out: &mut Vec<&'n Expr>) {
        for node in nodes {
            if let Node::Each {
                var, list, body, ..
            } = node
            {
                if matches!(list, Expr::Call(name, args, _)
                    if name == "stack" && matches!(args.as_slice(), [a] if self.is_slot(a)))
                {
                    self.choices(body, var, out);
                }
            }
        }
    }

    /// The launched route among a stack visit's choices of route by name,
    /// keyed by the visit's id.
    fn choices<'n>(&self, nodes: &'n [Node], entry: &str, out: &mut Vec<&'n Expr>) {
        for node in nodes {
            let Node::When {
                cond,
                then,
                otherwise,
                ..
            } = node
            else {
                continue;
            };
            match self.route_named(cond, entry) {
                Some(route) if route == self.launched => {
                    for node in then {
                        let Node::Element { attrs, .. } = node else {
                            continue;
                        };
                        if matches!(keyed_by(attrs), Some(Expr::Ident(e, _)) if e == entry) {
                            self.buttons(std::slice::from_ref(node), out);
                        }
                    }
                }
                Some(_) => self.choices(otherwise, entry, out),
                None => {}
            }
        }
    }

    /// The route a condition chooses the visit of: `e.name == "<route>"`
    /// for the stack's visit, or `top(nav).name == "<route>"`.
    fn route_named<'e>(&self, cond: &'e Expr, entry: &str) -> Option<&'e str> {
        let Expr::Binary(BinOp::Eq, a, b, _) = cond else {
            return None;
        };
        let named = |visit: &Expr, value: &'e Expr| match (visit, value) {
            (Expr::Member(base, field, _), Expr::Str(route, _)) if field == "name" => {
                let visit = match &**base {
                    Expr::Ident(e, _) => e == entry,
                    Expr::Call(top, args, _) => {
                        top == "top" && matches!(args.as_slice(), [a] if self.is_slot(a))
                    }
                    _ => false,
                };
                visit.then_some(route.as_str())
            }
            _ => None,
        };
        named(a, b).or_else(|| named(b, a))
    }

    /// The `press` of each `button` reached through elements only, none of
    /// which may hide it or name another action.
    fn buttons<'n>(&self, nodes: &'n [Node], out: &mut Vec<&'n Expr>) {
        for node in nodes {
            let Node::Element {
                tag,
                attrs,
                children,
                ..
            } = node
            else {
                continue;
            };
            // An ancestor's `press` or link, or any other handler, runs in
            // the same gesture.
            let other = link(tag, attrs)
                || attrs
                    .iter()
                    .any(|a| (a.name != "press" || tag != "button") && self.names_action(&a.value));
            if hidden(tag, attrs) || other {
                continue;
            }
            let attr = |name: &str| attrs.iter().rev().find(|a| a.name == name);
            match attr("press") {
                Some(press) => {
                    let enabled =
                        attr("disabled").is_none_or(|a| matches!(a.value, Expr::Bool(false, _)));
                    if enabled && !self.acts(children) {
                        out.push(&press.value);
                    }
                }
                None => self.buttons(children, out),
            }
        }
    }

    /// Whether anything under the nodes names an action, or is not an element.
    fn acts(&self, nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                link(tag, attrs)
                    || attrs.iter().any(|a| self.names_action(&a.value))
                    || self.acts(children)
            }
            _ => true,
        })
    }

    /// The route a location names when it is not the launched one, for a
    /// literal or a `path()` of literals.
    fn elsewhere(&self, location: &Expr) -> Option<String> {
        let url = match location {
            Expr::Str(url, _) => url.clone(),
            Expr::Call(path, args, _) if path == "path" => {
                let [Expr::Str(route, _), rest @ ..] = args.as_slice() else {
                    return None;
                };
                let values = rest
                    .iter()
                    .map(|a| match a {
                        Expr::Str(v, _) => Some(v.as_str()),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                self.table.path(route, &values).ok()?
            }
            _ => return None,
        };
        let to = self.table.matches(&url)?.name;
        (to != self.launched).then_some(to)
    }

    /// The route a pressed action's `replace` names, and where, when the
    /// router refuses it while the launched route is alone.
    fn refused(&self, handler: &Expr) -> Option<(String, Span)> {
        let name = match handler {
            Expr::Ident(name, _) => name,
            Expr::Call(name, args, _) if args.iter().all(literal) => name,
            _ => return None,
        };
        let action = self.root.actions.iter().find(|a| &a.name == name)?;
        if self.binds_router(action) {
            return None;
        }
        let mut statements = Vec::new();
        every(&action.body, &mut statements);
        let mut replaced = None;
        for s in statements {
            match s {
                Stmt::Assign { target, expr, .. } if target == self.slot => {
                    let Expr::Call(verb, args, span) = expr else {
                        return None;
                    };
                    match args.as_slice() {
                        [router, location]
                            if verb == "replace" && self.is_slot(router) && replaced.is_none() =>
                        {
                            replaced = Some((self.elsewhere(location)?, *span));
                        }
                        _ => return None,
                    }
                }
                Stmt::Assign { expr, .. } if literal(expr) => {}
                Stmt::Command { args, .. } if args.iter().all(literal) => {}
                _ => return None,
            }
        }
        replaced
    }

    fn message(&self, to: &str) -> String {
        let (nav, launched) = (self.slot, self.launched);
        format!("if the app boots and draws its launch screen, this `replace`, pressed there before anything else changes the router, is refused and `{to}` does not open: a launch at `/` opens `{launched}` as the only screen of its stack, where the router keeps the stack's root route and returns `{nav}` unchanged (the log says \"replace cannot change the tab's root route\"). A screen the person leaves for good, such as a welcome, is not a route of its own: make the main screen the root route and, while a `persist` flag is unset, show what `{launched}` shows inside it (docs/recipes/first-launch.md). To open `{to}` over `{launched}`, with a Back to it, write `push`")
    }
}

/// Whether an element may be hidden or not pressable.
fn hidden(tag: &str, attrs: &[Attr]) -> bool {
    let attr = |name: &str| attrs.iter().rev().find(|a| a.name == name);
    tag == "dialog"
        || ["popover", "hidden", "inert", "class"]
            .iter()
            .any(|n| attr(n).is_some())
        || attr("opacity").is_some_and(|a| !matches!(a.value, Expr::Number(n, _) if n >= 0.5))
        || HIDING.iter().any(|(name, values)| {
            attr(name).is_some_and(|a| match &a.value {
                Expr::Str(v, _) => values.contains(&v.as_str()),
                _ => true,
            })
        })
}

/// Whether an element is a link, which a click follows.
fn link(tag: &str, attrs: &[Attr]) -> bool {
    tag == "link" || attrs.iter().any(|a| a.name == "href")
}

/// Every attribute value in the view that may run an action by itself:
/// every one but a `press`, which a native view may send by itself too.
fn handlers<'n>(nodes: &'n [Node], out: &mut Vec<&'n Expr>) {
    for node in nodes {
        match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                let native = crate::is_module_tag(tag);
                out.extend(
                    attrs
                        .iter()
                        .filter(|a| a.name != "press" || native)
                        .map(|a| &a.value),
                );
                handlers(children, out);
            }
            Node::When {
                then, otherwise, ..
            } => {
                handlers(then, out);
                handlers(otherwise, out);
            }
            Node::Each { body, .. } => handlers(body, out),
            Node::Match { some, none, .. } => {
                handlers(&some.1, out);
                handlers(none, out);
            }
            Node::Use { args, children, .. } => {
                out.extend(args.iter().map(|a| &a.value));
                handlers(children, out);
            }
            Node::Children { .. } => {}
        }
    }
}

/// An action's statements, through branches and inlined calls.
fn every<'s>(body: &'s [Stmt], out: &mut Vec<&'s Stmt>) {
    for s in body {
        match s {
            Stmt::Call { body, .. } => every(body, out),
            Stmt::If {
                then, otherwise, ..
            } => {
                out.push(s);
                every(then, out);
                every(otherwise, out);
            }
            Stmt::Match { some, none, .. } => {
                out.push(s);
                every(&some.1, out);
                every(none, out);
            }
            other => out.push(other),
        }
    }
}

/// The `X` of an element's `navigationKey=`${X.id}``.
fn keyed_by(attrs: &[Attr]) -> Option<&Expr> {
    let key = attrs.iter().rev().find(|a| a.name == "navigationKey")?;
    match &key.value {
        Expr::Template(parts, _) => match parts.as_slice() {
            [TemplatePart::Expr(Expr::Member(base, id, _))] if id == "id" => Some(base),
            _ => None,
        },
        _ => None,
    }
}

/// Whether an expression names, or calls, a name `is` accepts.
fn mentions(e: &Expr, is: &dyn Fn(&str) -> bool) -> bool {
    match e {
        Expr::Ident(n, _) | Expr::Call(n, _, _) if is(n) => true,
        _ => parts(e).into_iter().any(|x| mentions(x, is)),
    }
}

/// An expression's own subexpressions.
fn parts(e: &Expr) -> Vec<&Expr> {
    match e {
        Expr::Call(_, args, _) | Expr::List(args, _) => args.iter().collect(),
        Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Some(x, _)
        | Expr::Unary(_, x, _)
        | Expr::Arrow { body: x, .. } => vec![x],
        Expr::Binary(_, a, b, _) => vec![a, b],
        Expr::Ternary(a, b, c, _) => vec![a, b, c],
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => vec![subject, some, none],
        Expr::Let { value, body, .. } => vec![value, body],
        Expr::Template(parts, _) => parts
            .iter()
            .filter_map(|p| match p {
                TemplatePart::Expr(x) => Some(x),
                TemplatePart::Text(_) => None,
            })
            .collect(),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => {
            Vec::new()
        }
    }
}

/// A value written out, which evaluates without reading or refusing.
fn literal(e: &Expr) -> bool {
    match e {
        Expr::Str(..) | Expr::Number(..) | Expr::Bool(..) | Expr::None(_) => true,
        Expr::List(items, _) => items.iter().all(literal),
        Expr::NamedArg(_, value, _) | Expr::Typed(value, _, _) | Expr::Some(value, _) => {
            literal(value)
        }
        _ => false,
    }
}
