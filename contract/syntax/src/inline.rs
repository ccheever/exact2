//! Component inlining: uses become the used component's view, props become
//! the use's argument expressions, and names the child binds are renamed
//! apart so an argument expression from the parent can never be captured.
//!
//! Purely syntactic, so both type inference (which needs to see a handler's
//! real call site through a prop) and lowering run on the same expansion.
//!
//! LLP 1017 P4a/P4b live here too: an `inject` is filled from the nearest
//! enclosing `provide` on the way down (the compiler's context — no runtime
//! lookup, a missing provider a refusal), and a `slot` component's
//! `children` node is replaced by the nodes indented under its use, inlined
//! in the *use site's* scope.

use crate::ast::{Action, Attr, Binding, Component, Expr, File, Node, Param, TypeExpr};
use crate::parser::SyntaxError;
use std::collections::BTreeMap;

mod derives;
mod subst;
#[cfg(test)]
mod tests;

use derives::resolved_derives;
use subst::{subst_expr, subst_stmts, substituted, Subst};

fn err<T>(
    id: &'static str,
    message: impl Into<String>,
    span: crate::Span,
) -> Result<T, SyntaxError> {
    Err(SyntaxError {
        id,
        message: message.into(),
        span,
    })
}

/// A name use `n` gives a child's declaration or view binder. `#` cannot
/// appear in an authored identifier, so no author's name can collide with it.
pub fn lifted(name: &str, n: u32) -> String {
    format!("{name}#{n}")
}

/// The root's view with every component use inlined.
pub fn inline(file: &File) -> Result<Vec<Node>, SyntaxError> {
    Ok(expand(file)?.root.view)
}

/// The root as the plan sees it (LLP 1017 P4c): its view inlined, plus the
/// `state`s and `action`s of every stateful child use, renamed apart with
/// the use's number — a root slot for a use outside any `each`, a row slot
/// (owned by the innermost enclosing `each`, named by its tag) inside one;
/// a child's `derive` is an expression substituted at each read.
pub struct Expanded {
    /// The root, with the children's declarations appended.
    pub root: Component,
    /// For each of `root.states`, the tag of the `each` that owns it, or
    /// `None` for a root slot.
    pub owners: Vec<Option<u32>>,
    /// Every component instantiation, in the order the inliner expanded
    /// them; entry 0 is the root (LLP 1035.005 D3). Empty unless source
    /// provenance was requested with `expand_mapped`. An element's
    /// `instance` and the two vectors below index it.
    pub instances: Vec<Instance>,
    /// For each of `root.states`, the instance whose component declared it
    /// (0 for the root's own; a lifted `name#N` names its child's).
    pub state_instances: Vec<u32>,
    /// For each of `root.actions`, the same.
    pub action_instances: Vec<u32>,
}

/// One component instantiation the inliner expanded (LLP 1035.005 D3):
/// which component, which instantiation's view holds the use, and where the
/// use is written there. The development map walks `parent` to render a
/// node's chain (`Bubble ← Messages app.contract:459`); refusal diagnostics
/// also trace supplied actions through it. None of it reaches the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// The component instantiated.
    pub component: String,
    /// The instance whose view holds the use; `None` for the root.
    pub parent: Option<u32>,
    /// The use site in the parent's view; the root's own span for the root.
    pub span: crate::Span,
}

/// Expand the file's root: inline every use and lift every child's own
/// declarations into it.
pub fn expand(file: &File) -> Result<Expanded, SyntaxError> {
    expand_with_sites(file, false)
}

/// Expand with development source provenance for the mapped compiler entry.
pub fn expand_mapped(file: &File) -> Result<Expanded, SyntaxError> {
    expand_with_sites(file, true)
}

fn expand_with_sites(file: &File, capture_sites: bool) -> Result<Expanded, SyntaxError> {
    let source = &file.components[0];
    // Expansion replaces the view; retain only the root declarations here.
    let mut root = Component {
        name: source.name.clone(),
        props: source.props.clone(),
        injects: source.injects.clone(),
        slot: source.slot,
        states: source.states.clone(),
        derives: source.derives.clone(),
        resources: source.resources.clone(),
        mutations: source.mutations.clone(),
        actions: source.actions.clone(),
        tasks: source.tasks.clone(),
        view: Vec::new(),
        span: source.span,
    };
    // @ref LLP 1038 D3 — a compiler slot, before authored initializers and
    // before the per-use states are lifted. `none` is only an AST placeholder;
    // types supplies Router and lowering leaves its initialization to launch.
    if let Some(routes) = &file.routes {
        root.states.insert(
            0,
            Binding {
                name: routes.slot.clone(),
                expr: Expr::None(routes.span),
                span: routes.span,
            },
        );
    }
    let mut counter = 0u32;
    let mut ctx = Ctx {
        file,
        counter: &mut counter,
        depth: 0,
        provides: Vec::new(),
        fill: None,
        each_stack: Vec::new(),
        next_tag: 1,
        extra_states: Vec::new(),
        extra_actions: Vec::new(),
        instance: 0,
        capture_sites,
        instances: if capture_sites {
            vec![Instance {
                component: file.components[0].name.clone(),
                parent: None,
                span: file.components[0].span,
            }]
        } else {
            Vec::new()
        },
    };
    let none = BTreeMap::new();
    let view = inline_nodes(&file.components[0].view, &mut Subst::new(&none), &mut ctx)?;
    root.view = view;
    let mut owners = vec![None; root.states.len()];
    let mut state_instances = if capture_sites {
        vec![0; root.states.len()]
    } else {
        Vec::new()
    };
    for (b, owner, instance) in ctx.extra_states {
        root.states.push(b);
        owners.push(owner);
        if capture_sites {
            state_instances.push(instance);
        }
    }
    let mut action_instances = if capture_sites {
        vec![0; root.actions.len()]
    } else {
        Vec::new()
    };
    for (a, instance) in ctx.extra_actions {
        root.actions.push(a);
        if capture_sites {
            action_instances.push(instance);
        }
    }
    Ok(Expanded {
        root,
        owners,
        instances: ctx.instances,
        state_instances,
        action_instances,
    })
}

/// What inlining carries down the tree besides the substitution.
struct Ctx<'a> {
    file: &'a File,
    counter: &'a mut u32,
    depth: u32,
    /// `provide`s in force, outermost first, each already substituted into
    /// the scope it was written in.
    provides: Vec<(String, Expr)>,
    /// The nodes that fill `children` here: `Some` inside a `slot`
    /// component's view (possibly empty), `None` elsewhere.
    fill: Option<Vec<Node>>,
    /// The tags of the `each`es enclosing the current site, outermost first.
    each_stack: Vec<u32>,
    /// The next `each` tag.
    next_tag: u32,
    /// The children's `state`s lifted into the root, with their owners and
    /// the instance that declared them.
    extra_states: Vec<(Binding, Option<u32>, u32)>,
    /// The children's `action`s lifted into the root, with their instance.
    extra_actions: Vec<(Action, u32)>,
    /// The instantiation whose view is being inlined: 0 at the root.
    instance: u32,
    capture_sites: bool,
    /// Every instantiation so far, the root first (LLP 1035.005 D3).
    instances: Vec<Instance>,
}

fn inline_nodes(
    nodes: &[Node],
    subst: &mut Subst<'_, Expr>,
    ctx: &mut Ctx<'_>,
) -> Result<Vec<Node>, SyntaxError> {
    let mut out = Vec::with_capacity(nodes.len());
    for n in nodes {
        match n {
            Node::Use {
                name,
                args,
                children,
                span,
            } => {
                if ctx.depth > 32 {
                    return err(
                        "syntax-inline-depth",
                        format!("component `{name}` nests too deeply (a cycle?)"),
                        *span,
                    );
                }
                let Some(c) = ctx.file.components.iter().find(|c| &c.name == name) else {
                    return err(
                        "syntax-unknown-component",
                        ctx.file.unknown_component_message(name),
                        *span,
                    );
                };
                let mut child_subst: BTreeMap<String, Expr> = BTreeMap::new();
                for p in &c.props {
                    let Some(a) = args.iter().find(|a| a.name == p.name) else {
                        return err("syntax-missing-prop", c.missing_props_message(args), *span);
                    };
                    // The argument is an expression in the parent's scope: substitute the parent's own substitutions first.
                    child_subst.insert(p.name.clone(), subst_expr(&a.value, subst));
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
                        let scopes = missing
                            .iter()
                            .map(|p| format!("`provide {} = …`", p.name))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let message = if missing.len() == 1 {
                            format!("`{name}` injects {names}, and nothing above this use provides it: wrap the use in {scopes}")
                        } else {
                            format!("`{name}` injects {names}, and nothing above this use provides them: wrap the use in nested {scopes} scopes")
                        };
                        return err("syntax-missing-provide", message, *span);
                    };
                    child_subst.insert(p.name.clone(), e.clone());
                }
                // The child's own `state`, `derive`, and `action` (LLP 1017 P4c):
                // renamed apart with this use's number and lifted into the
                // root — a derive as an expression substituted at each read.
                *ctx.counter += 1;
                let n = *ctx.counter;
                let owner = ctx.each_stack.last().copied();
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
                    child_subst
                        .insert(st.name.clone(), Expr::Ident(names[&st.name].clone(), *span));
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
                let derives = resolved_derives(c)?;
                let resolved: Vec<Expr> = {
                    let mut base = Subst::new(&child_subst);
                    derives
                        .iter()
                        .map(|(_, expr)| subst_expr(expr, &mut base))
                        .collect()
                };
                for ((derive, _), expr) in derives.iter().zip(resolved) {
                    child_subst.insert(derive.name.clone(), expr);
                }
                let mut child = Subst::new(&child_subst);
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
                    for (param, _, source_name) in &captures {
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
                        let mut base = Subst::new(&action_subst);
                        derives
                            .iter()
                            .map(|(_, expr)| subst_expr(expr, &mut base))
                            .collect()
                    };
                    for ((derive, _), expr) in derives.iter().zip(resolved) {
                        action_subst.insert(derive.name.clone(), expr);
                    }
                    // An action's declared parameters are still the
                    // innermost binders and shadow same-named captures.
                    for param in &a.params {
                        action_subst.remove(&param.name);
                    }
                    ctx.extra_actions.push((
                        Action {
                            name: names[&a.name].clone(),
                            params: captures
                                .iter()
                                .map(|(param, _, _)| param.clone())
                                .chain(a.params.iter().cloned())
                                .collect(),
                            writes: a
                                .writes
                                .iter()
                                .map(|(w, sp)| {
                                    (names.get(w).cloned().unwrap_or_else(|| w.clone()), *sp)
                                })
                                .collect(),
                            body: subst_stmts(&a.body, &mut Subst::new(&action_subst), &names),
                            span: a.span,
                        },
                        instance,
                    ));
                }
                // Release per-use resolved expressions before expanding nested children.
                drop(derives);
                if !children.is_empty() && !c.slot {
                    return err(
                        "syntax-no-slot",
                        format!("`{name}` declares no `slot`, so nothing can be indented under it"),
                        children[0].span(),
                    );
                }
                // The fill is the use site's: inlined here, in this scope.
                let fill = if c.slot {
                    Some(inline_nodes(children, subst, ctx)?)
                } else {
                    None
                };
                // Rename only the view: declarations were lifted above.
                let renamed = rename_nodes(&c.view, &BTreeMap::new(), n);
                let outer_fill = std::mem::replace(&mut ctx.fill, fill);
                let outer_instance = std::mem::replace(&mut ctx.instance, instance);
                ctx.depth += 1;
                let body = inline_nodes(&renamed, &mut child, ctx);
                ctx.depth -= 1;
                ctx.fill = outer_fill;
                ctx.instance = outer_instance;
                out.extend(body?);
            }
            Node::Provide {
                name, expr, body, ..
            } => {
                ctx.provides.push((name.clone(), subst_expr(expr, subst)));
                let inner = inline_nodes(body, subst, ctx);
                ctx.provides.pop();
                out.extend(inner?);
            }
            Node::Children { span } => match &ctx.fill {
                Some(fill) => out.extend(fill.iter().cloned()),
                None => {
                    return err(
                        "syntax-children-without-slot",
                        "`children` belongs in a component that declares `slot`",
                        *span,
                    )
                }
            },
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
                ..
            } => out.push(Node::Element {
                tag: tag.clone(),
                positional: positional.iter().map(|e| subst_expr(e, subst)).collect(),
                attrs: attrs
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: subst_expr(&a.value, subst),
                        span: a.span,
                    })
                    .collect(),
                children: inline_nodes(children, subst, ctx)?,
                span: *span,
                instance: ctx.instance,
            }),
            Node::When {
                cond,
                then,
                otherwise,
                span,
            } => out.push(Node::When {
                cond: subst_expr(cond, subst),
                then: inline_nodes(then, subst, ctx)?,
                otherwise: inline_nodes(otherwise, subst, ctx)?,
                span: *span,
            }),
            Node::Each {
                var,
                list,
                key,
                body,
                span,
                ..
            } => {
                let tag = ctx.next_tag;
                ctx.next_tag += 1;
                ctx.each_stack.push(tag);
                let body = inline_nodes(body, subst, ctx);
                ctx.each_stack.pop();
                out.push(Node::Each {
                    tag,
                    var: var.clone(),
                    list: subst_expr(list, subst),
                    key: subst_expr(key, subst),
                    body: body?,
                    span: *span,
                })
            }
            Node::Match {
                subject,
                some,
                none,
                span,
            } => out.push(Node::Match {
                subject: subst_expr(subject, subst),
                some: (some.0.clone(), inline_nodes(&some.1, subst, ctx)?),
                none: inline_nodes(none, subst, ctx)?,
                span: *span,
            }),
        }
    }
    Ok(out)
}

// Rename every name the view binds with a unique suffix so inlined bodies
// cannot capture parent names.
// Borrow renamed strings directly; subst_expr retains each reference's span.
fn rename_nodes(nodes: &[Node], map: &BTreeMap<String, String>, n: u32) -> Vec<Node> {
    nodes
        .iter()
        .map(|node| match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
                instance,
            } => Node::Element {
                tag: tag.clone(),
                positional: positional.iter().map(|e| substituted(e, map)).collect(),
                attrs: attrs
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: substituted(&a.value, map),
                        span: a.span,
                    })
                    .collect(),
                children: rename_nodes(children, map, n),
                span: *span,
                instance: *instance,
            },
            Node::Use {
                name,
                args,
                children,
                span,
            } => Node::Use {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: substituted(&a.value, map),
                        span: a.span,
                    })
                    .collect(),
                children: rename_nodes(children, map, n),
                span: *span,
            },
            Node::Provide {
                name,
                expr,
                body,
                span,
            } => Node::Provide {
                name: name.clone(),
                expr: substituted(expr, map),
                body: rename_nodes(body, map, n),
                span: *span,
            },
            Node::Children { span } => Node::Children { span: *span },
            Node::When {
                cond,
                then,
                otherwise,
                span,
            } => Node::When {
                cond: substituted(cond, map),
                then: rename_nodes(then, map, n),
                otherwise: rename_nodes(otherwise, map, n),
                span: *span,
            },
            Node::Each {
                tag,
                var,
                list,
                key,
                body,
                span,
            } => {
                let mut inner = map.clone();
                let fresh = lifted(var, n);
                inner.insert(var.clone(), fresh.clone());
                Node::Each {
                    tag: *tag,
                    var: fresh,
                    list: substituted(list, map),
                    key: substituted(key, &inner),
                    body: rename_nodes(body, &inner, n),
                    span: *span,
                }
            }
            Node::Match {
                subject,
                some,
                none,
                span,
            } => {
                let mut inner = map.clone();
                let fresh = lifted(&some.0, n);
                inner.insert(some.0.clone(), fresh.clone());
                Node::Match {
                    subject: substituted(subject, map),
                    some: (fresh, rename_nodes(&some.1, &inner, n)),
                    none: rename_nodes(none, map, n),
                    span: *span,
                }
            }
        })
        .collect()
}
