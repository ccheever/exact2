//! Component uses, checked where they are written and before the expanded
//! root, so a misspelled or mistyped argument is reported at its call site
//! rather than inside the component it was substituted into.

use super::{checks, err, infer, ComponentTypes, Ref, Scope, Sink, Ty, TypeError, Types};
use contract_syntax::{Component, File, Node};

/// Every use names only the used component's props (before expansion, which
/// would otherwise report the prop a misspelling left missing).
pub(crate) fn check_prop_names(
    nodes: &[Node],
    file: &File,
    sink: &mut Sink,
    explained: &mut Vec<contract_syntax::Span>,
) {
    for n in nodes {
        match n {
            Node::Use {
                name,
                args,
                children,
                span,
            } => {
                if let Some(target) = file.components.iter().find(|c| &c.name == name) {
                    if let Some(a) = args
                        .iter()
                        .find(|a| !target.props.iter().any(|p| p.name == a.name))
                    {
                        sink.push(checks::unknown_props(target, args, a.span));
                        let unknown: Vec<_> = args
                            .iter()
                            .filter(|a| !target.props.iter().any(|p| p.name == a.name))
                            .collect();
                        let missing: Vec<&str> = target
                            .props
                            .iter()
                            .filter(|p| !args.iter().any(|a| a.name == p.name))
                            .map(|p| p.name.as_str())
                            .collect();
                        if let ([given], [meant]) = (unknown.as_slice(), missing.as_slice()) {
                            if contract_syntax::suggestion(&given.name, [*meant]) == Some(*meant) {
                                explained.push(*span);
                            }
                        }
                    }
                }
                check_prop_names(children, file, sink, explained);
            }
            Node::Element { children, .. } => check_prop_names(children, file, sink, explained),
            Node::Provide { body, .. } => check_prop_names(body, file, sink, explained),
            Node::When {
                then, otherwise, ..
            } => {
                check_prop_names(then, file, sink, explained);
                check_prop_names(otherwise, file, sink, explained);
            }
            Node::Each { body, .. } => check_prop_names(body, file, sink, explained),
            Node::Match { some, none, .. } => {
                check_prop_names(&some.1, file, sink, explained);
                check_prop_names(none, file, sink, explained);
            }
            Node::Children { .. } => {}
        }
    }
}

/// The root's scope as far as its declarations alone type it, for checking
/// its call sites before the expanded view: anything not yet known is `?`,
/// which unifies with every type, so this scope can hide an error from
/// [`check_root_uses`] but never invent one.
fn provisional_scope(c: &Component, types: &Types, routes: bool) -> Scope {
    let shapes = &types.shapes;
    let resolve = |t: Option<&contract_syntax::TypeExpr>| {
        t.and_then(|t| shapes.resolve(t).ok())
            .unwrap_or(Ty::Unknown)
    };
    let mut ct = ComponentTypes {
        name: c.name.clone(),
        props: c
            .props
            .iter()
            .chain(&c.injects)
            .map(|p| resolve(p.ty.as_ref()))
            .collect(),
        slots: vec![Ty::Unknown; c.states.len()],
        derives: vec![Ty::Unknown; c.derives.len()],
        resources: c
            .resources
            .iter()
            .map(|r| resolve(Some(&r.shape)))
            .collect(),
        mutations: c
            .mutations
            .iter()
            .map(|m| resolve(Some(&m.shape)))
            .collect(),
        actions: c
            .actions
            .iter()
            .map(|a| a.params.iter().map(|p| resolve(p.ty.as_ref())).collect())
            .collect(),
        sources: Default::default(),
    };
    for i in 0..c.states.len() {
        ct.slots[i] = if i == 0 && routes {
            Ty::Record("Router".into())
        } else {
            infer(&c.states[i].expr, &types.component_scope(c, &ct), shapes).unwrap_or(Ty::Unknown)
        };
    }
    for _ in 0..c.derives.len() + 1 {
        let scope = types.component_scope(c, &ct);
        let derives: Vec<Ty> = c
            .derives
            .iter()
            .map(|d| infer(&d.expr, &scope, shapes).unwrap_or(Ty::Unknown))
            .collect();
        if derives == ct.derives {
            break;
        }
        ct.derives = derives;
    }
    types.component_scope(c, &ct)
}

/// The root's own uses, typed against [`provisional_scope`]. A refusal that
/// only a `?` produced is left for the full check, as the derive fixpoint does.
pub(crate) fn check_root_uses(
    root: &Component,
    expanded: &Component,
    types: &Types,
    file: &File,
    sink: &mut Sink,
) {
    let scope = provisional_scope(expanded, types, file.routes.is_some());
    let mut found = Sink::default();
    walk_uses(&root.view, &scope, types, file, false, &mut found);
    for e in found.errors {
        if !e.message.contains("`?`") {
            sink.push(e);
        }
    }
}

pub(crate) fn check_uses(
    nodes: &[Node],
    scope: &Scope,
    types: &Types,
    file: &File,
    sink: &mut Sink,
) {
    walk_uses(nodes, scope, types, file, true, sink)
}

/// `actions: false` checks only data arguments: an action argument (or a
/// provider's value) is refused where the child invokes it, which is what
/// its correction hints are built to follow (LLP 1006 §3).
fn walk_uses(
    nodes: &[Node],
    scope: &Scope,
    types: &Types,
    file: &File,
    actions: bool,
    sink: &mut Sink,
) {
    for n in nodes {
        let checked = walk_use(n, scope, types, file, actions, sink);
        sink.keep_unit(checked);
    }
}

/// One node's uses; a refusal ends only this node's walk.
fn walk_use(
    n: &Node,
    scope: &Scope,
    types: &Types,
    file: &File,
    actions: bool,
    sink: &mut Sink,
) -> Result<(), TypeError> {
    {
        match n {
            Node::Use {
                name,
                args,
                children,
                span,
            } => {
                walk_uses(children, scope, types, file, actions, sink);
                let Some(target) = file.components.iter().position(|c| &c.name == name) else {
                    return err(
                        "type-unknown-component",
                        file.unknown_component_message(name),
                        *span,
                    );
                };
                let target_c = &file.components[target];
                let target_t = &types.components[target];
                for (i, p) in target_c.props.iter().enumerate() {
                    let Some(arg) = args.iter().find(|a| a.name == p.name) else {
                        // At a use the root expands, a missing prop is expansion's refusal.
                        if !actions {
                            continue;
                        }
                        return err(
                            "type-missing-prop",
                            target_c.missing_props_message(args),
                            *span,
                        );
                    };
                    if !actions && matches!(target_t.props[i], Ty::Action(_)) {
                        continue;
                    }
                    let t = infer(&arg.value, scope, &types.shapes)?;
                    if !checks::can_unify(&t, &target_t.props[i]) {
                        return err(
                            "type-prop",
                            format!("`{}` expects `{}`, given `{t}`", p.name, target_t.props[i]),
                            arg.span,
                        );
                    }
                }
                for a in args {
                    if !target_c.props.iter().any(|p| p.name == a.name) {
                        return Err(checks::unknown_props(target_c, args, a.span));
                    }
                }
            }
            Node::Element { children, .. } => {
                walk_uses(children, scope, types, file, actions, sink)
            }
            Node::Provide { expr, body, .. } => {
                if actions {
                    infer(expr, scope, &types.shapes)?;
                }
                walk_uses(body, scope, types, file, actions, sink);
            }
            Node::Children { .. } => {}
            Node::When {
                then, otherwise, ..
            } => {
                walk_uses(then, scope, types, file, actions, sink);
                walk_uses(otherwise, scope, types, file, actions, sink);
            }
            Node::Each {
                var, list, body, ..
            } => {
                let lt = infer(list, scope, &types.shapes)?;
                let Ty::List(item) = lt else {
                    return err(
                        "type-each-list",
                        format!("`each` needs a list, given `{lt}`"),
                        list.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
                walk_uses(body, &inner, types, file, actions, sink);
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let st = infer(subject, scope, &types.shapes)?;
                let Ty::Option(item) = st else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{st}`"),
                        subject.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                walk_uses(&some.1, &inner, types, file, actions, sink);
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                walk_uses(none, &none_scope, types, file, actions, sink);
            }
        }
    }
    Ok(())
}
