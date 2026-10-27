//! A containing block a host gives a subtree (@ref LLP 1115.002 D8.2).
//!
//! A host shows some subtrees of one page in views of their own: a route in
//! a split's pane, a modal route in a sheet. Each is laid out in a box that
//! is not the window, and the host says which: a width and a height, each
//! the size (a pane reports its own), a maximum, or a content-fit limit
//! that an authored size may exceed. The kernel learns nothing else of panes or
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
//!
//! The kernel implies one block itself: the viewport, for each box in
//! HTML's top layer (a popover; `Kernel::imply_blocks`), fitting its
//! content as the UA sheet's `fit-content` does. A host's block for the
//! same box wins, and taking it back leaves the viewport.

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
    /// An automatic size fits its content under this limit. An authored
    /// size or minimum may exceed it; percentages still use this basis.
    Fit(f32),
    /// The region a box is placed in, CSS's inset-modified containing block
    /// for a `position-area` (a popover's option, LLP 1021 D2): across, an
    /// automatic size is `fit-content` in it, its content's min-content
    /// winning where that is wider; down, its content's height, which may
    /// overflow it (whether it does is what picks the option). Percentages
    /// use it, and an authored size is the author's.
    Region(f32),
}

impl BlockAxis {
    /// The length the host gave, the size or the maximum.
    pub fn length(self) -> f32 {
        match self {
            BlockAxis::Is(v) | BlockAxis::AtMost(v) | BlockAxis::Fit(v) | BlockAxis::Region(v) => v,
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

/// The blocks: a few routes and open popovers at most, so a list by slot,
/// as the covers are.
#[derive(Clone, Debug, Default)]
pub(crate) struct Blocks(Vec<Held>);

/// One node's block: the one in force, the host's or else the one the
/// kernel implies (a top-layer box's, the viewport: HTML's top layer), its
/// used size, and whether an ancestor hid it when it was last published.
#[derive(Clone, Debug)]
pub(crate) struct Held {
    slot: u32,
    block: HostBlock,
    used: (f32, f32),
    hidden: bool,
    given: Option<HostBlock>,
    implied: Option<HostBlock>,
    /// Whether a box above it was replaced whole when it was last styled
    /// (`apply` makes it `display: none` then).
    buried: bool,
}

impl Blocks {
    /// The block `slot` has, its host's or the kernel's.
    pub(crate) fn get(&self, slot: u32) -> Option<HostBlock> {
        self.held(slot).map(|b| b.block)
    }

    /// Whether `slot` is the root of a layout of its own.
    pub(crate) fn holds(&self, slot: u32) -> bool {
        !self.0.is_empty() && self.held(slot).is_some()
    }

    /// The roots of the blocks.
    pub(crate) fn roots(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().map(|b| b.slot)
    }

    /// Whether any node has one.
    pub(crate) fn any(&self) -> bool {
        !self.0.is_empty()
    }

    /// Whether a host gave any block.
    pub(crate) fn any_given(&self) -> bool {
        self.0.iter().any(|b| b.given.is_some())
    }

    /// The block a host gives `slot` (`None`: takes it back, leaving the
    /// implied one, if any).
    pub(crate) fn set(&mut self, slot: u32, block: Option<HostBlock>) {
        let implied = self.held(slot).and_then(|b| b.implied);
        self.put(slot, block, implied);
    }

    /// The block the kernel implies for `slot` (`None`: none).
    pub(crate) fn imply(&mut self, slot: u32, block: Option<HostBlock>) {
        let given = self.held(slot).and_then(|b| b.given);
        self.put(slot, given, block);
    }

    /// The block the kernel implies for `slot`.
    pub(crate) fn implied(&self, slot: u32) -> Option<HostBlock> {
        self.held(slot).and_then(|b| b.implied)
    }

    /// Every implied block: the slots that hold one.
    pub(crate) fn implied_slots(&self) -> Vec<u32> {
        let implied = self.0.iter().filter(|b| b.implied.is_some());
        implied.map(|b| b.slot).collect()
    }

    /// `slot` gone: neither block stands.
    pub(crate) fn remove(&mut self, slot: u32) {
        self.0.retain(|b| b.slot != slot);
    }

    fn held(&self, slot: u32) -> Option<&Held> {
        self.0.iter().find(|b| b.slot == slot)
    }

    fn held_mut(&mut self, slot: u32) -> Option<&mut Held> {
        self.0.iter_mut().find(|b| b.slot == slot)
    }

    fn put(&mut self, slot: u32, given: Option<HostBlock>, implied: Option<HostBlock>) {
        match (self.held_mut(slot), given.or(implied)) {
            (Some(held), Some(block)) => {
                (held.block, held.given, held.implied) = (block, given, implied)
            }
            (Some(_), None) => self.remove(slot),
            (None, Some(block)) => self.0.push(Held {
                slot,
                block,
                used: (0.0, 0.0),
                hidden: false,
                given,
                implied,
                buried: false,
            }),
            (None, None) => {}
        }
    }
}

/// Whether an ancestor of `slot` is `display: none`, as authored or as a
/// box a host replaced whole.
fn hidden_above(arena: &NodeArena, slot: u32) -> bool {
    let mut at = arena.parent(slot);
    while let Some(s) = at {
        if arena.style(s).display == crate::Display::None || replaced_whole(arena, s) {
            return true;
        }
        at = arena.parent(s);
    }
    false
}

fn replaced_whole(arena: &NodeArena, slot: u32) -> bool {
    arena.cover(slot) == Some(super::HostCover::Whole)
}

/// Whether an ancestor of `slot` is a box a host replaced whole.
fn replaced_above(arena: &NodeArena, slot: u32) -> bool {
    let mut at = arena.parent(slot);
    while let Some(s) = at {
        if replaced_whole(arena, s) {
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
    let hidden = hidden_above(arena, root) || replaced_whole(arena, root);
    let whole = arena.flags(root).has(NodeFlags::ATTACHED);
    let held = arena.blocks.held_mut(root);
    let was = held.map(|b| std::mem::replace(&mut b.hidden, hidden));
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
/// is in flow is laid out as such a box, at its static position. `Fit`
/// uses the same placement, but supplies a ceiling only for an automatic
/// size; CSS's own minimum can exceed that ceiling.
///
/// A block in a box a host replaced whole (a tab not yet built, LLP
/// 1075.003 §9.12) is `display: none` as that box is: none of its subtree
/// is measured, and it is published hidden until the box is given back.
pub(crate) fn apply(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    let Some(block) = arena.blocks.get(slot) else {
        return;
    };
    if replaced_above(arena, slot) {
        s.display = taffy::style::Display::None;
        return;
    }
    let _env = crate::style::LayoutEnv::set(arena.env());
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
    // A region sets no ceiling: the engine sizes an absolutely positioned
    // box with no inset as `fit-content` in its available width, which the
    // block's length is, min-content winning, and down as its content.
    let capped = |axis: BlockAxis, auto: bool| match axis {
        BlockAxis::AtMost(_) => true,
        BlockAxis::Fit(_) => auto,
        BlockAxis::Is(_) | BlockAxis::Region(_) => false,
    };
    if let BlockAxis::AtMost(most) | BlockAxis::Fit(most) | BlockAxis::Region(most) = block.width {
        unplaced(&mut s.inset.left, &mut s.margin.left);
        unplaced(&mut s.inset.right, &mut s.margin.right);
        if in_flow {
            s.position = Position::Absolute;
            (s.inset.top, s.inset.bottom) = (Lpa::auto(), Lpa::auto());
        }
        if capped(block.width, s.size.width.is_auto()) {
            s.max_size.width = ceiling(most, edges.horizontal_axis_sum(), s.max_size.width, most);
        }
    }
    if let BlockAxis::AtMost(most) | BlockAxis::Fit(most) | BlockAxis::Region(most) = block.height {
        unplaced(&mut s.inset.top, &mut s.margin.top);
        unplaced(&mut s.inset.bottom, &mut s.margin.bottom);
        if capped(block.height, s.size.height.is_auto()) {
            s.max_size.height = ceiling(most, edges.vertical_axis_sum(), s.max_size.height, most);
        }
    }
}

impl Kernel {
    /// Give `view` a containing block, or take it back (`None`): at the next
    /// layout its subtree is laid out in the block, or in its parent again
    /// (in the viewport for a top-layer box, the block the kernel implies).
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
            } else if self.arena.in_paragraph(slot) {
                Some("an inline run or image has no box of its own")
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
        self.arena.blocks.set(slot, block);
        self.block_changed(slot, before);
        Ok(())
    }

    /// Each top-layer box's block made the viewport, and a box that left the
    /// top layer (or became a root or an inline run, or its host replaced
    /// it whole) its own again: HTML's
    /// top layer, where `width: 50%` is half the viewport whatever box the
    /// popover was written in. Before each layout, after the viewport is
    /// the offer's; until a definite offer has given a viewport there is
    /// none, and a top-layer box stays in its positioned ancestor. One
    /// exception: under a registered content region a top-layer box stays
    /// in the region's trial (QUEUE, "A popover under a registered content
    /// region").
    pub(super) fn imply_blocks(&mut self) {
        let (wants, moved) = self.implied_changes();
        for (slot, want) in wants {
            let before = self.arena.blocks.get(slot);
            self.arena.blocks.imply(slot, want);
            self.block_changed(slot, before);
        }
        // A block moved under a box replaced whole, or out from under one
        // (its ancestors reparented), is styled again: `apply` reads that.
        for slot in moved {
            self.restyle_block(slot);
        }
    }

    /// What `imply_blocks` would change: each box whose implied block is
    /// to be another, and each block root to be styled again.
    fn implied_changes(&self) -> (Vec<(u32, Option<HostBlock>)>, Vec<u32>) {
        let arena = &self.arena;
        if arena.top_layer_slots.is_empty() && !arena.blocks.any() {
            return (Vec::new(), Vec::new());
        }
        let (env, lifted) = (arena.env(), &arena.top_layer_slots);
        let (width, height) = (env.viewport_width, env.viewport_height);
        let viewport = (width > 0.0 && height > 0.0).then_some(HostBlock {
            width: BlockAxis::Fit(width),
            height: BlockAxis::Fit(height),
        });
        let mut slots = arena.blocks.implied_slots();
        slots.extend(lifted.iter().filter(|s| arena.blocks.implied(*s).is_none()));
        let region = self.region.as_ref().map(|r| r.binding.owner.index);
        let wants: Vec<_> = slots
            .into_iter()
            .map(|slot| {
                // One its host replaced whole (a closed popover, a Linux
                // shell's menu) has no layout to lay out in it. One under a
                // registered content region stays in the region's trial
                // (theme2 09:54; the native region captures it: QUEUE).
                let boxed = !arena.is_root(slot)
                    && !arena.in_paragraph(slot)
                    && arena.cover(slot) != Some(super::HostCover::Whole)
                    && region.is_none_or(|owner| !arena.is_ancestor(owner, slot));
                (slot, viewport.filter(|_| lifted.contains(slot) && boxed))
            })
            .filter(|&(slot, want)| arena.blocks.implied(slot) != want)
            .collect();
        let moved: Vec<u32> = (arena.blocks.0.iter())
            .filter(|b| b.buried != replaced_above(arena, b.slot))
            .map(|b| b.slot)
            .collect();
        (wants, moved)
    }

    /// `slot`'s block root styled again, as its block or what is above it
    /// now says.
    fn restyle_block(&mut self, slot: u32) {
        let buried = replaced_above(&self.arena, slot);
        if let Some(held) = self.arena.blocks.held_mut(slot) {
            held.buried = buried;
        }
        if let (Some(node), Some(layout)) = (self.arena.taffy(slot), self.layout.as_deref_mut()) {
            layout.restyle(&self.arena, slot, node);
            layout.mark_dirty(node);
        }
    }

    /// `slot`'s block was `before`: restyled for the one it has now, and if
    /// it gained or lost one, moved out of its parent's layout or into it.
    fn block_changed(&mut self, slot: u32, before: Option<HostBlock>) {
        let block = self.arena.blocks.get(slot);
        if before == block {
            return;
        }
        self.restyle_block(slot);
        if before.is_some() == block.is_some() {
            return;
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
    }

    /// The containing block `view` is laid out in: its host's, or for a
    /// top-layer box the viewport, as of the last layout.
    pub fn host_block(&self, view: ViewId) -> Option<HostBlock> {
        self.arena.blocks.get(self.arena.slot_of(view)?)
    }

    /// Where a top-layer box laid out in the viewport's block sits in a
    /// viewport `width` by `height`, as a fixed box does by its authored
    /// insets (the block unset them, so its frame is at its margins): a
    /// side's inset from that side, its margin past it; with both insets an
    /// `auto` margin takes the room left, two sharing it; no inset on an
    /// axis, at its margin. Its border box's origin; `None` when the kernel implies
    /// no block for `view`. What a host that draws the top layer itself
    /// places it by (the terminal), and where the runner reads one no host
    /// has said it shows.
    pub fn top_layer_origin(
        &self,
        view: ViewId,
        (width, height): (f32, f32),
    ) -> Option<(f32, f32)> {
        if !self.host_block_implied(view) {
            return None;
        }
        let node = self.node(view)?;
        let (s, f, env) = (node.style, node.frame, self.arena.env_for(node.key.index));
        let used = self.host_block_size(view)?;
        // The end margins as laid out: the used size clamps a negative one.
        let tree = self.layout.as_deref().and_then(LayoutMirror::tree_ref);
        let laid = self.arena.taffy(node.key.index).zip(tree);
        let ends = laid.map_or(
            (used.0 - f.x - f.width, used.1 - f.y - f.height),
            |(n, t)| {
                let margin = t.layout(n).margin;
                (margin.right, margin.bottom)
            },
        );
        let length = |d: crate::Dimension, basis: f32| {
            (d.resolve(env) != crate::Dimension::Auto).then(|| d.at_basis(env, basis))
        };
        let auto = |m: crate::Dimension| m == crate::Dimension::Auto;
        // The containing block's direction, the root's: which inset wins
        // when both are written and neither margin is `auto`, and which
        // margin is zero when two `auto` ones would be negative.
        let mut root = node.key.index;
        while let Some(up) = self.arena.parent(root) {
            root = up;
        }
        let rtl = self
            .arena
            .computed_source(root, crate::StyleId::Direction)
            .direction
            == crate::generated::Direction::Rtl;
        // CSS 2 §10.3.7 and §10.6.4: with both insets, an `auto` margin
        // takes the room left and two share it; across, a negative share
        // leaves the start margin zero (the end one in RTL), down it stays.
        let axis = |inset: [crate::Dimension; 2],
                    margin: [crate::Dimension; 2],
                    at: f32,
                    size: f32,
                    end_margin: f32,
                    room: f32,
                    across: bool| {
            let from_end = |end: f32| room - end - end_margin - size;
            match (length(inset[0], room), length(inset[1], room)) {
                (Some(start), Some(end)) => match (auto(margin[0]), auto(margin[1])) {
                    (true, true) => {
                        let share = (room - start - end - size) / 2.0;
                        match (across && share < 0.0, rtl) {
                            (true, false) => start,
                            (true, true) => room - end - size,
                            (false, _) => start + share,
                        }
                    }
                    (true, false) => from_end(end),
                    (false, true) => start + at,
                    (false, false) if across && rtl => from_end(end),
                    (false, false) => start + at,
                },
                (Some(start), None) => start + at,
                (None, Some(end)) => from_end(end),
                (None, None) => at,
            }
        };
        let x = axis(
            [s.left, s.right],
            [s.margin_left, s.margin_right],
            f.x,
            f.width,
            ends.0,
            width,
            true,
        );
        let y = axis(
            [s.top, s.bottom],
            [s.margin_top, s.margin_bottom],
            f.y,
            f.height,
            ends.1,
            height,
            false,
        );
        Some((x, y))
    }

    /// `view`'s node, unless it is laid out in a block of its own (a
    /// host's, or a popover's viewport): its frame is in that block's space,
    /// not its parent's.
    pub fn node_unblocked(&self, view: ViewId) -> Option<crate::NodeRef<'_>> {
        self.node(view).filter(|_| self.host_block(view).is_none())
    }

    /// Whether `view`'s block is the one the kernel implies (a top-layer
    /// box's, the viewport), no host having given it one.
    pub fn host_block_implied(&self, view: ViewId) -> bool {
        let Some(slot) = self.arena.slot_of(view) else {
            return false;
        };
        let held = self.arena.blocks.held(slot);
        held.is_some_and(|b| b.given.is_none())
    }

    /// The used size of `view`'s block as last laid out: the given length
    /// where the block is the size, and where it is a maximum the root's
    /// border box and margins, clamped only for `AtMost`. Zero while the root is
    /// hidden; `None` when `view` has no block.
    pub fn host_block_size(&self, view: ViewId) -> Option<(f32, f32)> {
        let slot = self.arena.slot_of(view)?;
        self.arena.blocks.held(slot).map(|b| b.used)
    }

    /// The blocks under `root`, each with its offer: the lengths the host
    /// gave, which a maximum's root is sized inside (`apply`).
    fn blocks_under(&self, root: u32) -> Vec<(u32, Offer)> {
        let arena = &self.arena;
        let under = |slot: u32| slot != root && arena.is_ancestor(root, slot);
        let offer = |b: HostBlock| Offer::definite(b.width.length(), b.height.length());
        let held = arena.blocks.0.iter().filter(|b| under(b.slot));
        held.map(|b| (b.slot, offer(b.block))).collect()
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
        self.lay_out_blocks(root, &mut receipt)?;
        Ok(receipt)
    }

    /// Lay each subtree with a block under `root` out in its block and
    /// publish it, into `receipt`: after the page's nodes, or a content
    /// region's (a top-layer box leaves the region's trial).
    pub(super) fn lay_out_blocks(
        &mut self,
        root: u32,
        receipt: &mut LayoutReceipt,
    ) -> Result<(), LayoutError> {
        if !self.arena.blocks.any() {
            return Ok(());
        }
        for (slot, offer) in self.blocks_under(root) {
            let tree = engine(&mut self.layout);
            let provisional = tree.provisional_chrome();
            let part = layout::compute(&mut self.arena, tree, self.measurer.as_mut(), slot, offer);
            // One answer for the whole layout: a stand-in measure anywhere.
            tree.keep_provisional_chrome(provisional);
            let part = part?;
            let used = self.used_size(slot);
            let held = self.arena.blocks.held_mut(slot);
            let moved = held.is_some_and(|b| std::mem::replace(&mut b.used, used) != used);
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
        Ok(())
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
        used_in(block, &tree.layout(node))
    }

    /// The used size `view`'s subtree would have in `block`, had its host
    /// given that block instead of the one it holds: what `host_block_size`
    /// would read after `set_host_block` and a layout, with the margins that
    /// layout resolved (a percentage is of the block). A host weighing
    /// several blocks for one box (a popover's position-area options, each
    /// its own region) measures each, then gives the one it chose.
    ///
    /// The subtree is laid out as its block's root is, by the same path
    /// (`layout::compute`: the engine, flow and fragment settling, fitting,
    /// publication), in a copy of the arena and engine tree that is dropped
    /// after: no block, frame or engine record the kernel holds changes,
    /// and the next layout does what it would have done. Only the text
    /// measurer, whose answers are a pure function of their inputs, may
    /// remember what it was asked. `view` must hold a block (its host's, or
    /// a top-layer box's viewport); zero where that layout would hide it,
    /// as `host_block_size` reads then.
    pub fn measure_block(
        &mut self,
        view: ViewId,
        block: HostBlock,
    ) -> Result<BlockMeasure, KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        let lengths = [block.width.length(), block.height.length()];
        if !lengths.iter().all(|l| l.is_finite() && *l >= 0.0) {
            return Err(LayoutError::InvalidOffer.into());
        }
        if !self.arena.blocks.holds(slot) {
            return Err(
                LayoutError::HostBlock("only a block's root is measured in a block").into(),
            );
        }
        let tree = self.layout.as_deref().and_then(LayoutMirror::tree_ref);
        let (Some(node), Some(tree)) = (self.arena.taffy(slot), tree) else {
            return Err(LayoutError::HostBlock("not laid out yet").into());
        };
        // What only the kernel's next layout prepares, before it lays out,
        // is not a trial's to do: a block to imply or restyle, a symbol to
        // size; nor is a change under the root since the last layout (a
        // text edit), which a measurer holding what that layout showed
        // would take as the text it shows. Until that layout the host
        // places by the last one.
        let (wants, moved) = self.implied_changes();
        if !wants.is_empty() || !moved.is_empty() || self.symbols_pending() || tree.is_dirty(node) {
            return Err(LayoutError::HostBlock("a layout is due first").into());
        }
        let (mut arena, mut trial) = (self.arena.trial_copy(), tree.clone());
        arena.blocks.set(slot, Some(block));
        trial.restyle(&arena, slot, node);
        trial.mark_dirty(node);
        let offer = Offer::definite(lengths[0], lengths[1]);
        layout::compute(&mut arena, &mut trial, self.measurer.as_mut(), slot, offer)?;
        if arena.flags(slot).has(NodeFlags::HIDDEN) {
            return Ok(BlockMeasure::default());
        }
        let l = trial.layout(node);
        Ok(BlockMeasure {
            used: used_in(block, &l),
            border: (l.size.width, l.size.height),
            margin: [l.margin.top, l.margin.right, l.margin.bottom, l.margin.left],
        })
    }
}

/// What `Kernel::measure_block` answers: the used size, the border box, and
/// the margins, top, right, bottom and left, as the layout resolved them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockMeasure {
    /// The used size, as `host_block_size` reads it.
    pub used: (f32, f32),
    /// The border box's width and height.
    pub border: (f32, f32),
    /// The margins, top, right, bottom and left.
    pub margin: [f32; 4],
}

/// A block root's used size from its engine layout: the given length where
/// the block is the size, and where it is a maximum or a fit its border box
/// and margins, clamped only for `AtMost`.
fn used_in(block: HostBlock, l: &taffy::tree::Layout) -> (f32, f32) {
    let axis = |axis: BlockAxis, end: f32| match axis {
        BlockAxis::Is(length) => length,
        BlockAxis::AtMost(most) => end.clamp(0.0, most),
        BlockAxis::Fit(_) | BlockAxis::Region(_) => end.max(0.0),
    };
    (
        axis(block.width, l.location.x + l.size.width + l.margin.right),
        axis(block.height, l.location.y + l.size.height + l.margin.bottom),
    )
}
