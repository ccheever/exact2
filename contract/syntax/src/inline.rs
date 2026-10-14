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
use crate::notfound::{implicit_notfound, implicit_notfound_view};
use crate::parser::SyntaxError;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub mod calls;
mod derives;
mod subst;
#[cfg(test)]
mod tests;
mod use_node;

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
pub(crate) fn is_action(ty: &TypeExpr) -> bool {
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
        persist: source.persist.clone(),
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
    let discarded_slots = use_node::discarded_slots(file);
    let mut ctx = Ctx {
        file,
        records: &records,
        discarded_slots: &discarded_slots,
        counter: &mut counter,
        depth: 0,
        site_depth: 0,
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
    // @ref LLP 1116.009 D2 — an app with no `notfound` route is given one, and
    // its screen where the app draws a route per stack entry.
    let screen = implicit_notfound(file)
        .then(|| {
            let routes = file.routes.as_ref()?;
            implicit_notfound_view(routes, &source.view)
        })
        .flatten();
    let written;
    let source_view = match screen {
        Some(view) => {
            written = view;
            &written
        }
        None => &source.view,
    };
    let view = inline_nodes(source_view, &mut subst, &mut ctx).unwrap_or_default();
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
    /// Slot components whose views cannot reach their supplied fill.
    discarded_slots: &'a BTreeSet<String>,
    counter: &'a mut u32,
    depth: u32,
    /// Emitted view/region sites above the node being expanded. A component
    /// use and a slot's `children` are replaced, so neither consumes a site.
    site_depth: usize,
    /// Provided bindings in force, outermost component first, each already
    /// substituted into the scope of the component that provides it.
    provides: Vec<(String, Expr)>,
    /// What fills `children` here: `Some` inside a slot view that can render
    /// the supplied nodes, `None` elsewhere.
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
        let is_site = !matches!(n, Node::Use { .. } | Node::Children { .. });
        if is_site && ctx.site_depth >= 256 {
            ctx.refuse(
                "syntax-inline-depth",
                "inlined views nest more than 256 sites deep",
                n.span(),
            );
            continue;
        }
        ctx.site_depth += usize::from(is_site);
        match n {
            Node::Use { .. } => out.extend(use_node::inline_use(n, subst, ctx)?),
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
                        written: a.written,
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
        ctx.site_depth -= usize::from(is_site);
    }
    Ok(out)
}

// Rename every name the view binds with a unique suffix so inlined bodies
// cannot capture parent names.
// Borrow renamed strings directly; subst_expr retains each reference's span.
fn rename_nodes(
    nodes: &[Node],
    map: &BTreeMap<String, String>,
    n: u32,
    discarded_slots: &BTreeSet<String>,
) -> Vec<Node> {
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
                        written: a.written,
                    })
                    .collect(),
                children: rename_nodes(children, map, n, discarded_slots),
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
                        written: a.written,
                    })
                    .collect(),
                // A fill no one renders is not copied; one placeholder keeps the
                // span `syntax-no-slot` names for a slotless component.
                children: if discarded_slots.contains(name) {
                    children
                        .first()
                        .map(|c| Node::Children { span: c.span() })
                        .into_iter()
                        .collect()
                } else {
                    rename_nodes(children, map, n, discarded_slots)
                },
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
                then: rename_nodes(then, map, n, discarded_slots),
                otherwise: rename_nodes(otherwise, map, n, discarded_slots),
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
                    body: rename_nodes(body, &inner, n, discarded_slots),
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
                    some: (fresh, rename_nodes(&some.1, &inner, n, discarded_slots)),
                    none: rename_nodes(none, map, n, discarded_slots),
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
