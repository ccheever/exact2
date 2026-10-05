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
//! in the *use site's* scope — afresh at each `children` node, under the
//! region arms around it, so each place a fill renders is its own instance
//! with its own state (owned by the arm around that `children`).

use crate::ast::{Action, Attr, Binding, Component, Expr, File, Node, Param, Stmt, TypeExpr};
use crate::parser::SyntaxError;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub mod calls;
mod derives;
mod subst;
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
/// the use's number, each owned by the instance it belongs to ([`Owner`]);
/// a child's `derive` is an expression substituted at each read.
pub struct Expanded {
    /// The root, with the children's declarations appended.
    pub root: Component,
    /// For each of `root.states`, what owns it.
    pub owners: Vec<Owner>,
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

/// What owns a slot of the expanded root, and so when its initializer runs.
/// A child's state lives exactly as long as its instance: it is created,
/// its initializer evaluated in the use site's scope, when the instance is
/// (after settlement, so it may read derives, resources, a `match` binding
/// or a row's item), and dropped with it. It runs once per instance; a
/// later change to what it read does not run it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// The root's own `state`: initialized at boot, before settlement.
    Root,
    /// A child used outside every region: a root slot, initialized when
    /// the root instance first renders, after boot settlement.
    Instance,
    /// A child used inside a region: arm `arm` of the region tagged `tag`
    /// (an `each` row is arm 0; a `when`'s then/else and a `match`'s
    /// some/none are arms 0 and 1) owns it, one value per arm instance.
    Arm {
        /// The innermost enclosing region's tag.
        tag: u32,
        /// Which of its arms.
        arm: u8,
    },
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
/// declarations into it, every action call expanded (LLP 1089).
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
    let (mut expanded, errors) = expand_with_sites(file, mapped, false);
    hygiene(&mut expanded, file);
    (expanded, errors)
}

/// [`expand_all`] before [`hygiene`]: what the type pass checks, so a
/// caller's own names are still the author's when a `let` is refused.
pub fn expand_checked(file: &File, mapped: bool) -> (Expanded, Vec<SyntaxError>) {
    expand_with_sites(file, mapped, false)
}

/// [`expand_all`] with every typed prop's and inject's argument ascribed
/// its declared type ([`Expr::Typed`]), so the expansion alone says what
/// each use was checked against: what `contract lean` embeds, for the
/// semantics' checker to judge. The plan compiler expands without it.
/// Its calls are renamed apart as the plan's are ([`hygiene`]).
pub fn expand_typed(file: &File) -> (Expanded, Vec<SyntaxError>) {
    let (mut expanded, errors) = expand_with_sites(file, false, true);
    hygiene(&mut expanded, file);
    (expanded, errors)
}

/// Rename apart the parameters and binders of every action that calls
/// another ([`calls::hygiene`]): the last step of [`expand_all`].
pub fn hygiene(expanded: &mut Expanded, file: &File) {
    calls::hygiene(&mut expanded.root.actions, &record_constructors(file));
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

/// Whether `e` holds a `none` or a `[]` outside an [`Expr::Typed`]: a leaf
/// whose element type only a declaration can say.
fn untyped_leaf(e: &Expr) -> bool {
    use crate::ast::TemplatePart;
    match e {
        Expr::None(_) => true,
        Expr::List(items, _) => items.is_empty() || items.iter().any(untyped_leaf),
        Expr::Typed(..)
        | Expr::Number(..)
        | Expr::Str(..)
        | Expr::Bool(..)
        | Expr::Ident(..)
        | Expr::Arrow { .. } => false,
        Expr::Template(parts, _) => parts
            .iter()
            .any(|p| matches!(p, TemplatePart::Expr(x) if untyped_leaf(x))),
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Unary(_, x, _) => untyped_leaf(x),
        Expr::Call(_, args, _) => args.iter().any(untyped_leaf),
        Expr::Binary(_, a, b, _) => untyped_leaf(a) || untyped_leaf(b),
        Expr::Ternary(a, b, c, _) => untyped_leaf(a) || untyped_leaf(b) || untyped_leaf(c),
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => untyped_leaf(subject) || untyped_leaf(some) || untyped_leaf(none),
        Expr::Let { value, body, .. } => untyped_leaf(value) || untyped_leaf(body),
    }
}

/// `action`: a prop that names an action, never a value.
fn is_action(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Named(name, _) if name == "action")
}

/// The stand-in for a value a refused use could not supply.
fn absent(span: crate::Span) -> Expr {
    Expr::Ident("?".into(), span)
}

fn expand_with_sites(
    file: &File,
    capture_sites: bool,
    typed_props: bool,
) -> (Expanded, Vec<SyntaxError>) {
    // Same-component calls first, each in its own component's scope (LLP
    // 1089 D7): lifting then substitutes caller and callee alike.
    let (called, called_errors) = calls::expand_file(file);
    let file: &File = &called;
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
        arms: Vec::new(),
        next_tag: 1,
        extra_states: Vec::new(),
        extra_actions: Vec::new(),
        instance: 0,
        capture_sites,
        typed_props,
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
    let mut owners = vec![Owner::Root; root.states.len()];
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
    calls::resolve(&mut root.actions, &records, &mut ctx.errors);
    let mut errors = called_errors;
    errors.append(&mut ctx.errors);
    let expanded = Expanded {
        root,
        owners,
        instances: ctx.instances,
        state_instances,
        action_instances,
    };
    (expanded, errors)
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
    /// What fills `children` here: `Some` inside a `slot` component's view
    /// (possibly empty), `None` elsewhere.
    fill: Option<Rc<Fill>>,
    /// The region arms enclosing the current site, outermost first.
    arms: Vec<Owner>,
    /// The next region tag.
    next_tag: u32,
    /// The children's `state`s lifted into the root, with their owners and
    /// the instance that declared them.
    extra_states: Vec<(Binding, Owner, u32)>,
    /// The children's `action`s lifted into the root, with their instance.
    extra_actions: Vec<(Action, u32)>,
    /// The instantiation whose view is being inlined: 0 at the root.
    instance: u32,
    capture_sites: bool,
    /// Ascribe every typed prop's and inject's argument (`expand_typed`).
    typed_props: bool,
    /// Every instantiation so far, the root first (LLP 1035.005 D3).
    instances: Vec<Instance>,
    /// Uses that could not be expanded, in the order met.
    errors: Vec<SyntaxError>,
}

/// A `slot` component's fill: the nodes indented under its use, with what
/// they are inlined against there — the use site's substitution, providers,
/// own fill and instance. It is inlined at each `children` node it reaches,
/// under the region arms around that node, so a fill shown twice, or once
/// per row, is a separate instance each time, and a stateful child in it is
/// owned by the arm around `children`, not the arm around the use.
struct Fill {
    nodes: Vec<Node>,
    subst: BTreeMap<String, Expr>,
    provides: Vec<(String, Expr)>,
    outer: Option<Rc<Fill>>,
    instance: u32,
}

impl Ctx<'_> {
    /// A fresh region tag.
    fn tag(&mut self) -> u32 {
        let tag = self.next_tag;
        self.next_tag += 1;
        tag
    }

    /// Inline under arm `arm` of the region tagged `tag`: what a child used
    /// there declares, that arm's instance owns.
    fn in_arm<T>(&mut self, tag: u32, arm: u8, f: impl FnOnce(&mut Self) -> T) -> T {
        self.arms.push(Owner::Arm { tag, arm });
        let out = f(self);
        self.arms.pop();
        out
    }

    fn refuse(&mut self, id: &'static str, message: impl Into<String>, span: crate::Span) {
        // A fill inlined at two `children` nodes meets its errors twice.
        if self.errors.iter().any(|e| e.id == id && e.span == span) {
            return;
        }
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
                // An action prop or injected action an action calls (LLP
                // 1089 D7): the action it names and the arguments it was
                // curried with, captured as the props above are (they are
                // the parent's), and where it was bound.
                let mut captures = captures;
                let mut called: BTreeMap<String, (String, Vec<Expr>, crate::Span)> =
                    BTreeMap::new();
                for (pi, prop) in c.props.iter().chain(&c.injects).enumerate() {
                    let is_action = matches!(prop.ty.as_ref(), Some(TypeExpr::Named(name, _)) if name == "action");
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
                let fill = c.slot.then(|| {
                    Rc::new(Fill {
                        nodes: children.clone(),
                        subst: subst.map().clone(),
                        provides: ctx.provides.clone(),
                        outer: ctx.fill.clone(),
                        instance: ctx.instance,
                    })
                });
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
            Node::Children { span } => match ctx.fill.clone() {
                Some(fill) => {
                    // The use site's scope, providers, fill and instance;
                    // the region arms (and depth) are this node's.
                    let provides = std::mem::replace(&mut ctx.provides, fill.provides.clone());
                    let outer_fill = std::mem::replace(&mut ctx.fill, fill.outer.clone());
                    let outer_instance = std::mem::replace(&mut ctx.instance, fill.instance);
                    let mut site = Subst::new(&fill.subst, ctx.records);
                    let nodes = inline_nodes(&fill.nodes, &mut site, ctx);
                    ctx.provides = provides;
                    ctx.fill = outer_fill;
                    ctx.instance = outer_instance;
                    out.extend(nodes?);
                }
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
                ..
            } => {
                let tag = ctx.tag();
                let then = ctx.in_arm(tag, 0, |ctx| inline_nodes(then, subst, ctx));
                let otherwise = ctx.in_arm(tag, 1, |ctx| inline_nodes(otherwise, subst, ctx));
                out.push(Node::When {
                    tag,
                    cond: subst_expr(cond, subst),
                    then: then?,
                    otherwise: otherwise?,
                    span: *span,
                })
            }
            Node::Each {
                var,
                index,
                list,
                key,
                body,
                span,
                ..
            } => {
                let tag = ctx.tag();
                let body = ctx.in_arm(tag, 0, |ctx| inline_nodes(body, subst, ctx));
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
                ..
            } => {
                let tag = ctx.tag();
                let body = ctx.in_arm(tag, 0, |ctx| inline_nodes(&some.1, subst, ctx));
                let none = ctx.in_arm(tag, 1, |ctx| inline_nodes(none, subst, ctx));
                out.push(Node::Match {
                    tag,
                    subject: subst_expr(subject, subst),
                    some: (some.0.clone(), body?),
                    none: none?,
                    span: *span,
                })
            }
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
                tag,
                cond,
                then,
                otherwise,
                span,
            } => Node::When {
                tag: *tag,
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
                tag,
                subject,
                some,
                none,
                span,
            } => {
                let mut inner = map.clone();
                let fresh = lifted(&some.0, n);
                inner.insert(some.0.clone(), fresh.clone());
                Node::Match {
                    tag: *tag,
                    subject: renamed_locals(subject, map),
                    some: (fresh, rename_nodes(&some.1, &inner, n)),
                    none: rename_nodes(none, map, n),
                    span: *span,
                }
            }
        })
        .collect()
}

/// Whether a statement of `body`, in a called action's too, is `name(…)`.
fn commands(body: &[Stmt], name: &str) -> bool {
    body.iter().any(|s| match s {
        Stmt::Command { name: n, .. } => n == name,
        Stmt::If {
            then, otherwise, ..
        } => commands(then, name) || commands(otherwise, name),
        Stmt::Match { some, none, .. } => commands(&some.1, name) || commands(none, name),
        Stmt::Call { body, .. } => commands(body, name),
        _ => false,
    })
}

/// A lifted body whose same-component calls pass the instance's captures
/// first, as every lifted action of it takes them (LLP 1089 D9: a call's
/// arguments are the callee's whole parameter list). Each is bound again
/// in the call, to the same value.
fn with_captures(body: Vec<Stmt>, captures: &[(String, crate::Span)]) -> Vec<Stmt> {
    if captures.is_empty() {
        return body;
    }
    body.into_iter()
        .map(|s| match s {
            Stmt::If {
                cond,
                then,
                otherwise,
                span,
            } => Stmt::If {
                cond,
                then: with_captures(then, captures),
                otherwise: with_captures(otherwise, captures),
                span,
            },
            Stmt::Match {
                subject,
                some,
                none,
                span,
            } => Stmt::Match {
                subject,
                some: (some.0, with_captures(some.1, captures)),
                none: with_captures(none, captures),
                span,
            },
            Stmt::Call {
                action,
                args,
                body,
                authored,
                curried,
                binding,
                span,
            } => {
                let ident = |(name, at): &(String, crate::Span)| Expr::Ident(name.clone(), *at);
                let mut all: Vec<Expr> = captures.iter().map(ident).collect();
                all.extend(args);
                let mut inner: Vec<Stmt> = captures
                    .iter()
                    .map(|c| Stmt::Let {
                        name: c.0.clone(),
                        expr: ident(c),
                        span,
                    })
                    .collect();
                inner.extend(with_captures(body, captures));
                Stmt::Call {
                    action,
                    args: all,
                    body: inner,
                    authored,
                    curried,
                    binding,
                    span,
                }
            }
            other => other,
        })
        .collect()
}

/// A lifted body whose calls of an action prop or injected action, at
/// every position, are marked with the action it named, its curried
/// arguments first, for [`calls::resolve`].
fn call_marked(
    body: Vec<Stmt>,
    called: &BTreeMap<String, (String, Vec<Expr>, crate::Span)>,
) -> Vec<Stmt> {
    if called.is_empty() {
        return body;
    }
    body.into_iter()
        .map(|s| match s {
            Stmt::Command { name, args, span } => match called.get(name.as_str()) {
                Some((target, held, binding)) => {
                    let authored = args.len();
                    let mut all = held.clone();
                    all.extend(args);
                    Stmt::Call {
                        action: format!("{}{target}", calls::MARK),
                        args: all,
                        body: Vec::new(),
                        authored,
                        curried: 0,
                        binding: Some(*binding),
                        span,
                    }
                }
                None => Stmt::Command { name, args, span },
            },
            Stmt::If {
                cond,
                then,
                otherwise,
                span,
            } => Stmt::If {
                cond,
                then: call_marked(then, called),
                otherwise: call_marked(otherwise, called),
                span,
            },
            Stmt::Match {
                subject,
                some,
                none,
                span,
            } => Stmt::Match {
                subject,
                some: (some.0, call_marked(some.1, called)),
                none: call_marked(none, called),
                span,
            },
            Stmt::Call {
                action,
                args,
                body,
                authored,
                curried,
                binding,
                span,
            } => Stmt::Call {
                action,
                args,
                body: call_marked(body, called),
                authored,
                curried,
                binding,
                span,
            },
            other => other,
        })
        .collect()
}
