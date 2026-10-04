//! Analysis: what the types do not say.
//!
//! @ref LLP 1004 D2 (effect signatures, region structure)
//! / LLP 1035.005.000 D1 (an action's effects are inferred from its body)
//!
//! After types, a program can still be wrong in ways an implementer would
//! otherwise discover at runtime: a handler naming an action that does not
//! exist or with the wrong number of curried arguments, a `task` ticking an
//! unknown action, or a mutation's `then` sending that mutation. Every
//! rejection carries a stable id and a span.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod arity;
mod payload;
mod sends;

pub use payload::handler_arity_message;

use contract_syntax::{Component, Expr, File, Node, Span};
use contract_types::{Checked, Ref, Scope, Shapes, Ty};
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
                Node::Each { body, .. } => walk(body, declared)?,
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
    check_all(checked).map_err(|mut all| all.swap_remove(0))
}

/// Every independent refusal: each component's actions, tasks and view are
/// checked whatever the others found.
pub fn check_all(checked: &Checked<'_>) -> Result<Analysis, Vec<AnalyzeError>> {
    let Checked {
        file,
        types,
        expanded,
    } = checked;
    let mut errors = Vec::new();
    if let Err(e) = check_routes_root(file, true) {
        errors.push(e);
    }
    let Some(root) = file.components.first() else {
        return Err(vec![AnalyzeError {
            id: "analyze-no-component",
            message: "a file needs a component".into(),
            span: Span::point(1, 1),
            related: Vec::new(),
        }]);
    };
    if !root.props.is_empty() {
        errors.push(AnalyzeError {
            id: "analyze-root-props",
            message: "the root component (the first in the file) takes no props".into(),
            span: root.span,
            related: Vec::new(),
        });
    }
    // A child may own `state`, `derive`, and `action` (LLP 1017 P4c); that it
    // owns no `resource`, `mutation`, or `task` is the type pass's refusal
    // (`type-child-resource`), made before its view is checked.
    for (ci, c) in file.components.iter().enumerate() {
        let ct = &types.components[ci];
        let scoped = if ci == 0 { &expanded.root } else { c };
        let scope = types.component_scope(scoped, ct);
        errors.extend(check_tasks(c).err());
        errors.extend(check_mutation_then(c).err());
        // The root's actions after tail calls are inlined: a caller's send and
        // its callee's are one commit (LLP 1086 D8).
        errors.extend(sends::check(scoped));
        let view = View {
            file,
            actions: scoped,
            shapes: &types.shapes,
        };
        errors.extend(check_view(&c.view, &scope, &view).err());
    }
    errors.extend(check_controls(&expanded.root.view, false).err());
    errors.extend(arity::check(file, types, &expanded.root).err());
    if errors.is_empty() {
        Ok(Analysis {})
    } else {
        Err(errors)
    }
}

/// `mutation m as shape T then action` names an action that takes nothing:
/// it reads the answer from `m`, as the view does (LLP 1016.001).
fn check_mutation_then(c: &Component) -> Result<(), AnalyzeError> {
    for m in &c.mutations {
        let Some((name, span)) = &m.then else {
            continue;
        };
        let Some(a) = c.actions.iter().find(|a| &a.name == name) else {
            return err(
                "analyze-unknown-action",
                format!("`{name}` is not an action"),
                *span,
            );
        };
        if !a.params.is_empty() {
            return err(
                "analyze-handler-arity",
                format!(
                    "`{}` takes {} parameter(s); an answer passes none (read it from `{}`)",
                    a.name,
                    a.params.len(),
                    m.name
                ),
                *span,
            );
        }
        if let Some(send) = a
            .effects()
            .into_iter()
            .find(|e| e.send && e.target == m.name)
        {
            return err(
                "analyze-then-self-send",
                format!(
                    "`{}` cannot send `{}`: it runs when that mutation answers",
                    a.name, m.name
                ),
                send.span,
            );
        }
    }
    Ok(())
}

fn check_tasks(c: &Component) -> Result<(), AnalyzeError> {
    for t in &c.tasks {
        let Some(a) = c.actions.iter().find(|a| a.name == t.timer.1) else {
            return err(
                "analyze-unknown-action",
                format!("`{}` is not an action", t.timer.1),
                t.timer.2,
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
                t.timer.2,
            );
        }
    }
    Ok(())
}

/// The handler attributes (the web's events, LLP 1005 §3): `press`,
/// `change`, `input`, `hover`, `focus`, `blur`, `key`, `submit`, `load`, `message`.
pub const HANDLERS: [&str; 46] = [
    "press",
    "change",
    "input",
    // A file input's picker was dismissed (LLP 1069.002 D2).
    "cancel",
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
    // A touch or button went down on the node, and came up or was cancelled
    // (DOM's names; Charlie, 2026-10-03: hold-to-record, LLP 1005 §3); the
    // pointer moved over it or while held (LLP 1056 §3 stage 3). Each may
    // hand its action a `PointerEvent`.
    "pointerdown",
    "pointerup",
    "pointermove",
    // DOM's clipboard events at the focused node (spreadsheet F4, F14);
    // each may hand its action a `ClipboardEvent`.
    "copy",
    "cut",
    "paste",
    "swiperight",
    "refresh",
    "scroll",
    "pan",
    // A pan that began ended (LLP 1057 §10.6 phase 2): release velocity.
    "panrelease",
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

/// What a handler's event carries as its action's last argument: `input`
/// and `change` the new value (a text field's text, a checkbox's checked
/// state: LLP 1069.001 D4), `hover` whether the pointer is over, `key` the
/// key's name, `message` the posted string; the others nothing. An event
/// may then offer its record ([`contract_types::event_record`]).
pub fn handler_payload(attr: &str) -> Option<&'static str> {
    match attr {
        "change" | "input" | "key" | "message" | "navigate" | "error" => Some("string"),
        "timeupdate" | "durationchange" => Some("number"),
        "hover" => Some("bool"),
        "select" => Some("MarkdownSelection"),
        _ => None,
    }
}

/// The permitted action parameter counts after an event appends its payload.
/// `None` means that this event forbids the supplied explicit arguments.
/// Analysis and lowering share this rule, including navigate's optional payload.
/// A list's edges and a pull to refresh carry no payload, so their action takes
/// exactly the arguments bound at the site (LLP 1054.000.006).
pub fn handler_arity(attr: &str, given: usize) -> Option<std::ops::RangeInclusive<usize>> {
    if attr == "navigate" {
        return (given == 0).then_some(0..=1);
    }
    let payload = match attr {
        "transformgeometry" => 4,
        "transformrelease" => 6,
        "scroll" | "pan" | "panrelease" | "heightrelease" | "reorderdrop" => 2,
        _ => usize::from(handler_payload(attr).is_some()),
    };
    // Then the event's record, the action's to take or leave.
    let record = usize::from(contract_types::event_record(attr).is_some());
    Some(given + payload..=given + payload + record)
}

/// What the view check reads besides the scope: the file's components, the
/// component whose actions the scope's `Ref::Action`s index, and the shapes.
struct View<'a> {
    file: &'a File,
    actions: &'a Component,
    shapes: &'a Shapes,
}

fn check_view(nodes: &[Node], scope: &Scope, view: &View<'_>) -> Result<(), AnalyzeError> {
    let file = view.file;
    for n in nodes {
        match n {
            Node::Children { .. } => {}
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                let control = contract_syntax::input_control(tag, attrs);
                for a in attrs {
                    if HANDLERS.contains(&a.name.as_str()) {
                        check_handler(&a.name, &a.value, scope, a.span, control, view)?;
                    }
                }
                check_view(children, scope, view)?;
            }
            Node::Use {
                name,
                args,
                children,
                span,
            } => {
                check_view(children, scope, view)?;
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
                check_view(then, scope, view)?;
                check_view(otherwise, scope, view)?;
            }
            Node::Each {
                var,
                index,
                list,
                body,
                ..
            } => {
                let t = contract_types::infer(list, scope, view.shapes).ok();
                let item = match t {
                    Some(Ty::List(item)) => *item,
                    _ => Ty::Unknown,
                };
                let mut inner = scope.clone();
                inner.push_each(var, index.as_deref(), item);
                check_view(body, &inner, view)?;
            }
            Node::Match { some, none, .. } => {
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), Ty::Unknown)));
                check_view(&some.1, &inner, view)?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                check_view(none, &none_scope, view)?;
            }
        }
    }
    Ok(())
}

fn check_handler(
    attr: &str,
    value: &Expr,
    scope: &Scope,
    span: Span,
    control: Option<&str>,
    view: &View<'_>,
) -> Result<(), AnalyzeError> {
    let (name, args) = match value {
        Expr::Ident(n, _) => (n.as_str(), &[][..]),
        Expr::Call(n, args, _) => (n.as_str(), args.as_slice()),
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
    let given = args.len();
    // A prop of bare `action` type has unknown arity; only a real action is checked.
    if let Ref::Action(index) = r {
        let valid = handler_arity(attr, given).is_some_and(|range| range.contains(&params.len()));
        if !valid {
            let declared = view.actions.actions.get(index as usize);
            let params: Vec<(String, Ty)> = declared
                .map(|a| {
                    a.params
                        .iter()
                        .map(|p| p.name.clone())
                        .zip(params.iter().cloned())
                        .collect()
                })
                .unwrap_or_default();
            let arg_types: Vec<Option<Ty>> = args
                .iter()
                .map(|a| contract_types::infer(a, scope, view.shapes).ok())
                .collect();
            return err(
                "analyze-handler-arity",
                handler_arity_message(attr, control, name, args, &params, &arg_types),
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
            "pan" | "panrelease" | "heightrelease" | "transformgeometry" | "transformrelease"
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
            Node::Each { body, .. } => check_controls(body, in_canvas)?,
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
