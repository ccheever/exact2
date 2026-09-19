//! Analysis: what the types do not say.
//!
//! @ref LLP 1004 D2 (effect signatures, `writes` checking, region structure)
//! / LLP 0508 §10.3 (actions as reducers with declared writes; research)
//!
//! After types, a program can still be wrong in ways an implementer would
//! otherwise discover at runtime: an action writing a slot it did not declare,
//! a handler naming an action that does not exist or with the wrong number of
//! curried arguments, a `task` ticking an unknown action, derives that depend
//! on each other in a cycle, or a child component carrying state — which v1
//! does not support (all state lives in the root; children are pure views over
//! their props). Every rejection carries a stable id and a span.
//!
//! An `action` prop's arity is inferred from the component's own invocations
//! of it and checked at every use site's binding (LLP 1035.005 D2): a
//! mismatch names both sides.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use contract_syntax::{Component, Expr, File, Node, Related, Span, Stmt};
use contract_types::{Ref, Scope, Ty, Types};
use std::collections::{BTreeMap, BTreeSet};

/// A typed rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeError {
    /// Stable id.
    pub id: &'static str,
    /// What went wrong.
    pub message: String,
    /// Where.
    pub span: Span,
    /// The other places the rejection names — for `analyze-action-arity`,
    /// the prop's declaration and the caller's binding.
    pub related: Vec<Related>,
}

impl std::fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] {}", self.span, self.id, self.message)?;
        for r in &self.related {
            write!(f, "\n  {}: {}", r.span, r.note)?;
        }
        Ok(())
    }
}

fn err<T>(id: &'static str, message: impl Into<String>, span: Span) -> Result<T, AnalyzeError> {
    Err(AnalyzeError {
        id,
        message: message.into(),
        span,
        related: Vec::new(),
    })
}

/// What analysis established beyond the types. Every rule analysis checks
/// is a rejection or nothing, so this carries no data yet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Analysis {}

/// Check before merging `use` files so imported declarations cannot claim
/// the app's root slot. @ref LLP 1038 D2/D3.
pub fn check_routes_root(file: &File, root_file: bool) -> Result<(), AnalyzeError> {
    if let Some(routes) = &file.routes {
        if !root_file || file.components.is_empty() {
            return err(
                "analyze-routes-not-root",
                "`routes` belongs to the app's root file",
                routes.span,
            );
        }
    }
    Ok(())
}

/// Check a file against its types.
pub fn check(file: &File, types: &Types) -> Result<Analysis, AnalyzeError> {
    check_routes_root(file, true)?;
    let Some(root) = file.components.first() else {
        return err(
            "analyze-no-component",
            "a file needs a component",
            Span {
                line: 1,
                col: 1,
                end_col: 1,
            },
        );
    };
    if !root.props.is_empty() {
        return err(
            "analyze-root-props",
            "the root component (the first in the file) takes no props",
            root.span,
        );
    }
    // A child may own `state`, `derive`, and `action` (LLP 1017 P4c); that it
    // owns no `resource`, `mutation`, or `task` is the type pass's refusal
    // (`type-child-resource`), made before its view is checked.
    let expanded = contract_syntax::expand(file).map_err(|e| AnalyzeError {
        id: e.id,
        message: e.message,
        span: e.span,
        related: Vec::new(),
    })?;
    check_controls(&expanded.root.view, false)?;
    for (ci, c) in file.components.iter().enumerate() {
        let ct = &types.components[ci];
        let scoped = if ci == 0 { &expanded.root } else { c };
        let scope = types.component_scope(scoped, ct);
        check_actions(scoped)?;
        check_tasks(c)?;
        check_view(&c.view, &scope, file)?;
    }
    let arities = infer_arities(file, types)?;
    check_bindings(file, types, &arities)?;
    Ok(Analysis {})
}

/// How many arguments a handler's event adds to its action's: `scroll` two
/// (`scrollLeft`, `scrollTop`), the payload-carrying handlers one.
fn payload_count(attr: &str) -> usize {
    if attr == "scroll" {
        2
    } else {
        usize::from(handler_payload(attr).is_some())
    }
}

/// Where a component invokes or passes on an action: a handler on one of
/// its elements, or an argument to a child's prop.
enum Site<'a> {
    /// `attr=name` or `attr=name(args)` on an element.
    Handler {
        /// The handler attribute.
        attr: &'a str,
        /// The action or prop named.
        name: &'a str,
        /// Arguments written.
        given: usize,
        /// Where.
        span: Span,
    },
    /// `Child(prop=value)`: a binding at a use site.
    Bound {
        /// The child, by index in the file.
        component: usize,
        /// The prop, by index in the child.
        prop: usize,
        /// The bound value.
        value: &'a Expr,
        /// Where the binding is.
        span: Span,
    },
}

/// Every site in `nodes`, with the scope it resolves names in.
fn sites<'a>(nodes: &'a [Node], scope: &Scope, file: &File, out: &mut Vec<(Scope, Site<'a>)>) {
    for n in nodes {
        match n {
            Node::Provide { body, .. } => sites(body, scope, file, out),
            Node::Children { .. } => {}
            Node::Element {
                attrs, children, ..
            } => {
                for a in attrs {
                    // `navigate` takes or ignores its URL (LLP 1038), so
                    // it fixes no single arity. Lower checks its bound action.
                    if a.name == "navigate" || !HANDLERS.contains(&a.name.as_str()) {
                        continue;
                    }
                    let (name, given) = match &a.value {
                        Expr::Ident(n, _) => (n.as_str(), 0),
                        Expr::Call(n, args, _) => (n.as_str(), args.len()),
                        _ => continue,
                    };
                    out.push((
                        scope.clone(),
                        Site::Handler {
                            attr: &a.name,
                            name,
                            given,
                            span: a.span,
                        },
                    ));
                }
                sites(children, scope, file, out);
            }
            Node::Use {
                name,
                args,
                children,
                ..
            } => {
                sites(children, scope, file, out);
                let Some(component) = file.components.iter().position(|c| &c.name == name) else {
                    continue;
                };
                for a in args {
                    if let Some(prop) = file.components[component]
                        .props
                        .iter()
                        .position(|p| p.name == a.name)
                    {
                        out.push((
                            scope.clone(),
                            Site::Bound {
                                component,
                                prop,
                                value: &a.value,
                                span: a.span,
                            },
                        ));
                    }
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                sites(then, scope, file, out);
                sites(otherwise, scope, file, out);
            }
            Node::Each {
                var, list, body, ..
            } => {
                let item = match contract_types::infer(list, scope, &Default::default()).ok() {
                    Some(Ty::List(item)) => *item,
                    _ => Ty::Unknown,
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), item)));
                sites(body, &inner, file, out);
            }
            Node::Match { some, none, .. } => {
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), Ty::Unknown)));
                sites(&some.1, &inner, file, out);
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                sites(none, &none_scope, file, out);
            }
        }
    }
}

/// `(component, prop)` → the arity its own invocations fix, and the
/// invocation that fixed it.
type Arities = BTreeMap<(usize, usize), (usize, Span)>;

/// A name resolving to one of the component's own `action` props.
fn own_prop(scope: &Scope, c: &Component, name: &str) -> Option<usize> {
    match scope.lookup(name) {
        Some((Ref::Prop(pi), Ty::Action(_))) if (pi as usize) < c.props.len() => Some(pi as usize),
        _ => None,
    }
}

/// The arity of every `action` prop, from the component's own invocations:
/// a handler's arguments plus its event's payload, or — for a prop passed on
/// to a child — the child's arity plus the arguments bound. A prop invoked
/// two ways is `analyze-action-arity` at the second.
fn infer_arities(file: &File, types: &Types) -> Result<Arities, AnalyzeError> {
    let mut arities = Arities::new();
    // A prop passed on takes the child's arity, which may itself be passed
    // on: repeat until nothing new is learned, at most once per component.
    for _ in 0..=file.components.len() {
        let known = arities.len();
        for (ci, c) in file.components.iter().enumerate() {
            let scope = types.component_scope(c, &types.components[ci]);
            let mut found = Vec::new();
            sites(&c.view, &scope, file, &mut found);
            for (scope, site) in found {
                let (pi, arity, span) = match site {
                    Site::Handler {
                        attr,
                        name,
                        given,
                        span,
                    } => match own_prop(&scope, c, name) {
                        Some(pi) => (pi, given + payload_count(attr), span),
                        None => continue,
                    },
                    Site::Bound {
                        component,
                        prop,
                        value,
                        span,
                    } => {
                        let (name, given) = match value {
                            Expr::Ident(n, _) => (n.as_str(), 0),
                            Expr::Call(n, args, _) => (n.as_str(), args.len()),
                            _ => continue,
                        };
                        let (Some(pi), Some((child, _))) =
                            (own_prop(&scope, c, name), arities.get(&(component, prop)))
                        else {
                            continue;
                        };
                        (pi, given + child, span)
                    }
                };
                match arities.get(&(ci, pi)) {
                    Some((first, at)) if *first != arity => {
                        return Err(AnalyzeError {
                            id: "analyze-action-arity",
                            message: format!(
                                "`{}` is invoked with {arity} argument(s) here and with {first} at {at}",
                                c.props[pi].name
                            ),
                            span,
                            related: vec![
                                Related {
                                    span: c.props[pi].span,
                                    note: format!("`{}` declared here", c.props[pi].name),
                                },
                                Related {
                                    span: *at,
                                    note: format!("invoked with {first} here"),
                                },
                            ],
                        });
                    }
                    Some(_) => {}
                    None => {
                        arities.insert((ci, pi), (arity, span));
                    }
                }
            }
        }
        if arities.len() == known {
            break;
        }
    }
    Ok(arities)
}

/// Every use site's binding of an `action` prop against the arity the
/// child's invocations fixed (LLP 1017 P1: no quiet failures).
fn check_bindings(file: &File, types: &Types, arities: &Arities) -> Result<(), AnalyzeError> {
    for (ci, c) in file.components.iter().enumerate() {
        let scope = types.component_scope(c, &types.components[ci]);
        let mut found = Vec::new();
        sites(&c.view, &scope, file, &mut found);
        for (scope, site) in found {
            let Site::Bound {
                component,
                prop,
                value,
                span,
            } = site
            else {
                continue;
            };
            let Some((wanted, at)) = arities.get(&(component, prop)) else {
                continue;
            };
            let (name, given) = match value {
                Expr::Ident(n, _) => (n.as_str(), 0),
                Expr::Call(n, args, _) => (n.as_str(), args.len()),
                _ => continue,
            };
            let takes = match scope.lookup(name) {
                Some((Ref::Action(_), Ty::Action(params))) => params.len(),
                Some((Ref::Prop(pi), Ty::Action(_))) => match arities.get(&(ci, pi as usize)) {
                    Some((n, _)) => *n,
                    None => continue,
                },
                _ => continue,
            };
            let bound = takes.saturating_sub(given);
            if bound == *wanted && given <= takes {
                continue;
            }
            let child = &file.components[component];
            let how = if given == 0 {
                format!("`{name}`, which takes {takes}")
            } else {
                format!("`{name}` with {given} of its {takes} parameter(s) bound, leaving {bound}")
            };
            return Err(AnalyzeError {
                id: "analyze-action-arity",
                message: format!(
                    "`{}` of `{}` is invoked with {wanted} argument(s) here, but `{}` binds it to {how}",
                    child.props[prop].name, child.name, c.name
                ),
                span: *at,
                related: vec![
                    Related {
                        span: child.props[prop].span,
                        note: format!("`{}` declared here", child.props[prop].name),
                    },
                    Related {
                        span,
                        note: format!("bound to `{name}` here"),
                    },
                ],
            });
        }
    }
    Ok(())
}

fn check_actions(c: &Component) -> Result<(), AnalyzeError> {
    for a in &c.actions {
        let mut declared = BTreeSet::new();
        for (w, span) in &a.writes {
            if !c.states.iter().any(|s| &s.name == w) && !c.mutations.iter().any(|m| &m.name == w) {
                return err(
                    "analyze-writes-unknown-state",
                    format!("`{w}` in `writes` is not a state or a mutation"),
                    *span,
                );
            }
            if !declared.insert(w.clone()) {
                return err(
                    "analyze-writes-duplicate",
                    format!("`{w}` listed twice in `writes`"),
                    *span,
                );
            }
        }
        let mut targets = Vec::new();
        writes_of(&a.body, &mut targets);
        for (target, span, how) in targets {
            if !declared.contains(target) {
                return err(
                    "analyze-write-not-declared",
                    format!(
                        "`{}` {how} `{target}` but does not declare it: add `writes {target}`",
                        a.name
                    ),
                    *span,
                );
            }
        }
    }
    Ok(())
}

/// Every slot an action body writes or sends, through every branch of its
/// `if`s and `match`es (LLP 1017 P2): `writes` covers the whole body.
fn writes_of<'a>(stmts: &'a [Stmt], out: &mut Vec<(&'a String, &'a Span, &'static str)>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign { target, span, .. } => out.push((target, span, "writes")),
            Stmt::Send { target, span, .. } => out.push((target, span, "sends")),
            Stmt::If {
                then, otherwise, ..
            } => {
                writes_of(then, out);
                writes_of(otherwise, out);
            }
            Stmt::Match { some, none, .. } => {
                writes_of(&some.1, out);
                writes_of(none, out);
            }
            Stmt::Command { .. } | Stmt::Refresh { .. } => {}
        }
    }
}

fn check_tasks(c: &Component) -> Result<(), AnalyzeError> {
    for t in &c.tasks {
        let Some(a) = c.actions.iter().find(|a| a.name == t.every.1) else {
            return err(
                "analyze-unknown-action",
                format!("`{}` is not an action", t.every.1),
                t.every.2,
            );
        };
        if !a.params.is_empty() {
            return err(
                "analyze-handler-arity",
                format!(
                    "`{}` takes {} parameter(s); a timer passes none",
                    a.name,
                    a.params.len()
                ),
                t.every.2,
            );
        }
    }
    Ok(())
}

/// The handler attributes (the web's events, LLP 1005 §3): `press`,
/// `change`, `hover`, `focus`, `blur`, `key`, `submit`, `load`, `message`.
pub const HANDLERS: [&str; 14] = [
    "press",
    "change",
    "hover",
    "focus",
    "blur",
    "key",
    "submit",
    "load",
    "message",
    "contextmenu",
    "dblclick",
    "swiperight",
    "scroll",
    "navigate",
];

/// What a handler's event carries as its action's last argument: `change`
/// the new text, `hover` whether the pointer is over, `key` the key's name,
/// `message` the posted string; the others nothing.
pub fn handler_payload(attr: &str) -> Option<&'static str> {
    match attr {
        "change" | "key" | "message" | "navigate" => Some("string"),
        "hover" => Some("bool"),
        _ => None,
    }
}

fn check_view(nodes: &[Node], scope: &Scope, file: &File) -> Result<(), AnalyzeError> {
    for n in nodes {
        match n {
            Node::Provide { body, .. } => check_view(body, scope, file)?,
            Node::Children { .. } => {}
            Node::Element {
                attrs, children, ..
            } => {
                for a in attrs {
                    if HANDLERS.contains(&a.name.as_str()) {
                        check_handler(&a.name, &a.value, scope, a.span)?;
                    }
                }
                check_view(children, scope, file)?;
            }
            Node::Use {
                name,
                args,
                children,
                span,
            } => {
                check_view(children, scope, file)?;
                if !file.components.iter().any(|x| &x.name == name) {
                    return err(
                        "analyze-unknown-component",
                        format!("unknown component `{name}`"),
                        *span,
                    );
                }
                let mut seen = BTreeSet::new();
                for a in args {
                    if !seen.insert(a.name.clone()) {
                        return err(
                            "analyze-duplicate-arg",
                            format!("`{}` given twice", a.name),
                            a.span,
                        );
                    }
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                check_view(then, scope, file)?;
                check_view(otherwise, scope, file)?;
            }
            Node::Each {
                var, list, body, ..
            } => {
                let t = contract_types::infer(list, scope, &Default::default()).ok();
                let item = match t {
                    Some(Ty::List(item)) => *item,
                    _ => Ty::Unknown,
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), item)));
                check_view(body, &inner, file)?;
            }
            Node::Match { some, none, .. } => {
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), Ty::Unknown)));
                check_view(&some.1, &inner, file)?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                check_view(none, &none_scope, file)?;
            }
        }
    }
    Ok(())
}

fn check_handler(attr: &str, value: &Expr, scope: &Scope, span: Span) -> Result<(), AnalyzeError> {
    let (name, given) = match value {
        Expr::Ident(n, _) => (n.as_str(), 0usize),
        Expr::Call(n, args, _) => (n.as_str(), args.len()),
        _ => {
            return err(
                "analyze-handler-shape",
                format!("`{attr}=` needs an action name or `action(args)`"),
                span,
            )
        }
    };
    let Some((r, t)) = scope.lookup(name) else {
        return err(
            "analyze-unknown-action",
            format!("`{name}` is not an action"),
            span,
        );
    };
    let Ty::Action(params) = t else {
        return err(
            "analyze-unknown-action",
            format!("`{name}` is not an action"),
            span,
        );
    };
    if !matches!(r, Ref::Action(_) | Ref::Prop(_)) {
        return err(
            "analyze-unknown-action",
            format!("`{name}` is not an action"),
            span,
        );
    }
    // A prop of bare `action` type has unknown arity; only a real action is checked.
    if matches!(r, Ref::Action(_)) {
        let payload = if attr == "scroll" {
            2
        } else {
            usize::from(handler_payload(attr).is_some())
        };
        let valid = if attr == "navigate" {
            given == 0 && params.len() <= 1
        } else {
            given + payload == params.len()
        };
        if !valid {
            return err(
                "analyze-handler-arity",
                format!(
                    "`{name}` takes {} parameter(s); `{attr}=` supplies {given}{}",
                    params.len(),
                    match handler_payload(attr) {
                        Some("bool") => " plus whether the pointer is over",
                        Some(_) if attr == "key" => " plus the key's name",
                        Some(_) if attr == "message" => " plus the message",
                        Some(_) => " plus the new value",
                        None if attr == "scroll" => " plus scrollLeft and scrollTop",
                        None => "",
                    }
                ),
                span,
            );
        }
    }
    Ok(())
}

// Check the expanded tree: a component can supply a canvas's controls.
fn check_controls(nodes: &[Node], in_canvas: bool) -> Result<(), AnalyzeError> {
    for node in nodes {
        match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                for attr in attrs.iter().filter(|a| a.name == "action") {
                    if !in_canvas || tag != "button" {
                        return err(
                            "analyze-control-parent",
                            "`action` requires a button inside a canvas",
                            attr.span,
                        );
                    }
                }
                check_controls(children, in_canvas || tag == "canvas")?;
            }
            Node::Provide { body, .. } | Node::Each { body, .. } => {
                check_controls(body, in_canvas)?
            }
            Node::When {
                then, otherwise, ..
            } => {
                check_controls(then, in_canvas)?;
                check_controls(otherwise, in_canvas)?;
            }
            Node::Match { some, none, .. } => {
                check_controls(&some.1, in_canvas)?;
                check_controls(none, in_canvas)?;
            }
            Node::Use { children, .. } => check_controls(children, in_canvas)?,
            Node::Children { .. } => {}
        }
    }
    Ok(())
}
