//! Lexical declaration checking and resource dependency cycles.

use super::{
    err, infer, record_source, types_scope, ComponentTypes, Ref, Scope, Ty, TypeError, Types,
};
use contract_syntax::{Component, Expanded, Expr, Node, TemplatePart};
use std::collections::{BTreeMap, BTreeSet};

type Owners<'a> = Option<&'a [Option<u32>]>;

/// Infer child state in tree order, before visiting regions controlled by it.
pub(super) fn infer_owned_state_initializers(
    c: &Component,
    ct: &mut ComponentTypes,
    types: &Types,
    owners: Owners<'_>,
) -> Result<(), TypeError> {
    if owners.is_some() {
        walk_scopes(
            &c.view,
            &types_scope(c, ct, types),
            c,
            ct,
            types,
            owners,
            &mut BTreeMap::new(),
        )?;
    }
    Ok(())
}

fn refreshed(scope: &Scope, c: &Component, ct: &ComponentTypes, types: &Types) -> Scope {
    let mut current = types_scope(c, ct, types);
    current
        .frames
        .extend(scope.frames.iter().filter(|f| f.region).cloned());
    current
}

#[allow(clippy::too_many_arguments)]
fn walk_scopes(
    nodes: &[Node],
    scope: &Scope,
    c: &Component,
    ct: &mut ComponentTypes,
    types: &Types,
    owners: Owners<'_>,
    scopes: &mut BTreeMap<u32, Scope>,
) -> Result<(), TypeError> {
    for node in nodes {
        let scope = refreshed(scope, c, ct, types);
        match node {
            Node::Scope { tag, body, .. } => {
                let mut inner = scope.clone();
                inner.push_region(None);
                if let Some(owners) = owners {
                    for (i, state) in c.states.iter().enumerate() {
                        if owners[i] == Some(*tag) {
                            ct.slots[i] = infer(
                                &state.expr,
                                &refreshed(&inner, c, ct, types),
                                &types.shapes,
                            )?;
                        }
                    }
                }
                inner = refreshed(&inner, c, ct, types);
                scopes.insert(*tag, inner.clone());
                walk_scopes(body, &inner, c, ct, types, owners, scopes)?;
            }
            Node::Element { children, .. } | Node::Use { children, .. } => {
                walk_scopes(children, &scope, c, ct, types, owners, scopes)?;
            }
            Node::Provide { body, .. } => walk_scopes(body, &scope, c, ct, types, owners, scopes)?,
            Node::Children { .. } => {}
            Node::When {
                then, otherwise, ..
            } => {
                let mut inner = scope.clone();
                inner.push_region(None);
                walk_scopes(then, &inner, c, ct, types, owners, scopes)?;
                walk_scopes(otherwise, &inner, c, ct, types, owners, scopes)?;
            }
            Node::Each {
                var, list, body, ..
            } => {
                let ty = infer(list, &scope, &types.shapes)?;
                let Ty::List(item) = ty else {
                    return err(
                        "type-each-list",
                        format!("`each` needs a list, given `{ty}`"),
                        list.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
                walk_scopes(body, &inner, c, ct, types, owners, scopes)?;
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let ty = infer(subject, &scope, &types.shapes)?;
                let Ty::Option(item) = ty else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{ty}`"),
                        subject.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                walk_scopes(&some.1, &inner, c, ct, types, owners, scopes)?;
                let mut inner = scope.clone();
                inner.push_region(None);
                walk_scopes(none, &inner, c, ct, types, owners, scopes)?;
            }
        }
    }
    Ok(())
}

pub(super) fn check_resources_and_tasks(
    c: &Component,
    ct: &mut ComponentTypes,
    types: &Types,
    expanded: Option<&Expanded>,
) -> Result<(), TypeError> {
    let scope = types_scope(c, ct, types);
    let mut scopes = BTreeMap::new();
    if expanded.is_some() {
        walk_scopes(&c.view, &scope, c, ct, types, None, &mut scopes)?;
    }
    for (i, resource) in c.resources.iter().enumerate() {
        let owner = expanded.and_then(|ex| ex.resource_owners[i]);
        let scope = owner.and_then(|tag| scopes.get(&tag)).unwrap_or(&scope);
        let params = resource
            .args
            .iter()
            .map(|arg| infer(arg, scope, &types.shapes))
            .collect::<Result<Vec<_>, _>>()?;
        let result = ct.resources[i].clone();
        if let Some(fallback) = &resource.fallback {
            let got = infer(fallback, scope, &types.shapes)?;
            if result.unify(&got).is_none() {
                return err(
                    "type-resource-fallback",
                    format!(
                        "`{}` needs a `{result}` fallback, given `{got}`",
                        resource.name
                    ),
                    fallback.span(),
                );
            }
        }
        record_source(ct, &resource.source, params, result, resource.span)?;
    }
    for (i, task) in c.tasks.iter().enumerate() {
        let owner = expanded.and_then(|ex| ex.task_owners[i]);
        let scope = owner.and_then(|tag| scopes.get(&tag)).unwrap_or(&scope);
        if infer(&task.every.0, scope, &types.shapes)? != Ty::Number {
            return err(
                "type-timer",
                "`every` needs a number of milliseconds",
                task.every.2,
            );
        }
        if let Some(ex) = expanded {
            for arg in &ex.task_args[i] {
                infer(arg, scope, &types.shapes)?;
            }
        }
    }
    Ok(())
}

pub(super) fn check_cycles(c: &Component) -> Result<(), TypeError> {
    let names: BTreeMap<&str, usize> = c
        .resources
        .iter()
        .map(|r| r.name.as_str())
        .chain(c.derives.iter().map(|d| d.name.as_str()))
        .enumerate()
        .map(|(i, n)| (n, i))
        .collect();
    let mut edges = vec![BTreeSet::new(); names.len()];
    // Duplicate names are diagnosed by the declaration checker.
    if names.len() != c.resources.len() + c.derives.len() {
        return Ok(());
    }
    for (i, resource) in c.resources.iter().enumerate() {
        for expr in resource.args.iter().chain(resource.fallback.iter()) {
            dependencies(expr, &names, &BTreeSet::new(), &mut edges[i]);
        }
    }
    for (i, derive) in c.derives.iter().enumerate() {
        dependencies(
            &derive.expr,
            &names,
            &BTreeSet::new(),
            &mut edges[c.resources.len() + i],
        );
    }
    fn visit(
        i: usize,
        edges: &[BTreeSet<usize>],
        status: &mut [u8],
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        if status[i] == 2 {
            return None;
        }
        if status[i] == 1 {
            return Some(path[path.iter().position(|j| *j == i).unwrap()..].to_vec());
        }
        status[i] = 1;
        path.push(i);
        for next in &edges[i] {
            if let Some(cycle) = visit(*next, edges, status, path) {
                return Some(cycle);
            }
        }
        path.pop();
        status[i] = 2;
        None
    }
    let mut status = vec![0; edges.len()];
    for i in 0..edges.len() {
        if let Some(cycle) = visit(i, &edges, &mut status, &mut Vec::new()) {
            let resource = cycle.iter().find(|i| **i < c.resources.len());
            let (id, span) = match resource {
                Some(i) => ("type-resource-cycle", c.resources[*i].span),
                None => (
                    "type-derive-cycle",
                    c.derives[cycle[0] - c.resources.len()].span,
                ),
            };
            return err(id, "resource/derive dependencies contain a cycle", span);
        }
    }
    Ok(())
}

fn dependencies(
    expr: &Expr,
    names: &BTreeMap<&str, usize>,
    bound: &BTreeSet<String>,
    out: &mut BTreeSet<usize>,
) {
    match expr {
        Expr::Ident(name, _) => {
            if !bound.contains(name) {
                if let Some(i) = names.get(name.as_str()) {
                    out.insert(*i);
                }
            }
        }
        Expr::Call(_, args, _) | Expr::List(args, _) => {
            for arg in args {
                dependencies(arg, names, bound, out);
            }
        }
        Expr::Member(e, _, _) | Expr::Some(e, _) | Expr::Unary(_, e, _) => {
            dependencies(e, names, bound, out)
        }
        Expr::Binary(_, a, b, _) => {
            dependencies(a, names, bound, out);
            dependencies(b, names, bound, out);
        }
        Expr::Ternary(a, b, c, _) => {
            dependencies(a, names, bound, out);
            dependencies(b, names, bound, out);
            dependencies(c, names, bound, out);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            dependencies(subject, names, bound, out);
            let mut inner = bound.clone();
            inner.insert(var.clone());
            dependencies(some, names, &inner, out);
            dependencies(none, names, bound, out);
        }
        Expr::Template(parts, _) => {
            for part in parts {
                if let TemplatePart::Expr(e) = part {
                    dependencies(e, names, bound, out);
                }
            }
        }
        Expr::Number(..) | Expr::Bool(..) | Expr::Str(..) | Expr::None(..) => {}
    }
}
