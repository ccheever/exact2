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

use crate::ast::{
    Action, Attr, Binding, Component, Expr, File, Node, Param, Stmt, TemplatePart, TypeExpr,
};
use crate::parser::SyntaxError;
use std::collections::{BTreeMap, BTreeSet};

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
    /// (0 for the root's own; a lifted `name__N` names its child's).
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
    let mut root = file.components[0].clone();
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
    let view = inline_nodes(&file.components[0].view, &BTreeMap::new(), &mut ctx)?;
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
    subst: &BTreeMap<String, Expr>,
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
                    names.insert(st.name.clone(), format!("{}__{n}", st.name));
                }
                for a in &c.actions {
                    names.insert(a.name.clone(), format!("{}__{n}", a.name));
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
                for (derive, expr) in resolved_derives(c)? {
                    child_subst.insert(derive.name.clone(), subst_expr(&expr, &child_subst));
                }
                for st in &c.states {
                    ctx.extra_states.push((
                        Binding {
                            name: names[&st.name].clone(),
                            expr: subst_expr(&st.expr, &child_subst),
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
                    for (derive, expr) in resolved_derives(c)? {
                        action_subst.insert(derive.name.clone(), subst_expr(&expr, &action_subst));
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
                            body: subst_stmts(&a.body, &action_subst, &names),
                            span: a.span,
                        },
                        instance,
                    ));
                }
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
                let renamed = rename_component(c, n);
                let outer_fill = std::mem::replace(&mut ctx.fill, fill);
                let outer_instance = std::mem::replace(&mut ctx.instance, instance);
                ctx.depth += 1;
                let body = inline_nodes(&renamed.view, &child_subst, ctx);
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

/// Expand a component's derives through one another in dependency order.
/// Type inference admits either declaration order, so inlining must too; a
/// replacement is complete before it enters the substitution map.
fn resolved_derives(c: &Component) -> Result<Vec<(&Binding, Expr)>, SyntaxError> {
    let mut indices = BTreeMap::new();
    for (i, derive) in c.derives.iter().enumerate() {
        if indices.insert(derive.name.as_str(), i).is_some() {
            return err(
                "type-duplicate-name",
                format!("`{}` declared twice", derive.name),
                derive.span,
            );
        }
    }
    let mut resolved = vec![None; c.derives.len()];
    let mut visiting = BTreeSet::new();
    for i in 0..c.derives.len() {
        resolve_derive(i, c, &indices, &mut resolved, &mut visiting)?;
    }
    Ok(c.derives
        .iter()
        .zip(resolved.into_iter().map(Option::unwrap))
        .collect())
}

fn resolve_derive(
    i: usize,
    c: &Component,
    indices: &BTreeMap<&str, usize>,
    resolved: &mut [Option<Expr>],
    visiting: &mut BTreeSet<usize>,
) -> Result<Expr, SyntaxError> {
    if let Some(expr) = &resolved[i] {
        return Ok(expr.clone());
    }
    if !visiting.insert(i) {
        return err(
            "type-derive-cycle",
            format!(
                "cannot resolve `{}`: it depends on itself through other derives",
                c.derives[i].name
            ),
            c.derives[i].span,
        );
    }
    let mut dependencies = BTreeSet::new();
    derive_dependencies(
        &c.derives[i].expr,
        indices,
        &BTreeSet::new(),
        &mut dependencies,
    );
    let mut substitutions = BTreeMap::new();
    for dependency in dependencies {
        let expr = resolve_derive(dependency, c, indices, resolved, visiting)?;
        substitutions.insert(c.derives[dependency].name.clone(), expr);
    }
    let expr = subst_expr(&c.derives[i].expr, &substitutions);
    visiting.remove(&i);
    resolved[i] = Some(expr.clone());
    Ok(expr)
}

fn derive_dependencies(
    expr: &Expr,
    indices: &BTreeMap<&str, usize>,
    bound: &BTreeSet<String>,
    out: &mut BTreeSet<usize>,
) {
    match expr {
        Expr::Ident(name, _) => {
            if !bound.contains(name) {
                if let Some(i) = indices.get(name.as_str()) {
                    out.insert(*i);
                }
            }
        }
        Expr::Call(name, args, _) => {
            if !bound.contains(name) {
                if let Some(i) = indices.get(name.as_str()) {
                    out.insert(*i);
                }
            }
            for arg in args {
                derive_dependencies(arg, indices, bound, out);
            }
        }
        Expr::Member(object, _, _) | Expr::Some(object, _) | Expr::Unary(_, object, _) => {
            derive_dependencies(object, indices, bound, out);
        }
        Expr::Binary(_, left, right, _) => {
            derive_dependencies(left, indices, bound, out);
            derive_dependencies(right, indices, bound, out);
        }
        Expr::Ternary(cond, then, otherwise, _) => {
            derive_dependencies(cond, indices, bound, out);
            derive_dependencies(then, indices, bound, out);
            derive_dependencies(otherwise, indices, bound, out);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            derive_dependencies(subject, indices, bound, out);
            let mut inner = bound.clone();
            inner.insert(var.clone());
            derive_dependencies(some, indices, &inner, out);
            derive_dependencies(none, indices, bound, out);
        }
        Expr::Template(parts, _) => {
            for part in parts {
                if let TemplatePart::Expr(expr) = part {
                    derive_dependencies(expr, indices, bound, out);
                }
            }
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
    }
}

/// Substitute prop names by argument expressions. A curried handler
/// `prop(args)` where the prop's argument is an action `f` or `f(a…)`
/// becomes `f(a…, args)`.
fn subst_expr(e: &Expr, subst: &BTreeMap<String, Expr>) -> Expr {
    match e {
        Expr::Ident(n, span) => match subst.get(n) {
            // Renaming a child state does not move its reference to the use
            // site. Keep the expression's source span for diagnostics.
            Some(Expr::Ident(name, _)) => Expr::Ident(name.clone(), *span),
            Some(r) => r.clone(),
            None => e.clone(),
        },
        Expr::Call(n, args, span) => {
            let args: Vec<Expr> = args.iter().map(|a| subst_expr(a, subst)).collect();
            match subst.get(n) {
                Some(Expr::Ident(f, _)) => Expr::Call(f.clone(), args, *span),
                Some(Expr::Call(f, first, _)) => {
                    let mut all = first.clone();
                    all.extend(args);
                    Expr::Call(f.clone(), all, *span)
                }
                _ => Expr::Call(n.clone(), args, *span),
            }
        }
        Expr::Member(o, f, span) => Expr::Member(Box::new(subst_expr(o, subst)), f.clone(), *span),
        Expr::Some(x, span) => Expr::Some(Box::new(subst_expr(x, subst)), *span),
        Expr::Unary(op, x, span) => Expr::Unary(*op, Box::new(subst_expr(x, subst)), *span),
        Expr::Binary(op, a, b, span) => Expr::Binary(
            *op,
            Box::new(subst_expr(a, subst)),
            Box::new(subst_expr(b, subst)),
            *span,
        ),
        Expr::Ternary(a, b, c, span) => Expr::Ternary(
            Box::new(subst_expr(a, subst)),
            Box::new(subst_expr(b, subst)),
            Box::new(subst_expr(c, subst)),
            *span,
        ),
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            let mut inner = subst.clone();
            inner.remove(var);
            Expr::Match {
                subject: Box::new(subst_expr(subject, subst)),
                var: var.clone(),
                some: Box::new(subst_expr(some, &inner)),
                none: Box::new(subst_expr(none, subst)),
                span: *span,
            }
        }
        Expr::Template(parts, span) => Expr::Template(
            parts
                .iter()
                .map(|p| match p {
                    TemplatePart::Text(t) => TemplatePart::Text(t.clone()),
                    TemplatePart::Expr(x) => TemplatePart::Expr(subst_expr(x, subst)),
                })
                .collect(),
            *span,
        ),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => e.clone(),
    }
}

/// Rename every name a component's view binds (`each` vars, `match` vars)
/// with a unique suffix, so inlined bodies never capture parent names.
fn rename_component(c: &Component, n: u32) -> Component {
    let mut renamed = c.clone();
    renamed.view = rename_nodes(&c.view, &BTreeMap::new(), n);
    renamed
}

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
                positional: positional.iter().map(|e| rename_expr(e, map)).collect(),
                attrs: attrs
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: rename_expr(&a.value, map),
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
                        value: rename_expr(&a.value, map),
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
                expr: rename_expr(expr, map),
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
                cond: rename_expr(cond, map),
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
                let fresh = format!("{var}__{n}");
                inner.insert(var.clone(), fresh.clone());
                Node::Each {
                    tag: *tag,
                    var: fresh,
                    list: rename_expr(list, map),
                    key: rename_expr(key, &inner),
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
                let fresh = format!("{}__{n}", some.0);
                inner.insert(some.0.clone(), fresh.clone());
                Node::Match {
                    subject: rename_expr(subject, map),
                    some: (fresh, rename_nodes(&some.1, &inner, n)),
                    none: rename_nodes(none, map, n),
                    span: *span,
                }
            }
        })
        .collect()
}

fn rename_expr(e: &Expr, map: &BTreeMap<String, String>) -> Expr {
    let subst: BTreeMap<String, Expr> = map
        .iter()
        .map(|(k, v)| (k.clone(), Expr::Ident(v.clone(), e.span())))
        .collect();
    subst_expr(e, &subst)
}

/// A child action's body, its names substituted: assignment targets renamed
/// with `names`, expressions through `subst` (props, injects, renamed states
/// and actions, derives as expressions).
fn subst_stmts(
    stmts: &[Stmt],
    subst: &BTreeMap<String, Expr>,
    names: &BTreeMap<String, String>,
) -> Vec<Stmt> {
    stmts
        .iter()
        .map(|st| match st {
            Stmt::Assign { target, expr, span } => Stmt::Assign {
                target: names.get(target).cloned().unwrap_or_else(|| target.clone()),
                expr: subst_expr(expr, subst),
                span: *span,
            },
            Stmt::Command { name, args, span } => Stmt::Command {
                name: name.clone(),
                args: args.iter().map(|a| subst_expr(a, subst)).collect(),
                span: *span,
            },
            Stmt::Send {
                target,
                source,
                args,
                span,
            } => Stmt::Send {
                target: target.clone(),
                source: source.clone(),
                args: args.iter().map(|a| subst_expr(a, subst)).collect(),
                span: *span,
            },
            Stmt::Refresh { target, span } => Stmt::Refresh {
                target: target.clone(),
                span: *span,
            },
            Stmt::If {
                cond,
                then,
                otherwise,
                span,
            } => Stmt::If {
                cond: subst_expr(cond, subst),
                then: subst_stmts(then, subst, names),
                otherwise: subst_stmts(otherwise, subst, names),
                span: *span,
            },
            Stmt::Match {
                subject,
                some,
                none,
                span,
            } => {
                let mut inner = subst.clone();
                inner.remove(&some.0);
                Stmt::Match {
                    subject: subst_expr(subject, subst),
                    some: (some.0.clone(), subst_stmts(&some.1, &inner, names)),
                    none: subst_stmts(none, subst, names),
                    span: *span,
                }
            }
        })
        .collect()
}
