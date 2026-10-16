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

mod buttons;
mod cells;
mod class;
mod collection;
mod color_profile;
mod contain;
pub mod controls;
pub mod dataset;
mod disabled_look;
mod edits;
mod error;
pub mod expr;
mod fields;
mod fonts;
mod grouped;
mod handlers;
mod identity;
mod intersect;
mod keyframes;
mod lint;
mod media;
mod menus;
mod metrics;
mod native;
mod nodes;
mod routes;
mod shorthands;
mod sites;
mod sounds;
mod stmts;
mod strings;
pub mod style_names;
mod svg;
mod symbols;
pub mod tags;
mod theme;
mod timers;
mod values;
pub mod vocab;
mod warnings;

pub use dataset::{data_words, hatch_words};
pub use error::LowerError;
pub(crate) use error::{err, err_one};
pub use fields::Profile;
pub use identity::compiler_identity;
pub use lint::lint;
use lint::{unknown_attr, unknown_tag};
pub use native::{is_module_tag, module_tags};
pub use sites::{ButtonSite, Declared, NodeSite, Origin, Sites};
/// What a design is checked against (LLP 1121 D2): the schema's roles.
pub use theme::synthetic_candidates;
pub use theme::vocab as tamber_vocab;
pub use warnings::{warnings, MAX_WARNINGS};

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

/// A row the compiler implies, which no author wrote (LLP 1115.000 D2): a
/// tag's fixed row, a heading's style, a sheet's row, one derived from
/// another. Whether a row is written is said where it is emitted, never
/// worked out afterwards: an attribute's row takes [`Attr::written`].
pub(crate) const IMPLIED: bool = false;

/// A style row, `written` when its author wrote it on this node.
pub(crate) fn style_row(id: StyleId, expr: Code, written: bool) -> BindingsRow {
    let (kind, id) = (BindingKind::Style, id as u16);
    BindingsRow {
        kind,
        id,
        expr,
        written,
    }
}

/// A prop row, `written` as a style row is.
pub(crate) fn prop_row(id: exact_kernel::PropId, expr: Code, written: bool) -> BindingsRow {
    let (kind, id) = (BindingKind::Prop, id as u16);
    BindingsRow {
        kind,
        id,
        expr,
        written,
    }
}

pub(crate) struct Lowerer<'a> {
    pub b: PlanBuilder,
    /// The app's theme, resolved (LLP 1095 D4).
    pub theme: Vec<theme::ThemeEntry>,
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
    /// The surface the plan is for: a terminal entry's block link is not tinted.
    profile: Profile,
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
    lower_with_sites(checked, _analysis, asset_root, false, Profile::Web)
        .map(|(plan, _)| plan)
        .map_err(|mut all| all.swap_remove(0))
}

/// Lower, reporting every independent refusal: each element and each of its
/// attributes is lowered whatever its siblings' fate (at most
/// [`MAX_REFUSALS`]). `mapped` also returns the development source sites;
/// `profile` is the surface the plan is for.
pub fn lower_all(
    checked: &Checked<'_>,
    analysis: &Analysis,
    asset_root: Option<&Path>,
    mapped: bool,
    profile: Profile,
) -> Result<(Plan, Option<Sites>), Vec<LowerError>> {
    if mapped && checked.expanded.instances.is_empty() {
        return Err(vec![LowerError {
            id: "lower-source-sites",
            message: "mapped lowering needs source provenance from `check_all(file, true)`".into(),
            span: checked.expanded.root.span,
        }]);
    }
    lower_with_sites(checked, analysis, asset_root, mapped, profile)
}

fn lower_with_sites(
    checked: &Checked<'_>,
    _analysis: &Analysis,
    asset_root: Option<&Path>,
    capture_sites: bool,
    profile: Profile,
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
        theme: Vec::new(),
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
        profile,
    };
    l.declare_fonts(file, asset_root)?;
    l.declare_sounds(file, asset_root)?;
    // Bound before any style is parsed, the profiles first: an entry may be
    // a profile's colour.
    let _profiles = l.declare_color_profiles(file);
    let _theme = l.declare_theme(file, root);
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
                    let hint = fields::placeholder_hint(&a.name)
                        .or_else(|| tags::renamed(&a.name).map(|n| format!("; `{}` is spelled `{n}` here", a.name)))
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
        l.b.set_slot_persist(id, root.persists(&s.name));
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
            "the root of a view is a node; an `if`, `for`, or `match` cannot be the root (LLP 1010 §1: a keyed root could not reorder) — put it inside a `column` or a `main`",
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
            Node::Element { .. } => {
                let step = self
                    .prepare_element(n, parent, arm, order, scope, locals, parent_tag)
                    .map_err(|e| native::name_the_fix(n, e))?;
                self.svg_depth += step.enters as u32;
                let parent_positioned =
                    std::mem::replace(&mut self.parent_positioned, step.positioned);
                let new_button_context = step.button_context.or(self.button_context);
                let button_context =
                    std::mem::replace(&mut self.button_context, new_button_context);
                let popover_child = std::mem::replace(&mut self.popover_child, step.popover_child);
                let bounded = std::mem::replace(&mut self.parent_bounded_column, step.bounded);
                let nav_place = std::mem::replace(&mut self.nav_place, step.nav_place);
                let lowered = self.nodes(
                    step.children.as_ref(),
                    Some(step.id),
                    arm,
                    scope,
                    locals,
                    Some(step.tag),
                );
                self.nav_place = nav_place;
                self.parent_bounded_column = bounded;
                self.button_context = button_context;
                self.popover_child = popover_child;
                self.parent_positioned = parent_positioned;
                self.svg_depth -= step.enters as u32;
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
        // @ref LLP 1021 §5.3 — stable menu ids, never localized titles.
        if a.name == "-exact-menu"
            && (tag != "button"
                || !matches!(&a.value, Expr::Str(value, _) if matches!(value.as_str(),
                    "app" | "file" | "edit" | "view" | "go" | "window" | "help")))
        {
            return err(
                "lower-menu-placement",
                "`-exact-menu` belongs to a button and takes a literal app, file, edit, view, go, window or help",
                a.span,
            );
        }
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
            && !(tag == "image" && a.name == "load")
            && !module)
            || (!matches!(tag, "iframe" | "video" | "audio") && a.name == "src")
        {
            return err(
                "lower-attr-tag",
                format!(
                    "`{}` belongs to {}, not `{tag}`",
                    a.name,
                    match a.name.as_str() {
                        "message" => "`iframe` or `canvas`",
                        "load" => "`iframe`, `image` or a native module",
                        _ => "`iframe`",
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
        if let Some(why) = shorthands::text_box_trim_refusal(&a.name, tag, module)
            .or_else(|| fields::outline_refusal(&a.name, tag, control))
            .or_else(|| fields::placeholder_refusal(&a.name, tag, control))
        {
            return err("lower-attr-tag", why, a.span);
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
        if !matches!(tag, "text" | "input" | "textarea") && a.name == "selectionchange" {
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
                    if index < 2 && matches!(ty, Ty::String) {
                        return err("lower-attr-type", "a computed `flex` must be a number, a literal CSS shorthand or a choice of literal shorthands; for a computed basis write the longhands, as in `flex-grow=1 flex-shrink=1 flex-basis=w`", a.span);
                    }
                    self.check_style(&component, &[row], &ty, font, scope)?;
                    bindings.push(style_row(row, code, a.written));
                }
            }
            tags::AttrTarget::Styles(rows) => {
                if rows == [StyleId::FontFamily] {
                    let stacks = self.family_stacks(&a.value)?;
                    let (code, ty) = self.typed_code(&stacks, scope, locals)?;
                    if ty != Ty::String
                        && !(ty == Ty::Number && Self::literal_family_arms(&a.value))
                        && !matches!(ty, Ty::Option(ref inner) if **inner == Ty::String)
                    {
                        return err("lower-attr-type", "`font-family` needs a string", a.span);
                    }
                    bindings.push(style_row(StyleId::FontFamily, code, a.written));
                    return Ok(());
                }
                // CSS's one-to-four-value box shorthands: a binding a side.
                if values::four_sided(rows) {
                    if let Some(sides) = values::sides(&a.name, &a.value)? {
                        for (&row, value) in rows.iter().zip(sides) {
                            let (code, ty) = self.typed_code(&value, scope, locals)?;
                            let side = Attr { value, ..a.clone() };
                            self.check_style(&side, &[row], &ty, font, scope)?;
                            bindings.push(style_row(row, code, a.written));
                        }
                        return Ok(());
                    }
                }
                let (code, ty) = self.typed_code(&a.value, scope, locals)?;
                self.check_style(a, rows, &ty, font, scope)?;
                for &row in rows {
                    bindings.push(style_row(row, code, a.written));
                }
            }
            tags::AttrTarget::InvertedBoolProp(prop) => {
                // `not value`, compiled from the value itself.
                let (mut asm, mut depth) = (Asm::new(), locals);
                let ty = expr::compile(self, &mut asm, &a.value, scope, &mut depth)?;
                asm.simple(exact_plan::Opcode::Not);
                let code = self.b.code(asm);
                values::check_prop_value(&a.name, &a.value, a.span, prop, &ty)?;
                bindings.push(prop_row(prop, code, a.written));
            }
            tags::AttrTarget::Prop(prop) => {
                let glass;
                let aria_disabled = values::aria_disabled_literal(prop, &a.value);
                let value = if prop == exact_kernel::PropId::GlassGroup {
                    glass = values::glass_group(&a.value)?;
                    &glass
                } else if prop == exact_kernel::PropId::IntersectMargin {
                    glass = crate::intersect::margin(&a.value, a.span)?;
                    &glass
                } else if let Some(width) = values::split_width(prop, &a.value) {
                    glass = width;
                    &glass
                // @ref LLP 1116.007 D3.1 — a symbol by another spelling is its role.
                } else if let Some(role) = (prop == exact_kernel::PropId::ImageSource)
                    .then(|| symbols::roles(&a.value))
                    .flatten()
                {
                    glass = role;
                    &glass
                } else {
                    aria_disabled.as_ref().unwrap_or(&a.value)
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
                bindings.push(prop_row(prop, code, a.written));
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
            tags::AttrTarget::MediaMetadata => self.media_metadata(a, scope, locals, bindings)?,
        }
        Ok(())
    }
}
