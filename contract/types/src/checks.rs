//! Type diagnostics and component checks that require recursive traversal.

use super::{err, infer, ComponentTypes, Ref, Scope, Shapes, Ty, TypeError, Types};
use contract_syntax::{
    one_spelling_edit, Attr, Component, Expr, File, Node, Span, Stmt, TemplatePart, TypeExpr,
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
        | Expr::NamedArg(_, x, _) => calls_in(x, indices, out),
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
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    calls_in(x, indices, out);
                }
            }
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => {}
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
    TypeError {
        id: "type-unknown-prop",
        message: format!(
            "`{}` has no {noun} {}; {choices}",
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
        TypeError {
            id: "type-unknown-field",
            message: format!("`{shape}` has no field `{field}`; {hint}"),
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
        candidate == "pending"
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
        .chain(["pending", "path"]);
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

/// The lexical scope at each expanded `each` tag.
fn owner_scopes(
    c: &Component,
    ct: &ComponentTypes,
    types: &Types,
) -> Result<BTreeMap<u32, Scope>, TypeError> {
    let mut scopes = BTreeMap::new();
    collect_owner_scopes(
        &c.view,
        &types.component_scope(c, ct),
        &types.shapes,
        &mut scopes,
    )?;
    Ok(scopes)
}

/// Infer lifted row-slot initializers in the region frames that own them.
pub(super) fn infer_owned_state_initializers(
    c: &Component,
    ct: &mut ComponentTypes,
    types: &Types,
    owners: Option<&[Option<u32>]>,
) -> Result<(), TypeError> {
    let Some(owners) = owners else {
        return Ok(());
    };
    let scopes = owner_scopes(c, ct, types)?;
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
    for (i, state) in c.states.iter().enumerate() {
        if let Some(tag) = owners.get(i).copied().flatten() {
            let Some(owner_scope) = scopes.get(&tag) else {
                return err(
                    "type-row-slot",
                    format!("row state `{}` has no owning `each`", state.name),
                    state.span,
                );
            };
            let mut scope = Scope::default();
            scope.frames_reset(&names);
            scope.frames.extend(
                owner_scope
                    .frames
                    .iter()
                    .filter(|frame| frame.region)
                    .cloned(),
            );
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
    scopes: &mut BTreeMap<u32, Scope>,
) -> Result<(), TypeError> {
    for node in nodes {
        match node {
            Node::Element { children, .. } | Node::Use { children, .. } => {
                collect_owner_scopes(children, scope, shapes, scopes)?;
            }
            Node::Provide { body, .. } => collect_owner_scopes(body, scope, shapes, scopes)?,
            Node::Children { .. } => {}
            Node::When {
                then, otherwise, ..
            } => {
                collect_owner_scopes(then, scope, shapes, scopes)?;
                collect_owner_scopes(otherwise, scope, shapes, scopes)?;
            }
            Node::Each {
                tag,
                var,
                list,
                body,
                ..
            } => {
                let ty = infer(list, scope, shapes)?;
                let Ty::List(item) = ty else {
                    return err(
                        "type-each-list",
                        format!("`each` needs a list, given `{ty}`"),
                        list.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
                scopes.insert(*tag, inner.clone());
                collect_owner_scopes(body, &inner, shapes, scopes)?;
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let ty = infer(subject, scope, shapes)?;
                let Ty::Option(item) = ty else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{ty}`"),
                        subject.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                collect_owner_scopes(&some.1, &inner, shapes, scopes)?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                collect_owner_scopes(none, &none_scope, shapes, scopes)?;
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Fill {
    nodes: Vec<Node>,
    scope: Scope,
}

/// Check the concrete provider path to every inject after component typing.
pub(super) fn check_injects(
    nodes: &[Node],
    scope: &Scope,
    types: &Types,
    file: &File,
) -> Result<(), TypeError> {
    check_inject_nodes(nodes, scope, types, file, &mut Vec::new(), None, 0)
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
                let target = file
                    .components
                    .iter()
                    .position(|component| &component.name == name)
                    .expect("component uses were checked before injects");
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
                                "`provide {} = …` is `{got}`, but `{name}` injects `{want}`",
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
                });
                check_inject_nodes(
                    &target_c.view,
                    &target_scope,
                    types,
                    file,
                    provides,
                    child_fill.as_ref(),
                    depth + 1,
                )?;
            }
            Node::Provide {
                name,
                expr,
                body,
                span,
            } => {
                provides.push((name.clone(), infer(expr, scope, &types.shapes)?, *span));
                check_inject_nodes(body, scope, types, file, provides, fill, depth)?;
                provides.pop();
            }
            Node::Children { .. } => {
                if let Some(fill) = fill {
                    check_inject_nodes(
                        &fill.nodes,
                        &fill.scope,
                        types,
                        file,
                        provides,
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
                var, list, body, ..
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
                inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
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

/// Check an action body's statements through every branch (LLP 1017 P2).
pub(super) fn check_stmts(
    stmts: &[Stmt],
    scope: &Scope,
    c: &Component,
    ct: &mut ComponentTypes,
    shapes: &Shapes,
) -> Result<(), TypeError> {
    for stmt in stmts {
        match stmt {
            Stmt::Assign { target, expr, span } => {
                let Some(si) = c.states.iter().position(|s| &s.name == target) else {
                    // A mutation's slot may be assigned (`session = none`);
                    // its type is `option<T>` and is never inferred from here.
                    if let Some(mi) = c.mutations.iter().position(|m| &m.name == target) {
                        let t = infer(expr, scope, shapes)?;
                        let mt = Ty::Option(Box::new(ct.mutations[mi].clone()));
                        if !can_unify(&mt, &t) {
                            return err(
                                "type-assign",
                                format!("`{target}` is `{mt}`, cannot assign `{t}`"),
                                *span,
                            );
                        }
                        continue;
                    }
                    return err(
                        "type-assign-not-state",
                        format!("`{target}` is not a state or a mutation"),
                        *span,
                    );
                };
                let t = infer(expr, scope, shapes)?;
                match ct.slots[si].unify(&t) {
                    Some(u) => ct.slots[si] = u,
                    None => {
                        return err(
                            "type-assign",
                            format!("`{target}` is `{}`, cannot assign `{t}`", ct.slots[si]),
                            *span,
                        )
                    }
                }
            }
            Stmt::Command { args, .. } => {
                for arg in args {
                    infer(arg, scope, shapes)?;
                }
            }
            Stmt::Send {
                target,
                source,
                args,
                span,
                ..
            } => {
                if !c.mutations.iter().any(|m| &m.name == target) {
                    return err(
                        "type-send-not-mutation",
                        format!(
                            "`{target}` is not a mutation: declare `mutation {target} as shape T`"
                        ),
                        *span,
                    );
                }
                let mut params = Vec::with_capacity(args.len());
                for arg in args {
                    params.push(infer(arg, scope, shapes)?);
                }
                let mi = c
                    .mutations
                    .iter()
                    .position(|m| &m.name == target)
                    .expect("checked above");
                let result = ct.mutations[mi].clone();
                crate::record_source(ct, source, params, result, *span)?;
            }
            Stmt::Refresh { target, span } => {
                if !c.resources.iter().any(|r| &r.name == target) {
                    return err(
                        "type-refresh-not-resource",
                        format!("`{target}` is not a resource"),
                        *span,
                    );
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                if infer(cond, scope, shapes)? != Ty::Bool {
                    return err("type-condition", "`if` needs a bool", cond.span());
                }
                check_stmts(then, scope, c, ct, shapes)?;
                check_stmts(otherwise, scope, c, ct, shapes)?;
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                let ts = infer(subject, scope, shapes)?;
                let Ty::Option(inner) = ts else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{ts}`"),
                        subject.span(),
                    );
                };
                let mut inner_scope = scope.clone();
                inner_scope.push(vec![(some.0.clone(), Ref::Local(0), (*inner).clone())]);
                check_stmts(&some.1, &inner_scope, c, ct, shapes)?;
                check_stmts(none, scope, c, ct, shapes)?;
            }
        }
    }
    Ok(())
}

pub(super) fn check_view(nodes: &[Node], scope: &Scope, shapes: &Shapes) -> Result<(), TypeError> {
    for n in nodes {
        match n {
            Node::Provide { expr, body, .. } => {
                infer(expr, scope, shapes)?;
                check_view(body, scope, shapes)?;
            }
            Node::Children { .. } => {}
            Node::Element {
                positional,
                attrs,
                children,
                ..
            } => {
                for p in positional {
                    infer(p, scope, shapes)?;
                }
                for a in attrs {
                    if a.name == "class" {
                        // `class=Name` names a `style`, resolved at lowering.
                        if !matches!(a.value, Expr::Ident(..)) {
                            return err(
                                "type-class-name",
                                "`class=` names a style declared with `style Name`",
                                a.span,
                            );
                        }
                        continue;
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
                                            format!("use either named or positional surface arguments (`{}` is named)", args.iter().find_map(|arg| match arg { Expr::NamedArg(name, _, _) => Some(name), _ => None }).unwrap()),
                                            arg.span(),
                                        )
                                    }
                                    _ => arg,
                                };
                                infer(value, scope, shapes)?;
                            }
                        }
                        continue;
                    }
                    infer(&a.value, scope, shapes)?;
                }
                check_view(children, scope, shapes)?;
            }
            Node::Use { args, children, .. } => {
                check_view(children, scope, shapes)?;
                for a in args {
                    infer(&a.value, scope, shapes)?;
                }
            }
            Node::When {
                cond,
                then,
                otherwise,
                ..
            } => {
                if infer(cond, scope, shapes)? != Ty::Bool {
                    return err("type-condition", "`when` needs a bool", cond.span());
                }
                check_view(then, scope, shapes)?;
                check_view(otherwise, scope, shapes)?;
            }
            Node::Each {
                var,
                list,
                key,
                body,
                ..
            } => {
                let lt = infer(list, scope, shapes)?;
                let Ty::List(item) = lt else {
                    return err(
                        "type-each-list",
                        format!("`each` needs a list, given `{lt}`"),
                        list.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
                let kt = infer(key, &inner, shapes)?;
                if !matches!(kt, Ty::String | Ty::Number | Ty::Bool) {
                    return err(
                        "type-each-key",
                        format!("a key must be a string, number, or bool, not `{kt}`"),
                        key.span(),
                    );
                }
                check_view(body, &inner, shapes)?;
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let st = infer(subject, scope, shapes)?;
                let Ty::Option(item) = st else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{st}`"),
                        subject.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                check_view(&some.1, &inner, shapes)?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                check_view(none, &none_scope, shapes)?;
            }
        }
    }
    Ok(())
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
