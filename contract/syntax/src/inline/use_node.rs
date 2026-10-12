//! Component use expansion has its own frame, separate from each view site.

use super::*;

#[inline(never)]
pub(super) fn inline_use(
    node: &Node,
    subst: &mut Subst<'_, Expr>,
    ctx: &mut Ctx<'_>,
) -> Result<Vec<Node>, SyntaxError> {
    let Node::Use {
        name,
        args,
        children,
        span,
    } = node
    else {
        unreachable!("inline_use only receives uses")
    };
    if ctx.depth > 32 {
        ctx.refuse(
            "syntax-inline-depth",
            format!("component `{name}` nests too deeply (a cycle?)"),
            *span,
        );
        return Ok(Vec::new());
    }
    let Some(c) = ctx.file.components.iter().find(|c| &c.name == name) else {
        let message = ctx.file.unknown_component_message(name);
        ctx.refuse("syntax-unknown-component", message, *span);
        return Ok(Vec::new());
    };
    // Renaming walks the child's authored tree while the caller's inline
    // frames remain live. Refuse an over-deep expansion before that walk.
    if exceeds_site_budget(&c.view, ctx.file, ctx.discarded_slots, ctx.site_depth) {
        ctx.refuse(
            "syntax-inline-depth",
            "inlined views nest more than 256 sites deep",
            *span,
        );
        return Ok(Vec::new());
    }
    let mut child_subst: BTreeMap<String, Expr> = BTreeMap::new();
    if c.props
        .iter()
        .any(|p| !args.iter().any(|a| a.name == p.name))
    {
        ctx.refuse("syntax-missing-prop", c.missing_props_message(args), *span);
    }
    for p in &c.props {
        let Some(a) = args.iter().find(|a| a.name == p.name) else {
            child_subst.insert(p.name.clone(), absent(*span));
            continue;
        };
        // The argument is an expression in the parent's scope: substitute the parent's own substitutions first.
        let value = subst_expr(&a.value, subst);
        // A `none` or `[]` in it is typed by the prop's declaration,
        // not left `?` for the child's reads to trip on.
        let value = match &p.ty {
            Some(ty) if untyped_leaf(&value) || (ctx.typed_props && !is_action(ty)) => {
                let span = value.span();
                Expr::Typed(Box::new(value), ty.clone(), span)
            }
            _ => value,
        };
        child_subst.insert(p.name.clone(), value);
    }
    for p in &c.injects {
        let Some((_, e)) = ctx.provides.iter().rev().find(|(n, _)| n == &p.name) else {
            let missing: Vec<_> = c
                .injects
                .iter()
                .filter(|inject| !ctx.provides.iter().any(|(n, _)| n == &inject.name))
                .collect();
            let names = missing
                .iter()
                .map(|p| format!("`{}`", p.name))
                .collect::<Vec<_>>()
                .join(", ");
            let bindings = missing
                .iter()
                .map(|p| format!("`{} = …`", p.name))
                .collect::<Vec<_>>()
                .join(", ");
            let them = if missing.len() == 1 { "it" } else { "them" };
            let message = format!(
                "`{name}` injects {names}, and no component above this use provides {them}: \
                             in this component or one that uses it, write a `provide` section with \
                             {bindings} indented under it"
            );
            ctx.refuse("syntax-missing-provide", message, *span);
            child_subst.insert(p.name.clone(), absent(*span));
            continue;
        };
        let e = match &p.ty {
            Some(ty) if ctx.typed_props && !is_action(ty) => {
                Expr::Typed(Box::new(e.clone()), ty.clone(), e.span())
            }
            _ => e.clone(),
        };
        child_subst.insert(p.name.clone(), e);
    }
    // The child's own `state`, `derive`, and `action` (LLP 1017 P4c):
    // renamed apart with this use's number and lifted into the
    // root — a derive as an expression substituted at each read.
    *ctx.counter += 1;
    let n = *ctx.counter;
    let owner = ctx.arms.last().copied().unwrap_or(Owner::Instance);
    let instance = if ctx.capture_sites {
        let instance = ctx.instances.len() as u32;
        ctx.instances.push(Instance {
            component: name.clone(),
            parent: Some(ctx.instance),
            span: *span,
        });
        instance
    } else {
        0
    };
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for st in &c.states {
        names.insert(st.name.clone(), lifted(&st.name, n));
    }
    for a in &c.actions {
        names.insert(a.name.clone(), lifted(&a.name, n));
    }
    for st in &c.states {
        child_subst.insert(st.name.clone(), Expr::Ident(names[&st.name].clone(), *span));
    }
    // Closure-convert child props/injects into hidden action
    // parameters. A handler evaluates these curried arguments
    // at its exact node site, so intervening `when`/`match`
    // frames and arbitrarily nested rows cannot change what the
    // lifted action reads.
    let captures: Vec<(Param, Expr, String)> = c
        .props
        .iter()
        .chain(&c.injects)
        .filter(|prop| {
            !matches!(
                prop.ty.as_ref(),
                Some(TypeExpr::Named(name, _)) if name == "action"
            )
        })
        .enumerate()
        .map(|(i, prop)| {
            let hidden = format!("@capture:{n}:{i}");
            (
                Param {
                    name: hidden.clone(),
                    ty: prop.ty.clone(),
                    span: prop.span,
                },
                child_subst[&prop.name].clone(),
                prop.name.clone(),
            )
        })
        .collect();
    // An action prop or injected action an action calls (LLP
    // 1089 D7): the action it names and the arguments it was
    // curried with, captured as the props above are (they are
    // the parent's), and where it was bound.
    let mut captures = captures;
    let mut called: BTreeMap<String, (String, Vec<Expr>, crate::Span)> = BTreeMap::new();
    for (pi, prop) in c.props.iter().chain(&c.injects).enumerate() {
        let is_action =
            matches!(prop.ty.as_ref(), Some(TypeExpr::Named(name, _)) if name == "action");
        let used = is_action
            && !crate::HOST_COMMANDS.contains(&prop.name.as_str())
            && c.actions.iter().any(|a| commands(&a.body, &prop.name));
        let Some(value) = child_subst.get(&prop.name).filter(|_| used) else {
            continue;
        };
        let (target, curried) = match value {
            // A prop the use lacks, already refused.
            Expr::Ident(target, _) if target == "?" => continue,
            Expr::Ident(target, _) => (target.clone(), Vec::new()),
            Expr::Call(target, curried, _) => (target.clone(), curried.clone()),
            other => {
                ctx.refuse(
                                "syntax-call-target",
                                format!("`{}` is called by an action, so it must name an action, as `{}=act` or `{}=act(args)` does", prop.name, prop.name, prop.name),
                                other.span(),
                            );
                continue;
            }
        };
        let binding = args
            .iter()
            .find(|a| a.name == prop.name)
            .map_or(value.span(), |a| a.span);
        let mut held = Vec::new();
        for (j, arg) in curried.into_iter().enumerate() {
            let hidden = format!("@capture:{n}:a{pi}:{j}");
            held.push(Expr::Ident(hidden.clone(), prop.span));
            captures.push((
                Param {
                    name: hidden,
                    ty: None,
                    span: prop.span,
                },
                arg,
                String::new(),
            ));
        }
        called.insert(prop.name.clone(), (target, held, binding));
    }
    for action in &c.actions {
        child_subst.insert(
            action.name.clone(),
            Expr::Call(
                names[&action.name].clone(),
                captures.iter().map(|(_, value, _)| value.clone()).collect(),
                *span,
            ),
        );
    }
    // Resolve derives in the child's own scope before substituting
    // parent expressions for props. That distinction is what
    // keeps a parent `a` passed through a prop from being mistaken
    // for the child's derive `a`.
    // A resolved derive reads no other derive by name, so every
    // one is substituted against the same props, states and actions.
    let records = ctx.records;
    let derives = resolved_derives(c, records).unwrap_or_else(|e| {
        ctx.errors.push(e);
        c.derives.iter().map(|d| (d, absent(d.span))).collect()
    });
    let resolved: Vec<Expr> = {
        let mut base = Subst::new(&child_subst, records);
        derives
            .iter()
            .map(|(_, expr)| subst_expr(expr, &mut base))
            .collect()
    };
    for ((derive, _), expr) in derives.iter().zip(resolved) {
        child_subst.insert(derive.name.clone(), expr);
    }
    let mut child = Subst::new(&child_subst, records);
    for st in &c.states {
        ctx.extra_states.push((
            Binding {
                name: names[&st.name].clone(),
                expr: subst_expr(&st.expr, &mut child),
                span: st.span,
            },
            owner,
            instance,
        ));
    }
    for a in &c.actions {
        let mut action_subst = BTreeMap::new();
        for (param, _, source_name) in captures.iter().filter(|(_, _, n)| !n.is_empty()) {
            action_subst.insert(
                source_name.clone(),
                Expr::Ident(param.name.clone(), param.span),
            );
        }
        for st in &c.states {
            action_subst.insert(
                st.name.clone(),
                Expr::Ident(names[&st.name].clone(), st.span),
            );
        }
        let resolved: Vec<Expr> = {
            let mut base = Subst::new(&action_subst, records);
            derives
                .iter()
                .map(|(_, expr)| subst_expr(expr, &mut base))
                .collect()
        };
        for ((derive, _), expr) in derives.iter().zip(resolved) {
            action_subst.insert(derive.name.clone(), expr);
        }
        // A callee of this component reads these names as its
        // caller does: its body is substituted with all of them.
        let component_names = action_subst.clone();
        // An action's declared parameters are still the
        // innermost binders and shadow same-named captures.
        for param in &a.params {
            action_subst.remove(&param.name);
        }
        let body = subst_stmts(
            &a.body,
            &mut Subst::new(&action_subst, records).into_calls(&component_names),
            &names,
        );
        let held: Vec<(String, crate::Span)> = captures
            .iter()
            .map(|(param, _, _)| (param.name.clone(), param.span))
            .collect();
        ctx.extra_actions.push((
            Action {
                name: names[&a.name].clone(),
                params: captures
                    .iter()
                    .map(|(param, _, _)| param.clone())
                    .chain(a.params.iter().cloned())
                    .collect(),
                body: call_marked(with_captures(body, &held), &called),
                span: a.span,
            },
            instance,
        ));
    }
    // Release per-use resolved expressions before expanding nested children.
    drop(derives);
    if !children.is_empty() && !c.slot {
        ctx.refuse(
            "syntax-no-slot",
            format!("`{name}` declares no `slot`, so nothing can be indented under it"),
            children[0].span(),
        );
    }
    // The fill is the use site's: inlined in this scope, at each
    // `children` node of the child's view that renders it.
    let fill = (c.slot && !ctx.discarded_slots.contains(name)).then(|| {
        Rc::new(Fill {
            nodes: children.clone(),
            subst: subst.map().clone(),
            provides: ctx.provides.clone(),
            outer: ctx.fill.clone(),
            instance: ctx.instance,
        })
    });
    // Rename only the view: declarations were lifted above.
    let renamed = rename_nodes(&c.view, &BTreeMap::new(), n, ctx.discarded_slots);
    let outer_fill = std::mem::replace(&mut ctx.fill, fill);
    let outer_instance = std::mem::replace(&mut ctx.instance, instance);
    // The child's `provide` section covers its whole view, after
    // the fill took the use site's (LLP 1035.005.000 D9).
    let outer_provides = ctx.provides.len();
    for b in &c.provides {
        ctx.provides
            .push((b.name.clone(), subst_expr(&b.expr, &mut child)));
    }
    ctx.depth += 1;
    let body = inline_nodes(&renamed, &mut child, ctx);
    ctx.depth -= 1;
    ctx.provides.truncate(outer_provides);
    ctx.fill = outer_fill;
    ctx.instance = outer_instance;
    body
}

// The least fixed point matters for cycles: a slot reached again through a
// recursive component only renders a fill if another path reaches `children`.
// Each component is scanned once per newly proven renderer, so empty fills
// cannot cause an unbounded recursive search during the depth preflight.
pub(super) fn discarded_slots(file: &crate::File) -> BTreeSet<String> {
    let mut rendered = BTreeSet::new();
    loop {
        let mut changed = false;
        for component in &file.components {
            if component.slot
                && !rendered.contains(&component.name)
                && renders_fill(&component.view, &rendered)
            {
                rendered.insert(component.name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    file.components
        .iter()
        // A slotless component renders no fill either: its children are an
        // error (`syntax-no-slot`), refused before they are copied.
        .filter(|component| !component.slot || !rendered.contains(&component.name))
        .map(|component| component.name.clone())
        .collect()
}

fn renders_fill(nodes: &[Node], rendered: &BTreeSet<String>) -> bool {
    let mut pending: Vec<_> = nodes.iter().collect();
    while let Some(node) = pending.pop() {
        match node {
            Node::Children { .. } => return true,
            Node::Element { children, .. } => pending.extend(children),
            Node::When {
                then, otherwise, ..
            } => pending.extend(then.iter().chain(otherwise)),
            Node::Each { body, .. } => pending.extend(body),
            Node::Match { some, none, .. } => pending.extend(some.1.iter().chain(none)),
            Node::Use { name, children, .. } => {
                if !children.is_empty() && rendered.contains(name) {
                    pending.extend(children);
                }
            }
        }
    }
    false
}

fn exceeds_site_budget(
    nodes: &[Node],
    file: &crate::File,
    discarded_slots: &BTreeSet<String>,
    parent_depth: usize,
) -> bool {
    let mut pending: Vec<_> = nodes.iter().map(|node| (node, parent_depth)).collect();
    while let Some((node, parent_depth)) = pending.pop() {
        let depth =
            parent_depth + usize::from(!matches!(node, Node::Use { .. } | Node::Children { .. }));
        if depth > 256 {
            return true;
        }
        match node {
            Node::Element { children, .. } => {
                pending.extend(children.iter().map(|child| (child, depth)));
            }
            Node::When {
                then, otherwise, ..
            } => {
                pending.extend(then.iter().chain(otherwise).map(|child| (child, depth)));
            }
            Node::Each { body, .. } => {
                pending.extend(body.iter().map(|child| (child, depth)));
            }
            Node::Match { some, none, .. } => {
                pending.extend(some.1.iter().chain(none).map(|child| (child, depth)));
            }
            // A discarded slot fill is neither renamed nor emitted. Count a
            // fill only when the callee renders it, including forwarded slots.
            Node::Use { name, children, .. } => {
                if let Some(component) = file.components.iter().find(|c| &c.name == name) {
                    if component.slot && !discarded_slots.contains(name) {
                        pending.extend(children.iter().map(|child| (child, depth)));
                    }
                }
            }
            Node::Children { .. } => {}
        }
    }
    false
}
