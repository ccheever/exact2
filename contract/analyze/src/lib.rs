//! Analysis: what the types do not say.
//!
//! @ref LLP 1004 D2 (effect signatures, `writes` checking, region structure)
//! / LLP 0508 §10.3 (actions as reducers with declared writes; research)
//!
//! After types, a program can still be wrong in ways an implementer would
//! otherwise discover at runtime: an action writing a slot it did not declare,
//! a handler naming an action that does not exist or with the wrong number of
//! curried arguments, or a `task` ticking an unknown action. Child actions
//! declare effects on their own state (LLP 1017 P4c). Every rejection carries
//! a stable id and a span.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod arity;

use contract_syntax::{Action, Component, Expr, File, Node, Span, Stmt};
use contract_types::{Checked, Ref, Scope, Ty};
use std::collections::{BTreeMap, BTreeSet};

/// Another authored location needed to understand a rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    /// Original source identity and token range.
    pub span: Span,
    /// Why this location is relevant.
    pub note: String,
}

/// A typed rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeError {
    /// Stable id.
    pub id: &'static str,
    /// What went wrong.
    pub message: String,
    /// Where.
    pub span: Span,
    /// Other declarations or bindings involved in this rejection.
    pub related: Vec<Related>,
}

impl std::fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] {}", self.span, self.id, self.message)?;
        for related in &self.related {
            write!(f, "\n  {}: {}", related.span, related.note)?;
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

/// Check surface calls against an emitted module interface, before imports merge
/// so each refusal still belongs to its source file. No module is constructed.
pub fn check_surface_arguments(
    file: &File,
    declared: &BTreeMap<String, Vec<String>>,
) -> Result<(), AnalyzeError> {
    fn walk(nodes: &[Node], declared: &BTreeMap<String, Vec<String>>) -> Result<(), AnalyzeError> {
        for node in nodes {
            match node {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => {
                    for attr in attrs
                        .iter()
                        .filter(|a| tag == "canvas" && a.name == "surface")
                    {
                        let Expr::Call(name, args, span) = &attr.value else {
                            continue;
                        };
                        let Some(fields) = declared.get(name) else {
                            return err(
                                "analyze-surface-arguments",
                                format!("unknown surface `{name}`"),
                                *span,
                            );
                        };
                        for arg in args {
                            if let Expr::NamedArg(field, _, span) = arg {
                                if !fields.contains(field) {
                                    return err(
                                        "analyze-surface-arguments",
                                        format!("unknown surface argument `{field}` for `{name}`; declared names: {}", fields.join(", ")),
                                        *span,
                                    );
                                }
                            }
                        }
                        if !args.iter().any(|a| matches!(a, Expr::NamedArg(..)))
                            && args.len() > fields.len()
                        {
                            return err(
                                "analyze-surface-arguments",
                                format!(
                                    "surface `{name}` expected at most {} arguments ({}), got {}",
                                    fields.len(),
                                    fields.join(", "),
                                    args.len()
                                ),
                                *span,
                            );
                        }
                    }
                    walk(children, declared)?;
                }
                Node::Use { children, .. } => walk(children, declared)?,
                Node::Provide { body, .. } | Node::Each { body, .. } => walk(body, declared)?,
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, declared)?;
                    walk(otherwise, declared)?;
                }
                Node::Match { some, none, .. } => {
                    walk(&some.1, declared)?;
                    walk(none, declared)?;
                }
                Node::Children { .. } => {}
            }
        }
        Ok(())
    }
    for component in &file.components {
        walk(&component.view, declared)?;
    }
    Ok(())
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

/// Analyze the authored file using its type-checked expansion.
pub fn check(checked: &Checked<'_>) -> Result<Analysis, AnalyzeError> {
    let Checked {
        file,
        types,
        expanded,
    } = checked;
    check_routes_root(file, true)?;
    let Some(root) = file.components.first() else {
        return err(
            "analyze-no-component",
            "a file needs a component",
            Span::point(1, 1),
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
    for (ci, c) in file.components.iter().enumerate() {
        let ct = &types.components[ci];
        let scoped = if ci == 0 { &expanded.root } else { c };
        let scope = types.component_scope(scoped, ct);
        // Check authored actions once, not every lifted instance. Keep the
        // resolved slots: the root's router state is implicit in the source.
        let router = file
            .routes
            .as_ref()
            .filter(|_| ci == 0)
            .map(|r| r.slot.as_str());
        check_actions(c, scoped, router)?;
        check_tasks(c)?;
        check_view(&c.view, &scope, file)?;
    }
    check_controls(&expanded.root.view, false)?;
    arity::check(file, types, &expanded.root)?;
    Ok(Analysis {})
}

fn check_actions(
    component: &Component,
    slots: &Component,
    router: Option<&str>,
) -> Result<(), AnalyzeError> {
    for a in &component.actions {
        let mut declared = BTreeSet::new();
        for (w, span) in &a.writes {
            if !writable(slots, w) {
                return Err(unknown_writes(a, component, slots, router, *span));
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
        let mut missing = BTreeSet::new();
        targets.retain(|(target, _, _)| !declared.contains(*target) && missing.insert(*target));
        if let Some((target, span, how)) = targets.first() {
            let message = if targets.len() == 1 {
                format!(
                    "`{}` {how} `{target}` but does not declare it: add `writes {target}`",
                    a.name
                )
            } else {
                let names = targets
                    .iter()
                    .map(|(target, _, _)| format!("`{target}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "`{}` has undeclared effects on {names}; add these names to its `writes` declaration",
                    a.name
                )
            };
            return err("analyze-write-not-declared", message, **span);
        }
    }
    Ok(())
}

fn writable(slots: &Component, name: &str) -> bool {
    slots.states.iter().any(|s| s.name == name) || slots.mutations.iter().any(|m| m.name == name)
}

// Only the refusal path constructs choices. The expanded root determines
// membership, but its lifted child slots are not names the author can write.
fn unknown_writes(
    action: &Action,
    component: &Component,
    slots: &Component,
    router: Option<&str>,
    span: Span,
) -> AnalyzeError {
    let mut seen = BTreeSet::new();
    let unknown = action
        .writes
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !writable(slots, name) && seen.insert(*name))
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>();
    let predicate = if unknown.len() == 1 {
        "is not a state or a mutation"
    } else {
        "are not states or mutations"
    };
    let choices = component
        .states
        .iter()
        .map(|s| s.name.as_str())
        .chain(component.mutations.iter().map(|m| m.name.as_str()))
        .chain(router)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let hint = if choices.is_empty() {
        "this component has no state or mutation slots".to_owned()
    } else {
        format!("available writes: {choices}")
    };
    AnalyzeError {
        id: "analyze-writes-unknown-state",
        message: format!("{} in `writes` {predicate}; {hint}", unknown.join(", ")),
        span,
        related: Vec::new(),
    }
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
pub const HANDLERS: [&str; 36] = [
    "press",
    "change",
    "select",
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
    "pan",
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
    "reachstart",
    "reachend",
];

/// What a handler's event carries as its action's last argument: `change`
/// the new text, `hover` whether the pointer is over, `key` the key's name,
/// `message` the posted string; the others nothing.
pub fn handler_payload(attr: &str) -> Option<&'static str> {
    match attr {
        "change" | "key" | "message" | "navigate" | "error" => Some("string"),
        "timeupdate" | "durationchange" => Some("number"),
        "hover" => Some("bool"),
        "select" => Some("MarkdownSelection"),
        _ => None,
    }
}

/// The permitted action parameter counts after an event appends its payload.
/// `None` means that this event forbids the supplied explicit arguments.
/// Analysis and lowering share this rule, including navigate's optional payload.
pub fn handler_arity(attr: &str, given: usize) -> Option<std::ops::RangeInclusive<usize>> {
    if matches!(attr, "reachstart" | "reachend") {
        return (given == 0).then_some(0..=0);
    }
    if attr == "navigate" {
        return (given == 0).then_some(0..=1);
    }
    let payload = match attr {
        "transformgeometry" => 4,
        "transformrelease" => 6,
        "scroll" | "pan" | "heightrelease" | "reorderdrop" => 2,
        _ => usize::from(handler_payload(attr).is_some()),
    };
    Some(given + payload..=given + payload)
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
                        file.unknown_component_message(name),
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
        let valid = handler_arity(attr, given).is_some_and(|range| range.contains(&params.len()));
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
            "pan" | "heightrelease" | "transformgeometry" | "transformrelease"
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
