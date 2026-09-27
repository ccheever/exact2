//! One component's declarations, action bodies and view, checked in the
//! order their types need: initializers, derives to a fixpoint, resources,
//! handler call sites, action bodies, then the view.

use super::{
    checks::{check_stmts, check_view, infer_owned_state_initializers},
    err, infer, record_source, ComponentTypes, Ref, Scope, Shapes, Sink, Ty, TypeError, Types,
};
use contract_syntax::{Component, Expr, Node, Span, TemplatePart};
use std::{collections::BTreeMap, sync::Arc};

/// Check one component, recording each refusal in `sink` and carrying on
/// with `?` wherever a type could not be found, so one run reports every
/// independent mistake (a consequence of an earlier one mentions `?`, and
/// the sink drops it).
pub(crate) fn check_component(
    c: &Component,
    types: &Types,
    owners: Option<&[Option<u32>]>,
    sink: &mut Sink,
) -> ComponentTypes {
    let shapes = &types.shapes;
    let mut ct = ComponentTypes {
        name: c.name.clone(),
        ..ComponentTypes::default()
    };
    // Duplicate names across all declarations.
    let mut seen = BTreeMap::new();
    for (name, span) in c
        .props
        .iter()
        .map(|p| (&p.name, p.span))
        .chain(c.injects.iter().map(|p| (&p.name, p.span)))
        .chain(c.states.iter().map(|s| (&s.name, s.span)))
        .chain(c.derives.iter().map(|d| (&d.name, d.span)))
        .chain(c.resources.iter().map(|r| (&r.name, r.span)))
        .chain(c.mutations.iter().map(|m| (&m.name, m.span)))
        .chain(c.actions.iter().map(|a| (&a.name, a.span)))
    {
        if seen.insert(name.clone(), span).is_some() {
            sink.push(TypeError {
                id: "type-duplicate-name",
                message: format!("`{name}` declared twice"),
                span,
            });
        }
    }
    for p in &c.props {
        let ty = match &p.ty {
            Some(t) => sink.keep(shapes.resolve(t)),
            None => sink.keep(err(
                "type-prop-untyped",
                format!("prop `{}` needs a type", p.name),
                p.span,
            )),
        };
        ct.props.push(ty);
    }
    for p in &c.injects {
        let ty = match &p.ty {
            Some(t) => sink.keep(shapes.resolve(t)),
            None => sink.keep(err(
                "type-inject-untyped",
                format!("inject `{}` needs a type", p.name),
                p.span,
            )),
        };
        ct.props.push(ty);
    }
    for r in &c.resources {
        ct.resources.push(sink.keep(shapes.resolve(&r.shape)));
    }
    for m in &c.mutations {
        ct.mutations.push(sink.keep(shapes.resolve(&m.shape)));
        // @ref LLP 1054.000.000 D1 — what a send to this mutation refreshes.
        for (i, (name, span)) in m.refreshes.iter().enumerate() {
            if !c.resources.iter().any(|r| &r.name == name) {
                sink.push(TypeError {
                    id: "type-refreshes-not-resource",
                    message: format!(
                        "`{name}` is not a resource: `refreshes` names this component's resources"
                    ),
                    span: *span,
                });
            } else if m.refreshes[..i].iter().any(|(n, _)| n == name) {
                sink.push(TypeError {
                    id: "type-refreshes-duplicate",
                    message: format!("`{name}` is already named in `refreshes`"),
                    span: *span,
                });
            }
        }
    }
    // Slots from initializers (may hold `?` inside an option).
    if !c.states.is_empty() {
        let mut scope = Scope::default();
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
        scope.push(names);
        for (i, s) in c.states.iter().enumerate() {
            let t = if i == 0 && owners.is_some() && shapes.routes.is_some() {
                Ty::Record("Router".into())
            } else if owners
                .and_then(|owners| owners.get(i))
                .is_some_and(Option::is_some)
            {
                Ty::Unknown
            } else {
                sink.keep(infer(&s.expr, &scope, shapes))
            };
            // Duplicate declarations were refused above. Each initializer sees
            // only earlier slots, without copying their names and types again.
            scope.push_name((s.name.clone(), Ref::Slot(i as u32), t.clone()));
            ct.slots.push(t);
        }
    }
    // Actions: parameters (declared or `?`), then refine slots from writes.
    for a in &c.actions {
        let mut params = Vec::new();
        for p in &a.params {
            params.push(match &p.ty {
                Some(t) => sink.keep(shapes.resolve(t)),
                None => Ty::Unknown,
            });
        }
        ct.actions.push(params);
    }
    // Derives: iterate to a fixpoint so order does not matter and `?` fills.
    // Each is inferred after the derives it reads, and its type enters the
    // scope at once, so a set without a cycle settles in one round (and one
    // to confirm) rather than one round per link of a chain.
    ct.derives = vec![Ty::Unknown; c.derives.len()];
    // Every declaration now has a type entry before constructing a full scope.
    let first_derive = c.props.len() + c.injects.len() + c.states.len();
    let mut scope = types.component_scope(c, &ct);
    let order = derive_order(c);
    for _round in 0..(c.derives.len() + 2) {
        let mut changed = false;
        for &i in &order {
            // An expression over a derive not yet typed (`current.ok` while
            // `current` is still `?`) waits for a later round; the strict
            // pass below reports what never types.
            if let Ok(t) = infer(&c.derives[i].expr, &scope, shapes) {
                if t != ct.derives[i] {
                    scope.retype(first_derive + i, t.clone());
                    ct.derives[i] = t;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Everything must now type; re-infer derives strictly to surface errors.
    let scope = types.component_scope(c, &ct);
    for (i, d) in c.derives.iter().enumerate() {
        match infer(&d.expr, &scope, shapes) {
            // What types only incompletely depends on itself; what fails to
            // type is refused for that reason alone.
            Ok(t) if !t.is_complete() => sink.push(TypeError {
                id: "type-derive-cycle",
                message: format!(
                    "cannot infer the type of `{}`: it depends on itself through other derives",
                    d.name
                ),
                span: d.span,
            }),
            Ok(t) => ct.derives[i] = t,
            Err(e) => sink.push(e),
        }
    }
    let mut resource_args = Vec::with_capacity(c.resources.len());
    for r in &c.resources {
        let args: Vec<Ty> = r
            .args
            .iter()
            .map(|arg| sink.keep(infer(arg, &scope, shapes)))
            .collect();
        resource_args.push(args);
    }
    sink.keep_unit(infer_owned_state_initializers(c, &mut ct, types, owners));
    // Handler call sites give untyped parameters their types.
    // Row initializers have just resolved the lifted child slots. Curried
    // action-prop arguments must see those types too, not the earlier scope.
    let scope = types.component_scope(c, &ct);
    sink.keep_unit(refine_params_from_view(&c.view, &scope, c, &mut ct, shapes));
    // Action bodies: writes refine slots; assignments must unify.
    for (ai, a) in c.actions.iter().enumerate() {
        let mut scope = types.component_scope(c, &ct);
        scope.push(
            a.params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        p.name.clone(),
                        Ref::Param(i as u32),
                        ct.actions[ai][i].clone(),
                    )
                })
                .collect(),
        );
        check_stmts(&a.body, &scope, c, &mut ct, shapes, sink);
    }
    // The seam's signatures (LLP 1027 D2): every resource's arguments against
    // the final scope, unified with the sends' (recorded as their bodies were
    // checked). One source, one signature.
    {
        let scope = types.component_scope(c, &ct);
        for (i, r) in c.resources.iter().enumerate() {
            // An argument that failed above is reported there, once.
            let params: Vec<Ty> = r
                .args
                .iter()
                .zip(&resource_args[i])
                .map(|(arg, before)| match before {
                    Ty::Unknown => Ty::Unknown,
                    _ => infer(arg, &scope, shapes).unwrap_or(Ty::Unknown),
                })
                .collect();
            let result = ct.resources[i].clone();
            sink.keep_unit(record_source(&mut ct, &r.source, params, result, r.span));
        }
        // @ref LLP 1048.003 D6 — a placeholder is a source call over values:
        // the build answers it once for every launch, so its arguments read
        // no state, and its answer is the resource's shape.
        for (i, r) in c.resources.iter().enumerate() {
            let Some(p) = &r.placeholder else { continue };
            // @ref LLP 1054.000.002 D2 — `empty(field=value, …)` is the
            // compiler's constant, not a source.
            if p.source == crate::placeholder::EMPTY {
                if let Err(errors) = crate::placeholder::materialize(
                    &ct.resources[i],
                    &p.args,
                    shapes,
                    &r.name,
                    p.span,
                ) {
                    for e in errors {
                        sink.push(e);
                    }
                }
                continue;
            }
            if let Some((name, span)) = p.args.iter().find_map(|a| reads_state(a, &scope)) {
                sink.push(TypeError {
                    id: "type-placeholder-reads",
                    message: format!(
                        "`{}`'s placeholder reads `{name}`: a placeholder's arguments are values, answered once at build",
                        r.name
                    ),
                    span,
                });
                continue;
            }
            let values = Scope::default();
            let params = p
                .args
                .iter()
                .map(|arg| sink.keep(infer(arg, &values, shapes)))
                .collect();
            let result = ct.resources[i].clone();
            sink.keep_unit(record_source(&mut ct, &p.source, params, result, p.span));
        }
    }
    // A slot or parameter left `?` by a refusal above is not news.
    let failed = !sink.errors.is_empty();
    for (i, s) in c.states.iter().enumerate() {
        if !ct.slots[i].is_complete() && !(failed && ct.slots[i] == Ty::Unknown) {
            sink.push(TypeError {
                id: "type-cannot-infer",
                message: format!(
                    "cannot infer the type of `{}`: nothing writes a value into it",
                    s.name
                ),
                span: s.span,
            });
        }
    }
    for (ai, a) in c.actions.iter().enumerate() {
        for (i, p) in a.params.iter().enumerate() {
            if !ct.actions[ai][i].is_complete() {
                sink.push(TypeError { id: "type-cannot-infer", message: format!("cannot infer the type of parameter `{}`; write `{}: <type>` or call the action from a handler", p.name, p.name), span: p.span });
            }
        }
    }
    // The view types.
    let scope = types.component_scope(c, &ct);
    check_view(&c.view, &scope, shapes, sink);
    for t in &c.tasks {
        match infer(&t.timer.0, &scope, shapes) {
            Ok(Ty::Number) => {}
            Ok(_) => sink.push(TypeError {
                id: "type-timer",
                message: "a task needs a number of milliseconds".into(),
                span: t.timer.2,
            }),
            Err(e) => sink.push(e),
        }
    }
    ct
}

impl Scope {
    fn push_name(&mut self, name: (String, Ref, Ty)) {
        Arc::make_mut(self.frames.last_mut().expect("initializer scope frame"))
            .names
            .push(name);
    }

    /// Change the type of the bottom frame's `index`th name in place.
    pub(crate) fn retype(&mut self, index: usize, ty: Ty) {
        Arc::make_mut(&mut self.frames[0]).names[index].2 = ty;
    }

    pub(crate) fn frames_reset(&mut self, names: &[(String, Ref, Ty)]) {
        self.frames.clear();
        self.push(names.to_vec());
    }
}

fn refine_params_from_view(
    nodes: &[Node],
    scope: &Scope,
    c: &Component,
    ct: &mut ComponentTypes,
    shapes: &Shapes,
) -> Result<(), TypeError> {
    for n in nodes {
        match n {
            Node::Provide { body, .. } => refine_params_from_view(body, scope, c, ct, shapes)?,
            Node::Children { .. } => {}
            Node::Element {
                attrs, children, ..
            } => {
                for a in attrs {
                    if matches!(
                        a.name.as_str(),
                        "press"
                            | "change"
                            | "select"
                            | "hover"
                            | "focus"
                            | "blur"
                            | "key"
                            | "submit"
                            | "load"
                            | "message"
                            | "contextmenu"
                            | "dblclick"
                            | "swiperight"
                            | "refresh"
                            | "reachstart"
                            | "reachend"
                            | "scroll"
                            | "loadedmetadata"
                            | "durationchange"
                            | "timeupdate"
                            | "play"
                            | "playing"
                            | "pause"
                            | "ended"
                            | "waiting"
                            | "seeking"
                            | "seeked"
                            | "ratechange"
                            | "volumechange"
                            | "error"
                            | "canplay"
                            | "navigate"
                    ) {
                        let (name, args): (&str, &[Expr]) = match &a.value {
                            Expr::Ident(n, _) => (n, &[]),
                            Expr::Call(n, args, _) => (n, args),
                            _ => continue,
                        };
                        if let Some(ai) = c.actions.iter().position(|x| x.name == name) {
                            for (i, arg) in args.iter().enumerate() {
                                if i < ct.actions[ai].len() {
                                    let t = infer(arg, scope, shapes)?;
                                    if let Some(u) = ct.actions[ai][i].unify(&t) {
                                        ct.actions[ai][i] = u;
                                    }
                                }
                            }
                            // Event payloads: change/key/message are strings;
                            // hover is whether the pointer is over.
                            let payload = match a.name.as_str() {
                                "change" | "key" | "message" | "navigate" | "error" => {
                                    vec![Ty::String]
                                }
                                "timeupdate" | "durationchange" => vec![Ty::Number],
                                "hover" => vec![Ty::Bool],
                                "select" => vec![Ty::Record("MarkdownSelection".into())],
                                "scroll" => vec![Ty::Number, Ty::Number],
                                _ => vec![],
                            };
                            let start = ct.actions[ai].len().saturating_sub(payload.len());
                            for (offset, ty) in payload.into_iter().enumerate() {
                                let last = start + offset;
                                if args.len() < ct.actions[ai].len() && last < ct.actions[ai].len()
                                {
                                    let declared = ct.actions[ai][last].clone();
                                    let Some(unified) = declared.unify(&ty) else {
                                        return err(
                                            "type-handler-payload",
                                            format!(
                                                "`{}=` supplies `{ty}` to parameter `{}`, declared `{declared}`",
                                                a.name, c.actions[ai].params[last].name
                                            ),
                                            a.span,
                                        );
                                    };
                                    ct.actions[ai][last] = unified;
                                }
                            }
                        }
                    }
                }
                refine_params_from_view(children, scope, c, ct, shapes)?;
            }
            Node::Use { args, children, .. } => {
                refine_params_from_view(children, scope, c, ct, shapes)?;
                for a in args {
                    let _ = a;
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                refine_params_from_view(then, scope, c, ct, shapes)?;
                refine_params_from_view(otherwise, scope, c, ct, shapes)?;
            }
            Node::Each {
                var,
                index,
                list,
                body,
                ..
            } => {
                if let Ok(Ty::List(item)) = infer(list, scope, shapes) {
                    let mut inner = scope.clone();
                    inner.push_each(var, index.as_deref(), *item);
                    refine_params_from_view(body, &inner, c, ct, shapes)?;
                }
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                if let Ok(Ty::Option(item)) = infer(subject, scope, shapes) {
                    let mut inner = scope.clone();
                    inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                    refine_params_from_view(&some.1, &inner, c, ct, shapes)?;
                }
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                refine_params_from_view(none, &none_scope, c, ct, shapes)?;
            }
        }
    }
    Ok(())
}

/// Derive indices, each after the derives its expression names (a cycle is
/// left in declaration order; the fixpoint refuses it).
fn derive_order(c: &Component) -> Vec<usize> {
    fn names<'a>(e: &'a Expr, out: &mut Vec<&'a str>) {
        use contract_syntax::TemplatePart;
        match e {
            Expr::Ident(n, _) => out.push(n),
            Expr::Call(n, args, _) => {
                out.push(n);
                args.iter().for_each(|a| names(a, out));
            }
            Expr::Some(x, _)
            | Expr::Unary(_, x, _)
            | Expr::Member(x, _, _)
            | Expr::NamedArg(_, x, _) => names(x, out),
            Expr::Binary(_, a, b, _) => {
                names(a, out);
                names(b, out);
            }
            Expr::Ternary(a, b, x, _) => {
                names(a, out);
                names(b, out);
                names(x, out);
            }
            Expr::Match {
                subject,
                some,
                none,
                ..
            } => {
                names(subject, out);
                names(some, out);
                names(none, out);
            }
            Expr::Let { value, body, .. } => {
                names(value, out);
                names(body, out);
            }
            Expr::Template(parts, _) => parts.iter().for_each(|p| {
                if let TemplatePart::Expr(x) = p {
                    names(x, out)
                }
            }),
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
        }
    }
    fn visit(i: usize, reads: &[Vec<usize>], seen: &mut [bool], order: &mut Vec<usize>) {
        if std::mem::replace(&mut seen[i], true) {
            return;
        }
        for &d in &reads[i] {
            visit(d, reads, seen, order);
        }
        order.push(i);
    }
    let index: BTreeMap<&str, usize> = c
        .derives
        .iter()
        .enumerate()
        .map(|(i, d)| (d.name.as_str(), i))
        .collect();
    let reads: Vec<Vec<usize>> = c
        .derives
        .iter()
        .map(|d| {
            let mut out = Vec::new();
            names(&d.expr, &mut out);
            out.iter().filter_map(|n| index.get(n).copied()).collect()
        })
        .collect();
    let (mut seen, mut order) = (vec![false; c.derives.len()], Vec::new());
    for i in 0..c.derives.len() {
        visit(i, &reads, &mut seen, &mut order);
    }
    order
}

/// The first name in `e` the component declares: state a placeholder's
/// arguments may not read (LLP 1048.003 D6).
fn reads_state(e: &Expr, scope: &Scope) -> Option<(String, Span)> {
    fn walk(e: &Expr, scope: &Scope, bound: &mut Vec<String>) -> Option<(String, Span)> {
        let within = |name: &str, x: &Expr, bound: &mut Vec<String>| {
            bound.push(name.to_string());
            let found = walk(x, scope, bound);
            bound.pop();
            found
        };
        match e {
            Expr::Ident(name, span) => (!bound.contains(name) && scope.lookup(name).is_some())
                .then(|| (name.clone(), *span)),
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(..) => None,
            Expr::Template(parts, _) => parts.iter().find_map(|p| match p {
                TemplatePart::Expr(x) => walk(x, scope, bound),
                _ => None,
            }),
            Expr::Some(x, _)
            | Expr::Member(x, _, _)
            | Expr::NamedArg(_, x, _)
            | Expr::Unary(_, x, _) => walk(x, scope, bound),
            Expr::Call(_, args, _) => args.iter().find_map(|a| walk(a, scope, bound)),
            Expr::Binary(_, a, b, _) => walk(a, scope, bound).or_else(|| walk(b, scope, bound)),
            Expr::Ternary(a, b, c, _) => walk(a, scope, bound)
                .or_else(|| walk(b, scope, bound))
                .or_else(|| walk(c, scope, bound)),
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => walk(subject, scope, bound)
                .or_else(|| within(var, some, bound))
                .or_else(|| walk(none, scope, bound)),
            Expr::Let {
                name, value, body, ..
            } => walk(value, scope, bound).or_else(|| within(name, body, bound)),
        }
    }
    walk(e, scope, &mut Vec::new())
}
