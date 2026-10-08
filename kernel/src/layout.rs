//! Layout: the engine wrapper and frame publication.
//!
//! [`LayoutTree`] owns the Taffy tree as a derived structure over the arena's
//! columns. Text leaves carry their slot as the Taffy node context so the
//! measure closure can hand the host measurer the leaf's runs. After a pass,
//! [`compute`] publishes affected paths and moved subtrees into the arena's frame
//! column, resolves exclusions into sparse leaf coordinates, and returns
//! independent frame-change and flow-change receipts.
//!
//! An engine fault is never a panic: it is recorded, reported as
//! [`LayoutError::Engine`], and the kernel rebuilds the tree from the columns.
mod buttons;
#[cfg(test)]
mod containment_tests;
#[cfg(test)]
mod differential_tests;
mod fields;
mod hoist;
#[cfg(test)]
mod memo_tests;
mod order;
mod publication;

use crate::id::{IdMap, IdSet};
use crate::shared_style::Interner;
use std::collections::HashMap;
use std::rc::Rc;
use taffy::prelude::{AvailableSpace, NodeId, Size, TaffyTree};
use taffy::tree::{Baselines, LayoutInput};
use taffy::util::{MaybeResolve, ResolveOrZero};
use taffy::TraversePartialTree;

use crate::arena::NodeArena;
use crate::error::LayoutError;
use crate::generated::{FieldSizing, NodeType};
use crate::id::{AxisOffer, Frame, NodeFlags, NodeKey, Offer};
use crate::kernel::PresentedHeight;
use crate::style::taffy_style;
use crate::text::{TextMeasureRequest, TextMeasurer, TextMetrics, TextRun};

/// What a layout pass changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutReceipt {
    /// The kernel epoch the frames belong to.
    pub epoch: u64,
    /// The root that was laid out.
    pub root: NodeKey,
    /// Every node whose absolute frame changed, in preorder.
    pub changed: Vec<NodeKey>,
    /// Publication candidates in preorder: changed frames, parent-relative
    /// origins and scroll extents. Region passes may include unchanged nodes.
    /// Hosts accumulate these across silent passes before emission.
    pub updated: Vec<NodeKey>,
    /// Leaves whose resolved exclusions changed bitwise, in preorder (LLP 1043.000 D4).
    pub flow_changed: Vec<NodeKey>,
    /// Intersecting auto-height paragraphs that auto-height flow refused, in
    /// preorder; each names its `FlowRefusal` (LLP 1043.000 §8).
    pub flow_skipped: Vec<NodeKey>,
    /// Boxes a multi-column flow kept whole across a column's end where
    /// Chrome would fragment them; each names its `FragmentRefusal` (LLP
    /// 1093 D10).
    pub fragment_skipped: Vec<NodeKey>,
    /// Additional Taffy layouts performed by auto-height flow settlement.
    pub flow_passes: usize,
    /// Whole target-set comparisons, including the final comparison.
    /// Zero when settlement is skipped (no exclusions or a content region).
    pub flow_comparisons: usize,
}

fn to_available(offer: AxisOffer) -> AvailableSpace {
    match offer {
        AxisOffer::Definite(v) => AvailableSpace::Definite(v),
        AxisOffer::MaxContent => AvailableSpace::MaxContent,
        AxisOffer::MinContent => AvailableSpace::MinContent,
    }
}

fn from_available(space: AvailableSpace) -> AxisOffer {
    match space {
        AvailableSpace::Definite(v) => AxisOffer::Definite(v),
        AvailableSpace::MaxContent => AxisOffer::MaxContent,
        AvailableSpace::MinContent => AxisOffer::MinContent,
    }
}

/// Keep the full block/flex offer working set; four slots evicted within one
/// pass and repeated native measurement (LLP 1044 F6).
const LEAF_OFFERS: usize = 16;

// @ref LLP 1043.000 §3 D3 — keep the proof with Taffy's measured leaf.
#[derive(Default)]
struct MeasureContext {
    slot: u32,
    pass: u64,
    height_measured: bool,
    // At most LEAF_OFFERS per live leaf; text/style invalidation clears them.
    measurements: Vec<Measurement>,
    // @ref LLP 1043.000 §8 — the shapes an admitted auto-height leaf is
    // measured around (border-box coordinates), written only by settle_flow;
    // changing them clears `measurements`, whose key does not include them.
    flow: Vec<exact_textflow::FlowShape>,
}

#[derive(Clone, Copy)]
struct Measurement {
    width: AxisOffer,
    height: AxisOffer,
    metrics: TextMetrics,
}

/// The engine tree. Measured leaves retain their height proof and bounded offers.
pub struct LayoutTree {
    taffy: TaffyTree<MeasureContext>,
    pass: u64,
    fault: Option<String>,
    field_chrome: IdMap<NodeId, crate::FieldChrome>,
    field_minima: IdMap<NodeId, f32>,
    button_records: IdMap<NodeId, buttons::ButtonRecord>,
    provisional_chrome: bool,
    slots: IdMap<NodeId, u32>,
    // Invalidation sources since the last layout: true where the node's own
    // style changed (its parent's questions change), false for content only.
    deferred: IdMap<NodeId, bool>,
    // Each walked box's boundary during one layout's choice (outer None:
    // hidden or not under the root); empty between layouts.
    walks: IdMap<NodeId, Option<Option<NodeId>>>,
    offers: IdMap<NodeId, Offer>,
    #[cfg(test)]
    pub(crate) publication_visits: usize,
    /// Boundaries replayed whose parent-facing output held: layouts that
    /// stayed inside them.
    #[cfg(test)]
    pub(crate) boundary_replays: usize,
    // Derived heights only. Retain capacity across frames and owner changes.
    presented_heights: Vec<(NodeKey, NodeId, f32)>,
    // Slots whose MeasureContext carries flow shapes, and leaves whose shapes
    // did not reach a fixed point in the last settle (a defect, journalled).
    flowing: IdSet<u32>,
    pub(crate) unsettled: IdSet<u32>,
    // Equal engine styles are one allocation (`shared_style`).
    shared: Interner<taffy::Style, ()>,
    // @ref LLP 1074 T1 — the absolutely positioned boxes, and whether the
    // engine's record of which containing block holds each one that is not its
    // parent's is stale: a position or a child list changed since it was made.
    absolutes: IdSet<NodeId>,
    hoists_stale: bool,
    // The static boxes between such a box and its containing block. None is
    // a replay boundary: laying its subtree out alone would not place a box
    // that something above it contains. `hoist_paths_prior` holds the record
    // the last layout ran under, until this one has chosen its boundaries: a
    // box that was on a path is no boundary for the change that took it off
    // (its old containing block still counts the box that left).
    hoist_paths: IdSet<NodeId>,
    hoist_paths_prior: IdSet<NodeId>,
}

// A non-visible overflow on both axes establishes a formatting context and
// prevents descendants' scrollable overflow/margins from escaping the box.
// Whether its size depends on its content is the engine's record to prove.
fn boundary_style(s: &taffy::Style) -> bool {
    s.display != taffy::Display::None
        && s.position != taffy::Position::Absolute
        && matches!(
            s.overflow.x,
            taffy::Overflow::Hidden | taffy::Overflow::Scroll
        )
        && matches!(
            s.overflow.y,
            taffy::Overflow::Hidden | taffy::Overflow::Scroll
        )
}

/// A hash of the rows most styles differ in; equality decides the rest.
fn engine_hash(s: &taffy::Style) -> u64 {
    use std::hash::Hasher;
    let mut h = crate::id::IdHasher::default();
    let mut len = |l: taffy::CompactLength| {
        h.write_u64(((l.tag() as u64) << 32) | u64::from(l.value().to_bits()));
    };
    len(s.size.width.into_raw());
    len(s.size.height.into_raw());
    len(s.min_size.width.into_raw());
    len(s.min_size.height.into_raw());
    len(s.flex_basis.into_raw());
    len(s.margin.left.into_raw());
    len(s.margin.top.into_raw());
    len(s.padding.left.into_raw());
    len(s.padding.top.into_raw());
    len(s.gap.width.into_raw());
    h.write_u64(u64::from(s.flex_grow.to_bits()) << 32 | u64::from(s.flex_shrink.to_bits()));
    h.write_u64(
        (s.display as u64)
            | (s.position as u64) << 8
            | (s.flex_direction as u64) << 16
            | (s.box_sizing as u64) << 24,
    );
    h.finish()
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}

/// The engine's side of a commit: what the kernel's transaction and its
/// restyles call (LLP 1047 D4). [`LayoutTree`] mirrors the arena; a kernel
/// whose platform lays out (the browser's) holds none until a layout is first
/// asked for and commits through [`Unmirrored`]. The engine is boxed only on
/// the layout path, so an artifact that never lays out links none of it.
pub trait LayoutMirror {
    /// A leaf for `slot` from its current style; `None` without an engine.
    fn new_leaf(&mut self, arena: &NodeArena, slot: u32, measured: bool) -> Option<NodeId>;
    /// A leaf with the engine's default style, for a node whose style the
    /// same batch re-derives (`restyle`) before any layout.
    fn new_leaf_unstyled(&mut self, slot: u32, measured: bool) -> Option<NodeId>;
    /// Re-derive `node`'s engine style from `slot`'s current style.
    fn restyle(&mut self, arena: &NodeArena, slot: u32, node: NodeId);
    /// Give `node`, `parent`'s engine node, the engine nodes of `parent`'s
    /// children in order (none under a text node: its runs are measured, not
    /// laid out); returns how many.
    fn sync_children(&mut self, arena: &NodeArena, parent: u32, node: NodeId) -> usize;
    /// Remove a node.
    fn remove(&mut self, node: NodeId);
    /// Mark a node (and its ancestors) dirty.
    fn mark_dirty(&mut self, node: NodeId);
    /// Whether an engine node still has an engine parent.
    #[cfg(test)]
    fn attached(&self, node: NodeId) -> bool;
    /// The engine tree, for a layout.
    fn tree(&mut self) -> Option<&mut LayoutTree>;
    /// The engine tree, read.
    fn tree_ref(&self) -> Option<&LayoutTree>;
}

impl LayoutMirror for LayoutTree {
    fn new_leaf(&mut self, arena: &NodeArena, slot: u32, measured: bool) -> Option<NodeId> {
        Some(LayoutTree::new_leaf(
            self,
            taffy_style(arena, slot),
            slot,
            measured,
        ))
    }

    fn new_leaf_unstyled(&mut self, slot: u32, measured: bool) -> Option<NodeId> {
        Some(LayoutTree::new_leaf(
            self,
            taffy::Style::default(),
            slot,
            measured,
        ))
    }

    fn restyle(&mut self, arena: &NodeArena, slot: u32, node: NodeId) {
        self.set_style(node, taffy_style(arena, slot));
    }

    fn sync_children(&mut self, arena: &NodeArena, parent: u32, node: NodeId) -> usize {
        if !arena.node_type(parent).lays_out_children() {
            LayoutTree::set_children(self, node, &[]);
            return 0;
        }
        let ids = order::laid_out(arena, parent, &self.taffy, node, |c| arena.taffy(c));
        LayoutTree::set_children(self, node, &ids);
        // A list's rows are many boxes of few shapes, rebound and built as
        // it scrolls: each is laid out by replaying a row like it (Taffy
        // patch 29), where one was.
        if arena.node_type(parent) == NodeType::List {
            for &row in &ids {
                self.taffy.set_memo_root(row, true);
            }
        }
        ids.len()
    }

    fn remove(&mut self, node: NodeId) {
        LayoutTree::remove(self, node);
    }

    fn mark_dirty(&mut self, node: NodeId) {
        LayoutTree::mark_dirty(self, node);
    }

    #[cfg(test)]
    fn attached(&self, node: NodeId) -> bool {
        LayoutTree::attached(self, node)
    }

    fn tree(&mut self) -> Option<&mut LayoutTree> {
        Some(self)
    }

    fn tree_ref(&self) -> Option<&LayoutTree> {
        Some(self)
    }
}

/// A commit's engine when the kernel mirrors none: new nodes get no engine
/// node, so no other call reaches it.
pub struct Unmirrored;

impl LayoutMirror for Unmirrored {
    fn new_leaf(&mut self, _: &NodeArena, _: u32, _: bool) -> Option<NodeId> {
        None
    }

    fn new_leaf_unstyled(&mut self, _: u32, _: bool) -> Option<NodeId> {
        None
    }

    fn restyle(&mut self, _: &NodeArena, _: u32, _: NodeId) {}

    fn sync_children(&mut self, _: &NodeArena, _: u32, _: NodeId) -> usize {
        0
    }

    fn remove(&mut self, _: NodeId) {}

    fn mark_dirty(&mut self, _: NodeId) {}

    #[cfg(test)]
    fn attached(&self, _: NodeId) -> bool {
        false
    }

    fn tree(&mut self) -> Option<&mut LayoutTree> {
        None
    }

    fn tree_ref(&self) -> Option<&LayoutTree> {
        None
    }
}

impl LayoutTree {
    /// An empty tree.
    pub fn new() -> Self {
        // Frames are CSS pixel geometry, not a host's raster grid. Rounding
        // here loses subpixel edits and snaps Retina views to whole points.
        let mut taffy = TaffyTree::new();
        taffy.disable_rounding();
        taffy.set_calc_resolver(crate::style::resolve_calc);
        LayoutTree {
            taffy,
            pass: 0,
            fault: None,
            field_chrome: IdMap::default(),
            field_minima: IdMap::default(),
            button_records: IdMap::default(),
            provisional_chrome: false,
            slots: IdMap::default(),
            deferred: IdMap::default(),
            walks: IdMap::default(),
            offers: IdMap::default(),
            #[cfg(test)]
            publication_visits: 0,
            #[cfg(test)]
            boundary_replays: 0,
            presented_heights: Vec::new(),
            flowing: IdSet::default(),
            unsettled: IdSet::default(),
            shared: Interner::default(),
            absolutes: IdSet::default(),
            hoists_stale: false,
            hoist_paths: IdSet::default(),
            hoist_paths_prior: IdSet::default(),
        }
    }

    /// Return spare capacity: the shared styles no node holds and the
    /// per-node maps' room. The engine's own node store keeps its capacity
    /// (its handles are its slots, and a rebuild would drop every cached
    /// measurement).
    pub(crate) fn trim(&mut self) {
        self.shared.trim();
        if self.slots.capacity() > 2 * self.slots.len().max(64) {
            self.slots.shrink_to_fit();
            self.deferred.shrink_to_fit();
            self.walks.shrink_to_fit();
            self.offers.shrink_to_fit();
        }
    }

    /// One allocation for every engine style equal to `style`.
    fn share(&mut self, style: taffy::Style) -> Rc<taffy::Style> {
        let hash = engine_hash(&style);
        match self.shared.get(hash, |_, held| *held == style) {
            Some(found) => found,
            None => self.shared.insert(hash, (), style),
        }
    }

    fn note(&mut self, what: &str, result: Result<impl Sized, taffy::TaffyError>) {
        if let Err(e) = result {
            if self.fault.is_none() {
                self.fault = Some(format!("{what}: {e:?}"));
            }
        }
    }

    /// Lay out a list's rows by replaying rows like them (the default), or
    /// each by its own algorithm.
    pub fn set_row_memo(&mut self, on: bool) {
        self.taffy.enable_memo(on);
    }

    /// List rows laid out by a replay, and those computed and recorded.
    pub fn row_memo_counts(&self) -> (usize, usize) {
        self.taffy.memo_counts()
    }

    /// Whether the engine reported a fault since the last rebuild.
    pub fn faulted(&self) -> bool {
        self.fault.is_some()
    }

    /// Engine nodes.
    pub fn node_count(&self) -> usize {
        self.taffy.total_node_count()
    }

    /// Allocate a leaf; `measured` leaves carry their slot for the measure closure.
    pub fn new_leaf(&mut self, style: taffy::style::Style, slot: u32, measured: bool) -> NodeId {
        let boundary = boundary_style(&style);
        let absolute = style.position == taffy::Position::Absolute;
        let style = self.share(style);
        let result = if measured {
            self.taffy.new_leaf_with_context(
                style,
                MeasureContext {
                    slot,
                    ..Default::default()
                },
            )
        } else {
            self.taffy.new_leaf(style)
        };
        match result {
            Ok(node) => {
                self.slots.insert(node, slot);
                self.taffy.track_layout_input(node, boundary);
                if absolute {
                    self.absolutes.insert(node);
                    self.hoists_stale = true;
                }
                node
            }
            Err(e) => {
                // Unreachable: leaf allocation cannot fail. Record it and hand back a
                // placeholder that every later call reports as a fault.
                self.fault.get_or_insert_with(|| format!("new_leaf: {e:?}"));
                NodeId::from(usize::MAX)
            }
        }
    }

    /// Remove a node.
    pub fn remove(&mut self, node: NodeId) {
        self.field_chrome.remove(&node);
        self.field_minima.remove(&node);
        self.button_records.remove(&node);
        self.deferred.remove(&node);
        if let Some(slot) = self.slots.remove(&node) {
            self.flowing.remove(&slot);
        }
        self.offers.remove(&node);
        self.presented_heights
            .retain(|(_, active, _)| *active != node);
        self.hoists_stale |= self.absolutes.remove(&node) || !self.absolutes.is_empty();
        let r = self.taffy.remove(node);
        self.note("remove", r);
    }

    /// The offer `root` was last laid out under, if it has been.
    pub(crate) fn last_offer(&self, root: NodeId) -> Option<Offer> {
        self.offers.get(&root).copied()
    }

    /// Replace authored lowering, retaining an active presentation height.
    /// Dirty only when the resulting full derived style changes. This is also
    /// the path for environment/intrinsic updates that do not bump the epoch.
    pub fn set_style(&mut self, node: NodeId, mut style: taffy::style::Style) {
        if let Some((_, _, px)) = self
            .presented_heights
            .iter()
            .find(|(_, active, _)| *active == node)
        {
            style.size.height = taffy::style::Dimension::length(*px);
        }
        self.write_style(node, style);
    }

    fn write_style(&mut self, node: NodeId, style: taffy::style::Style) {
        if self.taffy.style(node).is_ok_and(|old| *old == style) {
            return;
        }
        // A changed position changes which box contains which absolute one.
        if self
            .taffy
            .style(node)
            .map_or(true, |old| old.position != style.position)
        {
            if style.position == taffy::Position::Absolute {
                self.absolutes.insert(node);
            } else {
                self.absolutes.remove(&node);
            }
            self.hoists_stale = true;
        }
        self.clear_measurements(node);
        self.taffy.track_layout_input(node, boundary_style(&style));
        let style = self.share(style);
        self.taffy.set_style_unmarked(node, style);
        self.deferred.insert(node, true);
    }

    /// Snapshot only the active numeric projections for a nonpublishing
    /// authored-target pass. This allocation is outside the motion-frame path.
    pub(crate) fn height_samples(&self, epoch: u64) -> Vec<PresentedHeight> {
        self.presented_heights
            .iter()
            .map(|(node, _, px)| PresentedHeight {
                node: *node,
                epoch,
                px: *px,
            })
            .collect()
    }

    /// Install preflighted samples, restoring current authored lowering for
    /// retired owners. Requests own epochs; this derived cache owns no targets.
    pub(crate) fn present_heights(&mut self, arena: &NodeArena, samples: &[PresentedHeight]) {
        // Resolve every engine node before changing any projection. A missing
        // derived node takes the ordinary rebuild path with the complete set.
        if samples.iter().any(|p| arena.taffy(p.node.index).is_none()) {
            self.fault
                .get_or_insert_with(|| "presented height has no engine node".into());
            return;
        }
        for i in 0..self.presented_heights.len() {
            let (key, node, _) = self.presented_heights[i];
            if !samples.iter().any(|p| p.node == key) {
                if let Some(slot) = arena.resolve(key) {
                    self.write_style(node, taffy_style(arena, slot));
                }
            }
        }
        self.presented_heights
            .retain(|(key, _, _)| samples.iter().any(|p| p.node == *key));
        for p in samples {
            let node = arena.taffy(p.node.index).expect("preflighted engine node");
            let next = (p.node, node, p.px);
            let existing = self
                .presented_heights
                .iter()
                .position(|(key, _, _)| *key == p.node);
            if existing.is_some_and(|i| self.presented_heights[i] == next) {
                continue;
            }
            // Replace directly: restoring first would dirty twice and could
            // momentarily reinstall an obsolete authored target.
            let mut style = taffy_style(arena, p.node.index);
            style.size.height = taffy::style::Dimension::length(p.px);
            self.write_style(node, style);
            if let Some(i) = existing {
                self.presented_heights[i] = next;
            } else {
                self.presented_heights.push(next);
            }
        }
    }

    /// A content region trial does not compose with height projections yet.
    pub(crate) fn has_presented_height(&self) -> bool {
        !self.presented_heights.is_empty()
    }

    /// Keep a registered region's content out of shell sizing. No-op when
    /// already cut, so unchanged shell layout continues to use its own cache.
    pub(crate) fn cut_children(&mut self, node: NodeId) {
        if self.taffy.child_count(node) > 0 {
            self.set_children(node, &[]);
        }
    }

    /// Replace a node's ordered children. A child taken from another parent
    /// invalidates both at once; otherwise this is the parent's content.
    pub fn set_children(&mut self, parent: NodeId, children: &[NodeId]) {
        self.hoists_stale |= !self.absolutes.is_empty();
        self.clear_measurements(parent);
        if self.taffy.set_children_unmarked(parent, children) {
            self.deferred.entry(parent).or_insert(false);
        } else {
            let r = self.taffy.set_children(parent, children);
            self.note("set_children", r);
        }
    }

    /// Whether an engine node still has an engine parent: removing it then
    /// scans that parent's children (the transaction's batching avoids it).
    #[cfg(test)]
    pub(crate) fn attached(&self, node: NodeId) -> bool {
        self.taffy.parent(node).is_some()
    }

    /// Mark a node (and its ancestors) dirty.
    pub fn mark_dirty(&mut self, node: NodeId) {
        self.clear_measurements(node);
        self.deferred.entry(node).or_insert(false);
    }

    // The nearest clipping box above a change (or at it, when only its content
    // changed) that the engine can replay: one whose ancestors, since they
    // were last invalidated through it, consumed no answer of its that
    // depended on its content other than its saved final layout (vendored
    // Taffy's record, EXACT PATCH 9). Flex, grid, percentage and intrinsic
    // sizing questions are in that record; the replayed output is compared.
    // A restyled box is never one: its parent asks it something new.
    // `walks` remembers each box's answer (outer None: hidden or not under
    // `root`), so sources that share ancestors walk them once per layout.
    fn boundary_for(
        &self,
        node: NodeId,
        root: NodeId,
        deferred: &IdMap<NodeId, bool>,
        walks: &mut IdMap<NodeId, Option<Option<NodeId>>>,
    ) -> Option<NodeId> {
        let restyled = |n| deferred.get(&n) == Some(&true);
        let start = if restyled(node) {
            self.taffy.parent(node)
        } else {
            Some(node)
        };
        let mut path = Vec::new();
        let mut at = start;
        let mut above = loop {
            let Some(n) = at else { break None };
            if let Some(&known) = walks.get(&n) {
                break known;
            }
            let Ok(s) = self.taffy.style(n) else {
                break None;
            };
            // A hidden box lays nothing out: saved inputs below it are stale.
            if s.display == taffy::Display::None {
                walks.insert(n, None);
                break None;
            }
            if n == root {
                walks.insert(n, Some(None));
                break Some(None);
            }
            path.push(n);
            at = self.taffy.parent(n);
        };
        for n in path.into_iter().rev() {
            if above.is_some()
                && boundary_style(self.taffy.style(n).expect("walked"))
                && !self.hoist_paths.contains(&n)
                && !self.hoist_paths_prior.contains(&n)
                && !restyled(n)
                && !self.taffy.dirty(n).unwrap_or(true)
                && self.taffy.last_layout_input(n).is_some()
            {
                above = Some(Some(n));
            }
            walks.insert(n, above);
        }
        above.flatten()
    }

    // A flex column aligns no item by its baseline and takes its own first
    // baseline from its startmost item only (CSS Flexbox §8.5, §9.4 step 8).
    // Wrapping in reverse makes which item that is depend on line breaks.
    fn baselines_unread(&self, node: NodeId) -> bool {
        use taffy::{FlexDirection, FlexWrap};
        let Some(parent) = self.taffy.parent(node) else {
            return false;
        };
        let Ok(s) = self.taffy.style(parent) else {
            return false;
        };
        let reverse = match (s.display, s.flex_direction, s.flex_wrap) {
            (taffy::Display::Flex, FlexDirection::Column, FlexWrap::NoWrap | FlexWrap::Wrap) => {
                false
            }
            (taffy::Display::Flex, FlexDirection::ColumnReverse, FlexWrap::NoWrap) => true,
            _ => return false,
        };
        let mut items = self.taffy.child_ids(parent).filter(|&c| {
            self.taffy.style(c).is_ok_and(|s| {
                s.position != taffy::Position::Absolute && s.display != taffy::Display::None
            })
        });
        let startmost = if reverse { items.last() } else { items.next() };
        startmost != Some(node)
    }

    fn prepare_boundaries(
        &mut self,
        root: NodeId,
        offer: Offer,
        arena: &NodeArena,
    ) -> Vec<(NodeId, LayoutInput, taffy::tree::LayoutOutput)> {
        let local = self.offers.get(&root) == Some(&offer)
            && matches!(
                (offer.width, offer.height),
                (AxisOffer::Definite(_), AxisOffer::Definite(_))
            )
            && arena.exclusion_slots.is_empty()
            && arena.flow.is_empty();
        // Both scratch maps keep their capacity from layout to layout.
        let mut deferred = std::mem::take(&mut self.deferred);
        let mut walks = std::mem::take(&mut self.walks);
        // Decide every source before marking any. Ordinary invalidation walks
        // first: one stopping at a box a local walk had already cleared would
        // leave that box's ancestors clean.
        let mut boundaries = IdSet::default();
        let mut contained = Vec::new();
        for &node in deferred.keys() {
            match local
                .then(|| self.boundary_for(node, root, &deferred, &mut walks))
                .flatten()
            {
                Some(boundary) => {
                    contained.push((node, boundary));
                    boundaries.insert(boundary);
                }
                None => {
                    let r = self.taffy.mark_dirty(node);
                    self.note("mark_dirty", r);
                }
            }
        }
        for (node, boundary) in contained {
            self.taffy.mark_dirty_to(node, boundary);
        }
        deferred.clear();
        walks.clear();
        self.deferred = deferred;
        self.walks = walks;
        // An unrelated source may have invalidated the root (a list whose
        // rows moved, a change no box contains): the root pass that follows
        // handles those, and each boundary is still replayed first. Its
        // caches were cleared through it, so an ancestor that pass computes
        // again finds this replay's layout or asks the box anew, never an
        // answer from before the change; a boundary an ordinary source
        // dirtied through has lost its saved input and is not replayed.
        // A boundary inside another is replayed first: when its output
        // stands, nothing above it reads the change, and the outer replay
        // (for its own sources) finds every box between them as it was. When
        // it does not, the replay marks its parent, and the outer box is
        // computed again through it. So a list whose rows moved, a boundary
        // for its spacers, no longer forfeits the boxes inside a row that
        // contain the row's own changes (it had every changed row laid out
        // whole).
        let depth = |mut node: NodeId| {
            let mut d = 0usize;
            while let Some(parent) = self.taffy.parent(node) {
                d += 1;
                node = parent;
            }
            d
        };
        let mut replays: Vec<(usize, NodeId, LayoutInput, taffy::tree::LayoutOutput)> = boundaries
            .into_iter()
            .filter_map(|node| {
                self.taffy
                    .last_layout_input(node)
                    .map(|(input, output)| (depth(node), node, input, output))
            })
            .collect();
        replays.sort_by(|a, b| b.0.cmp(&a.0).then(u64::from(a.1).cmp(&u64::from(b.1))));
        replays
            .into_iter()
            .map(|(_, node, input, output)| (node, input, output))
            .collect()
    }

    fn clear_measurements(&mut self, node: NodeId) {
        if let Some(context) = self.taffy.get_node_context_mut(node) {
            context.measurements.clear();
        }
    }

    /// Whether a node needs layout.
    pub fn is_dirty(&self, node: NodeId) -> bool {
        self.taffy.dirty(node).unwrap_or(true)
            || self.deferred.keys().any(|&dirty| {
                let mut at = Some(dirty);
                while let Some(n) = at {
                    if n == node {
                        return true;
                    }
                    at = self.taffy.parent(n);
                }
                false
            })
    }

    /// The engine's layout for a node, relative to its parent.
    pub fn layout(&self, node: NodeId) -> taffy::tree::Layout {
        self.taffy.layout(node).copied().unwrap_or_default()
    }

    /// Run the engine on `root` under `offer`.
    pub fn compute(
        &mut self,
        root: NodeId,
        offer: Offer,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
    ) -> Result<(), LayoutError> {
        self.compute_mapped(root, offer, arena, measurer, &|s| arena.taffy(s))
    }

    /// Region trial trees supply their local handle map; arena handles belong
    /// solely to the ordinary tree and must never be indexed in a trial tree.
    pub(crate) fn compute_mapped(
        &mut self,
        root: NodeId,
        offer: Offer,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
        node_for: &dyn Fn(u32) -> Option<NodeId>,
    ) -> Result<(), LayoutError> {
        self.prepare_fields(root, arena, measurer)?;
        let buttons = self.prepare_buttons(root, arena, measurer)?;
        // Percentage padding uses the containing block's final width. Settle
        // its frame floor in the engine before publication, never after paint.
        for _ in 0..3 {
            let minima = self.compute_pass(root, offer, arena, measurer, node_for, &buttons)?;
            if !self.settle_field_minima(minima) {
                return Ok(());
            }
        }
        Err(LayoutError::Engine(
            "field minimum did not settle in three passes".into(),
        ))
    }

    fn compute_pass(
        &mut self,
        root: NodeId,
        offer: Offer,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
        // A trait object, not a generic: each closure type would compile the
        // engine's whole pass, Taffy's algorithms with it, again into every
        // app (LLP 1047.001; LLP 1075.003 §9.11's trial tree did, +0.4 MB).
        node_for: &dyn Fn(u32) -> Option<NodeId>,
        buttons: &IdMap<u32, NodeId>,
    ) -> Result<IdMap<NodeId, f32>, LayoutError> {
        if let Some(fault) = &self.fault {
            return Err(LayoutError::Engine(fault.clone()));
        }
        // @ref LLP 1043.000 §3 D3 — invalidate only the text whose height
        // proof matters to live wrapping contexts. Clean roots retain their
        // cached proof; hidden, detached and other-root exclusions do no work.
        if self.is_dirty(root) && !arena.exclusion_slots.is_empty() {
            let mut contexts = crate::sorted::SlotSet::default();
            for slot in arena.exclusion_slots.iter() {
                let mut at = Some(slot);
                while let Some(s) = at {
                    if arena.style(s).display == crate::Display::None {
                        break;
                    }
                    let Some(node) = node_for(s) else {
                        break;
                    };
                    if node == root {
                        if let Some(parent) = arena.parent(slot) {
                            contexts.insert(parent);
                        }
                        break;
                    }
                    at = arena
                        .parent(s)
                        .filter(|&p| node_for(p) == self.taffy.parent(node));
                }
            }
            let mut stack: Vec<_> = contexts.iter().collect();
            let mut seen = crate::sorted::SlotSet::default();
            while let Some(slot) = stack.pop() {
                if !seen.insert(slot)
                    || arena.style(slot).display == crate::Display::None
                    || crate::flow::is_exclusion(arena, slot)
                {
                    continue;
                }
                if arena.node_type(slot) == NodeType::Text && !arena.is_inline_run(slot) {
                    if let Some(node) = node_for(slot) {
                        self.mark_dirty(node); // Taffy clears this leaf and its ancestors.
                        if let Some(context) = self.taffy.get_node_context_mut(node) {
                            context.height_measured = false;
                        }
                    }
                }
                stack.extend(arena.children(slot));
            }
        }
        self.refresh_hoists();
        let boundaries = self.prepare_boundaries(root, offer, arena);
        self.hoist_paths_prior.clear();
        let available = Size {
            width: to_available(offer.width),
            height: to_available(offer.height),
        };
        self.pass += 1;
        let pass = self.pass;
        let mut runs: Vec<TextRun<'_>> = Vec::new();
        let mut invalid_metrics = None;
        let mut invalid_button = None;
        let mut provisional_button = false;
        let height_free = measurer.height_free();
        let chrome = &self.field_chrome;
        let mut minima = IdMap::default();
        let baselines_unread: IdSet<_> = boundaries
            .iter()
            .filter(|(node, _, _)| self.baselines_unread(*node))
            .map(|(node, _, _)| *node)
            .collect();
        let button_records = &mut self.button_records;
        let mut measure = |inputs: LayoutInput,
                           node,
                           context: Option<&mut MeasureContext>,
                           style: &taffy::Style| {
            let mut first_baseline = None;
            let mut line_height = 0.0;
            let native_slot = context
                .as_ref()
                .map(|c| c.slot)
                .filter(|&s| arena.is_native_text_control(s));
            let mut adjusted;
            let style = if let Some(minimum) = native_slot.and_then(|slot| {
                fields::minimum(
                    arena,
                    slot,
                    inputs,
                    style,
                    chrome.get(&node).copied().unwrap_or_default(),
                )
            }) {
                minima.insert(node, minimum);
                adjusted = style.clone();
                adjusted.min_size.height = taffy::LengthPercentageAuto::length(minimum);
                &adjusted
            } else {
                style
            };

            if let Some(record) = context
                .as_ref()
                .and_then(|c| buttons.get(&c.slot))
                .and_then(|n| button_records.get_mut(n))
            {
                record.parent_width = inputs.parent_size.width;
            }

            // Patch 2's separate API is unnecessary: the upstream callback owns
            // LayoutOutput, including baselines in border-box coordinates.
            let inset = style
                .padding
                .resolve_or_zero(inputs.parent_size.width, |_, _| 0.0)
                + style
                    .border
                    .resolve_or_zero(inputs.parent_size.width, |_, _| 0.0);
            // @ref LLP 1043.000 §3 D3 — upstream supplies the full layout input.
            // The proof is separate from the ordinary size-only callback offers.
            let height_known = match (inputs.run_mode, inputs.sizing_mode) {
                (taffy::tree::RunMode::PerformLayout, taffy::tree::SizingMode::InherentSize) => {
                    inputs
                        .known_dimensions
                        .height
                        .or_else(|| {
                            style
                                .size
                                .maybe_resolve(inputs.parent_size, |_, _| 0.0)
                                .maybe_apply_aspect_ratio(style.aspect_ratio)
                                .height
                        })
                        .is_some()
                }
                // ContentSize ignores authored dimensions; ComputeSize carries
                // exactly upstream's border-box known dimensions (old Patch 7).
                _ => inputs.known_dimensions.height.is_some(),
            };
            let mut output = taffy::compute_leaf_layout(
                inputs,
                style,
                |_, _| 0.0,
                |known, space| {
                    let Some(context) = context else {
                        return Size::ZERO;
                    };
                    // Clear on the first actual measurement of this pass. A cache
                    // hit retains the proof of the pass that produced that layout.
                    if context.pass != pass {
                        context.pass = pass;
                        context.height_measured = false;
                    }
                    context.height_measured |= !height_known;
                    let slot = context.slot;
                    if let Some(size) =
                        crate::replaced::measure(arena, slot, style, inset, known, space)
                    {
                        return size;
                    }
                    if let Some(record) = buttons.get(&slot).and_then(|n| button_records.get_mut(n))
                    {
                        if let Some(answer) =
                            buttons::measure(record, arena, slot, measurer, known, space, inset)
                        {
                            if !answer.is_valid() {
                                invalid_button.get_or_insert_with(|| arena.local_id(slot));
                                return Size::ZERO;
                            }
                            provisional_button |= answer.provisional;
                            return Size {
                                width: known
                                    .width
                                    .unwrap_or((answer.width - inset.left - inset.right).max(0.0)),
                                height: known
                                    .height
                                    .unwrap_or((answer.height - inset.top - inset.bottom).max(0.0)),
                            };
                        }
                    }
                    if matches!(
                        arena.node_type(slot),
                        NodeType::Control | NodeType::NativeView
                    ) {
                        // @ref LLP 1069.001 D3 — the platform's size for its
                        // control (CSS leaves it to the UA), each axis
                        // overridden by a known one; until the host
                        // reports, the kind's default. Native modules use the
                        // same non-ratio seam, with no default content size
                        // (LLP 1024 D4).
                        let (iw, ih) = arena.intrinsic(slot).unwrap_or_else(|| {
                            crate::ControlKind::of(arena.node_type(slot), arena.props(slot))
                                .map_or((0.0, 0.0), crate::ControlKind::default_size)
                        });
                        return Size {
                            width: known.width.unwrap_or(iw),
                            height: known.height.unwrap_or(ih),
                        };
                    }
                    let width = if arena.node_type(slot) == NodeType::TextInput
                        && arena.style(slot).field_sizing == FieldSizing::Fixed
                    {
                        // A control's preferred row count does not increase when
                        // CSS constrains its width below the preferred columns.
                        AxisOffer::MaxContent
                    } else {
                        // The leaf engine has folded its known border-box size
                        // into content space, less padding and border. Wrap where
                        // the host paints, not at the wider border box.
                        from_available(space.width)
                    };
                    // @ref LLP 1093 D1 — a multi-column `text` breaks its
                    // lines at the column width; its height is the columns'.
                    let width = match (style.multicol, known.width) {
                        (Some(m), Some(u)) => AxisOffer::Definite(m.columns(u).1),
                        _ => width,
                    };
                    let height = from_available(space.height);
                    // Reuse before flattening runs or crossing the host seam. Height
                    // stays in the key, and the 0.14 proof above updates on hits too.
                    let metrics = if let Some(cached) = context
                        .measurements
                        .iter()
                        .find(|m| m.width == width && (height_free || m.height == height))
                    {
                        cached.metrics
                    } else {
                        let stamp = arena.paragraph_stamp(slot); // held: no runs to flatten
                        let known = stamp
                            .as_ref()
                            .filter(|_| {
                                context.flow.is_empty() || !matches!(width, AxisOffer::Definite(_))
                            })
                            .and_then(|stamp| measurer.measure_known(stamp, width, height));
                        let metrics = if let Some(metrics) = known {
                            metrics
                        } else {
                            runs.clear();
                            arena.text_runs(slot, &mut runs);
                            if runs.is_empty() {
                                return Size::ZERO;
                            }
                            // Direction and alignment inherit (a paragraph inside a
                            // centred column centres, as in CSS); the rest are its own.
                            let paragraph = arena.paragraph(slot);
                            // @ref LLP 1043.000 §8 — an admitted auto-height leaf is
                            // measured around its settled shapes, in content space as
                            // the painters flow it. Intrinsic probes stay unobstructed:
                            // a context's width never depends on what flows inside.
                            let shapes: Vec<_> = match width {
                                AxisOffer::Definite(_) => context
                                    .flow
                                    .iter()
                                    .map(|s| s.translate(-inset.left, -inset.top))
                                    .collect(),
                                _ => Vec::new(),
                            };
                            let request = TextMeasureRequest {
                                exclusions: &shapes,
                                runs: &runs,
                                paragraph,
                                width,
                                height,
                            };
                            match &stamp {
                                Some(stamp) => measurer.measure_identified(stamp, &request),
                                None => measurer.measure(&request),
                            }
                        };
                        if !metrics.is_valid() {
                            invalid_metrics.get_or_insert_with(|| arena.local_id(slot));
                            return Size::ZERO;
                        }
                        if context.measurements.len() == LEAF_OFFERS {
                            context.measurements.remove(0);
                        }
                        context.measurements.push(Measurement {
                            width,
                            height,
                            metrics,
                        });
                        metrics
                    };
                    let mut metrics = metrics;
                    if let Some(h) = style.multicol.and_then(|m| m.used_height) {
                        metrics.height = h;
                    }
                    if arena.node_type(slot) == NodeType::TextInput
                        && arena.style(slot).field_sizing == FieldSizing::Fixed
                        && arena.props(slot).str(crate::PropId::SemanticTag) == Some("textarea")
                    {
                        let rows = arena
                            .props(slot)
                            .get(crate::PropId::Rows)
                            .and_then(crate::PropValue::as_int)
                            .filter(|n| *n > 0)
                            .unwrap_or(2);
                        metrics.height *= rows as f32 / 2.0;
                    }
                    first_baseline = metrics.first_baseline;
                    line_height = metrics.height;
                    Size {
                        width: metrics.width,
                        height: metrics.height,
                    }
                },
            );
            let centered = native_slot.is_some_and(|slot| {
                crate::FieldKind::from_props(arena.props(slot)) != crate::FieldKind::Textarea
            });
            let offset = if centered {
                (output.size.height - inset.top - inset.bottom - line_height) / 2.0
            } else {
                0.0
            };
            output.baselines =
                Baselines::from_first(first_baseline.map(|b| b + inset.top + offset));
            output
        };
        #[cfg(test)]
        {
            self.boundary_replays = 0;
        }
        for (node, input, previous) in boundaries {
            let mut output = self
                .taffy
                .compute_boundary_with_measure(node, input, &mut measure);
            // Both axes clip here; the changed internal extent is published on
            // this box but cannot contribute to an ancestor's scrollable extent.
            output.scrollable_overflow_rect = previous.scrollable_overflow_rect;
            if baselines_unread.contains(&node) {
                output.baselines = previous.baselines;
            }
            if output != previous {
                if let Some(parent) = self.taffy.parent(node) {
                    self.taffy
                        .mark_dirty(parent)
                        .map_err(|e| LayoutError::Engine(format!("boundary: {e:?}")))?;
                }
            } else {
                #[cfg(test)]
                {
                    self.boundary_replays += 1;
                }
            }
        }
        // `&mut measure`, as the boundaries take it: one measure type, so the
        // engine's algorithms compile once (by value, every app carried two
        // copies; LLP 1047.001).
        let result = self
            .taffy
            .compute_layout_with_measure(root, available, &mut measure);
        result.map_err(|e| LayoutError::Engine(format!("compute_layout: {e:?}")))?;
        self.provisional_chrome |= provisional_button;
        if let Some(view) = invalid_button {
            return Err(LayoutError::InvalidButtonMeasure(view));
        }
        if let Some(view) = invalid_metrics {
            return Err(LayoutError::InvalidTextMetrics(view));
        }
        self.offers.insert(root, offer);
        Ok(minima)
    }

    pub(crate) fn height_measured(&self, node: NodeId) -> bool {
        self.taffy
            .get_node_context(node)
            .is_some_and(|c| c.height_measured)
    }

    pub(crate) fn button_containing_width(&self, node: NodeId) -> Option<Option<f32>> {
        self.button_records.get(&node).map(|r| r.parent_width)
    }

    // The absolute frame publication will write, computed the same way
    // (a fold from the root's 0.0), so its bits match the published frame.
    fn frame_of(&self, arena: &NodeArena, root: u32, slot: u32) -> Frame {
        let mut path = Vec::new();
        let mut at = slot;
        loop {
            path.push(at);
            if at == root {
                break;
            }
            match arena.parent(at) {
                Some(parent) => at = parent,
                None => return Frame::default(),
            }
        }
        let mut frame = Frame::default();
        for &s in path.iter().rev() {
            let Some(node) = arena.taffy(s) else {
                return Frame::default();
            };
            let l = self.layout(node);
            frame = Frame {
                x: frame.x + l.location.x,
                y: frame.y + l.location.y,
                width: l.size.width,
                height: l.size.height,
            };
        }
        frame
    }

    /// @ref LLP 1043.000 §8 — measure admitted auto-height leaves around the
    /// shapes their settled frames resolve to. Each pass re-lays out only
    /// leaves whose shapes changed (Taffy's caches keep the rest), so a still
    /// layout costs one comparison sweep; changes can propagate through later
    /// leaves. Under
    /// the admission rule a leaf's shapes depend only on content before it,
    /// so pass k fixes the k-th admitted leaf in document order: the loop ends
    /// within one pass per leaf (plus one per shape a leaf grows into), and
    /// ends only when every leaf was measured around exactly the shapes the
    /// frames now give it — the set publication writes, compared bitwise.
    /// A fresh replay reaches the same point from no shapes at all.
    fn settle_flow(
        &mut self,
        root: NodeId,
        root_slot: u32,
        offer: Offer,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
    ) -> Result<(usize, usize), LayoutError> {
        self.unsettled.clear();
        if arena.exclusion_slots.is_empty() && self.flowing.is_empty() {
            return Ok((0, 0));
        }
        let exclusions = crate::flow::visible_exclusions(arena, root_slot);
        let contexts = crate::flow::contexts(arena, &exclusions);
        let under_root = |slot: u32| {
            let mut at = slot;
            while at != root_slot {
                match arena.parent(at) {
                    Some(parent) => at = parent,
                    None => return false,
                }
            }
            true
        };
        let mut passes = 0;
        let mut bound = 0;
        loop {
            let mut targets = crate::flow::targets(
                arena,
                &exclusions,
                &contexts,
                |s| self.frame_of(arena, root_slot, s),
                |s| arena.taffy(s).is_some_and(|n| self.height_measured(n)),
            );
            let stored = |slot: u32| {
                arena
                    .taffy(slot)
                    .and_then(|n| self.taffy.get_node_context(n))
                    .map_or(&[][..], |c| c.flow.as_slice())
            };
            let mut changed: Vec<u32> = targets
                .iter()
                .filter(|(&slot, shapes)| !crate::flow::shapes_eq(stored(slot), shapes))
                .map(|(&slot, _)| slot)
                .collect();
            changed.extend(
                self.flowing
                    .iter()
                    .copied()
                    .filter(|s| !targets.contains_key(s) && under_root(*s)),
            );
            if changed.is_empty() {
                return Ok((passes, passes + 1));
            }
            bound = bound.max(targets.len() + exclusions.len() + 2);
            if passes == bound {
                debug_assert!(false, "wrap-flow did not settle in {passes} passes");
                self.unsettled.extend(changed);
                return Ok((passes, passes + 1));
            }
            for slot in changed {
                let shapes = targets.remove(&slot).unwrap_or_default();
                let Some(node) = arena.taffy(slot) else {
                    self.flowing.remove(&slot);
                    continue;
                };
                if shapes.is_empty() {
                    self.flowing.remove(&slot);
                } else {
                    self.flowing.insert(slot);
                }
                if let Some(context) = self.taffy.get_node_context_mut(node) {
                    context.flow = shapes;
                }
                self.mark_dirty(node);
            }
            self.compute(root, offer, arena, measurer)?;
            passes += 1;
        }
    }

    /// Reconstruct the whole engine tree from the arena's columns, writing the
    /// new handles back. This is rehydration: columns plus rebuild.
    pub fn rebuild(arena: &mut NodeArena) -> LayoutTree {
        let mut tree = LayoutTree::new();
        arena.clear_taffy();
        let slots: Vec<u32> = arena.iter_live().collect();
        for slot in &slots {
            let node = tree.new_leaf(
                taffy_style(arena, *slot),
                *slot,
                arena.node_type(*slot).is_measured_leaf(),
            );
            arena.set_taffy(*slot, Some(node));
        }
        for slot in &slots {
            // Inline runs and a select's options are never laid out.
            if matches!(arena.node_type(*slot), NodeType::Text | NodeType::Control) {
                continue;
            }
            if let Some(node) = arena.taffy(*slot) {
                let children = order::laid_out(arena, *slot, &tree.taffy, node, |c| arena.taffy(c));
                tree.set_children(node, &children);
            }
        }
        tree
    }

    /// A separate engine tree of `slot`'s subtree, for a trial that must
    /// leave the ordinary tree, its caches and its frames alone; its handles
    /// by slot. The arena's own handles are never written.
    pub(crate) fn of_subtree(arena: &NodeArena, slot: u32) -> (LayoutTree, HashMap<u32, NodeId>) {
        let mut tree = LayoutTree::new();
        let slots = arena.subtree(slot);
        let nodes: HashMap<u32, NodeId> = slots
            .iter()
            .map(|&s| {
                let node = tree.new_leaf(
                    taffy_style(arena, s),
                    s,
                    arena.node_type(s).is_measured_leaf(),
                );
                (s, node)
            })
            .collect();
        for s in slots {
            if matches!(arena.node_type(s), NodeType::Text | NodeType::Control) {
                continue;
            }
            let node = nodes[&s];
            let children = order::laid_out(arena, s, &tree.taffy, node, |c| nodes.get(&c).copied());
            tree.set_children(node, &children);
        }
        (tree, nodes)
    }
}

/// Lay out `root_slot` and publish frames and resolved flow. The receipt uses
/// epoch zero; the kernel facade supplies its transaction epoch. Inline
/// runs have no geometry of their own: their frames are zero and they never
/// appear in the changed list, but their dirty bits are consumed like any
/// other node's.
pub fn compute(
    arena: &mut NodeArena,
    tree: &mut LayoutTree,
    measurer: &mut dyn TextMeasurer,
    root_slot: u32,
    offer: Offer,
) -> Result<LayoutReceipt, LayoutError> {
    let root = arena
        .taffy(root_slot)
        .ok_or_else(|| LayoutError::Engine("root has no engine node".into()))?;
    tree.compute(root, offer, arena, measurer)?;
    let (flow_passes, flow_comparisons) =
        tree.settle_flow(root, root_slot, offer, arena, measurer)?;
    crate::fragment::settle(arena, tree, measurer, root_slot, offer)?;
    for (&node, record) in &tree.button_records {
        if let Some(&slot) = tree.slots.get(&node) {
            arena.button_bases.insert(slot, record.parent_width);
        }
    }
    let mut receipt = publication::publish(arena, tree, root_slot);
    receipt.flow_passes = flow_passes;
    receipt.flow_comparisons = flow_comparisons;
    Ok(receipt)
}

#[cfg(test)]
mod upstream_layout_differential;
