//! Lowering: the typed, analyzed AST → a plan.
//!
//! @ref LLP 1004 D2 (tables and bytecode, byte-identically, in the kernel's
//! vocabulary) / D4 (a `resource` names a source and its arguments) / D6
//! (the corpus compares canonical bytes)
//!
//! Three steps. **Inline**: every component use is replaced by the used
//! component's view with its props substituted by the use's argument
//! expressions and its bound names renamed apart, so the plan has one
//! component and no prop table — a child component is a view over its props
//! (LLP 1004 D3 as scoped by analysis). **Tables**: shapes, slots, derives,
//! resources, actions, timers, then the view as nodes, regions, arms,
//! bindings, and handlers through the tag/attribute table ([`tags`]).
//! **Code**: every expression through one assembler ([`expr`]).
//!
//! Row order is source order everywhere, so two compilations of one source
//! are byte-identical.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod expr;
pub mod tags;

use contract_analyze::Analysis;
use contract_syntax::{Attr, Expr, File, Node, Span, Stmt};
use contract_types::{Ref, Scope, Ty, Types};
use exact_plan::asm::Asm;
use exact_plan::builder::PlanBuilder;
use exact_plan::{
    ArmsId, BindingKind, BindingsRow, Code, EventKind, NodesId, Plan, RegionKind, TypeKind,
    TypesId, Value,
};
use std::collections::BTreeMap;

/// A typed rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    /// Stable id.
    pub id: &'static str,
    /// What went wrong.
    pub message: String,
    /// Where.
    pub span: Span,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] {}", self.span, self.id, self.message)
    }
}

pub(crate) fn err<T>(
    id: &'static str,
    message: impl Into<String>,
    span: Span,
) -> Result<T, LowerError> {
    Err(LowerError {
        id,
        message: message.into(),
        span,
    })
}

/// The compiler identity a plan carries: the crate version folded with the
/// configuration digest (there is no configuration yet).
pub fn compiler_identity() -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in concat!("contract-lower ", env!("CARGO_PKG_VERSION")).bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

pub(crate) struct Lowerer<'a> {
    pub b: PlanBuilder,
    pub types: &'a Types,
    pub root: &'a contract_syntax::Component,
    pub ty_ids: BTreeMap<String, TypesId>,
    pub slots: Vec<exact_plan::SlotsId>,
    pub derives: Vec<exact_plan::DerivesId>,
    pub resources: Vec<exact_plan::ResourcesId>,
    pub actions: Vec<exact_plan::ActionsId>,
}

/// Lower a checked file to a plan.
pub fn lower(file: &File, types: &Types, _analysis: &Analysis) -> Result<Plan, LowerError> {
    let root = &file.components[0];
    let root_types = &types.components[0];
    let mut l = Lowerer {
        b: PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, compiler_identity()),
        types,
        root,
        ty_ids: BTreeMap::new(),
        slots: Vec::new(),
        derives: Vec::new(),
        resources: Vec::new(),
        actions: Vec::new(),
    };
    // Shapes first, in declaration order, so type ids are stable.
    for s in &file.shapes {
        l.ty_id(&Ty::Record(s.name.clone()))?;
    }
    // Ids for every declaration before any body, so bodies may reference any of them.
    for (i, s) in root.states.iter().enumerate() {
        let ty = l.ty_id(&root_types.slots[i])?;
        let placeholder = l.b.constant(&Value::Unit);
        let id = l.b.slot(&s.name, ty, placeholder);
        l.slots.push(id);
    }
    for (i, d) in root.derives.iter().enumerate() {
        let ty = l.ty_id(&root_types.derives[i])?;
        let placeholder = l.b.constant(&Value::Unit);
        let id = l.b.derive(&d.name, ty, placeholder);
        l.derives.push(id);
    }
    for (i, r) in root.resources.iter().enumerate() {
        let ty = l.ty_id(&root_types.resources[i])?;
        let id = l.b.resource(&r.name, &r.source, &[], ty, None);
        l.resources.push(id);
    }
    for (i, a) in root.actions.iter().enumerate() {
        let params: Vec<(String, TypesId)> = a
            .params
            .iter()
            .enumerate()
            .map(|(pi, p)| Ok((p.name.clone(), l.ty_id(&root_types.actions[i][pi])?)))
            .collect::<Result<_, LowerError>>()?;
        let params_ref: Vec<(&str, TypesId)> =
            params.iter().map(|(n, t)| (n.as_str(), *t)).collect();
        let writes: Vec<exact_plan::SlotsId> = a
            .writes
            .iter()
            .map(|(w, _)| l.slots[root.states.iter().position(|s| &s.name == w).unwrap()])
            .collect();
        let placeholder = l.b.constant(&Value::Unit);
        let id = l.b.action(&a.name, &params_ref, &writes, placeholder);
        l.actions.push(id);
    }
    // Bodies.
    let scope = types.component_scope(root, root_types);
    for (i, s) in root.states.iter().enumerate() {
        let code = l.expr_code(&s.expr, &scope, 0)?;
        l.b.set_slot_init(l.slots[i], code);
    }
    for (i, d) in root.derives.iter().enumerate() {
        let code = l.expr_code(&d.expr, &scope, 0)?;
        l.b.set_derive_body(l.derives[i], code);
    }
    for (i, r) in root.resources.iter().enumerate() {
        let mut args = Vec::new();
        for a in &r.args {
            args.push(l.expr_code(a, &scope, 0)?);
        }
        let range = l.b.args(&args);
        l.b.set_resource_args(l.resources[i], range);
    }
    for (i, a) in root.actions.iter().enumerate() {
        let mut inner = scope.clone();
        inner.push(
            a.params
                .iter()
                .enumerate()
                .map(|(pi, p)| {
                    (
                        p.name.clone(),
                        Ref::Param(pi as u32),
                        root_types.actions[i][pi].clone(),
                    )
                })
                .collect(),
        );
        let mut asm = Asm::new();
        let mut locals = 0u16;
        for stmt in &a.body {
            match stmt {
                Stmt::Assign { target, expr, .. } => {
                    expr::compile(&mut l, &mut asm, expr, &inner, &mut locals)?;
                    let slot = l.slots[root.states.iter().position(|s| &s.name == target).unwrap()];
                    asm.store_slot(slot);
                }
                Stmt::Command { name, args, .. } => {
                    for arg in args {
                        expr::compile(&mut l, &mut asm, arg, &inner, &mut locals)?;
                    }
                    let name = l.b.str(name);
                    asm.command(name, args.len() as u16);
                }
            }
        }
        let code = l.b.code(asm);
        l.b.set_action_body(l.actions[i], code);
    }
    for t in &root.tasks {
        let Expr::Number(ms, _) = &t.every.0 else {
            return err(
                "lower-timer-literal",
                "`every` needs a literal number of milliseconds",
                t.every.2,
            );
        };
        if !(ms.is_finite() && ms.fract() == 0.0 && *ms >= 1.0 && *ms <= u32::MAX as f64) {
            return err(
                "lower-timer-interval",
                format!("`every` needs a whole number of milliseconds, at least 1; given {ms}"),
                t.every.2,
            );
        }
        let action = l.actions[root
            .actions
            .iter()
            .position(|a| a.name == t.every.1)
            .unwrap()];
        l.b.timer(*ms as u32, action);
    }
    // The view, inlined.
    let view = contract_syntax::inline(file).map_err(|e| LowerError {
        id: e.id,
        message: e.message,
        span: e.span,
    })?;
    if view.len() != 1 {
        return err(
            "lower-one-root",
            format!(
                "the root view must be exactly one node; found {}",
                view.len()
            ),
            root.span,
        );
    }
    l.nodes(&view, None, None, &scope, 0)?;
    l.b.finish().map_err(|e| LowerError {
        id: "lower-invalid-plan",
        message: format!("{e:?}"),
        span: root.span,
    })
}

impl<'a> Lowerer<'a> {
    /// The plan type id for a checked type.
    pub(crate) fn ty_id(&mut self, t: &Ty) -> Result<TypesId, LowerError> {
        Ok(match t {
            Ty::Number => self.b.primitive(TypeKind::Number),
            Ty::String => self.b.primitive(TypeKind::String),
            Ty::Bool => self.b.primitive(TypeKind::Bool),
            Ty::Unit => self.b.primitive(TypeKind::Unit),
            Ty::Option(inner) => {
                let e = self.ty_id(inner)?;
                self.b.option(e)
            }
            Ty::List(inner) => {
                let e = self.ty_id(inner)?;
                self.b.list(e)
            }
            Ty::Record(name) => {
                if let Some(id) = self.ty_ids.get(name) {
                    return Ok(*id);
                }
                let fields = self.types.shapes.map.get(name).cloned().unwrap_or_default();
                let mut resolved = Vec::new();
                for (fname, fty) in &fields {
                    resolved.push((fname.clone(), self.ty_id(fty)?));
                }
                let refs: Vec<(&str, TypesId)> =
                    resolved.iter().map(|(n, t)| (n.as_str(), *t)).collect();
                let id = self.b.record(name, &refs);
                self.ty_ids.insert(name.clone(), id);
                id
            }
            Ty::Action(_) | Ty::Unknown => {
                return err(
                    "lower-unlowerable-type",
                    format!("`{t}` has no plan type"),
                    Span::default(),
                )
            }
        })
    }

    pub(crate) fn expr_code(
        &mut self,
        e: &Expr,
        scope: &Scope,
        locals: u16,
    ) -> Result<Code, LowerError> {
        let mut asm = Asm::new();
        let mut locals = locals;
        expr::compile(self, &mut asm, e, scope, &mut locals)?;
        Ok(self.b.code(asm))
    }

    /// Lower sibling nodes under (`parent`, `arm`).
    fn nodes(
        &mut self,
        nodes: &[Node],
        parent: Option<NodesId>,
        arm: Option<ArmsId>,
        scope: &Scope,
        locals: u16,
    ) -> Result<(), LowerError> {
        for (order, n) in nodes.iter().enumerate() {
            self.node(n, parent, arm, order as u32, scope, locals)?;
        }
        Ok(())
    }

    fn node(
        &mut self,
        n: &Node,
        parent: Option<NodesId>,
        arm: Option<ArmsId>,
        order: u32,
        scope: &Scope,
        locals: u16,
    ) -> Result<(), LowerError> {
        match n {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
            } => {
                let Some(t) = tags::tag(tag) else {
                    return err("lower-unknown-tag", format!("unknown tag `{tag}`"), *span);
                };
                let mut bindings: Vec<BindingsRow> = Vec::new();
                let mut handlers: Vec<(EventKind, exact_plan::ActionsId, Vec<Code>)> = Vec::new();
                let mut surface: Option<exact_plan::SurfacesId> = None;
                for (style, value) in &t.fixed_styles {
                    // A fixed row is an enum's name or a number in points.
                    let v = match value.parse::<f64>() {
                        Ok(n) => Value::Number(n),
                        Err(_) => Value::str(value),
                    };
                    let code = self.b.constant(&v);
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: *style as u16,
                        expr: code,
                    });
                }
                for (prop, value) in &t.fixed_props {
                    let code = self.b.constant(&Value::str(value));
                    bindings.push(BindingsRow {
                        kind: BindingKind::Prop,
                        id: *prop as u16,
                        expr: code,
                    });
                }
                if let Some(first) = positional.first() {
                    let Some(prop) = t.positional else {
                        return err(
                            "lower-positional",
                            format!("`{tag}` takes no positional argument"),
                            first.span(),
                        );
                    };
                    let code = self.expr_code(first, scope, locals)?;
                    bindings.push(BindingsRow {
                        kind: BindingKind::Prop,
                        id: prop as u16,
                        expr: code,
                    });
                }
                if positional.len() > 1 {
                    return err(
                        "lower-positional",
                        format!("`{tag}` takes at most one positional argument"),
                        positional[1].span(),
                    );
                }
                for a in attrs {
                    self.attr(
                        tag,
                        a,
                        scope,
                        locals,
                        &mut bindings,
                        &mut handlers,
                        &mut surface,
                    )?;
                }
                let handler_refs: Vec<(EventKind, exact_plan::ActionsId, &[Code])> = handlers
                    .iter()
                    .map(|(e, a, c)| (*e, *a, c.as_slice()))
                    .collect();
                if !children.is_empty() && !t.node_type.can_hold_children() {
                    return err(
                        "lower-leaf-children",
                        format!("`{tag}` cannot hold children"),
                        children[0].span(),
                    );
                }
                if t.node_type.is_text_leaf() {
                    let run = |c: &Node| matches!(c, Node::Element { tag, .. } if tag == "text");
                    if let Some(bad) = children.iter().find(|c| !run(c)) {
                        return err(
                            "lower-leaf-children",
                            "`text` may hold only `text` runs",
                            bad.span(),
                        );
                    }
                }
                let id = self.b.node(
                    t.node_type as u8,
                    parent,
                    arm,
                    order,
                    &bindings,
                    &handler_refs,
                    surface,
                );
                self.nodes(children, Some(id), arm, scope, locals)
            }
            Node::Use { name, span, .. } => err(
                "lower-uninlined-use",
                format!("component `{name}` was not inlined"),
                *span,
            ),
            Node::When {
                cond,
                then,
                otherwise,
                ..
            } => {
                let subject = self.expr_code(cond, scope, locals)?;
                let unit = self.b.constant(&Value::Unit);
                let (_r, arms) =
                    self.b
                        .region(RegionKind::When, parent, arm, order, subject, unit, 2);
                let mut inner = scope.clone();
                inner.push_region(None);
                self.nodes(then, None, Some(arms[0]), &inner, locals)?;
                self.nodes(otherwise, None, Some(arms[1]), &inner, locals)
            }
            Node::Each {
                var,
                list,
                key,
                body,
                ..
            } => {
                let subject = self.expr_code(list, scope, locals)?;
                let item_ty = match contract_types::infer(list, scope, &self.types.shapes) {
                    Ok(Ty::List(t)) => *t,
                    _ => Ty::Unknown,
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), item_ty)));
                let key = self.expr_code(key, &inner, locals)?;
                let (_r, arms) =
                    self.b
                        .region(RegionKind::Each, parent, arm, order, subject, key, 1);
                self.nodes(body, None, Some(arms[0]), &inner, locals)
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let code = self.expr_code(subject, scope, locals)?;
                let bound_ty = match contract_types::infer(subject, scope, &self.types.shapes) {
                    Ok(Ty::Option(t)) => *t,
                    _ => Ty::Unknown,
                };
                let unit = self.b.constant(&Value::Unit);
                let (_r, arms) =
                    self.b
                        .region(RegionKind::Match, parent, arm, order, code, unit, 2);
                let mut some_scope = scope.clone();
                some_scope.push_region(Some((some.0.clone(), Ref::Bound(0), bound_ty)));
                self.nodes(&some.1, None, Some(arms[0]), &some_scope, locals)?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                self.nodes(none, None, Some(arms[1]), &none_scope, locals)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn attr(
        &mut self,
        tag: &str,
        a: &Attr,
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
        handlers: &mut Vec<(EventKind, exact_plan::ActionsId, Vec<Code>)>,
        surface: &mut Option<exact_plan::SurfacesId>,
    ) -> Result<(), LowerError> {
        let Some(target) = tags::attr(&a.name) else {
            return err(
                "lower-unknown-attr",
                format!("`{tag}` has no attribute `{}`", a.name),
                a.span,
            );
        };
        match target {
            tags::AttrTarget::Flex => {
                // CSS `flex: <n>` is `<n> 1 0%`: grow n, shrink 1, basis 0%.
                let grow = self.expr_code(&a.value, scope, locals)?;
                let one = self.b.constant(&Value::Number(1.0));
                let zero_basis = self.b.constant(&Value::str("0%"));
                for (row, code) in [
                    ("flex_grow", grow),
                    ("flex_shrink", one),
                    ("flex_basis", zero_basis),
                ] {
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: exact_kernel::StyleId::from_name(row).unwrap() as u16,
                        expr: code,
                    });
                }
            }
            tags::AttrTarget::Styles(rows) => {
                let code = self.expr_code(&a.value, scope, locals)?;
                for row in rows {
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: row as u16,
                        expr: code,
                    });
                }
            }
            tags::AttrTarget::Prop(prop) => {
                let code = self.expr_code(&a.value, scope, locals)?;
                bindings.push(BindingsRow {
                    kind: BindingKind::Prop,
                    id: prop as u16,
                    expr: code,
                });
            }
            tags::AttrTarget::Surface => {
                if tag != "canvas" {
                    return err(
                        "lower-surface-tag",
                        format!("`surface` belongs to `canvas`, not `{tag}`"),
                        a.span,
                    );
                }
                let (name, args): (&str, &[Expr]) = match &a.value {
                    Expr::Ident(n, _) => (n, &[]),
                    Expr::Call(n, args, _) => (n, args),
                    _ => {
                        return err(
                            "lower-surface",
                            "a surface is a name or `name(args)`",
                            a.span,
                        )
                    }
                };
                let mut codes = Vec::new();
                for arg in args {
                    codes.push(self.expr_code(arg, scope, locals)?);
                }
                *surface = Some(self.b.surface(name, &codes));
            }
            tags::AttrTarget::Handler(event) => {
                let (name, args): (&str, &[Expr]) = match &a.value {
                    Expr::Ident(n, _) => (n, &[]),
                    Expr::Call(n, args, _) => (n, args),
                    _ => {
                        return err(
                            "lower-handler",
                            "a handler is an action name or `action(args)`",
                            a.span,
                        )
                    }
                };
                let Some(ai) = self.root.actions.iter().position(|x| x.name == name) else {
                    return err(
                        "lower-unknown-action",
                        format!("`{name}` is not an action of the root"),
                        a.span,
                    );
                };
                let mut codes = Vec::new();
                for arg in args {
                    codes.push(self.expr_code(arg, scope, locals)?);
                }
                let kind = match event {
                    "press" => EventKind::Press,
                    "change" => EventKind::Change,
                    "hover" => EventKind::Hover,
                    "focus" => EventKind::Focus,
                    "blur" => EventKind::Blur,
                    _ => EventKind::Key,
                };
                handlers.push((kind, self.actions[ai], codes));
            }
        }
        Ok(())
    }
}
