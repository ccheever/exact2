//! Component inlining: uses become the used component's view, props become
//! the use's argument expressions, and names the child binds are renamed
//! apart so an argument expression from the parent can never be captured.
//!
//! Purely syntactic, so both type inference (which needs to see a handler's
//! real call site through a prop) and lowering run on the same expansion.
//!
//! LLP 1017 P4a/P4b live here too: an `inject` is filled from the nearest
//! enclosing component's `provide` section on the way down (the compiler's
//! context — no runtime lookup, a missing provider a refusal; LLP
//! 1035.005.000 D9), and a `slot` component's
//! `children` node is replaced by the nodes indented under its use, inlined
//! in the *use site's* scope.

use crate::ast::{Action, Attr, Binding, Component, Expr, File, Node, Param, TypeExpr};
use crate::parser::SyntaxError;
use std::collections::{BTreeMap, BTreeSet};

mod derives;
mod subst;
pub mod tail;
#[cfg(test)]
mod tests;

use derives::resolved_derives;
use subst::{renamed_locals, subst_expr, subst_stmts, substituted, Subst};

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
    first(expand_all(file, false))
}

/// Expand with development source provenance for the mapped compiler entry.
pub fn expand_mapped(file: &File) -> Result<Expanded, SyntaxError> {
    first(expand_all(file, true))
}

fn first((expanded, mut errors): (Expanded, Vec<SyntaxError>)) -> Result<Expanded, SyntaxError> {
    if errors.is_empty() {
        Ok(expanded)
    } else {
        Err(errors.swap_remove(0))
    }
}

/// Expand, recording every use that cannot be expanded and going on: a use
/// of an unknown component (or one nested too deeply) is left out, and a
/// prop, provider, or derive it lacks reads as `?`, so what depends on it is
/// a consequence the checker does not repeat. The expansion is complete only
/// when no error is returned; `mapped` keeps source provenance.
pub fn expand_all(file: &File, mapped: bool) -> (Expanded, Vec<SyntaxError>) {
    expand_with_sites(file, mapped)
}

/// The file's record constructors: every declared shape that is not also a
/// `fn`. A call naming one builds that record ahead of any name in scope
/// (LLP 1035.005.000 D3), so expansion never replaces such a call's head.
fn record_constructors(file: &File) -> BTreeSet<String> {
    file.shapes
        .iter()
        .map(|shape| &shape.name)
        .filter(|name| !file.fns.iter().any(|f| &f.name == *name))
        .cloned()
        .collect()
}

/// The stand-in for a value a refused use could not supply.
fn absent(span: crate::Span) -> Expr {
    Expr::Ident("?".into(), span)
}

fn expand_with_sites(file: &File, capture_sites: bool) -> (Expanded, Vec<SyntaxError>) {
    let source = &file.components[0];
    // Expansion replaces the view; retain only the root declarations here.
    let mut root = Component {
        name: source.name.clone(),
        props: source.props.clone(),
        injects: source.injects.clone(),
        provides: source.provides.clone(),
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
    let records = record_constructors(file);
    let mut ctx = Ctx {
        file,
        records: &records,
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
        errors: Vec::new(),
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
    let mut subst = Subst::new(&none, &records);
    for b in &source.provides {
        ctx.provides
            .push((b.name.clone(), subst_expr(&b.expr, &mut subst)));
    }
    let view = inline_nodes(&source.view, &mut subst, &mut ctx).unwrap_or_default();
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
    tail::resolve(&mut root.actions, &records, &mut ctx.errors);
    let expanded = Expanded {
        root,
        owners,
        instances: ctx.instances,
        state_instances,
        action_instances,
    };
    (expanded, ctx.errors)
}

/// What inlining carries down the tree besides the substitution.
struct Ctx<'a> {
    file: &'a File,
    /// The file's record constructors (`record_constructors`).
    records: &'a BTreeSet<String>,
    counter: &'a mut u32,
    depth: u32,
    /// Provided bindings in force, outermost component first, each already
    /// substituted into the scope of the component that provides it.
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
    /// Uses that could not be expanded, in the order met.
    errors: Vec<SyntaxError>,
}

impl Ctx<'_> {
    fn refuse(&mut self, id: &'static str, message: impl Into<String>, span: crate::Span) {
        self.errors.push(SyntaxError {
            id,
            message: message.into(),
            span,
        });
    }
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
                    ctx.refuse(
                        "syntax-inline-depth",
                        format!("component `{name}` nests too deeply (a cycle?)"),
                        *span,
                    );
                    continue;
                }
                let Some(c) = ctx.file.components.iter().find(|c| &c.name == name) else {
                    let message = ctx.file.unknown_component_message(name);
                    ctx.refuse("syntax-unknown-component", message, *span);
                    continue;
                };
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
                // An action prop an action calls last (its tail call): the
                // action it names, and the arguments it was curried with,
                // captured as the props above are (they are the parent's).
                let mut captures = captures;
                let mut tails: BTreeMap<String, (String, Vec<Expr>)> = BTreeMap::new();
                for (pi, prop) in c.props.iter().enumerate() {
                    let is_action = matches!(prop.ty.as_ref(), Some(TypeExpr::Named(name, _)) if name == "action");
                    let called = c
                        .actions
                        .iter()
                        .any(|a| tail_calls(&a.body).contains(&prop.name.as_str()));
                    let Some(value) = child_subst.get(&prop.name).filter(|_| is_action && called)
                    else {
                        continue;
                    };
                    let (target, curried) = match value {
                        Expr::Ident(target, _) => (target.clone(), Vec::new()),
                        Expr::Call(target, curried, _) => (target.clone(), curried.clone()),
                        other => {
                            ctx.refuse(
                                "syntax-tail-call",
                                format!("`{}` is called by an action, so it must name an action, as `{}=act` or `{}=act(args)` does", prop.name, prop.name, prop.name),
                                other.span(),
                            );
                            continue;
                        }
                    };
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
                    tails.insert(prop.name.clone(), (target, held));
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
                    for (param, _, source_name) in captures.iter().filter(|(_, _, n)| !n.is_empty())
                    {
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
                            body: tail_marked(
                                subst_stmts(
                                    &a.body,
                                    &mut Subst::new(&action_subst, records),
                                    &names,
                                ),
                                &tails,
                            ),
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
                out.extend(body?);
            }
            Node::Children { span } => match &ctx.fill {
                Some(fill) => out.extend(fill.iter().cloned()),
                None => ctx.refuse(
                    "syntax-children-without-slot",
                    "`children` belongs in a component that declares `slot`",
                    *span,
                ),
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
                index,
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
                    index: index.clone(),
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
                positional: positional.iter().map(|e| renamed_locals(e, map)).collect(),
                attrs: attrs
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: renamed_locals(&a.value, map),
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
                        value: renamed_locals(&a.value, map),
                        span: a.span,
                    })
                    .collect(),
                children: rename_nodes(children, map, n),
                span: *span,
            },
            Node::Children { span } => Node::Children { span: *span },
            Node::When {
                cond,
                then,
                otherwise,
                span,
            } => Node::When {
                cond: renamed_locals(cond, map),
                then: rename_nodes(then, map, n),
                otherwise: rename_nodes(otherwise, map, n),
                span: *span,
            },
            Node::Each {
                tag,
                var,
                index,
                list,
                key,
                body,
                span,
            } => {
                let mut inner = map.clone();
                let fresh = lifted(var, n);
                inner.insert(var.clone(), fresh.clone());
                let index = index.as_ref().map(|i| {
                    let fresh = lifted(i, n);
                    inner.insert(i.clone(), fresh.clone());
                    fresh
                });
                Node::Each {
                    tag: *tag,
                    var: fresh,
                    index,
                    list: renamed_locals(list, map),
                    key: renamed_locals(key, &inner),
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
                    subject: renamed_locals(subject, map),
                    some: (fresh, rename_nodes(&some.1, &inner, n)),
                    none: rename_nodes(none, map, n),
                    span: *span,
                }
            }
        })
        .collect()
}

/// The action props an action body calls in tail position (`close()` last,
/// or last in a last `if`/`match` branch): LLP 1017 P4c, the tail call
/// Charlie admitted 2026-10-03. The type pass refuses one anywhere else.
pub fn tail_calls(body: &[crate::ast::Stmt]) -> Vec<&str> {
    tail::tail_positions(body)
        .into_iter()
        .filter_map(|s| match s {
            crate::ast::Stmt::Command { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

/// The prefix a lifted action's tail call is marked with: `@tail:<action>`,
/// the root action the prop named. `@` cannot begin an authored name.
pub const TAIL: &str = "@tail:";

/// A lifted body whose tail calls name an action prop, each pointed at the
/// action the prop named, with its curried arguments first.
fn tail_marked(
    body: Vec<crate::ast::Stmt>,
    tails: &BTreeMap<String, (String, Vec<Expr>)>,
) -> Vec<crate::ast::Stmt> {
    use crate::ast::Stmt;
    let mut body = body;
    match body.last_mut() {
        Some(Stmt::Command { name, args, .. }) => {
            if let Some((target, held)) = tails.get(name.as_str()) {
                *name = format!("{TAIL}{target}");
                let mut all = held.clone();
                all.append(args);
                *args = all;
            }
        }
        Some(Stmt::If {
            then, otherwise, ..
        }) => {
            *then = tail_marked(std::mem::take(then), tails);
            *otherwise = tail_marked(std::mem::take(otherwise), tails);
        }
        Some(Stmt::Match { some, none, .. }) => {
            some.1 = tail_marked(std::mem::take(&mut some.1), tails);
            *none = tail_marked(std::mem::take(none), tails);
        }
        _ => {}
    }
    body
}
