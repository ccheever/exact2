//! Provider checks along component paths, bounded by emitted site depth.

use super::{err, infer, Ref, Scope, Ty, TypeError, Types};
use crate::checks::can_unify;
use contract_syntax::{Binding, File, Node, Span};

/// A slot's fill, checked where `children` stands with its caller's scope
/// and providers (LLP 1035.005.000 D9: the slot's own section does not
/// reach it).
#[derive(Clone)]
struct Fill<'a> {
    nodes: &'a [Node],
    scope: Scope,
    provides: Vec<(String, Ty, Span)>,
    outer: Option<Box<Fill<'a>>>,
}

/// Check the concrete provider path to every inject after component typing:
/// `provides` is the root's `provide` section, `scope` its scope.
pub(super) fn check_injects<'a>(
    nodes: &'a [Node],
    provides: &[Binding],
    scope: &Scope,
    types: &Types,
    file: &'a File,
) -> Result<(), TypeError> {
    let mut provided = Vec::new();
    for b in provides {
        provided.push((
            b.name.clone(),
            infer(&b.expr, scope, &types.shapes)?,
            b.span,
        ));
    }
    check_inject_nodes(nodes, scope, types, file, &mut provided, None, 0, 0)
}

#[allow(clippy::too_many_arguments)]
fn check_inject_nodes<'a>(
    nodes: &'a [Node],
    scope: &Scope,
    types: &Types,
    file: &'a File,
    provides: &mut Vec<(String, Ty, Span)>,
    fill: Option<&Fill<'a>>,
    depth: u32,
    site_depth: usize,
) -> Result<(), TypeError> {
    for node in nodes {
        let is_site = !matches!(node, Node::Use { .. } | Node::Children { .. });
        if is_site && site_depth >= 256 {
            return err(
                "syntax-inline-depth",
                "inlined views nest more than 256 sites deep",
                node.span(),
            );
        }
        let next_site_depth = site_depth + usize::from(is_site);
        match node {
            Node::Element { children, .. } => {
                check_inject_nodes(
                    children,
                    scope,
                    types,
                    file,
                    provides,
                    fill,
                    depth,
                    next_site_depth,
                )?;
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
                    nodes: children,
                    scope: scope.clone(),
                    provides: provides.clone(),
                    outer: fill.cloned().map(Box::new),
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
                    next_site_depth,
                );
                provides.truncate(outer);
                checked?;
            }
            Node::Children { .. } => {
                if let Some(fill) = fill {
                    check_inject_nodes(
                        fill.nodes,
                        &fill.scope,
                        types,
                        file,
                        &mut fill.provides.clone(),
                        fill.outer.as_deref(),
                        depth,
                        next_site_depth,
                    )?;
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                check_inject_nodes(
                    then,
                    scope,
                    types,
                    file,
                    provides,
                    fill,
                    depth,
                    next_site_depth,
                )?;
                check_inject_nodes(
                    otherwise,
                    scope,
                    types,
                    file,
                    provides,
                    fill,
                    depth,
                    next_site_depth,
                )?;
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
                        format!("`for` needs a list, given `{ty}`"),
                        list.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_each(var, index.as_deref(), *item);
                check_inject_nodes(
                    body,
                    &inner,
                    types,
                    file,
                    provides,
                    fill,
                    depth,
                    next_site_depth,
                )?;
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
                check_inject_nodes(
                    &some.1,
                    &inner,
                    types,
                    file,
                    provides,
                    fill,
                    depth,
                    next_site_depth,
                )?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                check_inject_nodes(
                    none,
                    &none_scope,
                    types,
                    file,
                    provides,
                    fill,
                    depth,
                    next_site_depth,
                )?;
            }
        }
    }
    Ok(())
}
