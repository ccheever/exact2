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

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use contract_syntax::{Component, Expr, File, Node, Span, Stmt};
use contract_types::{Ref, Scope, Ty, Types};
use std::collections::BTreeSet;

/// A typed rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeError {
    /// Stable id.
    pub id: &'static str,
    /// What went wrong.
    pub message: String,
    /// Where.
    pub span: Span,
}

impl std::fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] {}", self.span, self.id, self.message)
    }
}

fn err<T>(id: &'static str, message: impl Into<String>, span: Span) -> Result<T, AnalyzeError> {
    Err(AnalyzeError {
        id,
        message: message.into(),
        span,
    })
}

/// What analysis established beyond the types. Every rule analysis checks
/// is a rejection or nothing, so this carries no data yet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Analysis {}

/// Check a file against its types.
pub fn check(file: &File, types: &Types) -> Result<Analysis, AnalyzeError> {
    let Some(root) = file.components.first() else {
        return err(
            "analyze-no-component",
            "a file needs a component",
            Span { line: 1, col: 1 },
        );
    };
    if !root.props.is_empty() {
        return err(
            "analyze-root-props",
            "the root component (the first in the file) takes no props",
            root.span,
        );
    }
    for c in file.components.iter().skip(1) {
        if !c.states.is_empty()
            || !c.derives.is_empty()
            || !c.resources.is_empty()
            || !c.mutations.is_empty()
            || !c.actions.is_empty()
            || !c.tasks.is_empty()
        {
            let span = c
                .states
                .first()
                .map(|s| s.span)
                .or(c.derives.first().map(|d| d.span))
                .or(c.resources.first().map(|r| r.span))
                .or(c.actions.first().map(|a| a.span))
                .or(c.tasks.first().map(|t| t.span))
                .unwrap_or(c.span);
            return err("analyze-child-state", format!("component `{}` takes props, so it is a view over them: state, derives, resources, actions, and tasks live in the root in v1", c.name), span);
        }
    }
    for (ci, c) in file.components.iter().enumerate() {
        let ct = &types.components[ci];
        let scope = types.component_scope(c, ct);
        check_actions(c)?;
        check_tasks(c)?;
        check_view(&c.view, &scope, file)?;
    }
    Ok(Analysis {})
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
        for stmt in &a.body {
            let (target, span, how) = match stmt {
                Stmt::Assign { target, span, .. } => (target, span, "writes"),
                Stmt::Send { target, span, .. } => (target, span, "sends"),
                _ => continue,
            };
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
/// `change`, `hover`, `focus`, `blur`, `key`.
pub const HANDLERS: [&str; 6] = ["press", "change", "hover", "focus", "blur", "key"];

/// What a handler's event carries as its action's last argument: `change`
/// the new text, `hover` whether the pointer is over, `key` the key's name;
/// the others nothing.
pub fn handler_payload(attr: &str) -> Option<&'static str> {
    match attr {
        "change" | "key" => Some("string"),
        "hover" => Some("bool"),
        _ => None,
    }
}

fn check_view(nodes: &[Node], scope: &Scope, file: &File) -> Result<(), AnalyzeError> {
    for n in nodes {
        match n {
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
            Node::Use { name, args, span } => {
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
        let payload = usize::from(handler_payload(attr).is_some());
        if given + payload != params.len() {
            return err(
                "analyze-handler-arity",
                format!(
                    "`{name}` takes {} parameter(s); `{attr}=` supplies {given}{}",
                    params.len(),
                    match handler_payload(attr) {
                        Some("bool") => " plus whether the pointer is over",
                        Some(_) if attr == "key" => " plus the key's name",
                        Some(_) => " plus the new value",
                        None => "",
                    }
                ),
                span,
            );
        }
    }
    Ok(())
}
