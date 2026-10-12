//! A containing block a host gives a subtree (@ref LLP 1115.002 D8.2).
//!
//! A host shows some subtrees of one page in views of their own: a route in
//! a split's pane, a modal route in a sheet. Each is laid out in a box that
//! is not the window, and the host says which: a width and a height, each
//! the size (a pane reports its own) or a maximum (a sheet is as large as
//! its content, up to it). The kernel learns nothing else of panes or
//! sheets, and where the host puts the result is the host's.
//!
//! While a node has a block it is out of its parent's layout, as a box in
//! CSS's top layer is out of flow, and is laid out as a root is, in the
//! block instead of the viewport: it is the engine's root for its subtree,
//! so a block that changes lays out that subtree and nothing else. Its
//! frames are in the block's own space. Under a maximum it is sized as a
//! modal `dialog` is (`apply`). Viewport units, `env()`, `rem` and a
//! hairline's device pixel stay the window's: no block reaches the
//! environment, the root font size or the display's scale, which are
//! resolved into rows at a commit while a block never touches a row.
//! Authored rows never change, and a host that gives no block lays out
//! exactly as before.

use super::{engine, Kernel};
use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::id::{NodeFlags, Offer, ViewId};
use crate::layout::{self, LayoutMirror, LayoutReceipt};
use crate::style::resolve_calc;
use taffy::style::{BoxSizing, LengthPercentageAuto as Lpa, Position};
use taffy::util::{MaybeResolve, ResolveOrZero};

/// One axis of a host's containing block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockAxis {
    /// The block is this long: a pane's side.
    Is(f32),
    /// The block is at most this long, and as long as its content: a sheet's.
    AtMost(f32),
}

impl BlockAxis {
    /// The length the host gave, the size or the maximum.
    pub fn length(self) -> f32 {
        match self {
            BlockAxis::Is(v) | BlockAxis::AtMost(v) => v,
        }
    }
}

/// A containing block a host gives one node, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostBlock {
    /// Its width.
    pub width: BlockAxis,
    /// Its height.
    pub height: BlockAxis,
}

/// The blocks hosts gave: a few routes at most, so a list by slot, as the
/// covers are. Each holds its used size and whether an ancestor hid it when
/// it was last published.
#[derive(Clone, Debug, Default)]
pub(crate) struct Blocks(Vec<(u32, HostBlock, (f32, f32), bool)>);

impl Blocks {
    /// The block `slot` has, when a host gave it one.
    pub(crate) fn get(&self, slot: u32) -> Option<HostBlock> {
        self.0.iter().find(|b| b.0 == slot).map(|b| b.1)
    }

    /// Whether `slot` is the root of a layout of its own.
    pub(crate) fn holds(&self, slot: u32) -> bool {
        !self.0.is_empty() && self.0.iter().any(|b| b.0 == slot)
    }

    /// Whether any node has one.
    pub(crate) fn any(&self) -> bool {
        !self.0.is_empty()
    }

    pub(crate) fn set(&mut self, slot: u32, block: Option<HostBlock>) {
        match (self.0.iter_mut().find(|b| b.0 == slot), block) {
            (Some(held), Some(block)) => held.1 = block,
            (Some(_), None) => self.0.retain(|b| b.0 != slot),
            (None, Some(block)) => self.0.push((slot, block, (0.0, 0.0), false)),
            (None, None) => {}
        }
    }
}

/// Whether an ancestor of `slot` is `display: none`.
fn hidden_above(arena: &NodeArena, slot: u32) -> bool {
    let mut at = arena.parent(slot);
    while let Some(s) = at {
        if arena.style(s).display == crate::Display::None {
            return true;
        }
        at = arena.parent(s);
    }
    false
}

/// What publication starts a block's root with: whether an ancestor hides
/// it, and whether its whole subtree is published (it took or lost a block,
/// or what hides it changed). Neither for a root without a block.
pub(crate) fn publication_start(arena: &mut NodeArena, root: u32) -> (bool, bool) {
    if !arena.blocks.holds(root) {
        return (false, false);
    }
    let hidden = hidden_above(arena, root);
    let whole = arena.flags(root).has(NodeFlags::ATTACHED);
    let held = arena.blocks.0.iter_mut().find(|b| b.0 == root);
    let was = held.map(|b| std::mem::replace(&mut b.3, hidden));
    (hidden, whole || was != Some(hidden))
}

/// A block's part of its root's engine style. A root holds the absolutely
/// positioned boxes no positioned box inside it holds, so a `static` one is
/// `relative`. On an axis where the block is a maximum the root is sized as
/// a modal `dialog` is in its viewport: an automatic size is `fit-content`,
/// its border box is at most the maximum, and its insets and automatic
/// margins, which would place it against the edge being asked for, do not
/// apply. `fit-content` is what CSS gives an absolutely positioned box with
/// no inset, and the engine sizes one before it lays its children out, so
/// their percentages have their basis: under a maximum width a root that
/// is in flow is laid out as such a box, at its static position.
pub(crate) fn apply(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    let Some(block) = arena.blocks.get(slot) else {
        return;
    };
    let in_flow = s.position != Position::Absolute;
    if s.position == Position::Static {
        s.position = Position::Relative;
    }
    let basis = Some(block.width.length());
    let edges = s.padding.resolve_or_zero(basis, resolve_calc)
        + s.border.resolve_or_zero(basis, resolve_calc);
    let content_box = s.box_sizing == BoxSizing::ContentBox;
    let ceiling = |most: f32, edges: f32, own: Lpa, basis: f32| {
        let most = (most - if content_box { edges } else { 0.0 }).max(0.0);
        let own = own.maybe_resolve(Some(basis), resolve_calc);
        Lpa::length(own.map_or(most, |own: f32| own.min(most)))
    };
    let unplaced = |inset: &mut Lpa, margin: &mut Lpa| {
        *inset = Lpa::auto();
        if margin.is_auto() {
            *margin = Lpa::length(0.0);
        }
    };
    if let BlockAxis::AtMost(most) = block.width {
        unplaced(&mut s.inset.left, &mut s.margin.left);
        unplaced(&mut s.inset.right, &mut s.margin.right);
        if in_flow {
            s.position = Position::Absolute;
            (s.inset.top, s.inset.bottom) = (Lpa::auto(), Lpa::auto());
        }
        s.max_size.width = ceiling(most, edges.horizontal_axis_sum(), s.max_size.width, most);
    }
    if let BlockAxis::AtMost(most) = block.height {
        unplaced(&mut s.inset.top, &mut s.margin.top);
        unplaced(&mut s.inset.bottom, &mut s.margin.bottom);
        s.max_size.height = ceiling(most, edges.vertical_axis_sum(), s.max_size.height, most);
    }
}

impl Kernel {
    /// Give `view` a containing block, or take it back (`None`): at the next
    /// layout its subtree is laid out in the block, or in its parent again.
    /// Nonfinite or negative lengths are refused, as is a block for a root
    /// (the offer is its), for an inline run, and while a content region
    /// is registered.
    pub fn set_host_block(
        &mut self,
        view: ViewId,
        block: Option<HostBlock>,
    ) -> Result<(), KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        if let Some(b) = block {
            let lengths = [b.width.length(), b.height.length()];
            if !lengths.iter().all(|l| l.is_finite() && *l >= 0.0) {
                return Err(LayoutError::InvalidOffer.into());
            }
            let refused = if self.arena.is_root(slot) {
                Some("a root is laid out in its offer")
            } else if self.arena.is_inline_run(slot) {
                Some("an inline run has no box")
            } else if self.region.is_some() {
                Some("no block while a content region is registered")
            } else {
                None
            };
            if let Some(why) = refused {
                return Err(LayoutError::HostBlock(why).into());
            }
        }
        let before = self.arena.blocks.get(slot);
        if before == block {
            return Ok(());
        }
        self.arena.blocks.set(slot, block);
        if let (Some(node), Some(layout)) = (self.arena.taffy(slot), self.layout.as_deref_mut()) {
            layout.restyle(&self.arena, slot, node);
            layout.mark_dirty(node);
        }
        if before.is_some() == block.is_some() {
            return Ok(());
        }
        // It leaves its parent's layout, or returns to it: published whole
        // in the space it is now in, and its parent's children again.
        self.arena.flags_mut(slot).insert(NodeFlags::ATTACHED);
        if let Some(parent) = self.arena.parent(slot) {
            self.arena
                .flags_mut(parent)
                .insert(NodeFlags::CHILDREN_DIRTY);
            if let (Some(node), Some(layout)) =
                (self.arena.taffy(parent), self.layout.as_deref_mut())
            {
                layout.sync_children(&self.arena, parent, node);
                layout.mark_dirty(node);
            }
        }
        Ok(())
    }

    /// The containing block a host gave `view`.
    pub fn host_block(&self, view: ViewId) -> Option<HostBlock> {
        self.arena.blocks.get(self.arena.slot_of(view)?)
    }

    /// The used size of `view`'s block as last laid out: the given length
    /// where the block is the size, and where it is a maximum the root's
    /// border box and margins, at most the maximum. Zero while the root is
    /// hidden; `None` when `view` has no block.
    pub fn host_block_size(&self, view: ViewId) -> Option<(f32, f32)> {
        let slot = self.arena.slot_of(view)?;
        self.arena
            .blocks
            .0
            .iter()
            .find(|b| b.0 == slot)
            .map(|b| b.2)
    }

    /// The blocks under `root`, each with its offer: the lengths the host
    /// gave, which a maximum's root is sized inside (`apply`).
    fn blocks_under(&self, root: u32) -> Vec<(u32, Offer)> {
        let arena = &self.arena;
        let under = |slot: u32| slot != root && arena.is_ancestor(root, slot);
        let offer = |b: HostBlock| Offer::definite(b.width.length(), b.height.length());
        let held = arena.blocks.0.iter().filter(|b| under(b.0));
        held.map(|b| (b.0, offer(b.1))).collect()
    }

    /// Lay `root` out under `offer`, then each subtree with a block under
    /// it in its block, and publish them: one receipt, the page's nodes
    /// first and each block's after.
    pub(super) fn lay_out_roots(
        &mut self,
        root: u32,
        offer: Offer,
    ) -> Result<LayoutReceipt, LayoutError> {
        let mut receipt = layout::compute(
            &mut self.arena,
            engine(&mut self.layout),
            self.measurer.as_mut(),
            root,
            offer,
        )?;
        if !self.arena.blocks.any() {
            return Ok(receipt);
        }
        for (slot, offer) in self.blocks_under(root) {
            let tree = engine(&mut self.layout);
            let provisional = tree.provisional_chrome();
            let part = layout::compute(&mut self.arena, tree, self.measurer.as_mut(), slot, offer);
            // One answer for the whole layout: a stand-in measure anywhere.
            tree.keep_provisional_chrome(provisional);
            let part = part?;
            let used = self.used_size(slot);
            let held = self.arena.blocks.0.iter_mut().find(|b| b.0 == slot);
            let moved = held.is_some_and(|b| std::mem::replace(&mut b.2, used) != used);
            receipt.changed.extend(part.changed);
            receipt.updated.extend(part.updated);
            let key = self.arena.key(slot);
            if moved && !receipt.updated.contains(&key) {
                receipt.updated.push(key);
            }
            receipt.flow_changed.extend(part.flow_changed);
            receipt.flow_skipped.extend(part.flow_skipped);
            receipt.fragment_skipped = part.fragment_skipped;
            receipt.flow_passes += part.flow_passes;
            receipt.flow_comparisons += part.flow_comparisons;
        }
        Ok(receipt)
    }

    /// Lay each subtree with a block under `root` out in the engine alone,
    /// publishing nothing: what a measurement of authored heights reads.
    pub(super) fn lay_out_blocks_unpublished(&mut self, root: u32) -> Result<(), LayoutError> {
        if !self.arena.blocks.any() {
            return Ok(());
        }
        for (slot, offer) in self.blocks_under(root) {
            if let Some(node) = self.arena.taffy(slot) {
                engine(&mut self.layout).compute(
                    node,
                    offer,
                    &self.arena,
                    self.measurer.as_mut(),
                )?;
            }
        }
        Ok(())
    }

    fn used_size(&self, slot: u32) -> (f32, f32) {
        let shown = !self.arena.flags(slot).has(NodeFlags::HIDDEN);
        let tree = self.layout.as_deref().and_then(LayoutMirror::tree_ref);
        let (Some(block), Some(node), Some(tree)) = (
            self.arena.blocks.get(slot),
            self.arena.taffy(slot).filter(|_| shown),
            tree,
        ) else {
            return (0.0, 0.0);
        };
        let l = tree.layout(node);
        let axis = |axis: BlockAxis, end: f32| match axis {
            BlockAxis::Is(length) => length,
            BlockAxis::AtMost(most) => end.clamp(0.0, most),
        };
        (
            axis(block.width, l.location.x + l.size.width + l.margin.right),
            axis(block.height, l.location.y + l.size.height + l.margin.bottom),
        )
    }
}
