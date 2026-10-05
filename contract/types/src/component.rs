//! One component's declarations, action bodies and view, checked in the
//! order their types need: initializers, derives to a fixpoint, resources,
//! handler call sites, action bodies, then the view.

use super::{
    checks::{check_view, infer_owned_state_initializers},
    err, infer, record_source, ComponentTypes, Ref, Scope, Shapes, Sink, Ty, TypeError, Types,
};
use contract_syntax::{Component, Expr, Node, Owner, Span, TemplatePart};
use std::{collections::BTreeMap, sync::Arc};

/// Check one component, recording each refusal in `sink` and carrying on
/// with `?` wherever a type could not be found, so one run reports every
/// independent mistake (a consequence of an earlier one mentions `?`, and
/// the sink drops it).
pub(crate) fn check_component(
    c: &Component,
    types: &Types,
    owners: Option<&[Owner]>,
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
        let t = sink.keep(shapes.resolve(&m.shape));
        // A mutation holds `option<T>` of its answer (LLP 1090 D2).
        sink.keep_unit(shapes.bounded(&Ty::Option(Box::new(t.clone())), m.shape.span()));
        ct.mutations.push(t);
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
                .is_some_and(|o| *o != Owner::Root)
            {
                // A child's state, typed in its use site's scope once the
                // derives and resources it may read are
                // (`infer_owned_state_initializers`).
                Ty::Unknown
            } else if let Some(e) = initializer_scope(c, i, &scope, shapes) {
                sink.push(e);
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
    // Derives, then the slots action writes type, until neither changes: a
    // state `none` or `[]` declares is typed by its first write, and what
    // reads it (a derive, a view head, a handler's or a resource's
    // argument) must see that type, whatever the declaration order.
    ct.derives = vec![Ty::Unknown; c.derives.len()];
    // One ordered pass first, not a fixed point (LLP 1088 D6 is deferred): a
    // row's state is typed from its initializer before the derives that read
    // it (shop F3's `derive chosen = at(…, pick)` over a row's `state pick =
    // 0`). Its refusals are made where the same pass runs again below, once
    // the derives and resources a child's state may read have types.
    let _ = infer_owned_state_initializers(c, &mut ct, types, owners);
    let order = derive_order(c);
    for _round in 0..(c.states.len() + 2) {
        settle_derives(c, &mut ct, types, &order);
        if ct.slots.iter().all(Ty::is_complete) || !probe_writes(c, &mut ct, types) {
            break;
        }
    }
    // Everything must now type; re-infer derives strictly to surface errors.
    let scope = types.component_scope(c, &ct);
    for (i, d) in c.derives.iter().enumerate() {
        match infer(&d.expr, &scope, shapes) {
            // What types only incompletely depends on itself; what fails to
            // type is refused for that reason alone.
            // An `[]` nothing pairs with a typed list is the one leaf that
            // leaves a `?` without a cycle.
            Ok(t) if !t.is_complete() => sink.push(match empty_list_in(&d.expr) {
                Some(span) => TypeError {
                    id: "type-cannot-infer",
                    message: format!(
                        "cannot infer what `[]` holds in `{}`: pair it with a typed arm (`match`/`?:`) or write it where a `list<T>` is declared",
                        d.name
                    ),
                    span,
                },
                None => TypeError {
                    id: "type-derive-cycle",
                    message: format!(
                        "cannot infer the type of `{}`: it depends on itself through other derives",
                        d.name
                    ),
                    span: d.span,
                },
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
            .map(|arg| sink.keep(crate::source_argument(arg, &r.source, &scope, shapes)))
            .collect();
        resource_args.push(args);
    }
    sink.keep_unit(infer_owned_state_initializers(c, &mut ct, types, owners));
    // Handler call sites give untyped parameters their types.
    // Row initializers have just resolved the lifted child slots. Curried
    // action-prop arguments must see those types too, not the earlier scope.
    let scope = types.component_scope(c, &ct);
    sink.keep_unit(refine_params_from_view(&c.view, &scope, c, &mut ct, shapes));
    // Then the calls in action bodies (LLP 1089 D8), before any body is checked.
    crate::calls::refine(c, &mut ct, types);
    // Action bodies: writes refine slots; assignments must unify. Two
    // rounds, so a `send` whose argument is a state a later action writes
    // (`state q = none`, typed by `q = some(s)`) records the written type
    // whatever the declaration order: the first round refines the slots and
    // its findings are dropped, the second reports.
    for round in 0..2 {
        let mut scratch = Sink::default();
        let report = if round == 0 { &mut scratch } else { &mut *sink };
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
            scope.enter_action();
            // A lifted child action is `name#N` (LLP 1017 P4c).
            let lifted = a.name.contains('#');
            crate::actions::check_body(&a.body, &scope, lifted, c, &mut ct, shapes, report);
        }
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
                    _ => {
                        let t = infer(arg, &scope, shapes).unwrap_or(Ty::Unknown);
                        // Every write has now typed the slots; what is still
                        // `?` here has no plan type (a source's parameter
                        // is not filled in by another site).
                        if t != Ty::Unknown && !t.is_complete() {
                            sink.push(TypeError {
                                id: "type-cannot-infer",
                                message: format!(
                                    "cannot infer the type of this argument to `{}`: it is `{t}` after every write",
                                    r.source
                                ),
                                span: arg.span(),
                            });
                        }
                        t
                    }
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
                .map(|arg| sink.keep(crate::source_argument(arg, &p.source, &values, shapes)))
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
    // A child's `provide` section and the view type. The expanded root's
    // section is checked with its uses (`check_root`), after the expansion,
    // so a provided value is refused where a child reads it (LLP 1006 §3).
    let scope = types.component_scope(c, &ct);
    if owners.is_none() {
        for b in &c.provides {
            sink.keep(infer(&b.expr, &scope, shapes));
        }
    }
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
}

/// Derives to a fixpoint, so order does not matter and `?` fills. Each is
/// inferred after the derives it reads, and its type enters the scope at
/// once, so a set without a cycle settles in one round (and one to confirm)
/// rather than one round per link of a chain.
fn settle_derives(c: &Component, ct: &mut ComponentTypes, types: &Types, order: &[usize]) {
    let first_derive = c.props.len() + c.injects.len() + c.states.len();
    let mut scope = types.component_scope(c, ct);
    for _round in 0..(c.derives.len() + 2) {
        let mut changed = false;
        for &i in order {
            // An expression over a derive not yet typed (`current.ok` while
            // `current` is still `?`) waits for a later round; the strict
            // pass reports what never types.
            if let Ok(t) = infer(&c.derives[i].expr, &scope, &types.shapes) {
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
}

/// Check every action body against a copy of the types, its findings
/// dropped, and keep only what its writes say about the slots; whether a
/// slot changed. The bodies are checked for real once everything is typed.
fn probe_writes(c: &Component, ct: &mut ComponentTypes, types: &Types) -> bool {
    let mut probe = ct.clone();
    let mut scratch = Sink::default();
    for (ai, a) in c.actions.iter().enumerate() {
        let mut scope = types.component_scope(c, &probe);
        scope.push(
            a.params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        p.name.clone(),
                        Ref::Param(i as u32),
                        probe.actions[ai][i].clone(),
                    )
                })
                .collect(),
        );
        scope.enter_action();
        let lifted = a.name.contains('#');
        crate::actions::check_body(
            &a.body,
            &scope,
            lifted,
            c,
            &mut probe,
            &types.shapes,
            &mut scratch,
        );
    }
    let changed = probe.slots != ct.slots;
    ct.slots = probe.slots;
    changed
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
            Node::Children { .. } => {}
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                // A checkbox's `change` and `input` carry whether it is
                // checked (LLP 1069.001 D4); a text field's, its text.
                let control = contract_syntax::input_control(tag, attrs);
                let checkbox = control == Some("checkbox");
                // A file input's `change` carries the picked files (LLP
                // 1069.002 D3); its `cancel`, nothing.
                let file = control == Some("file");
                // A range's carry its number (LLP 1069.001 D4).
                let range = control == Some("range");
                // A bound `type` that could name a control is lowering's
                // refusal (`lower-input-type`), not a payload guessed here;
                // a choice between text fields' types carries text.
                let bound_type = tag == "input"
                    && attrs.iter().any(|a| {
                        a.name == "type"
                            && !matches!(a.value, Expr::Str(..))
                            && !contract_syntax::text_input_type(&a.value)
                    });
                for a in attrs {
                    if matches!(
                        a.name.as_str(),
                        "press"
                            | "change"
                            | "input"
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
                            | "pointerdown"
                            | "pointerup"
                            | "pointermove"
                            | "copy"
                            | "cut"
                            | "paste"
                            | "selectionchange"
                            | "beforeunload"
                            | "wheel"
                            | "drop"
                            | "swiperight"
                            | "refresh"
                            | "reachstart"
                            | "reachend"
                            | "scroll"
                            | "panrelease"
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
                            | "cancel"
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
                            // A parameter left to this handler to type has
                            // nothing to type it: said here, at the `type`,
                            // not as a parameter nothing calls.
                            if bound_type
                                && matches!(a.name.as_str(), "change" | "input")
                                && args.len() < ct.actions[ai].len()
                                && ct.actions[ai].last().is_some_and(|t| !t.is_complete())
                            {
                                return err(
                                    "type-cannot-infer",
                                    format!(
                                        "cannot infer what `{}=` carries to `{name}`: this `input`'s `type` is neither a literal nor a choice between text fields' (`shown ? \"text\" : \"password\"`)",
                                        a.name
                                    ),
                                    attrs
                                        .iter()
                                        .find(|t| t.name == "type")
                                        .map_or(a.span, |t| t.span),
                                );
                            }
                            // Event payloads: change/input/key/message are
                            // strings (a checkbox's are bools); hover is
                            // whether the pointer is over.
                            let mut payload = match a.name.as_str() {
                                "change" | "input" if bound_type => vec![],
                                "change" | "input" if checkbox => vec![Ty::Bool],
                                "change" | "input" if range => vec![Ty::Number],
                                "change" | "input" if file => {
                                    vec![Ty::List(Box::new(Ty::Record("Picked".into())))]
                                }
                                "change" | "input" | "key" | "message" | "navigate" | "error" => {
                                    vec![Ty::String]
                                }
                                "timeupdate" | "durationchange" => vec![Ty::Number],
                                "hover" => vec![Ty::Bool],
                                "select" => vec![Ty::Record("MarkdownSelection".into())],
                                "scroll" | "panrelease" => vec![Ty::Number, Ty::Number],
                                _ => vec![],
                            };
                            // Then the event's record, when the action
                            // declares one more parameter (`event_record`)
                            // of its type or none: another type there is an
                            // argument left unbound, analysis's arity
                            // refusal (`handler_accepts`).
                            if let Some(record) = crate::event_record(&a.name) {
                                let n = args.len() + payload.len() + 1;
                                let fits = ct.actions[ai].len() == n
                                    && match &ct.actions[ai][n - 1] {
                                        Ty::Unknown => true,
                                        Ty::Record(r) => r == record,
                                        _ => false,
                                    };
                                if fits {
                                    payload.push(Ty::Record(record.into()));
                                }
                            }
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
            | Expr::NamedArg(_, x, _)
            | Expr::Typed(x, _, _) => names(x, out),
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
            Expr::Arrow { body, .. } => names(body, out),
            Expr::Template(parts, _) => parts.iter().for_each(|p| {
                if let TemplatePart::Expr(x) = p {
                    names(x, out)
                }
            }),
            Expr::List(items, _) => items.iter().for_each(|x| names(x, out)),
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

/// The first `[]` in `e`, whose element type nothing fixed when `e` typed
/// as a `list<?>`.
fn empty_list_in(e: &Expr) -> Option<Span> {
    match e {
        Expr::List(items, span) if items.is_empty() => Some(*span),
        Expr::List(items, _) => items.iter().find_map(empty_list_in),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(..) | Expr::Ident(..) => {
            None
        }
        Expr::Template(parts, _) => parts.iter().find_map(|p| match p {
            TemplatePart::Expr(x) => empty_list_in(x),
            _ => None,
        }),
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => empty_list_in(x),
        Expr::Call(_, args, _) => args.iter().find_map(empty_list_in),
        Expr::Binary(_, a, b, _) => empty_list_in(a).or_else(|| empty_list_in(b)),
        Expr::Ternary(a, b, c, _) => empty_list_in(a)
            .or_else(|| empty_list_in(b))
            .or_else(|| empty_list_in(c)),
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => empty_list_in(subject)
            .or_else(|| empty_list_in(some))
            .or_else(|| empty_list_in(none)),
        Expr::Let { value, body, .. } => empty_list_in(value).or_else(|| empty_list_in(body)),
        Expr::Arrow { body, .. } => empty_list_in(body),
    }
}

/// The first name in `e` the component declares: state a placeholder's
/// arguments may not read (LLP 1048.003 D6).
fn reads_state(e: &Expr, scope: &Scope) -> Option<(String, Span)> {
    first_free(e, &|_| false, &|name| scope.lookup(name).is_some())
}

/// @ref LLP 1088 D4 — a state's initializer runs before any resource
/// answers and before any derive, so it reads only props, injects and the
/// states declared above it. A name it reads that the component declares
/// otherwise is refused as what it is; an unknown name keeps its
/// near-name suggestion.
pub(crate) fn initializer_scope(
    c: &Component,
    i: usize,
    scope: &Scope,
    shapes: &Shapes,
) -> Option<TypeError> {
    let state = c.states[i].name.split('#').next().unwrap_or_default();
    let declared = |name: &str| {
        c.states[i..].iter().any(|s| s.name == name)
            || c.derives.iter().any(|d| d.name == name)
            || c.resources.iter().any(|r| r.name == name)
            || c.mutations.iter().any(|m| m.name == name)
            || c.actions.iter().any(|a| a.name == name)
    };
    // A call's name resolves as `infer` resolves it: `failed`/`pending`, a
    // record's constructor and a file `fn` come before the component's
    // names, and of those only an action is callable; any other name a
    // call makes is the roster's (`state length = length("abc")`).
    let calls = |name: &str| {
        !matches!(name, "failed" | "pending")
            && !crate::records::is_record_call(name, shapes)
            && !shapes.fns.contains_key(name)
            && c.actions.iter().any(|a| a.name == name)
            && scope.lookup(name).is_none()
    };
    let (name, span) = first_free(&c.states[i].expr, &calls, &|name| {
        scope.lookup(name).is_none() && declared(name)
    })?;
    let shown = name.split('#').next().unwrap_or_default();
    let reads = "a state's initializer runs before any resource answers, and reads only props, injects and earlier states";
    let message = if c.resources.iter().any(|r| r.name == name) {
        let path = member_path(&c.states[i].expr, &name).unwrap_or_else(|| shown.to_string());
        format!(
            "`{shown}` is a resource; {reads}. Derive it from `{shown}`, or keep per-row state in a component used inside `each … in {path}`"
        )
    } else if c.derives.iter().any(|d| d.name == name) {
        format!(
            "`{shown}` is a derive; {reads}. Make `{state}` a derive too, or start it from a value and write it in an action"
        )
    } else if c.mutations.iter().any(|m| m.name == name) {
        format!("`{shown}` is a mutation; {reads}. Write its answer into `{state}` from the mutation's `then` action")
    } else if c.actions.iter().any(|a| a.name == name) {
        format!("`{shown}` is an action; {reads}")
    } else if c.states[i].name == name {
        format!("`{shown}` is the state this initializer starts; {reads}")
    } else {
        format!("`{shown}` is declared after `{state}`; {reads}: declare `{shown}` above `{state}`")
    };
    Some(TypeError {
        id: "type-initializer-scope",
        message,
        span,
    })
}

/// The longest member chain in `e` rooted at `name`, as written (`board.cols`).
fn member_path(e: &Expr, name: &str) -> Option<String> {
    fn chain(e: &Expr) -> Option<String> {
        match e {
            Expr::Ident(n, _) => Some(n.split('#').next().unwrap_or_default().to_string()),
            Expr::Member(inner, field, _) => chain(inner).map(|c| format!("{c}.{field}")),
            _ => None,
        }
    }
    fn root(e: &Expr) -> Option<&str> {
        match e {
            Expr::Ident(n, _) => Some(n),
            Expr::Member(inner, _, _) => root(inner),
            _ => None,
        }
    }
    let mut best: Option<String> = None;
    let mut visit = |x: &Expr| {
        if let Some(c) = chain(x).filter(|_| matches!(x, Expr::Member(..)) && root(x) == Some(name))
        {
            if best.as_ref().is_none_or(|b| c.len() > b.len()) {
                best = Some(c);
            }
        }
    };
    walk_exprs(e, &mut visit);
    best
}

/// Visit `e` and every expression inside it.
fn walk_exprs(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match e {
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(..) | Expr::Ident(..) => {}
        Expr::Template(parts, _) => parts.iter().for_each(|p| {
            if let TemplatePart::Expr(x) = p {
                walk_exprs(x, f)
            }
        }),
        Expr::List(items, _) => items.iter().for_each(|a| walk_exprs(a, f)),
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => walk_exprs(x, f),
        Expr::Call(_, args, _) => args.iter().for_each(|a| walk_exprs(a, f)),
        Expr::Binary(_, a, b, _) => {
            walk_exprs(a, f);
            walk_exprs(b, f);
        }
        Expr::Ternary(a, b, c, _) => {
            walk_exprs(a, f);
            walk_exprs(b, f);
            walk_exprs(c, f);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            walk_exprs(subject, f);
            walk_exprs(some, f);
            walk_exprs(none, f);
        }
        Expr::Let { value, body, .. } => {
            walk_exprs(value, f);
            walk_exprs(body, f);
        }
        Expr::Arrow { body, .. } => walk_exprs(body, f),
    }
}

/// The first free name in `e` (not bound by a `let`, a `match` or an arrow
/// inside it) that `hit` accepts, or the first name a call is made by that
/// `calls` accepts, with where.
fn first_free(
    e: &Expr,
    calls: &dyn Fn(&str) -> bool,
    hit: &dyn Fn(&str) -> bool,
) -> Option<(String, Span)> {
    fn walk(
        e: &Expr,
        calls: &dyn Fn(&str) -> bool,
        hit: &dyn Fn(&str) -> bool,
        bound: &mut Vec<String>,
    ) -> Option<(String, Span)> {
        let within = |name: &str, x: &Expr, bound: &mut Vec<String>| {
            bound.push(name.to_string());
            let found = walk(x, calls, hit, bound);
            bound.pop();
            found
        };
        match e {
            Expr::Ident(name, span) => {
                (!bound.contains(name) && hit(name)).then(|| (name.clone(), *span))
            }
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(..) => None,
            Expr::List(items, _) => items.iter().find_map(|a| walk(a, calls, hit, bound)),
            Expr::Template(parts, _) => parts.iter().find_map(|p| match p {
                TemplatePart::Expr(x) => walk(x, calls, hit, bound),
                _ => None,
            }),
            Expr::Some(x, _)
            | Expr::Member(x, _, _)
            | Expr::NamedArg(_, x, _)
            | Expr::Typed(x, _, _)
            | Expr::Unary(_, x, _) => walk(x, calls, hit, bound),
            Expr::Call(name, args, span) => {
                if !bound.contains(name) && calls(name) {
                    return Some((name.clone(), *span));
                }
                args.iter().find_map(|a| walk(a, calls, hit, bound))
            }
            Expr::Binary(_, a, b, _) => {
                walk(a, calls, hit, bound).or_else(|| walk(b, calls, hit, bound))
            }
            Expr::Ternary(a, b, c, _) => walk(a, calls, hit, bound)
                .or_else(|| walk(b, calls, hit, bound))
                .or_else(|| walk(c, calls, hit, bound)),
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => walk(subject, calls, hit, bound)
                .or_else(|| within(var, some, bound))
                .or_else(|| walk(none, calls, hit, bound)),
            Expr::Let {
                name, value, body, ..
            } => walk(value, calls, hit, bound).or_else(|| within(name, body, bound)),
            Expr::Arrow { params, body, .. } => {
                bound.extend(params.iter().cloned());
                let found = walk(body, calls, hit, bound);
                bound.truncate(bound.len() - params.len());
                found
            }
        }
    }
    walk(e, calls, hit, &mut Vec::new())
}
