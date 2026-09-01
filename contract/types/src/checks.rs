//! Component checks that require recursive action or view traversal.

use super::{err, infer, types_scope, ComponentTypes, Ref, Scope, Shapes, Ty, TypeError, Types};
use contract_syntax::{Component, Expr, File, Node, Span, Stmt, TypeExpr};
use std::collections::BTreeMap;

/// Reject shape cycles before lowering recursively materializes plan types.
pub(super) fn check_shape_cycles(file: &File) -> Result<(), TypeError> {
    let indices: BTreeMap<&str, usize> = file
        .shapes
        .iter()
        .enumerate()
        .map(|(i, shape)| (shape.name.as_str(), i))
        .collect();
    let mut states = vec![0u8; file.shapes.len()];
    let mut path = Vec::new();
    for i in 0..file.shapes.len() {
        visit_shape(i, file, &indices, &mut states, &mut path)?;
    }
    Ok(())
}

fn visit_shape(
    index: usize,
    file: &File,
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
        let mut names = Vec::new();
        shape_names(&field.ty, &mut names);
        for name in names {
            let Some(&next) = indices.get(name) else {
                continue;
            };
            if states[next] == 1 {
                let start = path.iter().position(|part| part == name).unwrap_or(0);
                let mut cycle = path[start..].to_vec();
                cycle.push(name.to_string());
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
            visit_shape(next, file, indices, states, path)?;
        }
    }
    path.pop();
    states[index] = 2;
    Ok(())
}

fn shape_names<'a>(ty: &'a TypeExpr, out: &mut Vec<&'a str>) {
    match ty {
        TypeExpr::Named(name, _) => out.push(name),
        TypeExpr::Option(inner, _) | TypeExpr::List(inner, _) => shape_names(inner, out),
    }
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
        &types_scope(c, ct, types),
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
                    if want.unify(got).is_none() {
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
                        if mt.unify(&t).is_none() {
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
                target, args, span, ..
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
                for arg in args {
                    infer(arg, scope, shapes)?;
                }
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
                            for arg in args {
                                infer(arg, scope, shapes)?;
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
