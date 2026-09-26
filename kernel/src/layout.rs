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

#[cfg(test)]
mod containment_tests;
mod publication;

use crate::id::{IdMap, IdSet};
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
}

// A non-visible overflow on both axes establishes a formatting context and
// prevents descendants' scrollable overflow/margins from escaping the box.
// Whether its size depends on its content is the engine's record to prove.
fn boundary_style(s: &taffy::Style) -> bool {
    s.display != taffy::Display::None
        && s.position == taffy::Position::Relative
        && matches!(
            s.overflow.x,
            taffy::Overflow::Hidden | taffy::Overflow::Scroll
        )
        && matches!(
            s.overflow.y,
            taffy::Overflow::Hidden | taffy::Overflow::Scroll
        )
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

    fn restyle(&mut self, arena: &NodeArena, slot: u32, node: NodeId) {
        self.set_style(node, taffy_style(arena, slot));
    }

    fn sync_children(&mut self, arena: &NodeArena, parent: u32, node: NodeId) -> usize {
        if arena.node_type(parent) == NodeType::Text {
            LayoutTree::set_children(self, node, &[]);
            return 0;
        }
        let ids: Vec<_> = arena
            .children(parent)
            .iter()
            .filter_map(|c| arena.taffy(*c))
            .collect();
        LayoutTree::set_children(self, node, &ids);
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
        }
    }

    fn note(&mut self, what: &str, result: Result<impl Sized, taffy::TaffyError>) {
        if let Err(e) = result {
            if self.fault.is_none() {
                self.fault = Some(format!("{what}: {e:?}"));
            }
        }
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
        self.deferred.remove(&node);
        if let Some(slot) = self.slots.remove(&node) {
            self.flowing.remove(&slot);
        }
        self.offers.remove(&node);
        self.presented_heights
            .retain(|(_, active, _)| *active != node);
        let r = self.taffy.remove(node);
        self.note("remove", r);
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
        self.clear_measurements(node);
        self.taffy.track_layout_input(node, boundary_style(&style));
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
            && !self.taffy.dirty(root).unwrap_or(true)
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
        // One unrelated dirty source may invalidate the root after the first
        // boundary was chosen. A regular root pass then handles all sources.
        if self.taffy.dirty(root).unwrap_or(true) {
            for node in boundaries {
                if let Some(parent) = self.taffy.parent(node) {
                    let r = self.taffy.mark_dirty(parent);
                    self.note("mark_dirty", r);
                }
            }
            return Vec::new();
        }
        // Nested candidates cannot be replayed independently: the outer one
        // owns the final constraints, so recompute the root in that rare case.
        if boundaries.iter().any(|&node| {
            let mut at = self.taffy.parent(node);
            while let Some(n) = at {
                if boundaries.contains(&n) {
                    return true;
                }
                at = self.taffy.parent(n);
            }
            false
        }) {
            for node in boundaries {
                if let Some(parent) = self.taffy.parent(node) {
                    let r = self.taffy.mark_dirty(parent);
                    self.note("mark_dirty", r);
                }
            }
            return Vec::new();
        }
        boundaries
            .into_iter()
            .filter_map(|node| {
                self.taffy
                    .last_layout_input(node)
                    .map(|(input, output)| (node, input, output))
            })
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
        self.compute_mapped(root, offer, arena, measurer, |s| arena.taffy(s))
    }

    /// Region trial trees supply their local handle map; arena handles belong
    /// solely to the ordinary tree and must never be indexed in a trial tree.
    pub(crate) fn compute_mapped(
        &mut self,
        root: NodeId,
        offer: Offer,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
        node_for: impl Fn(u32) -> Option<NodeId>,
    ) -> Result<(), LayoutError> {
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
        let boundaries = self.prepare_boundaries(root, offer, arena);
        let available = Size {
            width: to_available(offer.width),
            height: to_available(offer.height),
        };
        self.pass += 1;
        let pass = self.pass;
        let mut runs: Vec<TextRun<'_>> = Vec::new();
        let mut invalid_metrics = None;
        let mut measure = |inputs: LayoutInput,
                           _node,
                           context: Option<&mut MeasureContext>,
                           style: &taffy::Style| {
            let mut first_baseline = None;
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
                    if arena.node_type(slot).is_replaced() {
                        // A replaced element: its intrinsic size where nothing
                        // is known (Taffy has already applied the aspect ratio
                        // to a known dimension); nothing at all before it loads,
                        // as a broken `<img>` is 0×0.
                        let Some((iw, ih)) = arena.intrinsic(slot).or_else(|| {
                            (arena.node_type(slot) == NodeType::Video).then_some((300.0, 150.0))
                        }) else {
                            return Size::ZERO;
                        };
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
                    let height = from_available(space.height);
                    // Reuse before flattening runs or crossing the host seam. Height
                    // stays in the key, and the 0.14 proof above updates on hits too.
                    let metrics = if let Some(cached) = context
                        .measurements
                        .iter()
                        .find(|m| m.width == width && m.height == height)
                    {
                        cached.metrics
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
                        let metrics = match arena.paragraph_stamp(slot) {
                            Some(stamp) => measurer.measure_identified(&stamp, &request),
                            None => measurer.measure(&request),
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
                    first_baseline = metrics.first_baseline;
                    Size {
                        width: metrics.width,
                        height: metrics.height,
                    }
                },
            );
            output.baselines = Baselines::from_first(first_baseline.map(|b| b + inset.top));
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
            if self.baselines_unread(node) {
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
        let result = self
            .taffy
            .compute_layout_with_measure(root, available, measure);
        result.map_err(|e| LayoutError::Engine(format!("compute_layout: {e:?}")))?;
        if let Some(view) = invalid_metrics {
            return Err(LayoutError::InvalidTextMetrics(view));
        }
        self.offers.insert(root, offer);
        Ok(())
    }

    pub(crate) fn height_measured(&self, node: NodeId) -> bool {
        self.taffy
            .get_node_context(node)
            .is_some_and(|c| c.height_measured)
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
    /// layout costs one comparison and a moving shape one extra pass. Under
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
    ) -> Result<(), LayoutError> {
        self.unsettled.clear();
        if arena.exclusion_slots.is_empty() && self.flowing.is_empty() {
            return Ok(());
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
                return Ok(());
            }
            bound = bound.max(targets.len() + exclusions.len() + 2);
            if passes == bound {
                debug_assert!(false, "wrap-flow did not settle in {passes} passes");
                self.unsettled.extend(changed);
                return Ok(());
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
            if arena.node_type(*slot) == NodeType::Text {
                continue;
            }
            let children: Vec<NodeId> = arena
                .children(*slot)
                .iter()
                .filter_map(|c| arena.taffy(*c))
                .collect();
            if let Some(node) = arena.taffy(*slot) {
                tree.set_children(node, &children);
            }
        }
        tree
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
    tree.settle_flow(root, root_slot, offer, arena, measurer)?;
    Ok(publication::publish(arena, tree, root_slot))
}

#[cfg(test)]
mod upstream_layout_differential {
    use super::*;
    use crate::{
        Kernel, MonospaceMeasurer, Op, PropId, PropValue, StyleId, StyleProps, StyleValue,
        TextMetrics,
    };

    fn random(seed: &mut u64, bound: u64) -> f64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (*seed % bound) as f64
    }
    fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
        let mut patch = StyleProps::default();
        for (id, value) in rows {
            patch.set_dynamic(*id, value).unwrap();
        }
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    }
    fn t(s: &str) -> StyleValue {
        StyleValue::Text(s.into())
    }
    fn n(x: f64) -> StyleValue {
        StyleValue::Number(x)
    }
    #[derive(Default)]
    struct Metrics(Vec<[u32; 5]>);
    impl TextMeasurer for Metrics {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            let m = MonospaceMeasurer::default().measure(r);
            let offer = |a| match a {
                AxisOffer::Definite(n) => n.to_bits(),
                AxisOffer::MinContent => u32::MAX,
                AxisOffer::MaxContent => u32::MAX - 1,
            };
            self.0.push([
                offer(r.width),
                offer(r.height),
                m.width.to_bits(),
                m.height.to_bits(),
                m.first_baseline.unwrap_or(-1.).to_bits(),
            ]);
            m
        }
    }
    #[test]
    #[ignore = "async lane: 512 seeded trees, ~11 s warm; bun scripts/async.mjs runs it"]
    fn seeded_512_trees_match_fresh_frames_content_and_baselines() {
        let mut seed = 0x1043_0007_d1ff_u64;
        let mut measured = 0;
        for case in 0..512 {
            let mut k = Kernel::with_monospace();
            let mut ops = Vec::new();
            for id in 1..=32 {
                let kind = if id <= 3 {
                    NodeType::View
                } else if id == 5 || id == 7 || id % 5 == 0 {
                    NodeType::Image
                } else {
                    NodeType::Text
                };
                ops.push(Op::CreateView {
                    id,
                    node_type: kind,
                });
                if kind == NodeType::Text {
                    ops.push(Op::SetProp {
                        id,
                        prop: PropId::Text,
                        value: PropValue::Str(
                            "alpha beta longerword ".repeat(2 + random(&mut seed, 16) as usize),
                        ),
                    });
                    ops.push(style(
                        id,
                        &[
                            (StyleId::FontSize, n(10. + random(&mut seed, 20))),
                            (StyleId::PaddingLeft, n(random(&mut seed, 40))),
                            (StyleId::PaddingTop, n(random(&mut seed, 20))),
                            (StyleId::BorderWidthRight, n(random(&mut seed, 8))),
                            (StyleId::BorderWidthBottom, n(random(&mut seed, 5))),
                            (
                                StyleId::BoxSizing,
                                t(if case % 2 == 0 {
                                    "border-box"
                                } else {
                                    "content-box"
                                }),
                            ),
                        ],
                    ));
                }
            }
            ops.extend([
                style(
                    1,
                    &[
                        (StyleId::Width, n(320. + random(&mut seed, 480))),
                        (StyleId::Height, n(700.)),
                        (
                            StyleId::Display,
                            t(if case % 3 == 0 { "block" } else { "flex" }),
                        ),
                        (
                            StyleId::FlexDirection,
                            t(if case % 2 == 0 { "row" } else { "column" }),
                        ),
                        (StyleId::AlignItems, t("baseline")),
                    ],
                ),
                style(
                    2,
                    &[
                        (StyleId::Width, StyleValue::Percent(80.)),
                        (StyleId::Display, t("flex")),
                        (StyleId::FlexDirection, t("column")),
                    ],
                ),
                style(
                    3,
                    &[
                        (StyleId::Width, StyleValue::Percent(75.)),
                        (
                            StyleId::Display,
                            t(if case % 2 == 0 { "flex" } else { "block" }),
                        ),
                        (StyleId::AlignItems, t("baseline")),
                    ],
                ),
                style(
                    4,
                    &[
                        (StyleId::Width, n(300.)),
                        (StyleId::PaddingLeft, n(50.)),
                        (StyleId::PaddingRight, n(50.)),
                        (StyleId::FlexShrink, n(1.)),
                    ],
                ),
                style(
                    5,
                    &[
                        (StyleId::Width, n(100.)),
                        (StyleId::Height, n(0.)),
                        (StyleId::MarginTop, n(20. + random(&mut seed, 20))),
                        (StyleId::MarginBottom, n(30.)),
                    ],
                ),
                style(
                    6,
                    &[(StyleId::Width, n(250.)), (StyleId::FlexShrink, n(1.))],
                ),
                style(
                    7,
                    &[
                        (StyleId::Width, n(40. + random(&mut seed, 90))),
                        (StyleId::AspectRatio, n(2.)),
                    ],
                ),
                style(8, &[(StyleId::Width, StyleValue::Percent(90.))]),
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 4, 5, 6, 7],
                },
                Op::SetChildren {
                    id: 2,
                    children: vec![3],
                },
                Op::SetChildren {
                    id: 3,
                    children: (8..=32).collect(),
                },
                Op::AttachRoot { id: 1 },
            ]);
            k.apply(0, 0, &ops).unwrap();
            for id in [5, 7, 10, 15, 20, 25, 30] {
                k.set_intrinsic_size(
                    id,
                    Some((
                        100. + random(&mut seed, 200) as f32,
                        40. + random(&mut seed, 200) as f32,
                    )),
                )
                .unwrap();
            }
            let mut a = k.arena().clone();
            let mut b = a.clone();
            let mut incremental = LayoutTree::rebuild(&mut a);

            let (mut ma, mut mb) = (Metrics::default(), Metrics::default());
            for width in [320., 611.5, 480.] {
                let root = a.slot_of(1).unwrap();
                compute(
                    &mut a,
                    &mut incremental,
                    &mut ma,
                    root,
                    Offer::definite(width, 900.),
                )
                .unwrap();
                let mut fresh = LayoutTree::rebuild(&mut b);
                mb.0.clear();
                ma.0.clear();
                compute(
                    &mut b,
                    &mut fresh,
                    &mut mb,
                    root,
                    Offer::definite(width, 900.),
                )
                .unwrap();
                for slot in a.iter_live() {
                    let x = incremental.layout(a.taffy(slot).unwrap());
                    let y = fresh.layout(b.taffy(slot).unwrap());
                    let bits = |v: taffy::tree::Layout| {
                        [
                            v.location.x,
                            v.location.y,
                            v.size.width,
                            v.size.height,
                            v.scrollable_overflow_rect.right,
                            v.scrollable_overflow_rect.bottom,
                            v.border.left,
                            v.border.top,
                            v.padding.left,
                            v.padding.top,
                        ]
                        .map(f32::to_bits)
                    };
                    assert_eq!(bits(x), bits(y), "tree {case}, node {slot}, offer {width}");
                }
                measured += mb.0.len();
            }
        }
        assert!(measured > 10_000); // an empty or bypassed measurer cannot pass
        println!("Upstream differential: 512 seeded trees x 3 offers, 49152 node layouts equal to fresh; {measured} fresh measurements");
    }

    #[test]
    fn unpublished_layout_writes_are_bounded_by_live_nodes() {
        use taffy::prelude::TaffyMaxContent;
        let mut tree = LayoutTree::new();
        let node = tree.new_leaf(taffy::Style::default(), 0, false);
        for width in 1..=100 {
            let mut style = taffy::Style::default();
            style.size.width = taffy::Dimension::length(width as f32);
            tree.set_style(node, style);
            tree.taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        }
        assert_eq!(tree.taffy.take_layout_changes(), vec![node]);
        assert!(tree.taffy.take_layout_changes().is_empty());
        tree.set_style(node, taffy::Style::default());
        tree.taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        tree.remove(node);
        assert!(tree.taffy.take_layout_changes().is_empty());
    }
}
