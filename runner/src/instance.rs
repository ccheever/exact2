//! The instance tree, and the ops that keep the kernel equal to it.
//!
//! @ref LLP 0485 §8.2 (keyed instance management, never reconciliation of trees; research)
//!
//! The plan's nodes and regions are *sites*. An instance realizes a site as a
//! kernel view, an active `if`/`match` arm, or a `for` row per key. Updates
//! visit sites with changed inputs ([`deps`]) or enclosing rows/arms, evaluating
//! only stale bindings. They emit binding ops only for changed values,
//! `SetChildren` only for changed child lists, and create/destroy only for
//! added/removed keys. [`Update::full`] evaluates everything and is the reference
//! for incremental updates. No tree diff is needed: a row's key, not its position,
//! preserves its views across reorders.

/// Variable-height viewport collections and their portable host feedback seam.
mod bound;
pub mod collection;
mod deps;
mod document;
mod find;
mod foresee;
mod keys;
pub mod looks;
mod rare;
mod region;
pub mod tabs;
mod text;
pub mod theme;

use crate::bridge;
use crate::vm::{self, Env, Frame, RowSlots, Trap};
pub use deps::RowWrites;
use deps::{Bits, Reads, Seen};
pub(crate) use deps::{Deps, Input, Reads as DepReads};
pub use document::{DocNode, DocTree, DocTreeError};
use exact_kernel::{Kernel, NodeType, Op, StyleProps, ViewId};
use exact_plan::{ArmsId, BindingKind, Items, NodesId, Plan, RegionKind, RegionsId, Value};
pub use foresee::Mount;
pub(crate) use keys::key_text;
use keys::{disambiguate, ident, key_text_into, repeated};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// List engine entries (LLP 1047 D3; LLP 1047.000 §9), reached only through
/// [`LISTS`]. `RunnerLinks::ALL` and the web collections capability name it;
/// artifacts without virtualized lists carry no engine. Admission (D6)
/// refuses virtualized lists when this table is absent.
pub struct ListLinks {
    create: CreateList,
    update: UpdateList,
    typography: ListTypography,
    pub(crate) collections_json: fn(&Tree) -> String,
    /// `scrollIntoView` (LLP 1070.000): a request begun on its list.
    pub(crate) into_view: IntoViewLink,
    /// A logical row needed for a host's keyboard focus traversal.
    pub(crate) reveal_index: RevealIndexLink,
    pub(crate) scroll_anchor: ScrollAnchorLink,
    /// `state.scrollIntoView`: each list's latest request.
    pub(crate) into_view_json: fn(&Tree) -> String,
    /// `state.kept`: inner lists' kept positions (LLP 1070 §4.2).
    pub(crate) kept_json: fn(&Tree) -> String,
    /// What the platform covers of a list's port, where no row is aligned.
    pub(crate) covered: fn(&mut Tree, ViewId, [f64; 2]) -> bool,
}

type IntoViewLink = fn(
    &mut Tree,
    &mut Update<'_>,
    &collection::IntoView,
    &mut Option<collection::IntoView>,
) -> Result<collection::IntoViewStatus, InstanceError>;

type ScrollAnchorLink = fn(&Tree, &Kernel, ViewId, f64, f64, f64, f64) -> Option<(Value, f64)>;
type RevealIndexLink = fn(&mut Tree, &mut Update<'_>, ViewId, usize) -> Result<bool, InstanceError>;

type CreateList = fn(
    &mut Update<'_>,
    NodesId,
    ViewId,
    &[Frame],
) -> Result<Option<Box<collection::Collection>>, InstanceError>;
/// A list's data update, given whether it follows its end and its
/// `keep-mounted` value.
type UpdateList = fn(
    &mut collection::Collection,
    &mut Update<'_>,
    &[Frame],
    bool,
    Option<&Value>,
) -> Result<(), InstanceError>;
type ListTypography = fn(&mut [Child], &mut Update<'_>, &[Frame]) -> Result<(), InstanceError>;

/// The list engine.
pub static LISTS: ListLinks = ListLinks {
    create: collection::Collection::create,
    update: collection::update_collection,
    typography: collection::invalidate_typography,
    collections_json: collection::collections_json,
    into_view: Tree::scroll_into_view,
    reveal_index: Tree::reveal_collection_index,
    into_view_json: Tree::into_view_json,
    scroll_anchor: Tree::collection_scroll_anchor,
    kept_json: Tree::kept_positions_json,
    covered: Tree::set_list_covered,
};

/// A list engine's state where the plan has no way to reach it: admission
/// refused the plan, so this is the runner's defect, named rather than trapped on.
fn unlinked() -> InstanceError {
    InstanceError::Collection(
        "a list engine is in use but this artifact doesn't link lists (LLP 1047 D6)".into(),
    )
}

mod errors;
pub use errors::InstanceError;

/// One realized node.
#[derive(Debug)]
pub struct NodeInst {
    /// The site.
    pub node: NodesId,
    /// The kernel view.
    pub view: ViewId,
    /// Last emitted value per binding that can change, in binding order
    /// ([`NodeInst::slot`]); a constant binding's is the site index's.
    last: Box<[Option<Value>]>,
    /// Ordered children: static nodes and regions interleaved by `order`.
    children: Vec<Child>,
    /// Last emitted child list.
    last_children: Box<[ViewId]>,
    /// What few nodes keep, behind one pointer: `Child` is 112 bytes, a
    /// class of Apple's allocator, and most child vectors hold one.
    rare: Option<Box<rare::Rare>>,
    /// The app tuple its rows and children are in (LLP 1121 D5).
    tuple: u32,
    /// Which of its bindings' text read a token.
    themed: theme::Themed,
    /// One of its inherited colour rows was set since its children last
    /// took the text they inherit (D7): they are visited again.
    colour_moved: bool,
    /// The tuple around it when its rows were last set.
    around: u32,
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
    /// A `for` subject as last keyed: the same object again, with the key's
    /// other inputs unchanged, is the same keys.
    subject: Option<Value>,
}

#[derive(Debug)]
enum Active {
    /// `if` / `match`: the active arm and its roots.
    Arm {
        arm: Option<usize>,
        frame: ArmFrame,
        roots: Vec<Child>,
    },
    /// `for`: rows by key, in current order.
    Rows { rows: Vec<Row> },
}

/// A conditional arm has a match binding and optional owned state, but no
/// loop item or index. Expand those absent fields only while visiting its scope.
#[derive(Debug, Clone, Default)]
struct ArmFrame {
    bound: Option<Value>,
    row: Option<RowSlots>,
    region: Option<u32>,
}

impl ArmFrame {
    fn scope(&self) -> Frame {
        Frame {
            bound: self.bound.clone(),
            row: self.row.clone(),
            region: self.region,
            ..Frame::default()
        }
    }
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
    /// Ordinary stateless rows omit it unless nested state needs the identity.
    slots: Option<RowSlots>,
}

/// One step of the instance path from the plan's roots to a view: the
/// region crossed and, for a `for` row, its key; for an `if`/`match`
/// region, the active arm (LLP 1035.002 D6 — what identifies *this*
/// instance of a repeated site).
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceStep {
    /// A keyed row of a `for`.
    Row {
        /// The region.
        region: RegionsId,
        /// The row's key.
        key: Value,
    },
    /// The active arm of an `if`/`match`.
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
    /// Every instance node's view and site, destroyed ones kept till [`Ids::retain`] drops them.
    sites: exact_kernel::id::IdMap<ViewId, NodesId>,
    /// Each plan site's work across its instances' lifetimes, by node
    /// index; empty while nothing is measured (LLP 1079 D1).
    pub(crate) work: crate::perf::Work,
    /// Every row that read a theme token (LLP 1121 D7).
    pub theme: theme::ThemeIndex,
    pub(crate) tabs: tabs::Tabs, // tab panels waiting for their first selection (tabs.rs)
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
        self.theme.retain(&live);
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
    /// Rows examined, visited or not: a `for`'s kept rows, a list's
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
    /// Run-time installed-family stack ids shared across commits in a session.
    pub fonts: Option<Rc<RefCell<bridge::RuntimeFonts>>>,
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
    /// Every scope nested in it — the rows of a `for` in a child component
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
    /// List rows mounted out of their port, and those that showed (LLP 1055 D13).
    pub shown: collection::shown::RowsShown,
    /// Rebind retiring list rows to new items (LLP 1078), as the runner was
    /// told ([`crate::Runner::set_row_reuse`]).
    pub reuse: bool,
    /// Reuse operand and local storage while visiting this update's bindings.
    scratch: vm::Scratch,
    /// Empty lexical-frame buffers, reused within this update only.
    frame_buffers: Vec<Vec<Frame>>,
    /// Resolved CSS alignment insets, top/right/bottom/left (LLP 1010 §6.9).
    pub scroll_padding: Option<&'a dyn Fn(ViewId) -> [f64; 4]>,
    /// Resolved CSS target outsets, top/right/bottom/left.
    pub scroll_margin: Option<&'a dyn Fn(ViewId) -> [f64; 4]>,
    /// Row margins as the kernel lays them out, under a terminal's rule.
    pub cell_margins: Option<&'a dyn Fn(ViewId) -> [f64; 4]>,
    /// The app modes around the nodes being visited (LLP 1121 D5).
    pub theme: theme::Around,
}
impl<'a> Update<'a> {
    /// A context over `env` with no ops yet.
    pub fn new(env: Env<'a>, sites: &'a SiteIndex, ids: &'a mut Ids) -> Self {
        // A visit starts in the window's modes (LLP 1121 D5).
        let theme = theme::Around {
            tuple: ids.theme.window,
            retheme: std::mem::take(&mut ids.theme.moved),
            ..Default::default()
        };
        Update {
            env,
            fonts: None,
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
            shown: Default::default(),
            reuse: false,
            scratch: vm::Scratch::default(),
            frame_buffers: Vec::new(),
            scroll_padding: None,
            scroll_margin: None,
            cell_margins: None,
            theme,
        }
    }

    /// Detach removed views before destroying them, in the same atomic batch.
    /// Both groups keep their order; retaining the live-op buffer avoids
    /// allocating and growing a second copy of the whole update.
    fn detach_before_destroy(&mut self) {
        let gone: Vec<_> = self
            .ops
            .extract_if(.., |op| matches!(op, Op::DestroyView { .. }))
            .collect();
        self.ops.extend(gone);
    }

    fn with_frame<R>(
        &mut self,
        frames: &[Frame],
        frame: Frame,
        visit: impl FnOnce(&mut Self, &[Frame]) -> Result<R, InstanceError>,
    ) -> Result<R, InstanceError> {
        let mut inner = self.frame_buffers.pop().unwrap_or_default();
        inner.reserve_exact(frames.len() + 1);
        inner.extend_from_slice(frames);
        inner.push(frame);
        let result = visit(self, &inner);
        // A failed evaluation releases its scope too. Keep capacity, never
        // values or row slots, and let the whole pool go with this update.
        inner.clear();
        self.frame_buffers.push(inner);
        result
    }

    fn eval(&mut self, code: exact_plan::Code, frames: &[Frame]) -> Result<Value, Trap> {
        let env = Env { frames, ..self.env };
        vm::eval_value_reusing(self.env.plan.code(code), &env, &mut self.scratch)
    }

    fn eval_binding(
        &mut self,
        binding: exact_plan::BindingsId,
        code: exact_plan::Code,
        frames: &[Frame],
    ) -> Result<Value, Trap> {
        self.sites.projections[binding.0 as usize].eval(
            self.env.plan.code(code),
            &self.env,
            frames,
            &mut self.scratch,
        )
    }

    /// Whether anything `reads` reads may differ from what the tree shows.
    fn stale(&self, reads: &Reads) -> bool {
        self.stale_outside(reads, 0)
    }

    /// [`Update::stale`] for reads made `shift` scopes further in (a key or
    /// a row body seen from its region), counting only enclosing frames.
    fn stale_outside(&self, reads: &Reads, shift: u32) -> bool {
        // A changed tuple sets every themed row again (LLP 1121 D7).
        self.full || self.theme.retheme || self.changed_outside(reads, shift)
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
    /// How an `animation` list's names resolve, when linked.
    resolve_keyframes: bridge::KeyframesLink,
    /// A binding that reads nothing (a literal style row in a list row's
    /// template) has one value for the plan's life: evaluated once, by
    /// binding index, instead of once per instance; kept for the bindings
    /// evaluated (a plan has thousands, a screen evaluates hundreds).
    constants: RefCell<BTreeMap<u32, Value>>,
    projections: Vec<vm::Prepared>,
}

impl SiteIndex {
    /// Index the exact parent/arm pair, preserving authored order and rank
    /// ties, with every capability linked.
    pub fn new(plan: &Plan) -> Self {
        Self::linked(plan, crate::RunnerLinks::ALL.keyframes)
    }

    /// [`SiteIndex::new`], resolving keyframes as `keyframes` links.
    pub fn linked(plan: &Plan, keyframes: bridge::KeyframesLink) -> Self {
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
            resolve_keyframes: keyframes,
            constants: RefCell::default(),
            projections: plan
                .bindings
                .iter()
                .map(|b| vm::Prepared::new(plan.code(b.expr)))
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
    let children = u.sites.children(parent, arm);
    let mut out = Vec::new();
    for &(_, site) in children {
        let child = match site {
            Site::Node(id) => Child::Node(NodeInst::create(u, id, frames)?),
            Site::Region(id) => Child::Region(RegionInst::create(u, id, frames)?),
        };
        if out.is_empty() {
            out.reserve_exact(children.len());
        }
        out.push(child);
    }
    Ok(out)
}

fn roots_of(children: &[Child]) -> Vec<ViewId> {
    let mut out = Vec::new();
    push_roots(children, &mut out);
    out
}

/// The first of [`roots_of`], without collecting the rest.
fn first_root(children: &[Child]) -> Option<ViewId> {
    children.iter().find_map(|c| match c {
        Child::Node(n) => Some(n.view),
        Child::Region(r) => r.first_root(),
    })
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
    let saved = u.enter(dirty, row.slots.as_ref());
    let result = if u.stale(body) {
        u.with_frame(frames, row.frame.clone(), |u, inner| {
            update_all(u, &mut row.roots, inner)
        })
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
            last: vec![None; Self::slot(u.sites, &row, row.bindings.len as usize)]
                .into_boxed_slice(),
            children: Vec::new(),
            last_children: Box::default(),
            rare: None,
            tuple: u.theme.tuple,
            themed: theme::Themed::default(),
            colour_moved: false,
            around: u.theme.tuple,
        };
        inst.emit_bindings(u, frames, true)?;
        // Its children are created in what it sets: nothing to visit again.
        inst.colour_moved = false;
        let saved = inst.enter(u, inst.tuple);
        let lists = u.env.lists;
        let realized = (|| {
            let wait = tabs::wait(u, node, view);
            if let Some(lists) = lists.filter(|_| !wait) {
                inst.set_collection((lists.create)(u, node, view, frames)?);
            }
            if !wait && inst.collection().is_none() {
                inst.children = realize(u, Some(node), row.arm, frames)?;
                inst.emit_children(u);
            }
            Ok(())
        })();
        u.theme = saved;
        realized.map(|()| inst)
    }

    fn emit_bindings(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        fresh: bool,
    ) -> Result<(), InstanceError> {
        let plan = u.env.plan;
        let row = plan.node(self.node);
        let mut patch: Option<Box<StyleProps>> = None;
        let deps = &u.sites.deps;
        // A pin is read first: the node's other rows are in the tuple it
        // makes (LLP 1121 D5). Without a theme, nothing here runs.
        let themed_plan = !plan.theme.is_empty();
        let mode = themed_plan
            .then(|| theme::binding_of(plan, &row, exact_kernel::StyleId::Mode))
            .flatten();
        let (around, before) = (u.theme.tuple, self.tuple);
        // The tuple around moved: a pin's row is the tuple it makes over it.
        let around_moved = !fresh && (u.theme.retheme || around != self.around);
        self.around = around;
        let _tuple = theme::Bound::new(around);
        let mut tuned = mode.is_none();
        let mut force = false;
        if tuned {
            self.tuple = around;
            bridge::set_tuple(around);
            force = !fresh && (u.theme.retheme || around != before);
        }
        let order = mode
            .into_iter()
            .chain((0..row.bindings.len as usize).filter(|i| Some(*i) != mode));
        for i in order {
            let b = exact_plan::BindingsId(row.bindings.start + i as u32);
            if !tuned && Some(i) != mode {
                let pin = mode.and_then(|m| self.emitted(plan, u.sites, m));
                self.tuple = theme::tuple_of(plan, pin.as_ref(), around);
                bridge::set_tuple(self.tuple);
                force = !fresh && (u.theme.retheme || self.tuple != before);
                tuned = true;
            }
            let forced = match Some(i) == mode {
                true => around_moved,
                false => force && self.themed.get(i),
            };
            if !fresh && !forced && !u.stale(&deps.bindings[b.0 as usize]) {
                continue;
            }
            let binding = plan.binding(b);
            let reads = &deps.bindings[b.0 as usize];
            let evaluated = u.work.bindings_evaluated;
            let value = if reads.is_constant() {
                let known = u.sites.constants.borrow().get(&b.0).cloned();
                match known {
                    Some(value) => value,
                    None => {
                        u.work.bindings_evaluated += 1;
                        let value = u.eval_binding(b, binding.expr, frames)?;
                        u.sites.constants.borrow_mut().insert(b.0, value.clone());
                        value
                    }
                }
            } else {
                u.work.bindings_evaluated += 1;
                u.eval_binding(b, binding.expr, frames)?
            };
            // A constant's value is the one it was created with.
            let slot = (!reads.is_constant()).then(|| Self::slot(u.sites, &row, i));
            let unchanged = match slot {
                None => !fresh,
                Some(slot) => self.last[slot]
                    .as_ref()
                    .is_some_and(|last| crate::compare::equal(last, &value) == Some(true)),
            };
            if u.work.bindings_evaluated != evaluated {
                u.ids.evaluated(self.node, unchanged);
            }
            if unchanged && !forced {
                continue;
            }
            if u.discard {
                if binding.kind == BindingKind::Style
                    && !fresh
                    && !matches!(value, Value::Option(None))
                    && exact_kernel::StyleId::from_bit(binding.id as u32)
                        .is_some_and(|style| exact_kernel::StyleMask::TEXT.has(style))
                {
                    u.text_styled = true;
                }
                if let Some(slot) = slot {
                    self.last[slot] = Some(value);
                }
                continue;
            }
            // A binding is a declaration whose value is computed from state, as a `var()` reference
            // is. A value its row's grammar refuses is invalid at computed-value time (CSS Custom
            // Properties §3.1): the row is unset and a journal line says so (a `BridgeError::Unset`
            // unsets it silently). Not CSSOM's `setProperty`, which keeps the earlier value: the
            // view would then depend on history, not state. Unknown rows are plan defects.
            match binding.kind {
                BindingKind::Prop => match bridge::prop_value(binding.id, &value) {
                    Ok((prop, pv)) => {
                        let (id, mut refused) = (self.view, Vec::new());
                        let pv = bridge::icon_source(plan, prop, pv);
                        let value = bridge::atom_face_paint(plan, prop, pv, &mut refused);
                        for text in refused {
                            let shown = serde_json::Value::String(text);
                            u.notes.push(format!(
                                "view {id}: invalid atomFaces colour {shown}; its kind's paint stands"
                            ));
                        }
                        u.ops.push(Op::SetProp { value, id, prop })
                    }
                    Err(
                        e @ (bridge::BridgeError::PropKind { prop, .. }
                        | bridge::BridgeError::Unset(prop)),
                    ) => {
                        let id = self.view;
                        u.ops.push(Op::ClearProp { id, prop });
                        if matches!(e, bridge::BridgeError::PropKind { .. }) {
                            u.notes.push(invalid(id, prop.name(), &value));
                        }
                    }
                    Err(e) => return Err(InstanceError::Bridge(e)),
                },
                // A missing class row removes its declaration. An authored CSS
                // clear keeps declaration intent while inheriting or using the initial value.
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
                        written: binding.written && !matches!(value, Value::Option(None)),
                    });
                    self.unthemed(u, i, style);
                }
                BindingKind::Style => {
                    let p = patch.get_or_insert_with(Box::default);
                    let (family_value, family_id) =
                        bridge::family_binding(plan, u.fonts.as_deref(), &binding, &value);
                    // A row a runtime family or a refusal sets reads no token:
                    // the index forgets what it read before.
                    let row = exact_kernel::StyleId::from_bit(binding.id as u32);
                    let styled = if let Some(id) = family_id {
                        if let (true, Some(row)) = (themed_plan, row) {
                            self.unthemed(u, i, row);
                        }
                        bridge::set_runtime_font_style(
                            p,
                            &binding,
                            id,
                            plan.stacks.len() + u.fonts.as_ref().unwrap().borrow().names().len(),
                        )
                    } else if let Err(e) =
                        bridge::inherited_varying(plan, &binding, family_value, self.node)
                    {
                        if let (true, Some(row)) = (themed_plan, row) {
                            self.unthemed(u, i, row);
                        }
                        Err(e)
                    } else {
                        exact_kernel::style::theme::take_named();
                        let styled = bridge::set_binding_style(p, &binding, family_value, plan);
                        if themed_plan {
                            self.index(u, i, &binding, family_value, styled.is_ok());
                        }
                        styled
                    };
                    match styled {
                        // Unlinked, the plan names no rule: it was refused at
                        // boot (LLP 1047.001 D5).
                        Ok(exact_kernel::StyleId::Animation) => {
                            if let Some(resolve) = u.sites.resolve_keyframes {
                                u.notes
                                    .extend(resolve(&u.sites.keyframes, &mut p.animation));
                            }
                        }
                        // An exit names keyframes as `animation` does (LLP 1063).
                        Ok(exact_kernel::StyleId::ExitAnimation) => {
                            if let Some(resolve) = u.sites.resolve_keyframes {
                                u.notes.extend(resolve(
                                    &u.sites.keyframes,
                                    &mut p.rare.exit_animation,
                                ));
                            }
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
                                written: binding.written,
                            });
                            let name = style.name().replace('_', "-");
                            u.notes.push(invalid(self.view, &name, &value));
                        }
                        Err(e) => return Err(InstanceError::Bridge(e)),
                    }
                }
            }
            if let Some(slot) = slot {
                self.last[slot] = Some(value);
            }
        }
        if !tuned {
            let pin = mode.and_then(|m| self.emitted(plan, u.sites, m));
            self.tuple = theme::tuple_of(plan, pin.as_ref(), around);
        }
        if themed_plan && theme::boundary(plan, &row) {
            bridge::set_tuple(self.tuple);
            self.boundary_rows(
                u,
                &mut patch,
                fresh || u.theme.retheme || self.tuple != before,
            );
        }
        if let Some(p) = patch {
            u.ops.push(Op::SetStyle {
                id: self.view,
                patch: p,
            });
        }
        self.emit_looks(u, frames, fresh, u.theme.retheme || self.tuple != before)?;
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
            if self.rare.as_ref().and_then(|r| r.last_surface.as_ref()) != Some(&values) {
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
                self.rare().last_surface = Some(values);
            }
        }
        Ok(())
    }

    fn emit_children(&mut self, u: &mut Update<'_>) {
        let now = roots_of(&self.children);
        if *now != *self.last_children {
            u.ops.push(Op::SetChildren {
                id: self.view,
                children: now.clone(),
            });
            self.last_children = now.into_boxed_slice();
        }
    }

    fn update(&mut self, u: &mut Update<'_>, frames: &[Frame]) -> Result<(), InstanceError> {
        u.work.nodes_visited += 1;
        let before = self.tuple;
        self.emit_bindings(u, frames, false)?;
        let saved = self.enter(u, before);
        let updated = self.update_children(u, frames);
        u.theme = saved;
        updated
    }

    fn update_children(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
    ) -> Result<(), InstanceError> {
        let plan = u.env.plan;
        let follow = self.collection().is_some()
            && plan
                .node(self.node)
                .bindings
                .iter()
                .position(|b| {
                    let b = plan.binding(b);
                    b.kind == BindingKind::Prop
                        && b.id == exact_kernel::PropId::ScrollFollowEnd as u16
                })
                .is_some_and(|i| self.emitted(plan, u.sites, i) == Some(Value::Bool(true)));
        let keep = self
            .collection()
            .and_then(|_| self.bound_prop(plan, u.sites, exact_kernel::PropId::KeepMounted));
        if let Some(collection) = self.collection_mut() {
            collection.around = theme::Around {
                retheme: false,
                ..u.theme.clone()
            };
            (u.env.lists.ok_or_else(unlinked)?.update)(
                collection,
                u,
                frames,
                follow,
                keep.as_ref(),
            )?;
        } else if update_all(u, &mut self.children, frames)? {
            self.emit_children(u);
        }
        Ok(())
    }

    fn destroy(self, u: &mut Update<'_>) {
        // Destroying the view destroys its subtree in the kernel; the instance
        // tree just drops.
        collection::work::retire_node(u, &self);
        u.ops.push(Op::DestroyView { id: self.view });
    }
}

/// The journal line for a value a row refused.
fn invalid(view: ViewId, row: &str, value: &Value) -> String {
    let mut shown = String::new();
    crate::agent::untyped_json(value, &mut shown);
    format!("view {view}: invalid {row} value {shown}; unset")
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
        tree.open_tabs(u)?;
        tree.emit_roots(u);
        Ok(tree)
    }

    /// Bring every site whose reads changed since the last update up to date
    /// (every site, under [`Update::full`]).
    pub fn update(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        // New views cannot change the typography of already measured rows.
        let last_existing_view = u.ids.next;
        u.changed = Some(u.sites.deps.changed(&self.seen, &u.env));
        let roots = update_all(u, &mut self.children, &[]);
        u.changed = None;
        if roots? {
            self.emit_roots(u);
        }
        self.open_tabs(u)?;
        if self.has_collections && (u.text_styled || u.ops.iter().any(|op| matches!(op, Op::SetStyle { id, patch } if *id <= last_existing_view && patch.mask.intersects(exact_kernel::StyleMask::TEXT)))) {
            if let Some(lists) = u.env.lists {
                (lists.typography)(&mut self.children, u, &[])?;
            }
        }
        // Views are never reused within a runner. Detach removed children in
        // the final child lists before destroying them, so a removed list does
        // not rebuild its parent's siblings once per row. Still one atomic
        // batch; preserve relative destroy order for receipts and host effects.
        u.detach_before_destroy();
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
        self.site_in(view).map(|(node, _, path)| (node, path))
    }

    /// The app tuple `view`'s node is in (LLP 1121 D5): the window's,
    /// overridden by the pins above it, for an inspected node's report.
    pub fn tuple_of(&self, view: ViewId) -> Option<u32> {
        self.site_in(view).map(|(_, tuple, _)| tuple)
    }

    fn site_in(&self, view: ViewId) -> Option<(NodesId, u32, Vec<InstanceStep>)> {
        let mut path = Vec::new();
        for c in &self.children {
            let found = match c {
                Child::Node(n) => n.site(view, &mut path),
                Child::Region(r) => r.site(view, &mut path),
            };
            if let Some((node, tuple)) = found {
                return Some((node, tuple, path));
            }
        }
        None
    }
}

impl NodeInst {
    fn site(&self, view: ViewId, path: &mut Vec<InstanceStep>) -> Option<(NodesId, u32)> {
        if self.view == view {
            return Some((self.node, self.tuple));
        }
        if let Some(collection) = self.collection() {
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
    fn site(&self, view: ViewId, path: &mut Vec<InstanceStep>) -> Option<(NodesId, u32)> {
        let walk = |roots: &[Child], path: &mut Vec<InstanceStep>| -> Option<(NodesId, u32)> {
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
mod site_tests;
