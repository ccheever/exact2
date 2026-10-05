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
mod document;
mod find;
mod region;
mod text;

use crate::bridge;
use crate::vm::{self, Env, Frame, RowSlots, Trap};
pub use deps::RowWrites;
use deps::{Bits, Reads, Seen};
pub(crate) use deps::{Deps, Input, Reads as DepReads};
pub use document::{DocNode, DocTree, DocTreeError};
use exact_kernel::{NodeType, Op, StyleProps, ViewId};
use exact_plan::{ArmsId, BindingKind, Items, NodesId, Plan, RegionKind, RegionsId, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// The list engine's entries (LLP 1047 D3; LLP 1047.000 §9): the core
/// reaches virtualized collections only through this table, and only
/// [`LISTS`] names it. `RunnerLinks::ALL` and the web's collections
/// capability are what name `LISTS`, so an artifact whose plan virtualizes
/// no list carries no engine. Admission (D6) refuses a plan that
/// virtualizes a list where the table is absent.
pub struct ListLinks {
    create: CreateList,
    update: fn(
        &mut collection::Collection,
        &mut Update<'_>,
        &[Frame],
        bool,
    ) -> Result<(), InstanceError>,
    typography: ListTypography,
    pub(crate) collections_json: fn(&Tree) -> String,
    /// `scrollIntoView` (LLP 1070.000): a request begun on its list.
    pub(crate) into_view: IntoViewLink,
    /// `state.scrollIntoView`: each list's latest request.
    pub(crate) into_view_json: fn(&Tree) -> String,
    /// `state.kept`: inner lists' kept positions (LLP 1070 §4.2).
    pub(crate) kept_json: fn(&Tree) -> String,
}

type IntoViewLink = fn(
    &mut Tree,
    &mut Update<'_>,
    &collection::IntoView,
) -> Result<collection::IntoViewStatus, InstanceError>;

type CreateList = fn(
    &mut Update<'_>,
    NodesId,
    ViewId,
    &[Frame],
) -> Result<Option<Box<collection::Collection>>, InstanceError>;
type ListTypography = fn(&mut [Child], &mut Update<'_>, &[Frame]) -> Result<(), InstanceError>;

/// The list engine.
pub static LISTS: ListLinks = ListLinks {
    create: collection::Collection::create,
    update: collection::update_collection,
    typography: collection::invalidate_typography,
    collections_json: collection::collections_json,
    into_view: Tree::scroll_into_view,
    into_view_json: Tree::into_view_json,
    kept_json: Tree::kept_positions_json,
};

/// A list engine's state where the plan has no way to reach it: admission
/// refused the plan, so this is the runner's defect, named rather than
/// trapped on.
fn unlinked() -> InstanceError {
    InstanceError::Collection(
        "a list engine is in use but this artifact doesn't link lists (LLP 1047 D6)".into(),
    )
}

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
    key: Value,
    /// Which repeat of `key` this row is: 0 for the first; the data gave the
    /// rest the same key, and they are told apart by order (see [`ident`]).
    dup: u32,
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
    sites: exact_kernel::id::IdMap<ViewId, NodesId>,
    /// Each plan site's work across its instances' lifetimes, by node
    /// index; empty while nothing is measured (LLP 1079 D1).
    pub(crate) work: crate::perf::Work,
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

    /// Forget views `live` says are gone; a measured site keeps the count.
    pub fn retain(&mut self, live: impl Fn(ViewId) -> bool) {
        let work = &mut self.work.sites;
        self.sites.retain(|view, node| {
            let keep = live(*view);
            if !keep {
                if let Some(w) = work.get_mut(node.0 as usize) {
                    w.forgotten += 1;
                }
            }
            keep
        });
    }

    /// A binding of `node`'s evaluated, and whether it came out as before.
    fn evaluated(&mut self, node: NodesId, unchanged: bool) {
        if let Some(w) = self.work.sites.get_mut(node.0 as usize) {
            w.evaluated += 1;
            w.unchanged += unchanged as u64;
        }
    }

    /// How many views are remembered.
    pub fn remembered(&self) -> usize {
        self.sites.len()
    }
}

/// A value as the JSON a surface's arguments cross in: numbers shortest,
/// records and lists as arrays, `none` as `null`.
pub fn value_json(value: &Value, out: &mut String) {
    match value {
        Value::Number(n) if n.is_finite() => {
            exact_num::push_text!(out, "{}", exact_num::Shortest(*n))
        }
        Value::Number(_) | Value::Unit | Value::Option(None) => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        v @ exact_plan::str_value!() => crate::agent::quote(v.text(), out),
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
    /// Rows examined, visited or not: an `each`'s kept rows, a list's
    /// mounted rows, and the rows compared to find the event's view.
    pub rows_scanned: usize,
    /// Derive bodies settlement evaluated (the rest kept their values).
    pub derives_evaluated: usize,
    /// Store bytes copied so a refusal could put the store back.
    pub store_bytes_copied: usize,
    /// Retiring list rows rebound to another item instead of building one
    /// (LLP 1078).
    pub rows_rebound: usize,
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
    /// The fields of the innermost frame's value that changed, when bit 0
    /// of `dirty_frames` is set ([`crate::compare::changed_fields`]).
    dirty_fields: u64,
    /// Whether the scopes walked so far enclose every written row, or lie
    /// inside one.
    on_path: bool,
    /// Whether a row the scopes walked so far entered was itself written.
    /// Every scope nested in it — the rows of an `each` in a child component
    /// the row instantiated — reads that row's slots through its frames
    /// ([`vm::Frame::row_of`]), so none of them leaves the path.
    in_written: bool,
    /// Journal lines for what the data got wrong and the tree absorbed: an
    /// invalid style or prop value, a repeated key. Written with the commit.
    pub notes: Vec<String>,
    /// No kernel mirrors the tree (a detached kernel's runner, LLP
    /// 1048.004): bindings are evaluated and kept, and no style or prop op
    /// is built. The document is written from the tree ([`Tree::document`]).
    pub discard: bool,
    /// A discarded update changed a text row (`StyleMask::TEXT`), which a
    /// kept update would have written as a style op: what tells a list its
    /// rows' typography changed ([`Tree::update`]).
    text_styled: bool,
    /// The views of list rows rebound to another item (LLP 1078), every one
    /// a fresh mount to motion and to the host: the commit's receipt names
    /// them `renewed`.
    pub renewed: Vec<ViewId>,
    /// Rebind retiring list rows to new items (LLP 1078), as the runner was
    /// told ([`crate::Runner::set_row_reuse`]).
    pub reuse: bool,
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
            dirty_fields: 0,
            on_path: true,
            in_written: false,
            notes: Vec::new(),
            discard: false,
            text_styled: false,
            renewed: Vec::new(),
            reuse: false,
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
        self.full || self.changed_outside(reads, shift)
    }

    /// Whether an input `reads` names actually changed, whatever
    /// [`Update::full`] says: what decides host-visible protocol state (a
    /// list's measurement epochs and revision) identically in both modes.
    fn changed_outside(&self, reads: &Reads, shift: u32) -> bool {
        let Some(changed) = &self.changed else {
            return true;
        };
        let frames = reads.frames_outside(shift) & self.dirty_frames;
        reads.opaque
            || reads.bits.intersects(changed)
            // Only the innermost frame, seen from inside it: only if a field
            // read is a field that changed (LLP 1017.003 D6).
            || (frames == 1 && shift == 0 && reads.fields & self.dirty_fields != 0)
            || (frames != 0 && (frames != 1 || shift != 0))
            || (reads.row_slots
                && self.on_path
                && self
                    .rows
                    .writes
                    .iter()
                    .any(|(_, slot)| reads.bits.get(*slot as usize)))
    }

    /// Enter a row or arm scope whose frame value changed in the fields
    /// `dirty` names (none: unchanged).
    fn enter(&mut self, dirty: u64, row: Option<&RowSlots>) -> (u64, u64, bool, bool) {
        let saved = (
            self.dirty_frames,
            self.dirty_fields,
            self.on_path,
            self.in_written,
        );
        self.dirty_frames = deps::into_scope(self.dirty_frames, dirty != 0);
        self.dirty_fields = dirty;
        // A row off every written row's scope chain reads none of their
        // slots, unless it is nested in a written row: it reads that row's
        // slots as an enclosing frame's.
        if let Some(row) = row.filter(|_| !self.in_written) {
            let id = RowWrites::id(row);
            self.on_path &= self.rows.path.contains(&id);
            self.in_written = self.on_path && self.rows.writes.iter().any(|(w, _)| *w == id);
        }
        saved
    }

    fn leave(&mut self, saved: (u64, u64, bool, bool)) {
        (
            self.dirty_frames,
            self.dirty_fields,
            self.on_path,
            self.in_written,
        ) = saved;
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
    /// The plan's `@keyframes`, parsed once (LLP 1055 D5).
    keyframes: bridge::KeyframesTable,
    /// A binding that reads nothing (a literal style row in a list row's
    /// template) has one value for the plan's life: evaluated once, by
    /// binding index, instead of once per instance.
    constants: Vec<std::cell::OnceCell<Value>>,
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
        let key = |i: usize| (entries[i].0, entries[i].1, entries[i].2.rank());
        let order = stable_order(entries.len(), &|i, j| key(i) < key(j));
        let mut groups: Vec<(SiteParent, std::ops::Range<usize>)> = Vec::new();
        let mut sites = Vec::with_capacity(entries.len());
        for (parent, order, site) in order.into_iter().map(|i| entries[i]) {
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
            keyframes: bridge::keyframes(plan),
            constants: (0..plan.bindings.len())
                .map(|_| Default::default())
                .collect(),
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

/// The indices `0..n` in the order a stable sort by `less` gives. A bottom-up
/// merge sort written once, so each caller doesn't link its own
/// instantiation of core's sort (LLP 1047 §6).
fn stable_order(n: usize, less: &dyn Fn(usize, usize) -> bool) -> Vec<usize> {
    let mut a: Vec<usize> = (0..n).collect();
    let mut b = a.clone();
    let mut width = 1;
    while width < n {
        let mut start = 0;
        while start < n {
            let mid = (start + width).min(n);
            let end = (start + 2 * width).min(n);
            let (mut l, mut r) = (start, mid);
            for slot in &mut b[start..end] {
                // The right run's element goes first only when strictly less.
                let right = r < end && (l == mid || less(a[r], a[l]));
                if right {
                    *slot = a[r];
                    r += 1;
                } else {
                    *slot = a[l];
                    l += 1;
                }
            }
            start = end;
        }
        std::mem::swap(&mut a, &mut b);
        width *= 2;
    }
    a
}

/// For each of `n` texts, how many earlier texts equal it.
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
                if u.stale(&deps.regions[r.region.0 as usize]) {
                    roots |= r.update(u, frames, false)?;
                } else {
                    u.work.regions_skipped += 1;
                }
            }
        }
    }
    Ok(roots)
}

/// Visit a kept row when anything its body reads changed, the fields of its
/// own item that changed included (`dirty`, [`crate::compare::changed_fields`]);
/// whether its roots changed.
fn update_row(
    u: &mut Update<'_>,
    row: &mut Row,
    frames: &[Frame],
    dirty: u64,
    body: &Reads,
) -> Result<bool, InstanceError> {
    u.work.rows_scanned += 1;
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
        if let Some(w) = u.ids.work.sites.get_mut(node.0 as usize) {
            w.created += 1;
        }
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
        let lists = u.env.lists;
        inst.collection = match lists {
            Some(lists) => (lists.create)(u, node, view, frames)?,
            None => None,
        };
        if inst.collection.is_none() {
            inst.children = realize(u, Some(node), row.arm, frames)?;
            inst.emit_children(u);
        }
        Ok(inst)
    }

    /// The value a style binding last emitted.
    pub(super) fn bound_style(&self, plan: &Plan, style: exact_kernel::StyleId) -> Option<&Value> {
        plan.node(self.node)
            .bindings
            .iter()
            .enumerate()
            .find_map(|(i, b)| {
                let b = plan.binding(b);
                (b.kind == BindingKind::Style && b.id == style as u16)
                    .then(|| self.last[i].as_ref())
                    .flatten()
            })
    }

    /// The value a prop binding last emitted.
    pub(super) fn bound_prop(&self, plan: &Plan, prop: exact_kernel::PropId) -> Option<&Value> {
        plan.node(self.node)
            .bindings
            .iter()
            .enumerate()
            .find_map(|(i, b)| {
                let b = plan.binding(b);
                (b.kind == BindingKind::Prop && b.id == prop as u16)
                    .then(|| self.last[i].as_ref())
                    .flatten()
            })
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
            let reads = &deps.bindings[b.0 as usize];
            let before = u.work.bindings_evaluated;
            let value = if reads.is_constant() {
                match u.sites.constants[b.0 as usize].get() {
                    Some(value) => value.clone(),
                    None => {
                        u.work.bindings_evaluated += 1;
                        let value = u.eval(binding.expr, frames)?;
                        u.sites.constants[b.0 as usize].get_or_init(|| value.clone());
                        value
                    }
                }
            } else {
                u.work.bindings_evaluated += 1;
                u.eval(binding.expr, frames)?
            };
            let unchanged = self.last[i]
                .as_ref()
                .is_some_and(|last| crate::compare::equal(last, &value) == Some(true));
            if u.work.bindings_evaluated != before {
                u.ids.evaluated(self.node, unchanged);
            }
            if unchanged {
                continue;
            }
            if u.discard {
                if binding.kind == BindingKind::Style
                    && !matches!(value, Value::Option(None))
                    && exact_kernel::StyleId::from_bit(binding.id as u32)
                        .is_some_and(|style| exact_kernel::StyleMask::TEXT.has(style))
                {
                    u.text_styled = true;
                }
                self.last[i] = Some(value);
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
                // A class choice's row the chosen style does not set is
                // `none`: an explicit unset, cleared to the kernel's default.
                // So is CSS's `unset`, and `inherit` on an inherited row.
                BindingKind::Style
                    if matches!(value, Value::Option(None))
                        || bridge::unsets(binding.id, &value) =>
                {
                    let style = exact_kernel::StyleId::from_bit(binding.id as u32)
                        .expect("known to the bridge");
                    let mut mask = exact_kernel::StyleMask::default();
                    mask.set(style);
                    u.ops.push(Op::ClearStyle {
                        id: self.view,
                        mask,
                    });
                }
                BindingKind::Style => {
                    let p = patch.get_or_insert_with(StyleProps::default);
                    match bridge::set_plan_style(p, binding.id, &value, plan) {
                        Ok(exact_kernel::StyleId::Animation) => {
                            let dropped = u.sites.keyframes.resolve(&mut p.animation);
                            u.notes.extend(dropped);
                        }
                        // An exit names keyframes as `animation` does (LLP 1063).
                        Ok(exact_kernel::StyleId::ExitAnimation) => {
                            let dropped = u.sites.keyframes.resolve(&mut p.exit_animation);
                            u.notes.extend(dropped);
                        }
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
        self.emit_bindings(u, frames, false)?;

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
            (u.env.lists.ok_or_else(unlinked)?.update)(collection, u, frames, follow)?;
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

/// The journal line for a value a row refused.
fn invalid(view: ViewId, row: &str, value: &Value) -> String {
    let mut shown = String::new();
    crate::agent::untyped_json(value, &mut shown);
    format!("view {view}: invalid {row} value {shown}; unset")
}

/// A row's identity: its key's canonical text, or for the `dup`th repeat
/// of that key, `d{dup}:` before it — never a canonical text, which starts
/// `s:`, `n:` or `b:`.
fn ident(key: &Value, dup: u32) -> Option<String> {
    key_text(key).map(|text| disambiguate(text, dup))
}

fn disambiguate(text: String, dup: u32) -> String {
    if dup == 0 {
        text
    } else {
        format!("d{dup}:{text}")
    }
}

/// The journal line for a repeated key's row.
fn repeated(region: RegionsId, key: &Value, ident: &str) -> String {
    let mut shown = String::new();
    crate::agent::untyped_json(key, &mut shown);
    format!(
        "list {}: key {shown} repeats; this row is {ident}",
        region.0
    )
}

/// One canonical key text: strings, finite numbers (`-0` is `0`, matching the
/// VM's equality), bools. NaN is not a key.
fn key_text(v: &Value) -> Option<String> {
    let mut text = String::new();
    key_text_into(v, &mut text).then_some(text)
}

/// [`key_text`] appended to `out`; whether `v` is a key.
fn key_text_into(v: &Value, out: &mut String) -> bool {
    match v {
        v @ exact_plan::str_value!() => exact_num::push_text!(out, "s:{}", v.text()),
        Value::Number(n) if n.is_finite() => exact_num::push_text!(
            out,
            "n:{}",
            exact_num::Shortest(if *n == 0.0 { 0.0 } else { *n })
        ),
        Value::Bool(b) => exact_num::push_text!(out, "b:{}", b),
        _ => return false,
    }
    true
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
        if self.has_collections && (u.text_styled || u.ops.iter().any(|op| matches!(op, Op::SetStyle { patch, .. } if patch.mask.intersects(exact_kernel::StyleMask::TEXT)))) {
            if let Some(lists) = u.env.lists {
                (lists.typography)(&mut self.children, u, &[])?;
            }
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

    /// Resource `i`'s value was released to the plan's bytes
    /// ([`crate::held::Held::released`]): the last inputs hold it no more.
    pub(crate) fn release_resource(&mut self, i: usize, held: &crate::held::Held) {
        self.seen.release(i, held);
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
mod order_tests;

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
