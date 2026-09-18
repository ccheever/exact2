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
    // A child may own `state`, `derive`, and `action` (LLP 1017 P4c); that it
    // owns no `resource`, `mutation`, or `task` is the type pass's refusal
    // (`type-child-resource`), made before its view is checked.
    let expanded = contract_syntax::expand(file).map_err(|e| AnalyzeError {
        id: e.id,
        message: e.message,
        span: e.span,
    })?;
    for (ci, c) in file.components.iter().enumerate() {
        let ct = &types.components[ci];
        let scoped = if ci == 0 { &expanded.root } else { c };
        let scope = types.component_scope(scoped, ct);
        check_actions(scoped)?;
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
pub const HANDLERS: [&str; 32] = [
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
    "loadedmetadata",
    "durationchange",
    "timeupdate",
    "play",
    "playing",
    "pause",
    "ended",
    "waiting",
    "seeking",
    "seeked",
    "ratechange",
    "volumechange",
    "error",
    "canplay",
    "navigate",
    "heightrelease",
    "transformgeometry",
    "transformrelease",
    "reorderdrop",
];

/// What a handler's event carries as its action's last argument: `change`
/// the new text, `hover` whether the pointer is over, `key` the key's name,
/// `message` the iframe guest's string; the others nothing.
pub fn handler_payload(attr: &str) -> Option<&'static str> {
    match attr {
        "change" | "key" | "message" | "navigate" | "error" => Some("string"),
        "timeupdate" | "durationchange" => Some("number"),
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
        let payload = if attr == "transformgeometry" {
            4
        } else if attr == "transformrelease" {
            6
        } else if matches!(attr, "scroll" | "heightrelease" | "reorderdrop") {
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
                        Some(_) if attr == "message" => " plus the guest's message",
                        Some(_) => " plus the new value",
                        None if attr == "scroll" => " plus scrollLeft and scrollTop",
                        None if attr == "heightrelease" => " plus height and velocity",
                        None if attr == "transformgeometry" => " plus four geometry numbers",
                        None if attr == "transformrelease" => " plus six transform release numbers",
                        None => "",
                    }
                ),
                span,
            );
        }
        if attr == "reorderdrop"
            && params[given..] != [Ty::String, Ty::Option(Box::new(Ty::String))]
        {
            return err(
                "analyze-handler-type",
                "`reorderdrop` supplies string and option<string>",
                span,
            );
        }
        if matches!(
            attr,
            "heightrelease" | "transformgeometry" | "transformrelease"
        ) && params[given..].iter().any(|ty| *ty != Ty::Number)
        {
            return err(
                "analyze-handler-type",
                format!("`{attr}` supplies only numeric payload parameters"),
                span,
            );
        }
    }
    Ok(())
}
