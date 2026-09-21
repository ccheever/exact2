//! Lowering: the typed, analyzed AST → a plan.
//!
//! @ref LLP 1004 D2 (tables and bytecode, byte-identically, in the kernel's
//! vocabulary) / D4 (a `resource` names a source and its arguments) / D6
//! (the corpus compares canonical bytes)
//!
//! Input: the type-checked expansion, where every component use is replaced by the used
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

mod collection;
pub mod expr;
mod fonts;
mod media;
mod routes;
mod sites;
pub mod tags;
pub use sites::{Declared, NodeSite, Origin, Sites};

use contract_analyze::Analysis;
use contract_syntax::{Attr, Expr, File, FnDecl, Node, Span, Stmt, UnOp};
use contract_types::{Checked, Ref, Scope, Ty, Types};
use exact_kernel::{PropId, StyleId, StyleProps, StyleValue, StyleValueError};
use exact_plan::asm::Asm;
use exact_plan::builder::PlanBuilder;
use exact_plan::{
    ArmsId, BindingKind, BindingsRow, Code, EventKind, NodesId, Plan, RegionKind, StacksId,
    TypeKind, TypesId, Value,
};
use std::collections::BTreeMap;
use std::path::Path;

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

/// A literal, as an author wrote it, for a message.
fn literal_text(e: &Expr) -> String {
    match e {
        Expr::Number(n, _) => format!("{n}"),
        Expr::Str(s, _) => format!("\"{s}\""),
        Expr::Bool(b, _) => format!("{b}"),
        _ => "…".into(),
    }
}

fn numeric_literal(e: &Expr) -> Option<f64> {
    match e {
        Expr::Number(n, _) => Some(*n),
        Expr::Unary(UnOp::Neg, inner, _) => numeric_literal(inner).map(|n| -n),
        _ => None,
    }
}

fn whole_i64(n: f64) -> bool {
    n.is_finite() && n.fract() == 0.0 && n >= i64::MIN as f64 && n < -(i64::MIN as f64)
}

/// The kernel's refusal of a style value, in an author's words.
fn describe(e: &StyleValueError) -> String {
    match e {
        StyleValueError::WrongKind { expected, .. } => format!("expected {expected}"),
        StyleValueError::UnknownEnumValue { style } => format!(
            "expected one of {}",
            style.enum_names().iter().map(|name| format!("{name:?}")).collect::<Vec<_>>().join(", ")
        ),
        StyleValueError::AutoNotAdmitted { .. } => "`auto` is not admitted here".into(),
        StyleValueError::OutOfRange { .. } => "out of the row's range".into(),
        StyleValueError::BadColor { .. } => "a color is `#rgb`, `#rrggbb`, or `#rrggbbaa`".into(),
        StyleValueError::BadShapeOutside { .. } => "expected none, circle(), ellipse(), inset() with one round radius, or polygon() with at most 64 vertices; lengths are points/px or percentages".into(),
        StyleValueError::BadClipPath { .. } => "expected none or path() with explicit absolute M/L/Q/C/Z commands and separated finite coordinates".into(),
        StyleValueError::BadTransition { .. } => "not a CSS `transition` shorthand".into(),
        StyleValueError::Unsupported { .. } => "this row has no dynamic form".into(),
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
    sites: Option<Sites>,
    pub types: &'a Types,
    pub root: &'a contract_syntax::Component,
    pub ty_ids: BTreeMap<String, TypesId>,
    pub slots: Vec<exact_plan::SlotsId>,
    pub derives: Vec<exact_plan::DerivesId>,
    pub resources: Vec<exact_plan::ResourcesId>,
    pub mutations: Vec<exact_plan::MutationsId>,
    /// Each mutation's `option<T>` slot, by mutation index.
    pub mutation_slots: Vec<exact_plan::SlotsId>,
    pub actions: Vec<exact_plan::ActionsId>,
    /// The file's `style` declarations, by name (LLP 1017 P6).
    pub styles: BTreeMap<String, Vec<Attr>>,
    /// The file's `fn` declarations, by name, expanded inline at each call
    /// (LLP 1017 P5).
    pub fns: BTreeMap<String, FnDecl>,
    /// The region each `each` lowered to, by the inliner's tag (LLP 1017 P4c).
    pub each_regions: BTreeMap<u32, exact_plan::RegionsId>,
    /// The item/binding scope at each expanded `each`, for row-slot initializers.
    pub each_scopes: BTreeMap<u32, Scope>,
    /// Every generic and declared family name to its stack id.
    pub font_stacks: BTreeMap<String, StacksId>,
    /// Declared families, for the literal weight/style synthesis diagnostic.
    declared_fonts: BTreeMap<String, DeclaredFont>,
    /// How many `fn` bodies are being expanded right now (a guard; the type
    /// pass already refuses a cycle).
    pub fn_depth: u32,
}

#[derive(Debug, Clone)]
struct DeclaredFont {
    stack: StacksId,
    faces: Vec<(u16, bool)>,
}

#[derive(Debug, Clone)]
struct FontUse {
    font: DeclaredFont,
    /// `None` when `font-style` computes and the compiler cannot inspect it.
    italic: Option<bool>,
}

/// Lower a checked file to a plan.
pub fn lower(
    checked: &Checked<'_>,
    _analysis: &Analysis,
    asset_root: Option<&Path>,
) -> Result<Plan, LowerError> {
    lower_with_sites(checked, _analysis, asset_root, false).map(|(plan, _)| plan)
}

/// Lower with development-only source sites, separate from the plan bytes.
pub fn lower_mapped(
    checked: &Checked<'_>,
    analysis: &Analysis,
    asset_root: Option<&Path>,
) -> Result<(Plan, Sites), LowerError> {
    if checked.expanded.instances.is_empty() {
        return err(
            "lower-source-sites",
            "mapped lowering needs check_mapped source provenance",
            checked.expanded.root.span,
        );
    }
    lower_with_sites(checked, analysis, asset_root, true)
        .map(|(plan, sites)| (plan, sites.expect("sites requested")))
}

fn lower_with_sites(
    checked: &Checked<'_>,
    _analysis: &Analysis,
    asset_root: Option<&Path>,
    capture_sites: bool,
) -> Result<(Plan, Option<Sites>), LowerError> {
    // Keep the exact expansion whose root and row slots inference checked.
    let Checked {
        file,
        types,
        expanded: ex,
    } = checked;
    let root = &ex.root;
    let root_types = &types.components[0];
    let mut l = Lowerer {
        b: PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, compiler_identity()),
        sites: capture_sites.then(|| Sites::declared(ex)),
        types,
        root,
        ty_ids: BTreeMap::new(),
        slots: Vec::new(),
        derives: Vec::new(),
        resources: Vec::new(),
        mutations: Vec::new(),
        mutation_slots: Vec::new(),
        actions: Vec::new(),
        styles: BTreeMap::new(),
        fns: file
            .fns
            .iter()
            .map(|f| (f.name.clone(), f.clone()))
            .collect(),
        fn_depth: 0,
        each_regions: BTreeMap::new(),
        each_scopes: BTreeMap::new(),
        font_stacks: BTreeMap::new(),
        declared_fonts: BTreeMap::new(),
    };
    l.declare_fonts(file, asset_root)?;
    // Styles: rows only, literal only (the parser holds the second), by name.
    for s in &file.styles {
        for a in &s.attrs {
            match tags::attr(&a.name) {
                Some(tags::AttrTarget::Styles(_)) | Some(tags::AttrTarget::Flex) => {}
                Some(_) => {
                    return err(
                        "lower-style-attr",
                        format!(
                            "`{}` cannot be in `style {}`: a style holds style rows only — no `testId`, no handlers, no props",
                            a.name, s.name
                        ),
                        a.span,
                    )
                }
                None => {
                    let hint = tags::renamed(&a.name)
                        .map(|n| format!("; `{}` is spelled `{n}` here", a.name))
                        .or_else(|| tags::similar_attr(&a.name, true).map(|n| format!("; did you mean `{n}`?")))
                        .unwrap_or_default();
                    return err(
                        "lower-unknown-attr",
                        format!("`style {}` has no attribute `{}`{hint}", s.name, a.name),
                        a.span,
                    );
                }
            }
        }
        if l.styles.insert(s.name.clone(), s.attrs.clone()).is_some() {
            return err(
                "lower-style-duplicate",
                format!("`style {}` is declared twice", s.name),
                s.span,
            );
        }
    }
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
    l.declare_routes(file);
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
    // The seam's signatures (LLP 1027 D2), in name order.
    for (name, (params, ty)) in &root_types.sources {
        let mut ids = Vec::with_capacity(params.len());
        for p in params {
            ids.push(l.ty_id(p)?);
        }
        let ty = l.ty_id(ty)?;
        l.b.source(name, &ids, ty);
    }
    // A mutation is a slot of `option<T>`, `none` at boot, plus its row.
    for (i, m) in root.mutations.iter().enumerate() {
        let t = l.ty_id(&root_types.mutations[i])?;
        let ot = l.ty_id(&Ty::Option(Box::new(root_types.mutations[i].clone())))?;
        let init = l.b.constant(&Value::Option(None));
        let slot = l.b.slot(&m.name, ot, init);
        let id = l.b.mutation(&m.name, slot, t);
        l.mutation_slots.push(slot);
        l.mutations.push(id);
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
            .map(
                |(w, _)| match root.states.iter().position(|s| &s.name == w) {
                    Some(si) => l.slots[si],
                    None => {
                        l.mutation_slots[root.mutations.iter().position(|m| &m.name == w).unwrap()]
                    }
                },
            )
            .collect();
        let placeholder = l.b.constant(&Value::Unit);
        let id = l.b.action(&a.name, &params_ref, &writes, placeholder);
        l.actions.push(id);
    }
    // Bodies.
    let scope = types.component_scope(root, root_types);
    for (i, s) in root.states.iter().enumerate() {
        if ex.owners[i].is_none() && !(i == 0 && file.routes.is_some()) {
            let code = l.expr_code(&s.expr, &scope, 0)?;
            l.b.set_slot_init(l.slots[i], code);
        }
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
            l.stmt(&mut asm, stmt, &inner, &mut locals)?;
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
    // The view, inlined (by `expand`, above).
    let view = &root.view;
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
    if !matches!(view[0], Node::Element { .. }) {
        return err(
            "lower-root-region",
            "the root of a view is a node; a `when`, `each`, or `match` cannot be the root (LLP 1010 §1: a keyed root could not reorder) — put it inside a `column` or a `main`",
            view[0].span(),
        );
    }
    l.nodes(view, None, None, &scope, 0, None)?;
    // Row slots: each lifted state owned by an `each` names its region now
    // that the regions exist (LLP 1017 P4c).
    for (i, owner) in ex.owners.iter().enumerate() {
        if let Some(tag) = owner {
            let region = *l.each_regions.get(tag).ok_or_else(|| LowerError {
                id: "lower-row-slot",
                message: format!(
                    "row slot `{}` names an `each` that was not lowered",
                    root.states[i].name
                ),
                span: root.states[i].span,
            })?;
            let item_scope = l.each_scopes.get(tag).cloned().ok_or_else(|| LowerError {
                id: "lower-row-slot",
                message: format!(
                    "row slot `{}` names an `each` with no item scope",
                    root.states[i].name
                ),
                span: root.states[i].span,
            })?;
            let init = l.expr_code(&root.states[i].expr, &item_scope, 0)?;
            l.b.set_slot_init(l.slots[i], init);
            l.b.set_slot_owner(l.slots[i], region);
        }
    }
    let plan = l.b.finish().map_err(|e| LowerError {
        id: "lower-invalid-plan",
        message: format!("{e:?}"),
        span: root.span,
    })?;
    Ok((plan, l.sites))
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

    /// Lower sibling nodes under (`parent`, `arm`); `parent_tag` is the
    /// nearest enclosing element's tag (a region does not change it).
    #[allow(clippy::too_many_arguments)]
    fn nodes(
        &mut self,
        nodes: &[Node],
        parent: Option<NodesId>,
        arm: Option<ArmsId>,
        scope: &Scope,
        locals: u16,
        parent_tag: Option<&str>,
    ) -> Result<(), LowerError> {
        for (order, n) in nodes.iter().enumerate() {
            self.node(n, parent, arm, order as u32, scope, locals, parent_tag)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        n: &Node,
        parent: Option<NodesId>,
        arm: Option<ArmsId>,
        order: u32,
        scope: &Scope,
        locals: u16,
        parent_tag: Option<&str>,
    ) -> Result<(), LowerError> {
        match n {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
                instance,
            } => {
                let Some(t) = tags::tag(tag) else {
                    return err("lower-unknown-tag", format!("unknown tag `{tag}`"), *span);
                };
                // Two layout refusals the compiler can make without measuring
                // (LLP 1017 P1c; the measured ones are bake's). Conservative:
                // only the case nothing on the path can bound is refused.
                // `class=Name` expands its style's rows first; the node's own
                // attribute of the same name replaces the style's (LLP 1017 P6).
                let mut expanded: Vec<Attr> = Vec::new();
                let mut class_name = None;
                if let Some(c) = attrs.iter().find(|a| a.name == "class") {
                    let Expr::Ident(name, _) = &c.value else {
                        return err(
                            "lower-class-name",
                            "`class=` names a style declared with `style Name`",
                            c.span,
                        );
                    };
                    let Some(style) = self.styles.get(name).cloned() else {
                        return err(
                            "lower-unknown-class",
                            format!("`class={name}`: no `style {name}` in this file"),
                            c.span,
                        );
                    };
                    class_name = Some(name);
                    expanded.extend(
                        style
                            .into_iter()
                            .filter(|s| !attrs.iter().any(|a| a.name == s.name)),
                    );
                }
                let class_len = expanded.len();
                expanded.extend(attrs.iter().filter(|a| a.name != "class").cloned());
                // @ref LLP 1043.000 §3 D1 — dynamic positioning is checked by layout.
                if let Some(wrap) = expanded.iter().find(|a| a.name == "wrap-flow") {
                    if matches!(&wrap.value, Expr::Str(v, _) if v == "both") {
                        let position = expanded.iter().find(|a| a.name == "position");
                        let absolute = t
                            .fixed_styles
                            .iter()
                            .any(|(id, v)| *id == StyleId::PositionType && *v == "absolute");
                        if position.map_or(
                            !absolute,
                            |a| matches!(&a.value, Expr::Str(v, _) if v != "absolute"),
                        ) {
                            return err(
                                "lower-attr-value",
                                "`wrap-flow: both` requires `position: absolute` in exact2 v1",
                                wrap.span,
                            );
                        }
                    }
                }
                tags::validate_list(tag, &expanded, children, *span)?;
                self.check_collection(tag, &expanded, children, *span)?;
                let has =
                    |names: &[&str]| expanded.iter().any(|a| names.contains(&a.name.as_str()));
                let parent_stacks = !matches!(parent_tag, Some("row") | Some("canvas"));
                let clips_y = expanded.iter().any(|a| {
                    a.name == "overflow-y" && matches!(&a.value, Expr::Str(v, _) if v == "hidden")
                });
                if tag == "scroll"
                    && !clips_y
                    && parent_stacks
                    && !has(&["height", "max-height", "flex"])
                {
                    return err(
                        "lower-scroll-unbounded",
                        "`scroll` has no `height`, `max-height`, or `flex`, and its parent stacks it top to bottom, so it will grow with its content and never scroll",
                        *span,
                    );
                }
                if matches!(tag.as_str(), "button" | "link")
                    && children.is_empty()
                    && !has(&[
                        "width",
                        "height",
                        "flex",
                        "padding",
                        "padding-top",
                        "padding-right",
                        "padding-bottom",
                        "padding-left",
                        "min-width",
                        "min-height",
                    ])
                {
                    return err(
                        "lower-zero-size",
                        format!("`{tag}` has no children and no size, so it has zero area and nothing to press: give it children or a size"),
                        *span,
                    );
                }
                // @ref LLP 1038 D8 — only the first root selects navigation.
                if has(&["navigate"])
                    && (parent_tag.is_some()
                        || arm.is_some()
                        || order != 0
                        || !has(&["navigationKey"])
                        || !has(&["navigationBack"]))
                {
                    return err("lower-navigate-root", "`navigate` belongs to the navigation root (navigationKey and navigationBack)", *span);
                }
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
                    self.check_prop_value(tag, first, first.span(), prop, scope)?;
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
                let mut origins = self
                    .sites
                    .as_ref()
                    .map(|_| vec![Origin::Tag; bindings.len()]);
                let font = self.font_use(&expanded)?;
                for (index, a) in expanded.iter().enumerate() {
                    self.attr(
                        tag,
                        a,
                        scope,
                        locals,
                        &mut bindings,
                        &mut handlers,
                        &mut surface,
                        font.as_ref(),
                    )?;
                    if let Some(origins) = &mut origins {
                        let origin = if index < class_len {
                            Origin::Class(class_name.expect("class attribute").clone())
                        } else {
                            Origin::Own
                        };
                        origins.resize(bindings.len(), origin);
                    }
                }
                // Two bindings for one row — a style's and the node's own, a
                // tag's fixed row and an attribute — the last one wins.
                let mut seen: BTreeMap<(u8, u16), usize> = BTreeMap::new();
                let mut deduped: Vec<BindingsRow> = Vec::new();
                let mut deduped_origins = origins.as_ref().map(|_| Vec::new());
                for (index, b) in bindings.drain(..).enumerate() {
                    match seen.get(&(b.kind as u8, b.id)) {
                        Some(&i) => {
                            deduped[i] = b;
                            if let (Some(from), Some(to)) = (&origins, &mut deduped_origins) {
                                to[i] = from[index].clone();
                            }
                        }
                        None => {
                            seen.insert((b.kind as u8, b.id), deduped.len());
                            deduped.push(b);
                            if let (Some(from), Some(to)) = (&origins, &mut deduped_origins) {
                                to.push(from[index].clone());
                            }
                        }
                    }
                }
                let bindings = deduped;
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
                    // Regions have no node of their own: a dynamic Markdown
                    // paragraph's each/when still produces only text runs.
                    fn run(c: &Node) -> bool {
                        match c {
                            Node::Element { tag, .. } => tag == "text",
                            Node::Each { body, .. } => body.iter().all(run),
                            Node::When {
                                then, otherwise, ..
                            } => then.iter().chain(otherwise).all(run),
                            _ => false,
                        }
                    }
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
                if let Some(sites) = &mut self.sites {
                    debug_assert_eq!(sites.nodes.len(), id.0 as usize);
                    sites.nodes.push(sites::node_site(
                        *span,
                        *instance,
                        &bindings,
                        deduped_origins.as_deref().expect("site origins"),
                    ));
                }
                self.nodes(children, Some(id), arm, scope, locals, Some(tag))
            }
            Node::Use { name, span, .. } => err(
                "lower-uninlined-use",
                format!("component `{name}` was not inlined"),
                *span,
            ),
            Node::Provide { span, .. } | Node::Children { span } => err(
                "lower-uninlined-use",
                "`provide` and `children` are inlined away before lowering",
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
                self.nodes(then, None, Some(arms[0]), &inner, locals, parent_tag)?;
                self.nodes(otherwise, None, Some(arms[1]), &inner, locals, parent_tag)
            }
            Node::Each {
                tag,
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
                let (r, arms) =
                    self.b
                        .region(RegionKind::Each, parent, arm, order, subject, key, 1);
                self.each_regions.insert(*tag, r);
                self.each_scopes.insert(*tag, inner.clone());
                self.nodes(body, None, Some(arms[0]), &inner, locals, parent_tag)
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
                self.nodes(
                    &some.1,
                    None,
                    Some(arms[0]),
                    &some_scope,
                    locals,
                    parent_tag,
                )?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                self.nodes(none, None, Some(arms[1]), &none_scope, locals, parent_tag)
            }
        }
    }

    /// Lower one statement of an action body: assignments, commands, `send`,
    /// `refresh`, and — LLP 1017 P2 — `if`/`else` and `match`, as the
    /// ternary and the inline `match` are lowered in `expr.rs`: a forward
    /// jump over the arm not taken, the `match` binding a local for its
    /// `some` block. Still no loops; a body always terminates (LLP 1005 §2).
    fn stmt(
        &mut self,
        asm: &mut Asm,
        stmt: &Stmt,
        scope: &Scope,
        locals: &mut u16,
    ) -> Result<(), LowerError> {
        let root = self.root;
        match stmt {
            Stmt::Assign { target, expr, .. } => {
                expr::compile(self, asm, expr, scope, locals)?;
                let slot = match root.states.iter().position(|s| &s.name == target) {
                    Some(si) => self.slots[si],
                    None => {
                        self.mutation_slots[root
                            .mutations
                            .iter()
                            .position(|m| &m.name == target)
                            .unwrap()]
                    }
                };
                asm.store_slot(slot);
            }
            Stmt::Send {
                target,
                source,
                args,
                ..
            } => {
                for arg in args {
                    expr::compile(self, asm, arg, scope, locals)?;
                }
                let m = self.mutations[root
                    .mutations
                    .iter()
                    .position(|m| &m.name == target)
                    .unwrap()];
                let source = self.b.str(source);
                asm.send(m, source, args.len() as u16);
            }
            Stmt::Refresh { target, .. } => {
                let r = self.resources[root
                    .resources
                    .iter()
                    .position(|r| &r.name == target)
                    .unwrap()];
                asm.refresh(r);
            }
            Stmt::Command { name, args, .. } => {
                for arg in args {
                    expr::compile(self, asm, arg, scope, locals)?;
                }
                let name = self.b.str(name);
                asm.command(name, args.len() as u16);
            }
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                expr::compile(self, asm, cond, scope, locals)?;
                let els = asm.label();
                let end = asm.label();
                asm.jump_if_false(els);
                for s in then {
                    self.stmt(asm, s, scope, locals)?;
                }
                asm.jump(end);
                asm.place(els);
                for s in otherwise {
                    self.stmt(asm, s, scope, locals)?;
                }
                asm.place(end);
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                let bound_ty = match contract_types::infer(subject, scope, &self.types.shapes) {
                    Ok(Ty::Option(t)) => *t,
                    _ => Ty::Unknown,
                };
                expr::compile(self, asm, subject, scope, locals)?;
                let is_none = asm.label();
                let end = asm.label();
                asm.jump_if_none(is_none);
                asm.simple(exact_plan::Opcode::Unwrap);
                asm.bind_local();
                let index = *locals;
                *locals += 1;
                let mut inner = scope.clone();
                inner.push(vec![(some.0.clone(), Ref::Local(index as u32), bound_ty)]);
                for s in &some.1 {
                    self.stmt(asm, s, &inner, locals)?;
                }
                *locals -= 1;
                asm.drop_local();
                asm.jump(end);
                asm.place(is_none);
                asm.simple(exact_plan::Opcode::Pop);
                for s in none {
                    self.stmt(asm, s, scope, locals)?;
                }
                asm.place(end);
            }
        }
        Ok(())
    }

    /// A literal style value is checked now by the kernel's own parser
    /// (`StyleProps::set_dynamic`), so `width=true` and `align-items="middle"`
    /// are refused at compile time, not at the first frame; a computed value
    /// is checked by type — a number or a string (LLP 1017 P1a).
    fn check_style_value(
        &self,
        a: &Attr,
        rows: &[StyleId],
        scope: &Scope,
        font: Option<&FontUse>,
    ) -> Result<(), LowerError> {
        // Validate every authored literal result, including inactive branches.
        // Only the whole expression is type-checked here: match arms bind their
        // own local names, which the type pass resolves in the proper scope.
        let mut pending: Vec<(&Expr, Span)> = Vec::new();
        let mut current = (&a.value, a.span);
        loop {
            let (value, span) = current;
            match value {
                Expr::Ternary(_, yes, no, _) => {
                    pending.push((no, no.span()));
                    pending.push((yes, yes.span()));
                }
                Expr::Match { some, none, .. } => {
                    pending.push((none, none.span()));
                    pending.push((some, some.span()));
                }
                _ => {}
            }
            // @ref LLP 1043.000 §3 D1 — keep the full wire vocabulary, narrow authoring.
            if let Expr::Str(v, _) = value {
                if rows.contains(&StyleId::WrapFlow) && !matches!(v.as_str(), "auto" | "both") {
                    return err("lower-attr-value", "unsupported `wrap-flow` value: CSS Exclusions defines it; exact2 v1 implements `both` (or `auto`)", span);
                }
                if rows.contains(&StyleId::ShapeMargin) && v.trim().ends_with('%') {
                    return err("lower-attr-value", "percentage `shape-margin` is not implemented in exact2 v1; use a nonnegative length in points/px", span);
                }
            }
            let literal = match value {
                expr if numeric_literal(expr).is_some() => {
                    Some(StyleValue::Number(numeric_literal(expr).unwrap()))
                }
                Expr::Str(s, _) => Some(
                    // Enum keywords stay text, including `auto` (as in the runner).
                    // Other codecs retain their existing dimension/keyword handling.
                    if s == "auto"
                        && !rows
                            .iter()
                            .all(|row| row.codec() == exact_kernel::StyleCodec::Enum)
                    {
                        StyleValue::Auto
                    } else if let Some(pct) =
                        s.strip_suffix('%').and_then(|p| p.parse::<f64>().ok())
                    {
                        StyleValue::Percent(pct)
                    } else {
                        StyleValue::Text(s.clone())
                    },
                ),
                Expr::Bool(b, _) => {
                    return err(
                        "lower-attr-value",
                        format!(
                            "`{}={b}` — a style value is a number or a string, not a bool",
                            a.name
                        ),
                        span,
                    )
                }
                _ => None,
            };
            match literal {
                Some(v) => {
                    let mut probe = StyleProps::default();
                    for row in rows {
                        if let Err(e) = probe.set_dynamic(*row, &v) {
                            return err(
                                "lower-attr-value",
                                format!(
                                    "`{}={}` is not a value for `{}`: {}",
                                    a.name,
                                    literal_text(value),
                                    row.name(),
                                    describe(&e)
                                ),
                                span,
                            );
                        }
                    }
                }
                None if std::ptr::eq(value, &a.value) => {
                    if let Ok(t) = contract_types::infer(value, scope, &self.types.shapes) {
                        if !matches!(t, Ty::Number | Ty::String | Ty::Unknown) {
                            return err(
                                "lower-attr-type",
                                format!(
                                    "`{}` takes a number or a string; this expression is `{t}`",
                                    a.name
                                ),
                                span,
                            );
                        }
                    }
                }
                _ => {}
            }
            if let Some(font) = font {
                if rows.contains(&StyleId::FontStyle) {
                    let requested = match value {
                        Expr::Str(s, _) if s == "normal" => Some(false),
                        Expr::Str(s, _) if s == "italic" => Some(true),
                        _ => None,
                    };
                    if let Some(italic) = requested {
                        if !font
                            .font
                            .faces
                            .iter()
                            .any(|(_, face_italic)| *face_italic == italic)
                        {
                            return err(
                                "lower-font-face",
                                format!(
                                    "this family declares no real {} face; v1 never synthesizes one",
                                    if italic { "italic" } else { "normal" }
                                ),
                                span,
                            );
                        }
                    }
                }
                if rows.contains(&StyleId::FontWeight) {
                    if let (Expr::Number(weight, _), Some(italic)) = (value, font.italic) {
                        if *weight >= 600.0
                            && !font.font.faces.iter().any(|(face_weight, face_italic)| {
                                *face_italic == italic && *face_weight >= 600
                            })
                        {
                            return err(
                                "lower-font-face",
                                format!(
                                    "this family has no real {} face for font-weight={weight}; v1 never synthesizes one",
                                    if italic { "italic bold" } else { "bold" }
                                ),
                                span,
                            );
                        }
                    }
                }
            }
            let Some(next) = pending.pop() else { break };
            current = next;
        }
        Ok(())
    }

    /// A prop attribute's value by the prop's type: text for most, a bool for
    /// `disabled`, a whole number for `aria-level`.
    fn check_prop_value(
        &self,
        name: &str,
        value: &Expr,
        span: Span,
        prop: PropId,
        scope: &Scope,
    ) -> Result<(), LowerError> {
        media::check(name, value, span)?;
        let want = tags::prop_ty(prop);
        if prop == PropId::ImageSource {
            if let Expr::Str(source, _) = value {
                if let Some(role) = source.strip_prefix("symbol:") {
                    if exact_kernel::generated::symbol(role).is_none() {
                        return err(
                            "lower-attr-value",
                            format!(
                                "symbol `{role}` is not a role; roles: {}",
                                exact_kernel::generated::SYMBOL_ROLES.join(", ")
                            ),
                            span,
                        );
                    }
                }
            }
        }
        if want == tags::PropTy::Int {
            if let Some(number) = numeric_literal(value) {
                if !whole_i64(number) {
                    return err(
                        "lower-attr-value",
                        format!(
                            "`{name}` takes a whole number in the signed 64-bit range; given {number}"
                        ),
                        span,
                    );
                }
            }
        }
        let ty = match value {
            Expr::Number(_, _) => Some(Ty::Number),
            Expr::Str(_, _) | Expr::Template(_, _) => Some(Ty::String),
            Expr::Bool(_, _) => Some(Ty::Bool),
            other => contract_types::infer(other, scope, &self.types.shapes).ok(),
        };
        let ok = matches!(
            (want, &ty),
            (_, None)
                | (_, Some(Ty::Unknown))
                | (tags::PropTy::Str, Some(Ty::String))
                | (tags::PropTy::Bool, Some(Ty::Bool))
                | (tags::PropTy::Int | tags::PropTy::Float, Some(Ty::Number))
        );
        if !ok {
            return err(
                "lower-attr-type",
                format!(
                    "`{}` takes {}; this expression is `{}`",
                    name,
                    match want {
                        tags::PropTy::Str => "a string",
                        tags::PropTy::Bool => "a bool",
                        tags::PropTy::Int => "a whole number",
                        tags::PropTy::Float => "a number",
                    },
                    ty.unwrap_or(Ty::Unknown)
                ),
                span,
            );
        }
        Ok(())
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
        font: Option<&FontUse>,
    ) -> Result<(), LowerError> {
        let Some(target) = tags::attr(&a.name) else {
            let hint = match tags::renamed(&a.name) {
                Some(new) => format!(
                    "; `{}` is spelled `{new}` here, the CSS name (LLP 1017 §8.1)",
                    a.name
                ),
                None => tags::similar_attr(&a.name, false)
                    .map(|n| format!("; did you mean `{n}`?"))
                    .unwrap_or_default(),
            };
            return err(
                "lower-unknown-attr",
                format!("`{tag}` has no attribute `{}`{hint}", a.name),
                a.span,
            );
        };
        if (tag != "iframe" && matches!(a.name.as_str(), "sandbox" | "load" | "message"))
            || (tag != "iframe" && tag != "video" && a.name == "src")
        {
            return err(
                "lower-attr-tag",
                format!("`{}` belongs to `iframe`, not `{tag}`", a.name),
                a.span,
            );
        }
        if tag != "list" && matches!(a.name.as_str(), "reachstart" | "reachend") {
            return err(
                "lower-attr-tag",
                format!("`{}` belongs to `list`", a.name),
                a.span,
            );
        }
        match target {
            tags::AttrTarget::Flex => {
                // CSS `flex: <n>` is `<n> 1 0%`: grow n, shrink 1, basis 0%.
                self.check_style_value(
                    a,
                    &[StyleId::from_name("flex_grow").unwrap()],
                    scope,
                    font,
                )?;
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
                if rows.as_slice() == [StyleId::FontFamily] {
                    let Expr::Str(name, _) = &a.value else {
                        return err(
                            "lower-font-family-literal",
                            "`font-family` is literal-only in v1",
                            a.span,
                        );
                    };
                    let Some(stack) = self.font_stacks.get(name).copied() else {
                        return err(
                            "lower-font-undeclared",
                            format!("font family `{name}` is neither generic nor declared"),
                            a.span,
                        );
                    };
                    let code = self.b.constant(&Value::Number(stack.0 as f64));
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: StyleId::FontFamily as u16,
                        expr: code,
                    });
                    return Ok(());
                }
                self.check_style_value(a, &rows, scope, font)?;
                let code = self.expr_code(&a.value, scope, locals)?;
                for row in rows {
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: row as u16,
                        expr: code,
                    });
                }
            }
            tags::AttrTarget::InvertedBoolProp(prop) => {
                self.check_prop_value(&a.name, &a.value, a.span, prop, scope)?;
                let inverted = Expr::Unary(UnOp::Not, Box::new(a.value.clone()), a.span);
                let code = self.expr_code(&inverted, scope, locals)?;
                bindings.push(BindingsRow {
                    kind: BindingKind::Prop,
                    id: prop as u16,
                    expr: code,
                });
            }
            tags::AttrTarget::Prop(prop) => {
                self.check_prop_value(&a.name, &a.value, a.span, prop, scope)?;
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
                // The view is inlined, so a handler behind a child's `action`
                // prop names the real action here: its arity is checked now,
                // not at dispatch (LLP 1006 §8's circle-back; LLP 1017 P1b).
                let params = self.root.actions[ai].params.len();
                let valid = contract_analyze::handler_arity(event, args.len())
                    .is_some_and(|range| range.contains(&params));
                if !valid {
                    return err(
                        "lower-handler-arity",
                        format!(
                            "`{name}` takes {params} parameter(s); `{event}=` supplies {}{}",
                            args.len(),
                            match event {
                                "hover" => " plus whether the pointer is over",
                                "key" => " plus the key's name",
                                "change" => " plus the new value",
                                "message" => " plus the guest's message",
                                "scroll" => " plus scrollLeft and scrollTop",
                                "heightrelease" => " plus height and velocity",
                                "transformgeometry" => " plus four geometry numbers",
                                "transformrelease" => " plus six transform release numbers",
                                _ => "",
                            }
                        ),
                        a.span,
                    );
                }
                if event == "reorderdrop"
                    && self.types.components[0].actions[ai][args.len()..]
                        != [Ty::String, Ty::Option(Box::new(Ty::String))]
                {
                    return err(
                        "lower-handler-type",
                        "`reorderdrop` supplies string and option<string>",
                        a.span,
                    );
                }
                if matches!(
                    event,
                    "pan" | "heightrelease" | "transformgeometry" | "transformrelease"
                ) && self.types.components[0].actions[ai][args.len()..]
                    .iter()
                    .any(|ty| *ty != Ty::Number)
                {
                    return err(
                        "lower-handler-type",
                        format!("`{event}` supplies only numeric payload parameters"),
                        a.span,
                    );
                }
                let mut codes = Vec::new();
                for arg in args {
                    codes.push(self.expr_code(arg, scope, locals)?);
                }
                let kind =
                    EventKind::from_name(event).expect("tag table admitted an unknown handler");
                handlers.push((kind, self.actions[ai], codes));
            }
        }
        Ok(())
    }
}
