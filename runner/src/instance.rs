//! The instance tree, and the ops that keep the kernel equal to it.
//!
//! @ref LLP 0485 §8.2 (keyed instance management, never reconciliation of
//! trees; research)
//!
//! The plan's nodes and regions are *sites*. An instance is one realization
//! of a site: a kernel view for a node, an active arm for `when`/`match`, one
//! row per key for `each`. An update visits only the sites whose reads
//! ([`deps`]) include an input that changed since the last update, or an
//! enclosing row or arm whose value changed, and there evaluates only the
//! stale bindings; it emits exactly the kernel ops that make the kernel
//! equal to the result: a binding's op only when its value changed,
//! `SetChildren` only when a child list changed, create/destroy only when a
//! key appeared or went away. [`Update::full`] evaluates everything, the
//! reference an incremental update must equal. There is no tree diff: a
//! keyed row keeps its views across reorders because its key, not its
//! position, is its identity.

/// Variable-height viewport collections and their portable host feedback seam.
pub mod collection;
mod deps;
mod find;
mod heights;
mod text;
mod window;

use crate::bridge;
use crate::vm::{self, Env, Frame, RowSlots, Trap};
pub use deps::RowWrites;
use deps::{Bits, Reads, Seen};
pub(crate) use deps::{Deps, Input, Reads as DepReads};
use exact_kernel::{NodeType, Op, StyleProps, ViewId};
use exact_plan::{ArmsId, BindingKind, NodesId, Plan, RegionKind, RegionsId, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Why an instance could not be realized.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceError {
    Trap(Trap),
    Bridge(bridge::BridgeError),
    UnknownNodeType(u8),
    List(&'static str),
    SubjectKind {
        region: RegionsId,
    },
    KeyKind {
        region: RegionsId,
    },
    DuplicateKey {
        region: RegionsId,
    },
    SlotType {
        slot: String,
    },
    Collection(String),
    /// Host geometry rejected before changing any collection or kernel state.
    InvalidCollectionFeedback,
}

impl From<Trap> for InstanceError {
    fn from(t: Trap) -> Self {
        InstanceError::Trap(t)
    }
}

/// One realized node.
#[derive(Debug)]
pub struct NodeInst {
    /// The site.
    pub node: NodesId,
    /// The kernel view.
    pub view: ViewId,
    /// Last emitted value per binding, in binding order.
    last: Vec<Option<Value>>,
    /// Last published surface inputs (a canvas node).
    last_surface: Option<Vec<Value>>,
    /// Ordered children: static nodes and regions interleaved by `order`.
    children: Vec<Child>,
    /// Last emitted child list.
    last_children: Vec<ViewId>,
    collection: Option<Box<collection::Collection>>,
}

#[derive(Debug)]
enum Child {
    Node(NodeInst),
    Region(RegionInst),
}

/// One realized region.
#[derive(Debug)]
pub struct RegionInst {
    region: RegionsId,
    active: Active,
    window: Option<Box<window::ListWindow>>,
    /// An `each` subject as last keyed: the same object again, with the key's
    /// other inputs unchanged, is the same keys.
    subject: Option<Value>,
}

#[derive(Debug)]
enum Active {
    /// `when` / `match`: the active arm and its roots.
    Arm {
        arm: Option<usize>,
        frame: Frame,
        roots: Vec<Child>,
    },
    /// `each`: rows by key, in current order.
    Rows { rows: Vec<Row> },
}

#[derive(Debug)]
struct Row {
    wrapper: Option<ViewId>,
    key: Value,
    frame: Frame,
    roots: Vec<Child>,
    /// The row's own slots (LLP 1017 P4c): the `state` a child component
    /// declared, one value per row, kept across reorders with the key,
    /// dropped with the row, initialized when the row is created.
    slots: RowSlots,
}

/// One step of the instance path from the plan's roots to a view: the
/// region crossed and, for an `each` row, its key; for a `when`/`match`
/// region, the active arm (LLP 1035.002 D6 — what identifies *this*
/// instance of a repeated site).
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceStep {
    /// A keyed row of an `each`.
    Row {
        /// The region.
        region: RegionsId,
        /// The row's key.
        key: Value,
    },
    /// The active arm of a `when`/`match`.
    Arm {
        /// The region.
        region: RegionsId,
        /// Which arm, when one is active.
        arm: Option<usize>,
    },
}

/// Allocates kernel view ids — never reusing one within a runner's life —
/// and remembers which plan node each instance view realizes.
#[derive(Debug, Default)]
pub struct Ids {
    next: ViewId,
    /// Every instance node's view and site, destroyed ones included until
    /// [`Ids::retain`] drops them.
    sites: std::collections::HashMap<ViewId, NodesId>,
}

impl Ids {
    fn fresh(&mut self) -> ViewId {
        self.next += 1;
        self.next
    }

    /// The plan node `view` realized, if an instance node created it.
    pub fn site(&self, view: ViewId) -> Option<NodesId> {
        self.sites.get(&view).copied()
    }

    /// Every remembered instance view and its site.
    pub fn sites(&self) -> impl Iterator<Item = (ViewId, NodesId)> + '_ {
        self.sites.iter().map(|(v, n)| (*v, *n))
    }

    /// Forget views `live` says are gone.
    pub fn retain(&mut self, live: impl Fn(ViewId) -> bool) {
        self.sites.retain(|view, _| live(*view));
    }

    /// How many views are remembered.
    pub fn remembered(&self) -> usize {
        self.sites.len()
    }
}

/// A canvas node's surface inputs, evaluated against state: the runner's
/// side-output for the host's GPU module (LLP 1009 D2). Published only
/// after the commit that produced it applied.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceUpdate {
    /// The canvas node's kernel view.
    pub view: ViewId,
    /// The surface's name in the app's GPU module.
    pub name: String,
    /// The authored call mode survives even when no arguments were supplied.
    pub mode: exact_plan::SurfaceArgsMode,
    /// Argument names in source order, or empty for positional arguments.
    pub names: Vec<String>,
    /// Its arguments, evaluated.
    pub values: Vec<Value>,
}

impl SurfaceUpdate {
    /// Positional JSON array or named JSON object consumed by the surface module.
    /// Host reserialization may reorder keys: transport bytes are never hash inputs.
    pub fn arguments_json(&self) -> String {
        fn value_json(value: &Value, out: &mut String) {
            use std::fmt::Write;
            match value {
                Value::Number(n) if n.is_finite() => {
                    let _ = write!(out, "{n}");
                }
                Value::Number(_) | Value::Unit | Value::Option(None) => out.push_str("null"),
                Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                Value::Str(s) => crate::agent::quote(s, out),
                Value::Option(Some(v)) => value_json(v, out),
                Value::List(items) | Value::Record(items) => {
                    out.push('[');
                    for (i, value) in items.iter().enumerate() {
                        if i != 0 {
                            out.push(',');
                        }
                        value_json(value, out);
                    }
                    out.push(']');
                }
            }
        }
        let named = self.mode == exact_plan::SurfaceArgsMode::Named;
        let mut out = String::from(if named { "{" } else { "[" });
        for (i, value) in self.values.iter().enumerate() {
            if i != 0 {
                out.push(',');
            }
            if named {
                crate::agent::quote(&self.names[i], &mut out);
                out.push(':');
            }
            value_json(value, &mut out);
        }
        out.push(if named { '}' } else { ']' });
        out
    }
}

/// What one update needs: the environment and the id allocator, plus the op
/// batch under construction and the surface inputs that changed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InstanceWork {
    /// Created or revisited node instances (not native views).
    pub nodes_visited: usize,
    /// Rows whose key expression ran in this update.
    pub rows_keyed: usize,
    /// Rows retained without revisiting their unchanged subtree.
    pub rows_reused: usize,
    /// Outer keyed regions bypassed because their inputs were unchanged.
    pub regions_skipped: usize,
    /// Binding expressions evaluated (created or revisited nodes).
    pub bindings_evaluated: usize,
}

/// Per-commit evaluation context and deterministic work counters.
pub struct Update<'a> {
    /// The environment every expression sees.
    pub env: Env<'a>,
    /// Immutable child sites for this environment's plan.
    pub sites: &'a SiteIndex,
    /// The id allocator.
    pub ids: &'a mut Ids,
    /// Ops accumulated for one atomic `Kernel::apply`.
    pub ops: Vec<Op>,
    /// Surface inputs that changed, in tree order.
    pub surfaces: Vec<SurfaceUpdate>,
    /// Work performed during instance evaluation.
    pub work: InstanceWork,
    /// Evaluate every site, whatever changed: the reference an incremental
    /// update must equal. A runtime switch, never a build feature.
    pub full: bool,
    /// Rows an action wrote since the last update.
    pub rows: RowWrites,
    /// Inputs changed since the tree last updated; `None` outside
    /// [`Tree::update`], where everything is stale.
    changed: Option<Bits>,
    /// Enclosing frames whose value changed in this update, by relative depth.
    dirty_frames: u64,
    /// Whether the scopes walked so far enclose every written row.
    on_path: bool,
    /// Journal lines for what the data got wrong and the tree absorbed.
    /// Written with the commit.
    pub notes: Vec<String>,
}

impl<'a> Update<'a> {
    /// A context over `env` with no ops yet.
    pub fn new(env: Env<'a>, sites: &'a SiteIndex, ids: &'a mut Ids) -> Self {
        Update {
            env,
            sites,
            ids,
            ops: Vec::new(),
            surfaces: Vec::new(),
            work: InstanceWork::default(),
            full: false,
            rows: RowWrites::default(),
            changed: None,
            dirty_frames: 0,
            on_path: true,
            notes: Vec::new(),
        }
    }

    fn eval(&self, code: exact_plan::Code, frames: &[Frame]) -> Result<Value, Trap> {
        let env = Env { frames, ..self.env };
        Ok(vm::eval(self.env.plan.code(code), &env, &[])?.value)
    }

    /// Whether anything `reads` reads may differ from what the tree shows.
    fn stale(&self, reads: &Reads) -> bool {
        self.stale_outside(reads, 0)
    }

    /// [`Update::stale`] for reads made `shift` scopes further in (a key or
    /// a row body seen from its region), counting only enclosing frames.
    fn stale_outside(&self, reads: &Reads, shift: u32) -> bool {
        let Some(changed) = &self.changed else {
            return true;
        };
        self.full
            || reads.opaque
            || reads.bits.intersects(changed)
            || reads.frames_outside(shift) & self.dirty_frames != 0
            || (reads.row_slots
                && self.on_path
                && self
                    .rows
                    .writes
                    .iter()
                    .any(|(_, slot)| reads.bits.get(*slot as usize)))
    }

    /// Enter a row or arm scope whose frame value `dirty`-ly changed.
    fn enter(&mut self, dirty: bool, row: Option<&RowSlots>) -> (u64, bool) {
        let saved = (self.dirty_frames, self.on_path);
        self.dirty_frames = deps::into_scope(self.dirty_frames, dirty);
        if let Some(row) = row {
            self.on_path &= self.rows.path.contains(&RowWrites::id(row));
        }
        saved
    }

    fn leave(&mut self, saved: (u64, bool)) {
        (self.dirty_frames, self.on_path) = saved;
    }
}

type SiteParent = (Option<NodesId>, Option<ArmsId>);

/// Ordered child sites, built once from the runner's immutable plan.
#[derive(Debug)]
pub struct SiteIndex {
    groups: Vec<(SiteParent, std::ops::Range<usize>)>,
    sites: Vec<(u32, Site)>,
    /// What every binding and site reads.
    deps: Deps,
}

impl SiteIndex {
    /// Index the exact parent/arm pair, preserving authored order and rank ties.
    pub fn new(plan: &Plan) -> Self {
        let mut entries = Vec::with_capacity(plan.nodes.len() + plan.regions.len());
        for (i, n) in plan.nodes.iter().enumerate() {
            entries.push(((n.parent, n.arm), n.order, Site::Node(NodesId(i as u32))));
        }
        for (i, r) in plan.regions.iter().enumerate() {
            entries.push((
                (r.parent, r.arm),
                r.order,
                Site::Region(RegionsId(i as u32)),
            ));
        }
        entries.sort_by_key(|(parent, order, site)| (*parent, *order, site.rank()));
        let mut groups: Vec<(SiteParent, std::ops::Range<usize>)> = Vec::new();
        let mut sites = Vec::with_capacity(entries.len());
        for (parent, order, site) in entries {
            let end = sites.len() + 1;
            if let Some((previous, range)) = groups
                .last_mut()
                .filter(|(previous, _)| *previous == parent)
            {
                debug_assert_eq!(*previous, parent);
                range.end = end;
            } else {
                groups.push((parent, sites.len()..end));
            }
            sites.push((order, site));
        }
        groups.shrink_to_fit();
        let mut index = Self {
            groups,
            sites,
            deps: Deps::default(),
        };
        index.deps = Deps::new(plan, &index);
        index
    }

    /// What every binding, site, derive and resource argument reads.
    pub(crate) fn deps(&self) -> &Deps {
        &self.deps
    }

    fn children(&self, parent: Option<NodesId>, arm: Option<ArmsId>) -> &[(u32, Site)] {
        self.groups
            .binary_search_by_key(&(parent, arm), |(key, _)| *key)
            .map_or(&[], |i| &self.sites[self.groups[i].1.clone()])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Site {
    Node(NodesId),
    Region(RegionsId),
}

impl Site {
    fn rank(self) -> u32 {
        match self {
            Site::Node(n) => n.0,
            Site::Region(r) => r.0,
        }
    }
}

// A region adds one lexical frame. Reserve it with the inherited frames so
// row construction does not allocate and then immediately reallocate.
fn with_frame(frames: &[Frame], frame: Frame) -> Vec<Frame> {
    let mut inner = Vec::with_capacity(frames.len() + 1);
    inner.extend_from_slice(frames);
    inner.push(frame);
    inner
}

/// Realize the sites under (`parent`, `arm`) for the first time.
fn realize(
    u: &mut Update<'_>,
    parent: Option<NodesId>,
    arm: Option<ArmsId>,
    frames: &[Frame],
) -> Result<Vec<Child>, InstanceError> {
    let sites = u.sites;
    let mut out = Vec::new();
    for &(_, site) in sites.children(parent, arm) {
        out.push(match site {
            Site::Node(id) => Child::Node(NodeInst::create(u, id, frames)?),
            Site::Region(id) => Child::Region(RegionInst::create(u, id, frames)?),
        });
    }
    Ok(out)
}

fn roots_of(children: &[Child]) -> Vec<ViewId> {
    let mut out = Vec::new();
    push_roots(children, &mut out);
    out
}

fn push_roots(children: &[Child], out: &mut Vec<ViewId>) {
    for c in children {
        match c {
            Child::Node(n) => out.push(n.view),
            Child::Region(r) => r.collect_roots(out),
        }
    }
}

fn destroy_all(u: &mut Update<'_>, children: Vec<Child>) {
    for c in children {
        match c {
            Child::Node(n) => n.destroy(u),
            Child::Region(r) => r.destroy(u),
        }
    }
}

/// Update the children whose reads may have changed; whether the roots
/// they contribute to their parent's child list changed.
fn update_all(
    u: &mut Update<'_>,
    children: &mut [Child],
    frames: &[Frame],
) -> Result<bool, InstanceError> {
    let deps = &u.sites.deps;
    let mut roots = false;
    for c in children.iter_mut() {
        match c {
            Child::Node(n) => {
                if u.stale(&deps.nodes[n.node.0 as usize]) {
                    n.update(u, frames)?;
                }
            }
            Child::Region(r) => {
                if u.stale(&deps.regions[r.region.0 as usize])
                    || r.window.as_ref().is_some_and(|w| w.scroll_requested())
                {
                    roots |= r.update(u, frames, false)?;
                } else {
                    u.work.regions_skipped += 1;
                }
            }
        }
    }
    Ok(roots)
}

/// Visit a kept row when anything its body reads changed, the row's own
/// item included (`dirty`); whether its roots changed.
fn update_row(
    u: &mut Update<'_>,
    row: &mut Row,
    frames: &[Frame],
    dirty: bool,
    body: &Reads,
) -> Result<bool, InstanceError> {
    let saved = u.enter(dirty, Some(&row.slots));
    let result = if u.stale(body) {
        let inner = with_frame(frames, row.frame.clone());
        update_all(u, &mut row.roots, &inner)
    } else {
        u.work.rows_reused += 1;
        Ok(false)
    };
    u.leave(saved);
    result
}

impl NodeInst {
    fn create(
        u: &mut Update<'_>,
        node: NodesId,
        frames: &[Frame],
    ) -> Result<NodeInst, InstanceError> {
        u.work.nodes_visited += 1;
        let plan = u.env.plan;
        let row = plan.node(node);
        let node_type = NodeType::from_wire(row.node_type)
            .ok_or(InstanceError::UnknownNodeType(row.node_type))?;
        let view = u.ids.fresh();
        u.ids.sites.insert(view, node);
        u.ops.push(Op::CreateView {
            id: view,
            node_type,
        });
        let mut inst = NodeInst {
            node,
            view,
            last: vec![None; row.bindings.len as usize],
            last_surface: None,
            children: Vec::new(),
            last_children: Vec::new(),
            collection: None,
        };
        inst.emit_bindings(u, frames, true)?;
        inst.collection = collection::Collection::create(u, node, view, frames)?;
        if inst.collection.is_none() {
            inst.children = if node_type == NodeType::List
                && (inst
                    .bound_prop(u.env.plan, exact_kernel::PropId::ItemHeight)
                    .is_some()
                    || inst
                        .bound_prop(u.env.plan, exact_kernel::PropId::EstimatedItemHeight)
                        .is_some())
            {
                inst.list_children(u, frames)?
            } else {
                realize(u, Some(node), row.arm, frames)?
            };
            inst.emit_children(u);
        }
        Ok(inst)
    }

    fn emit_bindings(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        fresh: bool,
    ) -> Result<(), InstanceError> {
        let plan = u.env.plan;
        let row = plan.node(self.node);
        let mut patch: Option<StyleProps> = None;
        let deps = &u.sites.deps;
        for (i, b) in row.bindings.iter().enumerate() {
            if !fresh && !u.stale(&deps.bindings[b.0 as usize]) {
                continue;
            }
            let binding = plan.binding(b);
            u.work.bindings_evaluated += 1;
            let value = u.eval(binding.expr, frames)?;
            if self.last[i]
                .as_ref()
                .is_some_and(|last| crate::compare::equal(last, &value) == Some(true))
            {
                continue;
            }
            // A binding is a declaration whose value is computed from state,
            // as a `var()` reference is. A value its row's grammar refuses is
            // invalid at computed-value time (CSS Custom Properties §3.1): the
            // row is unset — inherited or initial — and a journal line says
            // so. Not CSSOM's `setProperty`, which keeps the earlier value:
            // the view would then depend on history, not state. Unknown rows
            // are plan defects.
            match binding.kind {
                BindingKind::Prop => match bridge::prop_value(binding.id, &value) {
                    Ok((prop, pv)) => u.ops.push(Op::SetProp {
                        id: self.view,
                        prop,
                        value: pv,
                    }),
                    Err(bridge::BridgeError::PropKind { prop, .. }) => {
                        u.ops.push(Op::ClearProp {
                            id: self.view,
                            prop,
                        });
                        u.notes.push(invalid(self.view, prop.name(), &value));
                    }
                    Err(e) => return Err(InstanceError::Bridge(e)),
                },
                BindingKind::Style => {
                    let p = patch.get_or_insert_with(StyleProps::default);
                    match bridge::set_style(p, binding.id, &value, plan.stacks.len()) {
                        Ok(_) => {}
                        Err(
                            bridge::BridgeError::Style(_) | bridge::BridgeError::StyleKind { .. },
                        ) => {
                            let style = exact_kernel::StyleId::from_bit(binding.id as u32)
                                .expect("known to the bridge");
                            let mut mask = exact_kernel::StyleMask::default();
                            mask.set(style);
                            u.ops.push(Op::ClearStyle {
                                id: self.view,
                                mask,
                            });
                            let name = style.name().replace('_', "-");
                            u.notes.push(invalid(self.view, &name, &value));
                        }
                        Err(e) => return Err(InstanceError::Bridge(e)),
                    }
                }
            }
            self.last[i] = Some(value);
        }
        if let Some(p) = patch {
            u.ops.push(Op::SetStyle {
                id: self.view,
                patch: Box::new(p),
            });
        }
        // A canvas's surface inputs: evaluated with the bindings (so a trap
        // refuses the commit whole), published only with a successful apply.
        if let Some(surface) = row.surface.filter(|s| {
            fresh
                || plan
                    .surface(*s)
                    .args
                    .iter()
                    .any(|a| u.stale(&deps.surface_args[a.0 as usize]))
        }) {
            let s = plan.surface(surface);
            let mut values = Vec::with_capacity(s.args.len as usize);
            for a in s.args.iter() {
                values.push(u.eval(plan.surface_arg(a).expr, frames)?);
            }
            if self.last_surface.as_ref() != Some(&values) {
                u.surfaces.push(SurfaceUpdate {
                    view: self.view,
                    name: plan.str(s.name).to_string(),
                    mode: s.mode,
                    names: s
                        .args
                        .iter()
                        .map(|a| plan.str(plan.surface_arg(a).name))
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    values: values.clone(),
                });
                self.last_surface = Some(values);
            }
        }
        Ok(())
    }

    fn emit_children(&mut self, u: &mut Update<'_>) {
        let now = roots_of(&self.children);
        if now != self.last_children {
            u.ops.push(Op::SetChildren {
                id: self.view,
                children: now.clone(),
            });
            self.last_children = now;
        }
    }

    fn update(&mut self, u: &mut Update<'_>, frames: &[Frame]) -> Result<(), InstanceError> {
        u.work.nodes_visited += 1;
        let old_top = self
            .bound_prop(u.env.plan, exact_kernel::PropId::ScrollTop)
            .cloned();
        self.emit_bindings(u, frames, false)?;
        self.prepare_list(u.env.plan, old_top)?;

        if let Some(collection) = &mut self.collection {
            let follow = u
                .env
                .plan
                .node(self.node)
                .bindings
                .iter()
                .enumerate()
                .find_map(|(i, b)| {
                    let b = u.env.plan.binding(b);
                    (b.kind == BindingKind::Prop
                        && b.id == exact_kernel::PropId::ScrollFollowEnd as u16)
                        .then_some(self.last[i] == Some(Value::Bool(true)))
                })
                .unwrap_or(false);
            collection.follow_end(follow);
            collection.update_data(u, frames, false)?;
        } else if update_all(u, &mut self.children, frames)? {
            self.emit_children(u);
        }
        Ok(())
    }

    fn destroy(self, u: &mut Update<'_>) {
        // Destroying the view destroys its subtree in the kernel; the instance
        // tree just drops.
        u.ops.push(Op::DestroyView { id: self.view });
    }
}

impl RegionInst {
    fn create(
        u: &mut Update<'_>,
        region: RegionsId,
        frames: &[Frame],
    ) -> Result<RegionInst, InstanceError> {
        let mut inst = RegionInst {
            region,
            window: None,
            subject: None,
            active: match u.env.plan.region(region).kind {
                RegionKind::Each => Active::Rows { rows: Vec::new() },
                _ => Active::Arm {
                    arm: None,
                    frame: Frame::default(),
                    roots: Vec::new(),
                },
            },
        };
        inst.update(u, frames, true)?;
        Ok(inst)
    }

    /// Bring the region up to date; whether its roots changed. `fresh`: its
    /// first realization, where nothing has been evaluated yet.
    fn update(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        fresh: bool,
    ) -> Result<bool, InstanceError> {
        let plan = u.env.plan;
        let deps = &u.sites.deps;
        let index = self.region.0 as usize;
        let row = plan.region(self.region);
        let subject_stale = fresh || u.stale(&deps.subjects[index]);
        if let Some(mut window) = self.window.take() {
            let result = if subject_stale || u.stale_outside(&deps.keys[index], 1) {
                match u.eval(row.subject, frames) {
                    Ok(Value::List(items)) => {
                        window.replace(u, &mut self.active, self.region, items, frames)
                    }
                    Ok(_) => Err(InstanceError::SubjectKind {
                        region: self.region,
                    }),
                    Err(trap) => Err(trap.into()),
                }
            } else {
                // The same items and keys: only mounted rows may be stale.
                window.refresh(u, &mut self.active, self.region, frames)
            };
            self.window = Some(window);
            // The window's content view is its one root.
            return result.map(|_| false);
        }
        match (&row.kind, &mut self.active) {
            (RegionKind::When, Active::Arm { arm, frame, roots }) => {
                let want = if !subject_stale {
                    *arm
                } else {
                    match u.eval(row.subject, frames)? {
                        Value::Bool(true) => Some(0),
                        Value::Bool(false) if row.arms.len > 1 => Some(1),
                        Value::Bool(false) => None,
                        _ => {
                            return Err(InstanceError::SubjectKind {
                                region: self.region,
                            })
                        }
                    }
                };
                Self::switch(
                    u,
                    self.region,
                    arm,
                    frame,
                    roots,
                    want,
                    Frame::default(),
                    frames,
                )
            }
            (RegionKind::Match, Active::Arm { arm, frame, roots }) => {
                let (want, new_frame) = if !subject_stale {
                    (*arm, frame.clone())
                } else {
                    match u.eval(row.subject, frames)? {
                        Value::Option(Some(v)) => (
                            Some(0),
                            Frame {
                                item: None,
                                bound: Some((*v).clone()),
                                ..Default::default()
                            },
                        ),
                        Value::Option(None) => (
                            if row.arms.len > 1 { Some(1) } else { None },
                            Frame::default(),
                        ),
                        _ => {
                            return Err(InstanceError::SubjectKind {
                                region: self.region,
                            })
                        }
                    }
                };
                Self::switch(u, self.region, arm, frame, roots, want, new_frame, frames)
            }
            (RegionKind::Each, Active::Rows { rows }) => {
                let body = &deps.bodies[index];
                let keys_stale = fresh || u.stale_outside(&deps.keys[index], 1);
                let subject = if subject_stale || keys_stale {
                    Some(u.eval(row.subject, frames)?)
                } else {
                    None
                };
                // The same list object keyed by unchanged inputs is the same keys.
                let rekey = match (&subject, &self.subject) {
                    (None, _) => false,
                    (Some(new), Some(old)) => keys_stale || !crate::compare::same(new, old),
                    (Some(_), None) => true,
                };
                if !rekey {
                    let mut roots = false;
                    for r in rows.iter_mut() {
                        roots |= update_row(u, r, frames, false, body)?;
                    }
                    return Ok(roots);
                }
                let subject = subject.expect("rekeyed");
                let Value::List(items) = &subject else {
                    return Err(InstanceError::SubjectKind {
                        region: self.region,
                    });
                };
                let arm = row.arms.iter().next();
                // Key every item; refuse duplicates.
                let mut keyed: Vec<(String, Value, Frame)> = Vec::with_capacity(items.len());
                let mut seen: BTreeMap<String, ()> = BTreeMap::new();
                for item in items.iter() {
                    u.work.rows_keyed += 1;
                    let frame = Frame {
                        item: Some(item.clone()),
                        bound: None,
                        ..Default::default()
                    };
                    let inner = with_frame(frames, frame.clone());
                    let key = u.eval(row.key, &inner)?;
                    let key_text = key_text(&key).ok_or(InstanceError::KeyKind {
                        region: self.region,
                    })?;
                    if seen.insert(key_text.clone(), ()).is_some() {
                        return Err(InstanceError::DuplicateKey {
                            region: self.region,
                        });
                    }
                    keyed.push((key_text, key, frame));
                }
                // Reuse rows by key, create the new, destroy the gone; order follows the items.
                // A linear search per row made an unchanged 10,000-row list
                // quadratic. Reuse the same canonical keys as duplicate checking.
                // Keep old order for destruction, which is observable in receipts.
                let mut old: Vec<Option<Row>> =
                    std::mem::take(rows).into_iter().map(Some).collect();
                let by_key: BTreeMap<String, usize> = old
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            key_text(&r.as_ref().unwrap().key).expect("validated key"),
                            i,
                        )
                    })
                    .collect();
                let mut roots = keyed.len() != old.len();
                let mut next = Vec::with_capacity(keyed.len());
                for (position, (key_text, key, mut frame)) in keyed.into_iter().enumerate() {
                    let found = by_key.get(&key_text).copied();
                    let existing = found.and_then(|i| old[i].take());
                    frame.region = Some(self.region.0);
                    match existing {
                        Some(mut r) => {
                            roots |= found != Some(position);
                            // Row bodies may distinguish signed zero (`1 / n > 0`):
                            // compare by bits, not by the language's `==`. An
                            // equivalent item keeps its object for nested memos.
                            let dirty = !crate::compare::equivalent_opt(&r.frame.item, &frame.item);
                            if !dirty {
                                frame.item = r.frame.item.take();
                            }
                            frame.row = Some(r.slots.clone());
                            r.frame = frame;
                            roots |= update_row(u, &mut r, frames, dirty, body)?;
                            next.push(r);
                        }
                        None => {
                            roots = true;
                            // A new row: its slots start from their initializers,
                            // evaluated here so an initializer may read the item.
                            let slots: RowSlots = Rc::new(RefCell::new(BTreeMap::new()));
                            frame.row = Some(slots.clone());
                            let inner = with_frame(frames, frame.clone());
                            for (i, s) in plan.slots.iter().enumerate() {
                                if s.owner == Some(self.region) {
                                    let v = u.eval(s.init, &inner)?;
                                    if !v.conforms(plan, s.ty) {
                                        return Err(InstanceError::SlotType {
                                            slot: plan.str(s.name).to_string(),
                                        });
                                    }
                                    slots.borrow_mut().insert(i as u32, v);
                                }
                            }
                            let roots = realize(u, None, arm, &inner)?;
                            next.push(Row {
                                wrapper: None,
                                key,
                                frame,
                                roots,
                                slots,
                            });
                        }
                    }
                }
                for gone in old.into_iter().flatten() {
                    destroy_all(u, gone.roots);
                }
                *rows = next;
                self.subject = Some(subject);
                Ok(roots)
            }
            _ => Ok(false),
        }
    }

    /// Show arm `want`; whether the roots changed.
    #[allow(clippy::too_many_arguments)]
    fn switch(
        u: &mut Update<'_>,
        region: RegionsId,
        arm: &mut Option<usize>,
        frame: &mut Frame,
        roots: &mut Vec<Child>,
        want: Option<usize>,
        new_frame: Frame,
        frames: &[Frame],
    ) -> Result<bool, InstanceError> {
        let plan = u.env.plan;
        if *arm == want {
            // An equivalent binding keeps its object for nested memos.
            let dirty = !crate::compare::equivalent_opt(&frame.bound, &new_frame.bound);
            if dirty {
                *frame = new_frame;
            }
            let saved = u.enter(dirty, None);
            let result = update_all(u, roots, &with_frame(frames, frame.clone()));
            u.leave(saved);
            return result;
        }
        let inner = with_frame(frames, new_frame.clone());
        let old = std::mem::take(roots);
        destroy_all(u, old);
        *arm = want;
        *frame = new_frame;
        if let Some(i) = want {
            let arm_id = plan.region(region).arms.iter().nth(i);
            *roots = realize(u, None, arm_id, &inner)?;
        }
        Ok(true)
    }

    fn collect_roots(&self, out: &mut Vec<ViewId>) {
        if let Some(window) = &self.window {
            out.push(window.content);
            return;
        }
        match &self.active {
            Active::Arm { roots, .. } => push_roots(roots, out),
            Active::Rows { rows } => {
                for r in rows {
                    push_roots(&r.roots, out);
                }
            }
        }
    }

    fn destroy(self, u: &mut Update<'_>) {
        if let Some(window) = self.window {
            u.ops.push(Op::DestroyView { id: window.content });
            return;
        }
        match self.active {
            Active::Arm { roots, .. } => destroy_all(u, roots),
            Active::Rows { rows } => {
                for r in rows {
                    destroy_all(u, r.roots);
                }
            }
        }
    }
}

/// The journal line for a value a row refused.
fn invalid(view: ViewId, row: &str, value: &Value) -> String {
    let mut shown = String::new();
    crate::agent::untyped_json(value, &mut shown);
    format!("view {view}: invalid {row} value {shown}; unset")
}

/// One canonical key text: strings, finite numbers (`-0` is `0`, matching the
/// VM's equality), bools. NaN is not a key.
fn key_text(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(format!("s:{s}")),
        Value::Number(n) if n.is_finite() => {
            Some(format!("n:{}", if *n == 0.0 { 0.0 } else { *n }))
        }
        Value::Bool(b) => Some(format!("b:{b}")),
        _ => None,
    }
}

/// The root of the instance tree: the plan's top-level sites.
#[derive(Debug)]
pub struct Tree {
    has_collections: bool,
    children: Vec<Child>,
    last_roots: Vec<ViewId>,
    /// The inputs the kernel shows, as of the last create or update.
    seen: Seen,
    /// Work performed by the last instance update.
    pub last_work: InstanceWork,
}

impl Tree {
    /// Realize the plan's root sites.
    pub fn create(u: &mut Update<'_>) -> Result<Tree, InstanceError> {
        let children = realize(u, None, None, &[])?;
        let mut tree = Tree {
            has_collections: u.env.plan.bindings.iter().any(|b| {
                b.kind == BindingKind::Prop
                    && b.id == exact_kernel::PropId::Virtualized as u16
                    && u.env.plan.code(b.expr)
                        != [
                            exact_plan::Opcode::Bool as u8,
                            0,
                            exact_plan::Opcode::Return as u8,
                        ]
            }),
            children,
            last_roots: Vec::new(),
            seen: Seen::of(&u.env),
            last_work: u.work,
        };
        tree.emit_roots(u);
        Ok(tree)
    }

    /// Bring every site whose reads changed since the last update up to date
    /// (every site, under [`Update::full`]).
    pub fn update(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        u.changed = Some(u.sites.deps.changed(&self.seen, &u.env));
        let roots = update_all(u, &mut self.children, &[]);
        u.changed = None;
        if roots? {
            self.emit_roots(u);
        }
        if self.has_collections && u.ops.iter().any(|op| matches!(op, Op::SetStyle { patch, .. } if patch.mask.intersects(exact_kernel::StyleMask::TEXT))) {
            collection::invalidate_typography(&mut self.children, u, &[])?;
        }
        // Views are never reused within a runner. Detach removed children in
        // the final child lists before destroying them, so a removed list does
        // not rebuild its parent's siblings once per row. Still one atomic
        // batch; preserve relative destroy order for receipts and host effects.
        let (mut live, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut u.ops)
            .into_iter()
            .partition(|op| !matches!(op, Op::DestroyView { .. }));
        live.extend(gone);
        u.ops = live;
        self.seen = Seen::of(&u.env);
        self.last_work = u.work;
        Ok(())
    }

    fn emit_roots(&mut self, u: &mut Update<'_>) {
        let roots = roots_of(&self.children);
        for r in &roots {
            if !self.last_roots.contains(r) {
                u.ops.push(Op::AttachRoot { id: *r });
            }
        }
        self.last_roots = roots;
    }

    /// The current kernel roots.
    pub fn roots(&self) -> Vec<ViewId> {
        roots_of(&self.children)
    }

    /// The site owning `view` and the instance path to it: the regions
    /// crossed, with the row key or the active arm at each (LLP 1035.002).
    pub fn site(&self, view: ViewId) -> Option<(NodesId, Vec<InstanceStep>)> {
        let mut path = Vec::new();
        for c in &self.children {
            let found = match c {
                Child::Node(n) => n.site(view, &mut path),
                Child::Region(r) => r.site(view, &mut path),
            };
            if let Some(node) = found {
                return Some((node, path));
            }
        }
        None
    }
}

impl NodeInst {
    fn site(&self, view: ViewId, path: &mut Vec<InstanceStep>) -> Option<NodesId> {
        if self.view == view {
            return Some(self.node);
        }
        if let Some(collection) = &self.collection {
            if let Some(found) = collection.site(view, path) {
                return Some(found);
            }
        }
        for c in &self.children {
            let found = match c {
                Child::Node(n) => n.site(view, path),
                Child::Region(r) => r.site(view, path),
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }
}

impl RegionInst {
    fn site(&self, view: ViewId, path: &mut Vec<InstanceStep>) -> Option<NodesId> {
        let walk = |roots: &[Child], path: &mut Vec<InstanceStep>| -> Option<NodesId> {
            for c in roots {
                let found = match c {
                    Child::Node(n) => n.site(view, path),
                    Child::Region(r) => r.site(view, path),
                };
                if found.is_some() {
                    return found;
                }
            }
            None
        };
        match &self.active {
            Active::Arm { roots, arm, .. } => {
                path.push(InstanceStep::Arm {
                    region: self.region,
                    arm: *arm,
                });
                if let Some(found) = walk(roots, path) {
                    return Some(found);
                }
                path.pop();
                None
            }
            Active::Rows { rows } => {
                for r in rows {
                    path.push(InstanceStep::Row {
                        region: self.region,
                        key: r.key.clone(),
                    });
                    if let Some(found) = walk(&r.roots, path) {
                        return Some(found);
                    }
                    path.pop();
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod site_tests {
    use super::*;
    use exact_plan::builder::PlanBuilder;

    #[test]
    fn site_index_keeps_parent_arm_and_stable_mixed_order() {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let root = b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
        let late = b.node(NodeType::Text as u8, Some(root), None, 5, &[], &[], None);
        let early = b.node(NodeType::Text as u8, Some(root), None, 1, &[], &[], None);
        let yes = b.constant(&Value::Bool(true));
        let unit = b.constant(&Value::Unit);
        let (first, arms) = b.region(RegionKind::When, Some(root), None, 5, yes, unit, 2);
        let (second, _) = b.region(RegionKind::When, Some(root), None, 5, yes, unit, 2);
        let left = b.node(NodeType::Text as u8, None, Some(arms[0]), 0, &[], &[], None);
        let right = b.node(NodeType::Text as u8, None, Some(arms[1]), 0, &[], &[], None);
        let nested = b.node(
            NodeType::Text as u8,
            Some(left),
            Some(arms[0]),
            0,
            &[],
            &[],
            None,
        );
        let plan = b.finish().unwrap();
        let sites = SiteIndex::new(&plan);
        assert_eq!(sites.children(None, None), &[(0, Site::Node(root))]);
        assert_eq!(
            sites.children(Some(root), None),
            &[
                (1, Site::Node(early)),
                (5, Site::Region(first)),
                (5, Site::Node(late)),
                (5, Site::Region(second)),
            ]
        );
        assert_eq!(
            sites.children(None, Some(arms[0])),
            &[(0, Site::Node(left))]
        );
        assert_eq!(
            sites.children(None, Some(arms[1])),
            &[(0, Site::Node(right))]
        );
        assert_eq!(
            sites.children(Some(left), Some(arms[0])),
            &[(0, Site::Node(nested))]
        );
        assert!(sites.children(Some(left), Some(arms[1])).is_empty());
        assert!(sites.children(Some(right), Some(arms[1])).is_empty());
    }
}
