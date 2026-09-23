//! One component's declarations, action bodies and view, checked in the
//! order their types need: initializers, derives to a fixpoint, resources,
//! handler call sites, action bodies, then the view.

use super::{
    checks::{check_stmts, check_view, infer_owned_state_initializers},
    err, infer, record_source, ComponentTypes, Ref, Scope, Shapes, Sink, Ty, TypeError, Types,
};
use contract_syntax::{Component, Expr, Node};
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
    ct.derives = vec![Ty::Unknown; c.derives.len()];
    // Every declaration now has a type entry before constructing a full scope.
    for _round in 0..(c.derives.len() + 2) {
        let scope = types.component_scope(c, &ct);
        let mut changed = false;
        for (i, d) in c.derives.iter().enumerate() {
            match infer(&d.expr, &scope, shapes) {
                Ok(t) => {
                    if t != ct.derives[i] {
                        ct.derives[i] = t;
                        changed = true;
                    }
                }
                Err(e)
                    if e.id == "type-unknown-name"
                        && c.derives
                            .iter()
                            .any(|x| e.message.contains(&format!("`{}`", x.name))) => {}
                // An expression over a derive this round has not typed yet
                // (`current.ok` while `current` is still `?`): the next round
                // has it, and the strict pass below reports what never types.
                Err(_) => {}
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
        match infer(&t.every.0, &scope, shapes) {
            Ok(Ty::Number) => {}
            Ok(_) => sink.push(TypeError {
                id: "type-timer",
                message: "`every` needs a number of milliseconds".into(),
                span: t.every.2,
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
                                if args.len() < ct.actions[ai].len() {
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
                var, list, body, ..
            } => {
                if let Ok(Ty::List(item)) = infer(list, scope, shapes) {
                    let mut inner = scope.clone();
                    inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
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
