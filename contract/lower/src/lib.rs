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

mod class;
mod collection;
mod contain;
pub mod controls;
pub mod dataset;
pub mod expr;
mod fonts;
mod grouped;
mod handlers;
mod keyframes;
mod lint;
mod media;
mod menus;
mod native;
mod routes;
mod shorthands;
mod sites;
mod stmts;
mod strings;
mod svg;
pub mod tags;
mod timers;
mod values;
pub mod vocab;

pub use dataset::{data_words, hook_words};
pub use lint::lint;
use lint::{unknown_attr, unknown_tag};
pub use native::{is_module_tag, module_tags};
pub use sites::{Declared, NodeSite, Origin, Sites};

use contract_analyze::Analysis;
use contract_syntax::{Attr, Expr, File, FnDecl, Node, Owner, Span};
use contract_types::{Checked, Ref, Scope, Ty, Types};
use exact_kernel::{NodeType, StyleId};
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

/// One refusal, as the plural result lowering returns.
fn err_one(id: &'static str, message: impl Into<String>, span: Span) -> Vec<LowerError> {
    vec![LowerError {
        id,
        message: message.into(),
        span,
    }]
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
    /// The `keyframes` declarations, by name, with what each animates (LLP 1055 D5).
    pub keyframes: BTreeMap<String, Vec<exact_motion::Property>>,
    /// The file's `fn` declarations, by name, expanded inline at each call
    /// (LLP 1017 P5), each with its body's repeated calls bound once.
    pub fns: BTreeMap<&'a str, (&'a FnDecl, &'a Expr)>,
    /// The arm each region arm lowered to, by the inliner's tag and arm
    /// index, with the scope in force inside it: what a lifted child state
    /// owned by that arm is initialized in (LLP 1017 P4c).
    pub arm_scopes: BTreeMap<(u32, u8), (exact_plan::ArmsId, Scope)>,
    /// Every generic and declared family name to its stack id.
    pub font_stacks: BTreeMap<String, StacksId>,
    /// Declared families, for the literal weight/style synthesis diagnostic.
    declared_fonts: BTreeMap<String, DeclaredFont>,
    /// How many `fn` bodies are being expanded right now (a guard; the type
    /// pass already refuses a cycle).
    pub fn_depth: u32,
    /// Tags' fixed row values already built (`Lowerer::fixed`).
    fixed: BTreeMap<(bool, &'static str), Code>,
    /// Refusals so far: an element or attribute that fails is recorded and
    /// its siblings are lowered anyway.
    errors: Vec<LowerError>,
    /// The locale slot, once a `t` call made it (LLP 1060 D4).
    locale: Option<exact_plan::SlotsId>,
    /// Every key a `t` call names, the only ones baked.
    texts_used: std::collections::BTreeSet<String>,
    /// How many `svg` elements enclose the node being lowered: `text`
    /// inside one is SVG text, outside a box (LLP 1055.000 D11).
    pub(crate) svg_depth: u32,
    /// Whether the enclosing element contains its exclusions (LLP 1043.000).
    parent_positioned: bool,
    parent_bounded_column: bool,
    /// Where a native button may not be, from the nearest ancestor that says.
    pub(crate) button_context: Option<&'static str>,
    /// Whether the element being lowered is a popover's direct child.
    pub(crate) popover_child: bool,
    /// Where the element being lowered sits relative to a navigation root.
    nav_place: routes::NavPlace,
    host_transforms: std::collections::BTreeSet<(Span, u32)>,
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

/// At most this many refusals from one lowering.
pub const MAX_REFUSALS: usize = 20;

impl From<LowerError> for Vec<LowerError> {
    fn from(e: LowerError) -> Self {
        vec![e]
    }
}

/// Lower a checked file to a plan.
pub fn lower(
    checked: &Checked<'_>,
    _analysis: &Analysis,
    asset_root: Option<&Path>,
) -> Result<Plan, LowerError> {
    lower_with_sites(checked, _analysis, asset_root, false)
        .map(|(plan, _)| plan)
        .map_err(|mut all| all.swap_remove(0))
}

/// Lower, reporting every independent refusal: each element and each of its
/// attributes is lowered whatever its siblings' fate (at most
/// [`MAX_REFUSALS`]). `mapped` also returns the development source sites.
pub fn lower_all(
    checked: &Checked<'_>,
    analysis: &Analysis,
    asset_root: Option<&Path>,
    mapped: bool,
) -> Result<(Plan, Option<Sites>), Vec<LowerError>> {
    if mapped && checked.expanded.instances.is_empty() {
        return Err(vec![LowerError {
            id: "lower-source-sites",
            message: "mapped lowering needs source provenance from `check_all(file, true)`".into(),
            span: checked.expanded.root.span,
        }]);
    }
    lower_with_sites(checked, analysis, asset_root, mapped)
}

fn lower_with_sites(
    checked: &Checked<'_>,
    _analysis: &Analysis,
    asset_root: Option<&Path>,
    capture_sites: bool,
) -> Result<(Plan, Option<Sites>), Vec<LowerError>> {
    // Keep the exact expansion whose root and row slots inference checked.
    let Checked {
        file,
        types,
        expanded: ex,
    } = checked;
    let root = &ex.root;
    let root_types = &types.components[0];
    let is_fn = |name: &str| file.fns.iter().any(|f| f.name == name);
    let shared: Vec<Expr> = file
        .fns
        .iter()
        .map(|f| contract_syntax::share_calls(&f.body, &is_fn))
        .collect();
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
        keyframes: BTreeMap::new(),
        fns: file
            .fns
            .iter()
            .zip(&shared)
            .map(|(f, body)| (f.name.as_str(), (f, body)))
            .collect(),
        fn_depth: 0,
        svg_depth: 0,
        parent_positioned: true,
        parent_bounded_column: false,
        button_context: None,
        popover_child: false,
        nav_place: Default::default(),
        host_transforms: Default::default(),
        arm_scopes: BTreeMap::new(),
        font_stacks: BTreeMap::new(),
        declared_fonts: BTreeMap::new(),
        fixed: BTreeMap::new(),
        errors: Vec::new(),
        locale: None,
        texts_used: Default::default(),
    };
    l.declare_fonts(file, asset_root)?;
    // Styles: rows only, literal only (the parser holds the second), by name.
    for s in &file.styles {
        for a in &s.attrs {
            match tags::attr(&a.name) {
                Some(tags::AttrTarget::Styles(_) | tags::AttrTarget::Flex | tags::AttrTarget::Shorthand) => {}
                Some(tags::AttrTarget::Prop(p)) if p.styleable() => {} // LLP 1069.011 D12
                Some(_) => l.errors.push(LowerError {
                    id: "lower-style-attr",
                    message: format!(
                        "`{}` cannot be in `style {}`: a style holds style rows (and `buttonStyle`) only — no `testId`, no handlers, no other props",
                        a.name, s.name
                    ),
                    span: a.span,
                }),
                None => {
                    let hint = tags::renamed(&a.name)
                        .map(|n| format!("; `{}` is spelled `{n}` here", a.name))
                        .or_else(|| tags::similar_attr(&a.name, true).map(|n| format!("; did you mean `{n}`?")))
                        .unwrap_or_default();
                    l.errors.push(LowerError {
                        id: "lower-unknown-attr",
                        message: format!("`style {}` has no attribute `{}`{hint}", s.name, a.name),
                        span: a.span,
                    });
                }
            }
        }
        if l.styles.insert(s.name.clone(), s.attrs.clone()).is_some() {
            l.errors.push(LowerError {
                id: "lower-style-duplicate",
                message: format!("`style {}` is declared twice", s.name),
                span: s.span,
            });
        }
    }
    let refused = l.declare_keyframes(file);
    l.errors.extend(refused);
    // Shapes first, in declaration order, so type ids are stable.
    for s in &file.shapes {
        l.ty_id(&Ty::Record(s.name.clone()))?;
    }
    // Ids for every declaration before any body, so bodies may reference any
    // of them; each starts with the one placeholder body its own replaces.
    let placeholder = l.b.constant(&Value::Unit);
    for (i, s) in root.states.iter().enumerate() {
        let ty = l.ty_id(&root_types.slots[i])?;
        let id = l.b.slot(&s.name, ty, placeholder);
        l.slots.push(id);
    }
    l.declare_routes(file);
    for (i, d) in root.derives.iter().enumerate() {
        let ty = l.ty_id(&root_types.derives[i])?;
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
        // @ref LLP 1054.000.000 D1 — the type check named only resources.
        let refreshes: Vec<_> = m
            .refreshes
            .iter()
            .map(|(name, _)| {
                l.resources[root.resources.iter().position(|r| &r.name == name).unwrap()]
            })
            .collect();
        if !refreshes.is_empty() {
            l.b.mutation_refreshes(id, &refreshes);
        }
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
        // @ref LLP 1035.005.000 D1 — the VM's allowlist is exactly what the
        // body assigns or sends through every branch, in slot order.
        let mut writes: Vec<_> = a.effects().iter().map(|e| l.slot_named(e.target)).collect();
        writes.sort_unstable_by_key(|slot| slot.0);
        writes.dedup();
        let id = l.b.action(&a.name, &params_ref, &writes, placeholder);
        l.actions.push(id);
    }
    // Bodies.
    let scope = types.component_scope(root, root_types);
    for (i, s) in root.states.iter().enumerate() {
        if ex.owners[i] == Owner::Root && !(i == 0 && file.routes.is_some()) {
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
        // @ref LLP 1027.005 D6 — declare first, install the full argument
        // list, then count its context tail. No `with` keeps the default 0.
        if let Some(identity) = r.identity {
            let context = u16::try_from(r.args.len() - identity).map_err(|_| LowerError {
                id: "lower-resource-context",
                message: "a resource supports at most 65535 request context values".into(),
                span: r.span,
            })?;
            l.b.set_resource_context(l.resources[i], context);
        }
    }
    // @ref LLP 1048.003 D6 — a declared placeholder is a row of its own,
    // after every authored row; its arguments read no state.
    for (i, r) in root.resources.iter().enumerate() {
        let Some(p) = &r.placeholder else { continue };
        // @ref LLP 1054.000.002 D3 — `empty(…)` rides the resource's row.
        if p.source == contract_types::placeholder::EMPTY {
            let value = contract_types::placeholder::materialize(
                &root_types.resources[i],
                &p.args,
                &l.types.shapes,
                &r.name,
                p.span,
            )
            .map_err(|errors| LowerError {
                id: errors[0].id,
                message: errors[0].message.clone(),
                span: errors[0].span,
            })?;
            l.b.set_resource_placeholder_value(l.resources[i], &value);
            continue;
        }
        let ty = l.ty_id(&root_types.resources[i])?;
        let mut args = Vec::new();
        for a in &p.args {
            args.push(l.expr_code(a, &Scope::default(), 0)?);
        }
        let row =
            l.b.resource(&format!("{}#else", r.name), &p.source, &[], ty, None);
        let range = l.b.args(&args);
        l.b.set_resource_args(row, range);
        l.b.set_resource_placeholder(l.resources[i], row);
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
        l.block(&mut asm, &a.body, &inner, &mut locals)?;
        let code = l.b.code(asm);
        l.b.set_action_body(l.actions[i], code);
    }
    for (i, m) in root.mutations.iter().enumerate() {
        if m.queue {
            l.b.set_mutation_queue(l.mutations[i]);
        }
        if let Some((name, _)) = &m.then {
            let action = l.actions[root.actions.iter().position(|a| &a.name == name).unwrap()];
            l.b.set_mutation_then(l.mutations[i], action);
        }
    }
    l.timers(&root.tasks, &scope)?;
    // The view, inlined (by `expand`, above).
    let view = &root.view;
    if view.len() != 1 {
        return Err(err_one(
            "lower-one-root",
            format!(
                "the root view must be exactly one node; found {}",
                view.len()
            ),
            root.span,
        ));
    }
    if !matches!(view[0], Node::Element { .. }) {
        return Err(err_one(
            "lower-root-region",
            "the root of a view is a node; a `when`, `each`, or `match` cannot be the root (LLP 1010 §1: a keyed root could not reorder) — put it inside a `column` or a `main`",
            view[0].span(),
        ));
    }
    l.host_transforms = tags::host_transform_recipients(&l, view);
    l.nodes(view, None, None, &scope, 0, None)?;
    if !l.errors.is_empty() {
        // Row slots name regions a refused element may not have lowered.
        l.errors.truncate(MAX_REFUSALS);
        return Err(l.errors);
    }
    // Child state (LLP 1017 P4c): each lifted state is initialized where its
    // instance is created — in its owning arm's scope, now that the arms
    // exist, or, for a use outside every region, at the boot render.
    for (i, owner) in ex.owners.iter().enumerate() {
        let state = &root.states[i];
        match *owner {
            Owner::Root => {}
            Owner::Instance => {
                let init = l.expr_code(&state.expr, &scope, 0)?;
                l.b.set_slot_init(l.slots[i], init);
                l.b.set_slot_late(l.slots[i]);
            }
            Owner::Arm { tag, arm } => {
                let (id, arm_scope) =
                    l.arm_scopes
                        .get(&(tag, arm))
                        .cloned()
                        .ok_or_else(|| LowerError {
                            id: "lower-row-slot",
                            message: format!(
                                "child state `{}` names a region that was not lowered",
                                state.name
                            ),
                            span: state.span,
                        })?;
                let init = l.expr_code(&state.expr, &arm_scope, 0)?;
                l.b.set_slot_init(l.slots[i], init);
                l.b.set_slot_owner(l.slots[i], id);
            }
        }
    }
    l.bake_texts()?;
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

    /// A tag's fixed row value as a constant body, built once per compile.
    fn fixed(&mut self, style: bool, value: &'static str) -> Code {
        if let Some(code) = self.fixed.get(&(style, value)) {
            return *code;
        }
        // A fixed style is an enum's name or a number in points.
        let v = match value.parse::<f64>() {
            Ok(n) if style => Value::Number(n),
            _ => Value::str(value),
        };
        let code = self.b.constant(&v);
        self.fixed.insert((style, value), code);
        code
    }

    pub(crate) fn expr_code(
        &mut self,
        e: &Expr,
        scope: &Scope,
        locals: u16,
    ) -> Result<Code, LowerError> {
        self.typed_code(e, scope, locals).map(|(code, _)| code)
    }

    /// A code body and the type of the value it leaves.
    pub(crate) fn typed_code(
        &mut self,
        e: &Expr,
        scope: &Scope,
        locals: u16,
    ) -> Result<(Code, Ty), LowerError> {
        let mut asm = Asm::new();
        let mut locals = locals;
        let ty = expr::compile(self, &mut asm, e, scope, &mut locals)?;
        Ok((self.b.code(asm), ty))
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
            if let Err(e) = self.node(n, parent, arm, order as u32, scope, locals, parent_tag) {
                self.errors.push(e);
            }
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
                let Some(t) = tags::tag(tag).or_else(|| native::tag(tag)) else {
                    return Err(unknown_tag(tag, *span));
                };
                // Two layout refusals the compiler can make without measuring
                // (LLP 1017 P1c; the measured ones are bake's). Conservative:
                // only the case nothing on the path can bound is refused.
                // `class=` expands its style's rows first; the node's own
                // attribute of the same name replaces the style's (LLP 1017 P6).
                // @ref LLP 1084 D7 — a grouped list's sheet, under its classes.
                let (mut sheet, unmarked) = grouped::split(attrs);
                let attrs = unmarked.as_ref().unwrap_or(attrs);
                let (class_label, mut expanded) = self.class_rows(attrs)?.unzip();
                let native = expanded
                    .iter()
                    .flatten()
                    .chain(attrs.iter())
                    .rev()
                    .find(|a| a.name == "appearance");
                if tag == "button"
                    && native.is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == "auto"))
                {
                    grouped::native_rows(&mut sheet);
                }
                let class_len = expanded.as_ref().map_or(0, Vec::len) + sheet.len();
                let expanded = match &mut expanded {
                    Some(rows) => {
                        rows.splice(0..0, sheet.iter().cloned());
                        rows.extend(attrs.iter().filter(|a| a.name != "class").cloned());
                        rows.as_slice()
                    }
                    None if !sheet.is_empty() => {
                        expanded = Some([sheet.clone(), attrs.to_vec()].concat());
                        expanded.as_deref().expect("just set")
                    }
                    None => attrs.as_slice(),
                };
                values::check_position_area(expanded)?;
                tags::check_exclusion(&t, expanded, self.parent_positioned)?;
                let composed = self.compose_animation(expanded)?;
                let expanded = composed.as_deref().unwrap_or(expanded);
                let lengths =
                    svg::coerce_lengths(tag, svg::in_svg(self.svg_depth > 0, parent_tag), expanded);
                let expanded = lengths.as_deref().unwrap_or(expanded);
                self.check_svg(tag, parent_tag, expanded, *span)?;
                // @ref LLP 1055.000 D4 — an `svg` inside an `svg` is a viewport.
                // Inside an `svg`, `svg` is a viewport and `text` is SVG text
                // (LLP 1055.000 D4, D11).
                let t = match tag.as_str() {
                    "svg" if svg::in_svg(self.svg_depth > 0, parent_tag) => tags::Tag {
                        node_type: NodeType::SvgViewport,
                        ..t
                    },
                    "text" if svg::in_svg(self.svg_depth > 0, parent_tag) => tags::Tag {
                        node_type: NodeType::SvgText,
                        ..t
                    },
                    _ => t,
                };
                // @ref LLP 1069.001 D1 — `input`'s `type` is a literal: a text
                // type is a text field, `checkbox` a form control.
                let canonical_type = controls::canonical_type_attrs(tag, expanded);
                let expanded = canonical_type.as_deref().unwrap_or(expanded);
                let control = controls::control(tag, expanded)?;
                let t = control.map_or(t.clone(), |kind| controls::tag(kind, t.clone()));
                let face = (control == Some("button"))
                    .then(|| grouped::unsheet(children))
                    .flatten();
                let children = face.as_ref().unwrap_or(children);
                if control == Some("button") {
                    self.check_native_button(expanded, children, *span)?;
                }
                controls::check_nesting(tag, parent_tag, *span)?;
                self.check_menu_shapes(tag, expanded, children, *span)?;
                let numeric = controls::range_attrs(control, expanded);
                let expanded = numeric.as_deref().unwrap_or(expanded);
                // @ref LLP 1084 D1, D3 — a grouped list's sheet, before its
                // author's rows, and its sections' shape.
                let grouped = grouped::style(tag, expanded)?;
                let (listed, sections);
                let (expanded, children) = match grouped {
                    Some(style) => {
                        listed = [grouped::list_rows(style, *span), expanded.to_vec()].concat();
                        sections = grouped::sections(style, children)?;
                        (listed.as_slice(), &sections)
                    }
                    None => (expanded, children),
                };
                tags::validate_list(tag, expanded, *span)?;
                self.check_collection(tag, expanded, children, *span, scope)?;
                // A row list is a flex item of its column like any carousel;
                // CSS's own fix keeps its spacers' extent from widening that
                // column: `min-width: 0`, unless the author set one (LLP 1070
                // §3.1).
                let row_list = (tag == "list"
                    && collection::row_list(expanded)
                    && expanded.iter().any(|a| {
                        a.name == "virtualized" && matches!(a.value, Expr::Bool(true, _))
                    })
                    && !expanded.iter().any(|a| a.name == "min-width"))
                .then(|| {
                    let mut attrs = expanded.to_vec();
                    attrs.push(contract_syntax::Attr {
                        name: "min-width".into(),
                        value: Expr::Number(0.0, *span),
                        span: *span,
                    });
                    attrs
                });
                let expanded = row_list.as_deref().unwrap_or(expanded);
                let audio = media::audio_rows(tag, expanded, *span)?;
                let expanded = audio.as_deref().unwrap_or(expanded);
                // @ref LLP 1074 T1 — a box that contains its absolutely positioned
                // descendants on every host is lowered `position: relative`.
                let in_svg = svg::in_svg(self.svg_depth > 0, parent_tag);
                let relative = contain::positioned(
                    &t,
                    expanded,
                    in_svg,
                    *span,
                    self.host_transforms.contains(&(*span, *instance)),
                    !self.may_hold_absolute(children),
                )?;
                let expanded = relative.as_deref().unwrap_or(expanded);
                values::check_glass_group(&t, expanded)?;
                let has =
                    |names: &[&str]| expanded.iter().any(|a| names.contains(&a.name.as_str()));
                let parent_stacks = !matches!(parent_tag, Some("row") | Some("canvas"));
                let clips_y = expanded.iter().any(|a| {
                    a.name == "overflow-y" && matches!(&a.value, Expr::Str(v, _) if v == "hidden")
                });
                if tag == "scroll"
                    && !clips_y
                    && parent_stacks
                    && !has(&["height", "max-height"])
                    && !expanded
                        .iter()
                        .any(|a| a.name == "flex" && values::flex_bounds(&a.value))
                    && !values::shrinking_scroll(expanded, self.parent_bounded_column)
                {
                    return err(
                        "lower-scroll-unbounded",
                        "`scroll` has no `height`, `max-height`, or `flex`, and its parent stacks it top to bottom, so it will grow with its content and never scroll",
                        *span,
                    );
                }
                controls::check_zero_size(tag, expanded, children, *span)?;
                let nav_place = self.nav_place.enter(tag, expanded, *span)?;
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
                for (style, value) in t.fixed_styles {
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: *style as u16,
                        expr: self.fixed(true, value),
                    });
                }
                for (prop, value) in t.fixed_props {
                    bindings.push(BindingsRow {
                        kind: BindingKind::Prop,
                        id: *prop as u16,
                        expr: self.fixed(false, value),
                    });
                }
                // @ref LLP 1048.003 D4 — the page scrolls where this does.
                let positional = match positional.as_slice() {
                    [word] if contract_syntax::is_scroll_document(tag, word) => {
                        if let Some(bound) = attrs.iter().find(|a| a.name == "document") {
                            return err(
                                "lower-attr-tag",
                                "`scroll` takes `document` or `document=(…)`, not both",
                                bound.span,
                            );
                        }
                        bindings.push(BindingsRow {
                            kind: BindingKind::Prop,
                            id: exact_kernel::PropId::ScrollDocument as u16,
                            expr: self.b.constant(&Value::Bool(true)),
                        });
                        &[][..]
                    }
                    // @ref LLP 1069.001 D1, LLP 1069.002 D1 — HTML's `switch`
                    // and `multiple`.
                    [word] if self.control_word(tag, word, control, &mut bindings)? => &[][..],
                    all => all,
                };
                if let Some(first) = positional.first() {
                    let Some(prop) = t.positional else {
                        return err(
                            "lower-positional",
                            if tag == "scroll" {
                                "`scroll` takes one word, `document`: `scroll document`".to_owned()
                            } else {
                                format!("`{tag}` takes no positional argument")
                            },
                            first.span(),
                        );
                    };
                    let (code, ty) = self.typed_code(first, scope, locals)?;
                    values::check_prop_value(tag, first, first.span(), prop, &ty)?;
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
                let font = self.font_use(expanded)?;
                // @ref LLP 1024 D1 — a module tag's own attribute named like
                // a row its box never uses is refused, not bound to nothing;
                // a class's rows are the style's, never a prop. By name over
                // the authored list: `expanded` is rewritten above (an
                // animation shorthand and its longhands compose), so a
                // position in it does not say whose an attribute is.
                let own: Vec<&Attr> = attrs.iter().filter(|a| a.name != "class").collect();
                let refused: Vec<&str> = own
                    .iter()
                    .filter(|a| native::refused(tag, a).is_some())
                    .map(|a| a.name.as_str())
                    .collect();
                for a in &own {
                    self.errors.extend(native::refused(tag, a));
                }
                let flex = tags::flex_container(tag, expanded);
                for (index, a) in expanded.iter().enumerate() {
                    if let Some(e) = flex.and_then(|f| tags::multicol_on_flex(f, a).err()) {
                        self.errors.push(e);
                        continue;
                    }
                    if native::leftover(tag, a)
                        || refused.contains(&a.name.as_str())
                        || dataset::word(&a.name).is_some()
                    {
                        continue;
                    }
                    if let Err(e) = self.attr(
                        tag,
                        contract_syntax::input_control(tag, attrs),
                        a,
                        scope,
                        locals,
                        &mut bindings,
                        &mut handlers,
                        &mut surface,
                        &font,
                    ) {
                        self.errors.push(e);
                    }
                    if let Some(origins) = &mut origins {
                        let origin = if index < sheet.len() {
                            Origin::Tag
                        } else if index < class_len {
                            Origin::Class(class_label.clone().expect("class attribute"))
                        } else {
                            Origin::Own
                        };
                        origins.resize(bindings.len(), origin);
                    }
                }
                // @ref LLP 1057.003 C6 — a transform drag needs both halves:
                // the page's geometry and the release. A handle with one is
                // never admitted by any host, so it would sit inert, unsaid.
                let has = |k: EventKind| handlers.iter().any(|(kind, ..)| *kind == k);
                let (geometry, release) = (
                    has(EventKind::Transformgeometry),
                    has(EventKind::Transformrelease),
                );
                if geometry != release {
                    let (present, missing) = if geometry {
                        ("transformgeometry", "transformrelease")
                    } else {
                        ("transformrelease", "transformgeometry")
                    };
                    let span = expanded
                        .iter()
                        .find(|a| a.name == present)
                        .map_or(*span, |a| a.span);
                    self.errors.extend(
                        err::<()>(
                            "lower-transform-drag-handlers",
                            format!("`{present}` without `{missing}`: a transform drag needs both (the page's geometry and the release), or it never starts; add `{missing}=`"),
                            span,
                        )
                        .err(),
                    );
                }
                // @ref LLP 1069.001 D8 — rows a control derives.
                if control.is_some() && controls::derived_rows(&mut bindings) {
                    if let Some(origins) = &mut origins {
                        origins.resize(bindings.len(), Origin::Own);
                    }
                }
                if t.node_type == NodeType::NativeView {
                    let rest: Vec<&Attr> = expanded
                        .iter()
                        .filter(|a| native::leftover(tag, a))
                        .collect();
                    if let Err(e) = self.native_bindings(tag, &rest, scope, locals, &mut bindings) {
                        self.errors.push(e);
                    }
                    if let Some(origins) = &mut origins {
                        origins.resize(bindings.len(), Origin::Own);
                    }
                }
                self.data_row(tag, expanded, scope, locals, &mut bindings, &mut origins);
                // Two bindings for one row — a style's and the node's own, a
                // tag's fixed row and an attribute — the last one wins.
                if bindings.len() > 1 {
                    let mut seen: BTreeMap<(u8, u16), usize> = BTreeMap::new();
                    let mut unique = 0;
                    for index in 0..bindings.len() {
                        let b = &bindings[index];
                        let target = match seen.get(&(b.kind as u8, b.id)) {
                            Some(&i) => i,
                            None => {
                                seen.insert((b.kind as u8, b.id), unique);
                                unique += 1;
                                unique - 1
                            }
                        };
                        // Targets are in the consumed prefix: unread rows stay intact.
                        if target != index {
                            bindings.swap(target, index);
                            if let Some(origins) = &mut origins {
                                origins.swap(target, index);
                            }
                        }
                    }
                    bindings.truncate(unique);
                    if let Some(origins) = &mut origins {
                        origins.truncate(unique);
                    }
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
                        origins.as_deref().expect("site origins"),
                    ));
                }
                let enters = tag == "svg";
                self.svg_depth += enters as u32;
                let parent_positioned = self.parent_positioned;
                self.parent_positioned = parent_tag.is_none()
                    || expanded
                        .iter()
                        .rev()
                        .find(|a| a.name == "position")
                        .is_some_and(|a| !matches!(&a.value, Expr::Str(v, _) if v == "static"))
                    || t.fixed_styles
                        .iter()
                        .any(|(id, v)| *id == StyleId::PositionType && *v != "static");
                let button_context = self.button_context;
                self.button_context =
                    controls::button_context(tag, control, self.popover_child).or(button_context);
                // Whether the children lowered next are a popover's direct
                // children, its rows (LLP 1069.011.000 D5).
                let popover_child = std::mem::replace(
                    &mut self.popover_child,
                    expanded.iter().any(|a| a.name == "popover"),
                );
                let bounded = self.parent_bounded_column;
                self.parent_bounded_column = values::bounded_column(tag, expanded, bounded);
                let nav_place = std::mem::replace(&mut self.nav_place, nav_place);
                let lowered = self.nodes(children, Some(id), arm, scope, locals, Some(tag));
                self.nav_place = nav_place;
                self.parent_bounded_column = bounded;
                self.button_context = button_context;
                self.popover_child = popover_child;
                self.parent_positioned = parent_positioned;
                self.svg_depth -= enters as u32;
                lowered
            }
            Node::Use { name, span, .. } => err(
                "lower-uninlined-use",
                format!("component `{name}` was not inlined"),
                *span,
            ),
            Node::Children { span } => err(
                "lower-uninlined-use",
                "`children` is inlined away before lowering",
                *span,
            ),
            Node::When {
                tag,
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
                self.arm_scopes.insert((*tag, 0), (arms[0], inner.clone()));
                self.arm_scopes.insert((*tag, 1), (arms[1], inner.clone()));
                self.nodes(then, None, Some(arms[0]), &inner, locals, parent_tag)?;
                self.nodes(otherwise, None, Some(arms[1]), &inner, locals, parent_tag)
            }
            Node::Each {
                tag,
                var,
                index,
                list,
                key,
                body,
                ..
            } => {
                let (subject, list_ty) = self.typed_code(list, scope, locals)?;
                let item_ty = match list_ty {
                    Ty::List(t) => *t,
                    _ => Ty::Unknown,
                };
                let mut inner = scope.clone();
                inner.push_each(var, index.as_deref(), item_ty);
                let key = self.expr_code(key, &inner, locals)?;
                let (_r, arms) =
                    self.b
                        .region(RegionKind::Each, parent, arm, order, subject, key, 1);
                self.arm_scopes.insert((*tag, 0), (arms[0], inner.clone()));
                self.nodes(body, None, Some(arms[0]), &inner, locals, parent_tag)
            }
            Node::Match {
                tag,
                subject,
                some,
                none,
                ..
            } => {
                let (code, subject_ty) = self.typed_code(subject, scope, locals)?;
                let bound_ty = match subject_ty {
                    Ty::Option(t) => *t,
                    _ => Ty::Unknown,
                };
                let unit = self.b.constant(&Value::Unit);
                let (_r, arms) =
                    self.b
                        .region(RegionKind::Match, parent, arm, order, code, unit, 2);
                let mut some_scope = scope.clone();
                some_scope.push_region(Some((some.0.clone(), Ref::Bound(0), bound_ty)));
                self.arm_scopes
                    .insert((*tag, 0), (arms[0], some_scope.clone()));
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
                self.arm_scopes
                    .insert((*tag, 1), (arms[1], none_scope.clone()));
                self.nodes(none, None, Some(arms[1]), &none_scope, locals, parent_tag)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn attr(
        &mut self,
        tag: &str,
        control: Option<&str>,
        a: &Attr,
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
        handlers: &mut Vec<(EventKind, exact_plan::ActionsId, Vec<Code>)>,
        surface: &mut Option<exact_plan::SurfacesId>,
        font: &[FontUse],
    ) -> Result<(), LowerError> {
        let Some(mut target) = tags::attr_valued(&a.name, &a.value) else {
            return Err(unknown_attr(tag, a));
        };
        // HTML's global `title` on any element but `head`: advisory text, the
        // platform's tooltip (studio diary R24); `head`'s is the document's.
        if a.name == "title" && tag != "head" {
            target = tags::AttrTarget::Prop(exact_kernel::PropId::Title);
        }
        // @ref LLP 1024 D1 — `load` and `message` are a module's too.
        let module = native::is_module_tag(tag) && a.name != "sandbox";
        if (tag != "iframe"
            && matches!(a.name.as_str(), "sandbox" | "load" | "message")
            && !(tag == "canvas" && a.name == "message")
            && !module)
            || (!matches!(tag, "iframe" | "video" | "audio") && a.name == "src")
        {
            return err(
                "lower-attr-tag",
                format!(
                    "`{}` belongs to {}, not `{tag}`",
                    a.name,
                    if a.name == "message" {
                        "`iframe` or `canvas`"
                    } else {
                        "`iframe`"
                    }
                ),
                a.span,
            );
        }
        // An SVG element's own attribute (a filter primitive's `mode`, `in`,
        // `values`…) does nothing on a box: refused by name rather than kept
        // as a prop no host reads (feed F19: CSS `order` once landed here).
        let svg_tag = svg::is_element(tag) || matches!(tag, "svg" | "text" | "tspan");
        if !svg_tag && !module && svg::svg_only_prop(&a.name) {
            return err(
                "lower-attr-tag",
                format!(
                    "`{}` is an SVG element's attribute; it does nothing on `{tag}`",
                    a.name
                ),
                a.span,
            );
        }
        if a.name == "alt" && tag != "image" {
            return err(
                "lower-attr-tag",
                format!("`alt` belongs to `image`, not `{tag}`; another element's accessible name is `aria-label`"),
                a.span,
            );
        }
        if !svg_tag && !module && a.name == "mask" {
            return err(
                "lower-attr-tag",
                "`mask` masks SVG elements so far; a box takes `mask-image` (a gradient)",
                a.span,
            );
        }
        // @ref LLP 1048.003 D1 — a document's metadata, and nothing else.
        let head_field =
            tags::HEAD_FIELDS.contains(&a.name.as_str()) && !(a.name == "title" && tag != "head");
        if head_field != (tag == "head") {
            return err(
                "lower-attr-tag",
                if head_field {
                    format!("`{}` belongs to `head`, not `{tag}`", a.name)
                } else {
                    format!(
                        "`head` takes only {}; `{}` is not one",
                        tags::HEAD_FIELDS.join(", "),
                        a.name
                    )
                },
                a.span,
            );
        }
        if a.name == "document" && tag != "scroll" {
            return err(
                "lower-attr-tag",
                format!("`document` belongs to `scroll`, not `{tag}`"),
                a.span,
            );
        }
        // The page's HTTP status, known at compile time: a not-found view's
        // 404 or 410, or a view of failed data's 503 (LLP 1048.003 D1, LLP
        // 1048.000 D11).
        if a.name == "status"
            && !matches!(a.value, Expr::Number(n, _) if n == 404.0 || n == 410.0 || n == 503.0)
        {
            return err(
                "lower-attr-value",
                "`status` is 404, 410 or 503, as a literal: the page answers with it",
                a.span,
            );
        }
        // @ref LLP 1064 D6 — a native field shows its value as typed.
        if a.name == "text-transform" && matches!(tag, "input" | "textarea") {
            return err(
                "lower-attr-tag",
                format!("`text-transform` does not apply to `{tag}`: a field shows what was typed on every host (the web's form controls reset it too); transform the value instead"),
                a.span,
            );
        }
        if tag != "text" && a.name == "selectionchange" {
            return err(
                "lower-attr-tag",
                format!("`selectionchange` belongs to `text`: it reports the part of the reader's text selection inside one paragraph, not `{tag}`"),
                a.span,
            );
        }
        // @ref LLP 1045 D3 — `none` or `markdown`, styling a `text`'s or a
        // `textarea`'s own string; another word did nothing (notes #1).
        if a.name == "markup" {
            if !matches!(tag, "text" | "textarea") {
                let message = format!("`markup` belongs to `text` (the reader) or `textarea` (the editor), not `{tag}`");
                return err("lower-attr-tag", message, a.span);
            }
            if matches!(&a.value, Expr::Str(s, _) if s != "markdown" && s != "none") {
                return err(
                    "lower-attr-value",
                    "`markup` is \"markdown\" or \"none\"",
                    a.span,
                );
            }
        }
        if tag != "list" && matches!(a.name.as_str(), "reachstart" | "reachend") {
            return err(
                "lower-attr-tag",
                format!("`{}` belongs to `list`", a.name),
                a.span,
            );
        }
        match target {
            tags::AttrTarget::Shorthand => self.bind_shorthand(a, scope, locals, font, bindings)?,
            tags::AttrTarget::Flex => {
                for (index, row) in [StyleId::FlexGrow, StyleId::FlexShrink, StyleId::FlexBasis]
                    .into_iter()
                    .enumerate()
                {
                    let value = values::flex_component(&a.value, index)?;
                    let component = Attr { value, ..a.clone() };
                    let (code, ty) = self.typed_code(&component.value, scope, locals)?;
                    values::check_style_value(&component, &[row], &ty, font)?;
                    if index < 2 && matches!(ty, Ty::String) {
                        return err("lower-attr-type", "a computed `flex` must be a number; write a literal CSS shorthand or a choice of literal shorthands", a.span);
                    }
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: row as u16,
                        expr: code,
                    });
                }
            }
            tags::AttrTarget::Styles(rows) => {
                if rows == [StyleId::FontFamily] {
                    // @ref LLP 1053 G7 — each arm's family is a stack id now.
                    let stacks = self.family_stacks(&a.value)?;
                    let (code, _) = self.typed_code(&stacks, scope, locals)?;
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: StyleId::FontFamily as u16,
                        expr: code,
                    });
                    return Ok(());
                }
                // CSS's one-to-four-value box shorthands: a binding a side.
                if values::four_sided(rows) {
                    if let Some(sides) = values::sides(&a.name, &a.value)? {
                        for (&row, value) in rows.iter().zip(sides) {
                            let (code, ty) = self.typed_code(&value, scope, locals)?;
                            let side = Attr { value, ..a.clone() };
                            values::check_style_value(&side, &[row], &ty, font)?;
                            bindings.push(BindingsRow {
                                kind: BindingKind::Style,
                                id: row as u16,
                                expr: code,
                            });
                        }
                        return Ok(());
                    }
                }
                let (code, ty) = self.typed_code(&a.value, scope, locals)?;
                values::check_style_value(a, rows, &ty, font)?;
                for &row in rows {
                    bindings.push(BindingsRow {
                        kind: BindingKind::Style,
                        id: row as u16,
                        expr: code,
                    });
                }
            }
            tags::AttrTarget::InvertedBoolProp(prop) => {
                // `not value`, compiled from the value itself.
                let (mut asm, mut depth) = (Asm::new(), locals);
                let ty = expr::compile(self, &mut asm, &a.value, scope, &mut depth)?;
                asm.simple(exact_plan::Opcode::Not);
                let code = self.b.code(asm);
                values::check_prop_value(&a.name, &a.value, a.span, prop, &ty)?;
                bindings.push(BindingsRow {
                    kind: BindingKind::Prop,
                    id: prop as u16,
                    expr: code,
                });
            }
            tags::AttrTarget::Prop(prop) => {
                let glass;
                let value = if prop == exact_kernel::PropId::GlassGroup {
                    glass = values::glass_group(&a.value)?;
                    &glass
                } else {
                    &a.value
                };
                let (mut asm, mut depth) = (Asm::new(), locals);
                let ty = expr::compile(self, &mut asm, value, scope, &mut depth)?;
                values::check_prop_value(&a.name, value, a.span, prop, &ty)?;
                // ARIA's word-valued states; a bool is written as `true`/`false`.
                if values::aria_words(prop).is_some() && ty == Ty::Bool {
                    asm.call(exact_plan::Stdlib::ToString);
                }
                // An enumerated attribute whose IDL attribute is a bool takes
                // one, as its words (shop diary F7).
                if let (Ty::Bool, Some((yes, no))) = (&ty, values::bool_words(prop)) {
                    let word = |w: &str| Box::new(Expr::Str(w.into(), a.span));
                    let words = Expr::Ternary(Box::new(value.clone()), word(yes), word(no), a.span);
                    (asm, depth) = (Asm::new(), locals);
                    expr::compile(self, &mut asm, &words, scope, &mut depth)?;
                }
                let code = self.b.code(asm);
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
                    let (name, value) = match arg {
                        Expr::NamedArg(name, value, _) => (name.as_str(), value.as_ref()),
                        _ => ("", arg),
                    };
                    codes.push((name, self.expr_code(value, scope, locals)?));
                }
                *surface = Some(self.b.surface(name, &codes));
            }
            tags::AttrTarget::Handler(event) => {
                self.handler(tag, event, control, a, scope, locals, handlers)?;
            }
        }
        Ok(())
    }
}
