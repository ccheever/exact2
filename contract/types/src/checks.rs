//! Type diagnostics and component checks that require recursive traversal.

use super::{
    arms, disagree, err, infer, ComponentTypes, Ref, Scope, Shapes, Sink, Ty, TypeError, Types,
};
use contract_syntax::{
    one_spelling_edit, Attr, Binding, Component, Expr, File, Node, Owner, Span, TemplatePart,
    TypeExpr,
};
use std::collections::{BTreeMap, BTreeSet};

/// Reject recursive functions without revisiting completed subgraphs.
pub(super) fn check_function_cycles(file: &File) -> Result<(), TypeError> {
    let indices: BTreeMap<&str, usize> = file
        .fns
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.as_str(), i))
        .collect();
    let graph: Vec<Vec<usize>> = file
        .fns
        .iter()
        .map(|f| {
            let mut out = Vec::new();
            calls_in(&f.body, &indices, &mut out);
            out
        })
        .collect();
    fn visit(
        index: usize,
        graph: &[Vec<usize>],
        states: &mut [u8],
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        match states[index] {
            2 => return None,
            1 => {
                path.push(index);
                return Some(path.clone());
            }
            _ => {}
        }
        states[index] = 1;
        path.push(index);
        for &callee in &graph[index] {
            if let Some(cycle) = visit(callee, graph, states, path) {
                return Some(cycle);
            }
        }
        path.pop();
        states[index] = 2;
        None
    }
    // Keep completed subgraphs across roots: repeated shared helpers otherwise
    // take exponential work even when no helper is expanded into the app.
    let mut states = vec![0; file.fns.len()];
    let mut path = Vec::new();
    for (i, f) in file.fns.iter().enumerate() {
        if let Some(cycle) = visit(i, &graph, &mut states, &mut path) {
            let names: Vec<_> = cycle.iter().map(|&i| file.fns[i].name.as_str()).collect();
            return err(
                "type-fn-recursive",
                format!(
                    "`fn {}` calls itself ({}): a fn is expanded where it is called, so it cannot recurse — a traversal is the data crate's",
                    f.name,
                    names.join(" → ")
                ),
                f.span,
            );
        }
    }
    Ok(())
}

/// Function calls in expression order, excluding calls outside the authored graph.
fn calls_in(e: &Expr, indices: &BTreeMap<&str, usize>, out: &mut Vec<usize>) {
    match e {
        Expr::Call(n, args, _) => {
            if let Some(&index) = indices.get(n.as_str()) {
                out.push(index);
            }
            for a in args {
                calls_in(a, indices, out);
            }
        }
        Expr::Some(x, _)
        | Expr::Unary(_, x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _) => calls_in(x, indices, out),
        Expr::Binary(_, a, b, _) => {
            calls_in(a, indices, out);
            calls_in(b, indices, out);
        }
        Expr::Ternary(a, b, c, _) => {
            calls_in(a, indices, out);
            calls_in(b, indices, out);
            calls_in(c, indices, out);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            calls_in(subject, indices, out);
            calls_in(some, indices, out);
            calls_in(none, indices, out);
        }
        Expr::Let { value, body, .. } => {
            calls_in(value, indices, out);
            calls_in(body, indices, out);
        }
        Expr::Arrow { body, .. } => calls_in(body, indices, out),
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    calls_in(x, indices, out);
                }
            }
        }
        Expr::Number(..)
        | Expr::Str(..)
        | Expr::Bool(..)
        | Expr::None(_)
        | Expr::EmptyList(_)
        | Expr::Ident(..) => {}
    }
}

/// Check compatibility without constructing a discarded merged type.
pub(super) fn can_unify(a: &Ty, b: &Ty) -> bool {
    match (a, b) {
        (Ty::Unknown, _) | (_, Ty::Unknown) => true,
        (Ty::Option(a), Ty::Option(b)) | (Ty::List(a), Ty::List(b)) => can_unify(a, b),
        (Ty::Action(a), Ty::Action(b)) => {
            a.is_empty()
                || b.is_empty()
                || (a.len() == b.len() && a.iter().zip(b).all(|(a, b)| can_unify(a, b)))
        }
        (a, b) => a == b,
    }
}

/// Format the already-resolved signature only after an arity refusal.
pub(super) fn call_arity<P: std::fmt::Display>(
    name: &str,
    given: usize,
    params: impl IntoIterator<Item = P>,
) -> String {
    let params: Vec<_> = params.into_iter().map(|p| p.to_string()).collect();
    format!(
        "`{name}` takes {} argument(s), given {given}; expected `{name}({})`",
        params.len(),
        params.join(", ")
    )
}

/// Describe unknown props and declared choices after the first unknown is found.
pub(super) fn unknown_props(component: &Component, args: &[Attr], span: Span) -> TypeError {
    let mut seen = BTreeSet::new();
    let unknown = args
        .iter()
        .filter(|arg| !component.props.iter().any(|prop| prop.name == arg.name))
        .filter(|arg| seen.insert(arg.name.as_str()))
        .map(|arg| format!("`{}`", arg.name))
        .collect::<Vec<_>>();
    let noun = if unknown.len() == 1 { "prop" } else { "props" };
    let choices = if component.props.is_empty() {
        "this component declares no props".to_owned()
    } else {
        let names = component
            .props
            .iter()
            .map(|prop| format!("`{}`", prop.name))
            .collect::<Vec<_>>()
            .join(", ");
        format!("available props: {names}")
    };
    // One misspelling of one prop the call leaves out is named outright.
    let hint = match unknown.as_slice() {
        [_] => {
            let given = args
                .iter()
                .find(|a| !component.props.iter().any(|p| p.name == a.name));
            let missing = component
                .props
                .iter()
                .filter(|p| !args.iter().any(|a| a.name == p.name))
                .map(|p| p.name.as_str());
            given
                .and_then(|a| contract_syntax::suggestion(&a.name, missing))
                .map(|name| format!("; did you mean `{name}`?"))
                .unwrap_or_default()
        }
        _ => String::new(),
    };
    TypeError {
        id: "type-unknown-prop",
        message: format!(
            "`{}` has no {noun} {}; {choices}{hint}",
            component.name,
            unknown.join(", ")
        ),
        span,
    }
}

impl Shapes {
    pub(super) fn unknown_type(&self, name: &str, span: Span) -> TypeError {
        let primitives = ["number", "string", "bool", "unit", "action"];
        let names = primitives
            .into_iter()
            .chain(
                self.map
                    .keys()
                    .map(String::as_str)
                    .filter(|name| !primitives.contains(name)),
            )
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        TypeError {
            id: "type-unknown",
            message: format!("unknown type `{name}`; known named types: {names}"),
            span,
        }
    }

    pub(super) fn unknown_field(&self, shape: &str, field: &str, span: Span) -> TypeError {
        let fields = self.map.get(shape).map(Vec::as_slice).unwrap_or_default();
        let names = fields
            .iter()
            .map(|(name, _)| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let hint = if fields.is_empty() {
            "this shape declares no fields".to_owned()
        } else {
            format!("available fields: {names}")
        };
        let guess = contract_syntax::suggestion(field, fields.iter().map(|(n, _)| n.as_str()))
            .map(|name| format!("; did you mean `{name}`?"))
            .unwrap_or_default();
        TypeError {
            id: "type-unknown-field",
            message: format!("`{shape}` has no field `{field}`; {hint}{guess}"),
            span,
        }
    }
}

// Suggestions change only a refusal's text. Global functions have authored
// names here; lifted child actions do not, so scoped actions only disambiguate.
pub(super) fn unknown_function(
    name: &str,
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> TypeError {
    // The web's list operations and number formatters Contract refuses
    // (LLP 1017.003 §Diagnostics) say what to do instead.
    if let Some(why) = contract_syntax::idioms::refusal(name) {
        return TypeError {
            id: "type-refused-idiom",
            message: why,
            span,
        };
    }
    let mut message = format!(
        "`{name}` is not in the stdlib roster and is not an action; data comes from a `resource`"
    );
    if let Some(candidate) = similar_function(name, scope, shapes) {
        message.push_str(&format!("; did you mean `{candidate}`?"));
    }
    TypeError {
        id: "type-unknown-function",
        message,
        span,
    }
}

fn similar_function<'a>(name: &str, scope: &'a Scope, shapes: &'a Shapes) -> Option<&'a str> {
    if !name.is_ascii() || !(3..=64).contains(&name.len()) {
        return None;
    }
    let global = |candidate: &str| {
        matches!(candidate, "pending" | "failed")
            || (candidate == "path" && shapes.routes.is_some())
            || shapes.fns.contains_key(candidate)
            || super::Stdlib::from_name(candidate)
                .is_some_and(|f| super::routes::require_table(f, shapes, Span::default()).is_ok())
    };
    let names = shapes
        .fns
        .keys()
        .map(String::as_str)
        .chain(super::Stdlib::ALL.iter().map(|f| f.name()))
        .chain(["pending", "failed", "path"]);
    let mut found = None;
    for candidate in names {
        if !one_spelling_edit(name.as_bytes(), candidate.as_bytes()) || !global(candidate) {
            continue;
        }
        if found.is_some_and(|previous| previous != candidate) {
            return None;
        }
        found = Some(candidate);
    }
    let candidate = found?;
    for frame in &scope.frames {
        for (scoped, _, _) in &frame.names {
            // Expansion can append instance suffixes. The stem is only a
            // conservative ambiguity veto, never an offered correction.
            let authored = scoped.split('#').next().unwrap();
            if authored != candidate
                && one_spelling_edit(name.as_bytes(), authored.as_bytes())
                && matches!(
                    scope.lookup(scoped),
                    Some((Ref::Action(_) | Ref::Prop(_), Ty::Action(_)))
                )
            {
                return None;
            }
        }
    }
    Some(candidate)
}

/// Reject shape cycles before lowering recursively materializes plan types.
pub(super) fn check_shape_cycles(file: &File, shapes: &Shapes) -> Result<(), TypeError> {
    let indices: BTreeMap<&str, usize> = file
        .shapes
        .iter()
        .enumerate()
        .map(|(i, shape)| (shape.name.as_str(), i))
        .collect();
    let mut states = vec![0u8; file.shapes.len()];
    let mut path = Vec::new();
    for i in 0..file.shapes.len() {
        visit_shape(i, file, shapes, &indices, &mut states, &mut path)?;
    }
    Ok(())
}

fn visit_shape(
    index: usize,
    file: &File,
    shapes: &Shapes,
    indices: &BTreeMap<&str, usize>,
    states: &mut [u8],
    path: &mut Vec<String>,
) -> Result<(), TypeError> {
    if states[index] == 2 {
        return Ok(());
    }
    states[index] = 1;
    path.push(file.shapes[index].name.clone());
    for field in &file.shapes[index].fields {
        // Wrapper types still depend on their leaf shape. Ask the same
        // resolver as field typing: primitive names take precedence even if
        // an authored shape has that spelling.
        let mut leaf = &field.ty;
        while let TypeExpr::Option(inner, _) | TypeExpr::List(inner, _) = leaf {
            leaf = inner;
        }
        let Ty::Record(name) = shapes.resolve(leaf)? else {
            continue;
        };
        let Some(&next) = indices.get(name.as_str()) else {
            continue;
        };
        if states[next] == 1 {
            let start = path.iter().position(|part| part == &name).unwrap_or(0);
            let mut cycle = path[start..].to_vec();
            cycle.push(name);
            return err(
                "type-shape-recursive",
                format!(
                    "shape field `{}` makes a recursive type cycle ({}); plan values are finite trees",
                    field.name,
                    cycle.join(" -> ")
                ),
                field.span,
            );
        }
        visit_shape(next, file, shapes, indices, states, path)?;
    }
    path.pop();
    states[index] = 2;
    Ok(())
}

/// The lexical scope at each expanded region arm, by tag and arm, whose
/// subject types now, and whether every region's subject typed. A region
/// over a list a later write completes (`state items = []`) is skipped,
/// with all it holds: the view check refuses a list that never types
/// (@ref LLP 1088 D6's two repros, without its fixed point).
fn owner_scopes(
    c: &Component,
    ct: &ComponentTypes,
    types: &Types,
) -> (BTreeMap<(u32, u8), Scope>, bool) {
    let mut scopes = BTreeMap::new();
    let complete = collect_owner_scopes(
        &c.view,
        &types.component_scope(c, ct),
        &types.shapes,
        &mut scopes,
    );
    (scopes, complete)
}

/// Infer each lifted child state's initializer where its instance is
/// created: in its use site's scope, after derives and resources have
/// types, so it may read them, a row's item, a `match` binding, and every
/// earlier state (LLP 1017 P4c).
pub(super) fn infer_owned_state_initializers(
    c: &Component,
    ct: &mut ComponentTypes,
    types: &Types,
    owners: Option<&[Owner]>,
) -> Result<(), TypeError> {
    let Some(owners) = owners else {
        return Ok(());
    };
    if owners.iter().all(|o| *o == Owner::Root) {
        return Ok(());
    }
    let (scopes, complete) = owner_scopes(c, ct, types);
    let mut names: Vec<(String, Ref, Ty)> = c
        .props
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.clone(), Ref::Prop(i as u32), ct.props[i].clone()))
        .collect();
    for (j, p) in c.injects.iter().enumerate() {
        let i = c.props.len() + j;
        names.push((p.name.clone(), Ref::Prop(i as u32), ct.props[i].clone()));
    }
    // What settled before any instance renders: derives and resources.
    let mut settled = Vec::new();
    for (i, d) in c.derives.iter().enumerate() {
        settled.push((d.name.clone(), Ref::Derive(i as u32), ct.derives[i].clone()));
    }
    for (i, r) in c.resources.iter().enumerate() {
        settled.push((
            r.name.clone(),
            Ref::Resource(i as u32),
            ct.resources[i].clone(),
        ));
    }
    for (i, m) in c.mutations.iter().enumerate() {
        let t = Ty::Option(Box::new(ct.mutations[i].clone()));
        settled.push((m.name.clone(), Ref::Mutation(i as u32), t));
    }
    for (i, state) in c.states.iter().enumerate() {
        let regions = match owners.get(i).copied().unwrap_or(Owner::Root) {
            Owner::Root => None,
            Owner::Instance => Some(Vec::new()),
            Owner::Arm { tag, arm } => {
                let Some(owner_scope) = scopes.get(&(tag, arm)) else {
                    if !complete {
                        // Its region was skipped: the view check says why.
                        names.push((state.name.clone(), Ref::Slot(i as u32), ct.slots[i].clone()));
                        continue;
                    }
                    return err(
                        "type-row-slot",
                        format!("child state `{}` has no owning region", state.name),
                        state.span,
                    );
                };
                Some(
                    owner_scope
                        .frames
                        .iter()
                        .filter(|frame| frame.region)
                        .cloned()
                        .collect(),
                )
            }
        };
        if let Some(regions) = regions {
            let mut scope = Scope::default();
            scope.push(settled.clone());
            scope.push(names.clone());
            scope.frames.extend(regions);
            if let Some(e) = super::component::initializer_scope(c, i, &scope, &types.shapes) {
                return Err(e);
            }
            ct.slots[i] = infer(&state.expr, &scope, &types.shapes)?;
        }
        names.push((state.name.clone(), Ref::Slot(i as u32), ct.slots[i].clone()));
    }
    Ok(())
}

fn collect_owner_scopes(
    nodes: &[Node],
    scope: &Scope,
    shapes: &Shapes,
    scopes: &mut BTreeMap<(u32, u8), Scope>,
) -> bool {
    let mut complete = true;
    for node in nodes {
        match node {
            Node::Element { children, .. } | Node::Use { children, .. } => {
                complete &= collect_owner_scopes(children, scope, shapes, scopes);
            }
            Node::Children { .. } => {}
            Node::When {
                tag,
                then,
                otherwise,
                ..
            } => {
                scopes.insert((*tag, 0), scope.clone());
                scopes.insert((*tag, 1), scope.clone());
                complete &= collect_owner_scopes(then, scope, shapes, scopes);
                complete &= collect_owner_scopes(otherwise, scope, shapes, scopes);
            }
            Node::Each {
                tag,
                var,
                index,
                list,
                body,
                ..
            } => {
                let Ok(Ty::List(item)) = infer(list, scope, shapes) else {
                    complete = false;
                    continue;
                };
                let mut inner = scope.clone();
                inner.push_each(var, index.as_deref(), *item);
                scopes.insert((*tag, 0), inner.clone());
                complete &= collect_owner_scopes(body, &inner, shapes, scopes);
            }
            Node::Match {
                tag,
                subject,
                some,
                none,
                ..
            } => {
                let Ok(Ty::Option(item)) = infer(subject, scope, shapes) else {
                    complete = false;
                    continue;
                };
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                scopes.insert((*tag, 0), inner.clone());
                complete &= collect_owner_scopes(&some.1, &inner, shapes, scopes);
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                scopes.insert((*tag, 1), none_scope.clone());
                complete &= collect_owner_scopes(none, &none_scope, shapes, scopes);
            }
        }
    }
    complete
}

/// A slot's fill, checked where `children` stands with its caller's scope
/// and providers (LLP 1035.005.000 D9: the slot's own section does not
/// reach it).
#[derive(Clone)]
struct Fill {
    nodes: Vec<Node>,
    scope: Scope,
    provides: Vec<(String, Ty, Span)>,
}

/// Check the concrete provider path to every inject after component typing:
/// `provides` is the root's `provide` section, `scope` its scope.
pub(super) fn check_injects(
    nodes: &[Node],
    provides: &[Binding],
    scope: &Scope,
    types: &Types,
    file: &File,
) -> Result<(), TypeError> {
    let mut provided = Vec::new();
    for b in provides {
        provided.push((
            b.name.clone(),
            infer(&b.expr, scope, &types.shapes)?,
            b.span,
        ));
    }
    check_inject_nodes(nodes, scope, types, file, &mut provided, None, 0)
}

#[allow(clippy::too_many_arguments)]
fn check_inject_nodes(
    nodes: &[Node],
    scope: &Scope,
    types: &Types,
    file: &File,
    provides: &mut Vec<(String, Ty, Span)>,
    fill: Option<&Fill>,
    depth: u32,
) -> Result<(), TypeError> {
    for node in nodes {
        match node {
            Node::Element { children, .. } => {
                check_inject_nodes(children, scope, types, file, provides, fill, depth)?;
            }
            Node::Use {
                name,
                children,
                span,
                ..
            } => {
                // A use of an unknown component was refused and never expanded.
                let Some(target) = file
                    .components
                    .iter()
                    .position(|component| &component.name == name)
                else {
                    continue;
                };
                let target_c = &file.components[target];
                let target_t = &types.components[target];
                for (j, inject) in target_c.injects.iter().enumerate() {
                    let Some((_, got, provided_at)) = provides
                        .iter()
                        .rev()
                        .find(|(provided, _, _)| provided == &inject.name)
                    else {
                        continue;
                    };
                    let want = &target_t.props[target_c.props.len() + j];
                    if !can_unify(want, got) {
                        return err(
                            "type-provide",
                            format!(
                                "the provided `{}` is `{got}`, but `{name}` injects `{want}`",
                                inject.name
                            ),
                            *provided_at,
                        );
                    }
                }
                if depth >= 32 {
                    return err(
                        "syntax-inline-depth",
                        format!("component `{name}` nests too deeply (a cycle?)"),
                        *span,
                    );
                }
                let target_scope = types.component_scope(target_c, target_t);
                let child_fill = target_c.slot.then(|| Fill {
                    nodes: children.clone(),
                    scope: scope.clone(),
                    provides: provides.clone(),
                });
                let outer = provides.len();
                for b in &target_c.provides {
                    let got = infer(&b.expr, &target_scope, &types.shapes)?;
                    provides.push((b.name.clone(), got, b.span));
                }
                let checked = check_inject_nodes(
                    &target_c.view,
                    &target_scope,
                    types,
                    file,
                    provides,
                    child_fill.as_ref(),
                    depth + 1,
                );
                provides.truncate(outer);
                checked?;
            }
            Node::Children { .. } => {
                if let Some(fill) = fill {
                    check_inject_nodes(
                        &fill.nodes,
                        &fill.scope,
                        types,
                        file,
                        &mut fill.provides.clone(),
                        None,
                        depth,
                    )?;
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                check_inject_nodes(then, scope, types, file, provides, fill, depth)?;
                check_inject_nodes(otherwise, scope, types, file, provides, fill, depth)?;
            }
            Node::Each {
                var,
                index,
                list,
                body,
                ..
            } => {
                let ty = infer(list, scope, &types.shapes)?;
                let Ty::List(item) = ty else {
                    return err(
                        "type-each-list",
                        format!("`each` needs a list, given `{ty}`"),
                        list.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_each(var, index.as_deref(), *item);
                check_inject_nodes(body, &inner, types, file, provides, fill, depth)?;
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let ty = infer(subject, scope, &types.shapes)?;
                let Ty::Option(item) = ty else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{ty}`"),
                        subject.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                check_inject_nodes(&some.1, &inner, types, file, provides, fill, depth)?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                check_inject_nodes(none, &none_scope, types, file, provides, fill, depth)?;
            }
        }
    }
    Ok(())
}

/// The commands a host answers (LLP 1005 §3): every name an action body may
/// call. The web host's `command` op, the Apple session's queue, and the Linux
/// presenter's `run_commands` match these by name; any other name would reach
/// them and be refused there, silently to the author, so it is refused here.
pub(super) const HOST_COMMANDS: &[&str] = &[
    "blur",
    "copyText",
    "deliveryActivate",
    "deliveryCheck",
    "focus",
    "format",
    // @ref LLP 1077 D14 — `haptic("success" | "warning" | "error" | …)`.
    "haptic",
    "openURL",
    // `reload()`: the development host boots the app again, as its dev
    // menu's Reload does; a host without a dev menu refuses it.
    "reload",
    "selectText",
    "setScheme",
    // @ref LLP 1069.002 D2 — `HTMLInputElement.showPicker()` on a file input.
    "showPicker",
    "share",
    // @ref LLP 1069.010 D3 — export: the host copies an `app:/` file out.
    "saveFile",
    // @ref LLP 1069.010 D2 — the File System Access API's pickers.
    "showOpenFilePicker",
    "showDirectoryPicker",
    "showSaveFilePicker",
    // @ref LLP 1070.000 — a virtualized list's row brought into view, by key.
    "scrollIntoView",
    // The inverse of a canvas's `message=`: `postMessage(text, "world")` queues
    // text into the surface of that name, delivered in order, never coalesced.
    "postMessage",
    // `event.preventDefault()` for the `key` event that ran the action: the
    // host skips the key's default action (docs/contract-grammar.md#events).
    "preventDefault",
];

/// The three pickers' positional arguments (LLP 1069.010 D2): an element
/// id, then `multiple` (a bool) for `showOpenFilePicker` or
/// `suggestedName` (a string) for `showSaveFilePicker`.
fn picker_args(
    name: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    let (usage, second) = match name {
        "showOpenFilePicker" => (
            "showOpenFilePicker(id) or showOpenFilePicker(id, multiple)",
            Some((Ty::Bool, false)),
        ),
        "showSaveFilePicker" => (
            "showSaveFilePicker(id, suggestedName)",
            Some((Ty::String, true)),
        ),
        _ => ("showDirectoryPicker(id)", None),
    };
    let wrong = |at| {
        err(
            "type-file-picker-argument",
            format!("`{name}` takes `{usage}`"),
            at,
        )
    };
    let most = if second.is_some() { 2 } else { 1 };
    let least = match second {
        Some((_, true)) => 2,
        _ => 1,
    };
    if args.len() < least
        || args.len() > most
        || args.iter().any(|a| matches!(a, Expr::NamedArg(..)))
    {
        return wrong(span);
    }
    for (i, arg) in args.iter().enumerate() {
        let want = if i == 0 {
            Ty::String
        } else {
            second.as_ref().map_or(Ty::String, |(t, _)| t.clone())
        };
        let t = infer(arg, scope, shapes)?;
        if want.unify(&t).is_none() {
            return wrong(arg.span());
        }
    }
    Ok(())
}

/// `saveFile(id, from, suggestedName)` (LLP 1069.010 D3): three strings,
/// positional. Whether `from` is granted is the host's to refuse.
fn save_file_args(
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    if args.len() != 3 || args.iter().any(|a| matches!(a, Expr::NamedArg(..))) {
        return err(
            "type-save-file-argument",
            "`saveFile` takes three strings: `saveFile(\"export-file\", \"app:/data/export.json\", \"export.json\")`",
            span,
        );
    }
    for arg in args {
        let t = infer(arg, scope, shapes)?;
        if Ty::String.unify(&t).is_none() {
            return err(
                "type-save-file-argument",
                format!("`saveFile`'s arguments are strings, not `{t}`"),
                arg.span(),
            );
        }
    }
    Ok(())
}

/// `share(title=, text=, url=)` (LLP 1069.003 D1): the Web Share API's
/// member names, named only, each a string, at least one of `text` and
/// `url`. Whether `url` is absolute is the host's to refuse: a value is not
/// known here.
fn share_args(args: &[Expr], scope: &Scope, shapes: &Shapes, span: Span) -> Result<(), TypeError> {
    const NAMES: [&str; 3] = ["title", "text", "url"];
    let mut seen = BTreeSet::new();
    for arg in args {
        let Expr::NamedArg(name, value, at) = arg else {
            return err(
                "type-share-argument",
                "`share` takes named arguments: `share(title=…, text=…, url=…)`",
                arg.span(),
            );
        };
        if !NAMES.contains(&name.as_str()) {
            return err(
                "type-share-argument",
                format!("`share` has no argument `{name}`; it takes title=, text= and url="),
                *at,
            );
        }
        if !seen.insert(name.as_str()) {
            return err(
                "type-share-argument",
                format!("`{name}=` is given twice"),
                *at,
            );
        }
        let t = infer(value, scope, shapes)?;
        if Ty::String.unify(&t).is_none() {
            return err(
                "type-share-argument",
                format!("`{name}=` is a string, not `{t}`"),
                value.span(),
            );
        }
    }
    if !seen.contains("text") && !seen.contains("url") {
        return err(
            "type-share-argument",
            "`share` needs `text=` or `url=`",
            span,
        );
    }
    Ok(())
}

/// `scrollIntoView("list-id", key, block=, inline=, behavior=, row=)` (LLP
/// 1070.000 §1): a virtualized list's `id` as a literal, a row key, and the
/// web's `ScrollIntoViewOptions` by name with literal values; `row=` names an
/// inner list's outer row. Whether the list exists is the runner's to find.
fn into_view_args(
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    const USAGE: &str = "`scrollIntoView(\"list-id\", key, block=\"start\", inline=\"nearest\", behavior=\"auto\", row=outerKey)`";
    let positional: Vec<_> = args
        .iter()
        .filter(|a| !matches!(a, Expr::NamedArg(..)))
        .collect();
    let [list, key] = positional.as_slice() else {
        return err(
            "type-scroll-into-view",
            format!("{USAGE}: a list's `id` and a row's key, then options by name"),
            span,
        );
    };
    if !matches!(list, Expr::Str(..)) {
        return err(
            "type-scroll-into-view",
            format!("the list is named by its literal `id`: {USAGE}"),
            list.span(),
        );
    }
    infer(key, scope, shapes)?;
    let mut seen = BTreeSet::new();
    for arg in args {
        let Expr::NamedArg(name, value, at) = arg else {
            continue;
        };
        if !seen.insert(name.as_str()) {
            return err(
                "type-scroll-into-view",
                format!("`{name}` is given twice"),
                *at,
            );
        }
        let allowed: &[&str] = match name.as_str() {
            "block" | "inline" => &["start", "center", "end", "nearest"],
            "behavior" => &["auto", "instant", "smooth"],
            "row" => {
                infer(value, scope, shapes)?;
                continue;
            }
            _ => {
                return err(
                    "type-scroll-into-view",
                    format!("`scrollIntoView` has no option `{name}`: {USAGE}"),
                    *at,
                )
            }
        };
        match value.as_ref() {
            Expr::Str(s, _) if allowed.contains(&s.as_str()) => {}
            _ => {
                return err(
                    "type-scroll-into-view",
                    format!("`{name}` is one of {}", allowed.join(", ")),
                    *at,
                )
            }
        }
    }
    Ok(())
}

/// A host command's name and arguments (`name(args)` in an action body).
pub(super) fn check_command(
    name: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    if !HOST_COMMANDS.contains(&name) {
        let message = match scope.lookup(name) {
            Some((Ref::Action(_), Ty::Action(_))) => format!(
                "`{name}` is an action, not a host command: an action is not callable from an action; put its statements here, or bind it to an element (`press={name}`)"
            ),
            Some((Ref::Prop(_), Ty::Action(_))) => format!(
                "`{name}` is an action prop: an action calls one only as its last statement (its tail call), or binds it to an element (`press={name}`)"
            ),
            _ => format!(
                "`{name}` is not a host command; the hosts answer {}",
                HOST_COMMANDS.join(", ")
            ),
        };
        return err("type-unknown-command", message, span);
    }
    if name == "preventDefault" && !args.is_empty() {
        return err(
            "type-prevent-default",
            "`preventDefault()` takes no arguments: it prevents the default action of the key event that ran this action",
            span,
        );
    }
    if name == "share" {
        return share_args(args, scope, shapes, span);
    }
    if name == "saveFile" {
        return save_file_args(args, scope, shapes, span);
    }
    if name.starts_with("show") && name.ends_with("Picker") && name != "showPicker" {
        return picker_args(name, args, scope, shapes, span);
    }
    if name == "scrollIntoView" {
        return into_view_args(args, scope, shapes, span);
    }
    if name == "postMessage" {
        // The web's argument order, `postMessage(message, target)`: the target
        // is a surface's literal name, checked against the app's canvases.
        const USAGE: &str = "`postMessage(text, \"world\")`";
        let [text, surface] = args else {
            return err(
                "type-post-message",
                format!("{USAGE}: the message, then a surface's literal name"),
                span,
            );
        };
        if !matches!(infer(text, scope, shapes)?, Ty::String) {
            return err(
                "type-post-message",
                format!("the message is a string: {USAGE}"),
                text.span(),
            );
        }
        if !matches!(surface, Expr::Str(..)) {
            return err(
                "type-post-message",
                format!("the surface is named literally, as in `surface=world()`: {USAGE}"),
                surface.span(),
            );
        }
        return Ok(());
    }
    for arg in args {
        infer(arg, scope, shapes)?;
    }
    Ok(())
}

/// Check a view, recording each attribute's refusal and moving on. A
/// region whose subject does not type has no scope for its body, which is
/// then left to the next run.
pub(super) fn check_view(nodes: &[Node], scope: &Scope, shapes: &Shapes, sink: &mut Sink) {
    for n in nodes {
        match n {
            Node::Children { .. } => {}
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                ..
            } => {
                for p in positional {
                    // `scroll document` and `input switch` are words, not names
                    // (LLP 1048.003 D4, LLP 1069.001 D1).
                    if !contract_syntax::is_scroll_document(tag, p)
                        && !contract_syntax::is_input_switch(tag, p)
                        && !contract_syntax::is_input_multiple(tag, p)
                    {
                        sink.keep(infer(p, scope, shapes));
                    }
                }
                for a in attrs {
                    sink.keep_unit(check_attr(a, scope, shapes));
                }
                check_view(children, scope, shapes, sink);
            }
            Node::Use { args, children, .. } => {
                check_view(children, scope, shapes, sink);
                for a in args {
                    sink.keep(infer(&a.value, scope, shapes));
                }
            }
            Node::When {
                cond,
                then,
                otherwise,
                ..
            } => {
                match infer(cond, scope, shapes) {
                    Ok(Ty::Bool) => {}
                    Ok(_) => sink.push(TypeError {
                        id: "type-condition",
                        message: "`when` needs a bool".into(),
                        span: cond.span(),
                    }),
                    Err(e) => sink.push(e),
                }
                check_view(then, scope, shapes, sink);
                check_view(otherwise, scope, shapes, sink);
            }
            Node::Each {
                var,
                index,
                list,
                key,
                body,
                ..
            } => {
                let item = match infer(list, scope, shapes) {
                    // `each x in []`: nothing says what `x` would be.
                    Ok(Ty::List(item)) if !item.is_complete() => {
                        sink.push(TypeError {
                            id: "type-cannot-infer",
                            message: format!(
                                "cannot infer what `{}` holds: `each` needs a typed list, and `[]` shows nothing",
                                var
                            ),
                            span: list.span(),
                        });
                        continue;
                    }
                    Ok(Ty::List(item)) => *item,
                    Ok(lt) => {
                        sink.push(TypeError {
                            id: "type-each-list",
                            message: format!("`each` needs a list, given `{lt}`"),
                            span: list.span(),
                        });
                        continue;
                    }
                    Err(e) => {
                        sink.push(e);
                        continue;
                    }
                };
                let mut inner = scope.clone();
                inner.push_each(var, index.as_deref(), item);
                match infer(key, &inner, shapes) {
                    Ok(Ty::String | Ty::Number | Ty::Bool) => {}
                    Ok(kt) => sink.push(TypeError {
                        id: "type-each-key",
                        message: format!("a key must be a string, number, or bool, not `{kt}`"),
                        span: key.span(),
                    }),
                    Err(e) => sink.push(e),
                }
                check_view(body, &inner, shapes, sink);
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let item = match infer(subject, scope, shapes) {
                    Ok(Ty::Option(item)) => Some(*item),
                    Ok(st) => {
                        sink.push(TypeError {
                            id: "type-match-subject",
                            message: format!("`match` needs an option, given `{st}`"),
                            span: subject.span(),
                        });
                        None
                    }
                    Err(e) => {
                        sink.push(e);
                        None
                    }
                };
                if let Some(item) = item {
                    let mut inner = scope.clone();
                    inner.push_region(Some((some.0.clone(), Ref::Bound(0), item)));
                    check_view(&some.1, &inner, shapes, sink);
                }
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                check_view(none, &none_scope, shapes, sink);
            }
        }
    }
}

/// One element attribute's value.
fn check_attr(a: &Attr, scope: &Scope, shapes: &Shapes) -> Result<(), TypeError> {
    if a.name == "class" {
        // `class=Name` names a `style`, resolved at lowering; `class=(cond ?
        // A : B)` chooses between two, and only its condition is typed here.
        return match &a.value {
            Expr::Ident(..) => Ok(()),
            Expr::Ternary(cond, yes, no, _)
                if matches!((&**yes, &**no), (Expr::Ident(..), Expr::Ident(..))) =>
            {
                let tc = infer(cond, scope, shapes)?;
                if tc != Ty::Bool {
                    return err(
                        "type-condition",
                        format!("a condition must be a bool, given `{tc}`"),
                        cond.span(),
                    );
                }
                Ok(())
            }
            _ => err(
                "type-class-name",
                "`class=` names a style declared with `style Name`, or chooses between two: `class=(cond ? A : B)`",
                a.span,
            ),
        };
    }
    if a.name == "surface" {
        // `surface=name(args)`: the name is the GPU module's,
        // not a function; the arguments are expressions.
        if let Expr::Call(_, args, _) = &a.value {
            let named = args.iter().any(|arg| matches!(arg, Expr::NamedArg(..)));
            let mut names = std::collections::BTreeSet::new();
            for arg in args {
                let value = match arg {
                    Expr::NamedArg(name, value, span) => {
                        if !names.insert(name) {
                            return err(
                                "type-surface-argument",
                                format!("duplicate surface argument `{name}`"),
                                *span,
                            );
                        }
                        value.as_ref()
                    }
                    _ if named => {
                        return err(
                            "type-surface-argument",
                            format!(
                                "use either named or positional surface arguments (`{}` is named)",
                                args.iter()
                                    .find_map(|arg| match arg {
                                        Expr::NamedArg(name, _, _) => Some(name),
                                        _ => None,
                                    })
                                    .unwrap()
                            ),
                            arg.span(),
                        )
                    }
                    _ => arg,
                };
                infer(value, scope, shapes)?;
            }
        }
        return Ok(());
    }
    if shapes.style_attr.is_some_and(|style| style(&a.name)) {
        return style_value(&a.value, scope, shapes).map(|_| ());
    }
    // @ref LLP 1053.000.000.000 D1 — `glassGroup` alone may put the literal
    // `"auto"` beside numbers in a choice; lowering rewrites it to `-1`.
    if a.name == "glassGroup" {
        return glass_group_value(&a.value, scope, shapes).map(|_| ());
    }
    infer(&a.value, scope, shapes).map(|_| ())
}

/// `glassGroup`'s value: a number, the literal `"auto"` (typed as the number
/// it lowers to), or a choice between them, through a `let` a shared derive
/// makes. A number beside a string a component's prop forwards is unknown
/// here; lowering sees the value expanded, the forwarded `"auto"` a literal.
fn glass_group_value(e: &Expr, scope: &Scope, shapes: &Shapes) -> Result<Ty, TypeError> {
    match e {
        Expr::Str(s, _) if s == "auto" => Ok(Ty::Number),
        Expr::Ternary(..) | Expr::Match { .. } => {
            let (ta, tb) = arms(e, scope, shapes, glass_group_value)?;
            let either = |a: &Ty, b: &Ty| {
                matches!((a, b), (Ty::Number, Ty::String) | (Ty::String, Ty::Number))
            };
            match ta.unify(&tb) {
                Some(t) => Ok(t),
                None if either(&ta, &tb) => Ok(Ty::Unknown),
                None => Err(disagree(e, &ta, &tb)),
            }
        }
        Expr::Let {
            name, value, body, ..
        } => {
            let t = if spacing_tree(value) {
                glass_group_value(value, scope, shapes)?
            } else {
                infer(value, scope, shapes)?
            };
            let mut inner = scope.clone();
            inner.push(vec![(name.clone(), Ref::Local(0), t)]);
            glass_group_value(body, &inner, shapes)
        }
        _ => infer(e, scope, shapes),
    }
}

/// Whether a value is a spacing as written: `"auto"`, a number, or a choice
/// of them — so a `let` binding one is rewritten and one binding a condition
/// is not.
fn spacing_tree(e: &Expr) -> bool {
    match e {
        Expr::Str(s, _) => s == "auto",
        Expr::Number(..) => true,
        Expr::Unary(_, inner, _) => matches!(**inner, Expr::Number(..)),
        Expr::Ternary(_, a, b, _) => spacing_tree(a) && spacing_tree(b),
        Expr::Match { some, none, .. } => spacing_tree(some) && spacing_tree(none),
        _ => false,
    }
}

/// A style row is one CSS value space — a length or a keyword — so a style
/// attribute's ternary or `match` may put a number in one arm and a string
/// in the other; lowering checks each literal against the row and the
/// runner converts each value. Arms that disagree otherwise are refused as
/// any expression's are.
fn style_value(e: &Expr, scope: &Scope, shapes: &Shapes) -> Result<Ty, TypeError> {
    if !matches!(e, Expr::Ternary(..) | Expr::Match { .. }) {
        return infer(e, scope, shapes);
    }
    let (ta, tb) = arms(e, scope, shapes, style_value)?;
    let dimension = |t: &Ty| matches!(t, Ty::Number | Ty::String);
    match ta.unify(&tb) {
        Some(t) => Ok(t),
        None if dimension(&ta) && dimension(&tb) => Ok(Ty::Unknown),
        None => Err(disagree(e, &ta, &tb)),
    }
}

#[cfg(test)]
mod tests {
    use super::{can_unify, Ty};

    #[test]
    fn compatibility_preserves_unknowns_nested_types_and_action_wildcards() {
        let atoms = vec![
            Ty::Number,
            Ty::String,
            Ty::Bool,
            Ty::Unit,
            Ty::Unknown,
            Ty::Record("A".into()),
            Ty::Record("B".into()),
            Ty::Action(vec![]),
        ];
        let mut types = atoms.clone();
        for t in &atoms {
            types.push(Ty::Option(Box::new(t.clone())));
            types.push(Ty::List(Box::new(t.clone())));
            types.push(Ty::Action(vec![t.clone()]));
            types.push(Ty::Option(Box::new(Ty::List(Box::new(t.clone())))));
            for u in &atoms {
                types.push(Ty::Action(vec![t.clone(), u.clone()]));
            }
        }
        for a in &types {
            for b in &types {
                assert_eq!(can_unify(a, b), a.unify(b).is_some(), "{a:?} / {b:?}");
            }
        }
    }
}
