//! What a native container covers of a box (@ref LLP 1075.003 §3.5, §3.7).
//!
//! The host measures what its controller's bars take of a route — a
//! navigation bar over its top edge, a tab bar over its bottom — and the
//! kernel lays the route out inside what is left, as it lays a page out
//! inside `env(safe-area-inset-*)`: the covered edges add to the box's
//! padding. A box that a native bar replaces outright (a header whose
//! heading the bar shows) is covered whole and laid out as `display: none`.
//! Authored rows never change, and a host that reports nothing (the web,
//! Linux, the agent's presentation) lays out exactly as authored.
//!
//! A scroller whose content the platform itself insets — UIKit's adjusted
//! content inset under a navigation bar and over the home indicator — is
//! reported by those insets (`Inset`): its content box for layout excludes
//! them, so content that fills the scroller's height rests between the bar
//! and the indicator, while the content keeps its place (the platform's
//! inset moves it) and its published scroll extent leaves them out (the
//! platform adds them back). Scrolling content still passes under both.
//!
//! A replaced header that cleared the status bar itself (the web's
//! `padding-top: env(safe-area-inset-top)` under `viewport-fit=cover`)
//! hands that inset to its route's top cover, so the route's content starts
//! at the bar's bottom whichever of the two padded it (LLP 1075.003 §9.10).
//!
//! The margins a platform's own list gives a grouped list's cards and cells
//! are a claim of the same kind (`grouped::GroupedMargins`), set here.

use super::Kernel;
use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::id::ViewId;
use crate::layout::LayoutMirror;
use crate::style::{Dimension, Edge, Region};

/// A host container's claim on one box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HostCover {
    /// Points the container's bars cover at the top, right, bottom and left
    /// edges, added to the box's padding.
    Edges([f32; 4]),
    /// Replaced by a native control: the box takes no space.
    Whole,
    /// A scroller the platform insets by these points at its top and bottom
    /// edges: subtracted from its content box's height for layout, never
    /// moving the content, and left out of its published extent.
    Inset([f32; 2]),
    /// Space occupied by native scrollers on the right and bottom edges.
    /// Overlay scrollers report zero; authored padding stays unchanged.
    Gutter([f32; 2]),
}

impl Kernel {
    /// The bottom padding `key`'s cover adds to its layout as it is applied
    /// (`apply`): a bar's clearance, or none where the scroller at the box's
    /// bottom edge takes it as its platform inset.
    pub fn cover_bottom(&self, key: crate::NodeKey) -> f32 {
        match self.arena.cover(key.index) {
            Some(HostCover::Edges(_)) => applied_padding(&self.arena, key.index)[2],
            _ => 0.0,
        }
    }

    /// Set or clear (`None`) what a host container covers of `view`.
    /// Nonfinite or negative edges are refused.
    pub fn set_host_cover(
        &mut self,
        view: ViewId,
        cover: Option<HostCover>,
    ) -> Result<(), KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        let edges: &[f32] = match &cover {
            Some(HostCover::Edges(e)) => e,
            Some(HostCover::Inset(e) | HostCover::Gutter(e)) => e,
            _ => &[],
        };
        if !edges.iter().all(|e| e.is_finite() && *e >= 0.0) {
            return Err(LayoutError::InvalidIntrinsicSize(view).into());
        }
        let cover =
            cover.filter(|c| *c != HostCover::Edges([0.0; 4]) && *c != HostCover::Inset([0.0; 2]));
        let before = self.arena.cover(slot);
        if before == cover {
            return Ok(());
        }
        self.arena.set_cover(slot, cover);
        if let Some(r) = &mut self.region {
            r.intrinsic(slot);
        }
        // A box replaced or given back changes what its parent's top cover
        // takes from it (`header_inset`), and a scroller the platform
        // insets at the bottom edge, or stops insetting, what the covered
        // box it ends pads there (`scroller_insets_bottom`), however deep.
        let whole = |c: Option<HostCover>| c == Some(HostCover::Whole);
        let bottom = |c: Option<HostCover>| matches!(c, Some(HostCover::Inset([_, b])) if b > 0.0);
        // A box replaced or given back also leaves the chain or joins it.
        // So does a box that comes to be covered, or stops: the walk stops
        // at it (`scroller_insets_bottom`).
        // And one inset at its top alone, where the walk stops too.
        let edges = |c: Option<HostCover>| matches!(c, Some(HostCover::Edges(_)));
        let inset = |c: Option<HostCover>| matches!(c, Some(HostCover::Inset(_)));
        let chain = (self.layout.is_some()
            && (bottom(before) != bottom(cover)
                || inset(before) != inset(cover)
                || whole(before) != whole(cover)
                || edges(before) != edges(cover)))
        .then(|| covering(&self.arena, slot))
        .flatten();
        let parent = (whole(before) != whole(cover))
            .then(|| self.arena.parent(slot))
            .flatten();
        // And a block under it is hidden or shown with it (`block::apply`).
        let blocks: Vec<u32> = if whole(before) != whole(cover) {
            let arena = &self.arena;
            (arena.blocks.roots())
                .filter(|b| *b != slot && arena.is_ancestor(slot, *b))
                .collect()
        } else {
            Vec::new()
        };
        // And an absolutely positioned child is placed past the edges the
        // cover names (`apply`), so its engine style is derived again.
        // A replaced first child's inset is part of its parent's top cover
        // (`header_inset`), which the parent's absolute children start past.
        let mut positioned = if edges(before) || edges(cover) {
            positioned_children(&self.arena, slot)
        } else {
            Vec::new()
        };
        if let Some(p) = parent {
            positioned.extend(positioned_children(&self.arena, p));
        }
        for slot in std::iter::once(slot)
            .chain(parent)
            .chain(chain)
            .chain(blocks)
            .chain(positioned)
        {
            if let (Some(node), Some(layout)) = (self.arena.taffy(slot), self.layout.as_deref_mut())
            {
                layout.restyle(&self.arena, slot, node);
                layout.mark_dirty(node);
            }
        }
        Ok(())
    }
}

impl Kernel {
    /// Set or clear (`None`) the margin the platform's cell gives the rows of
    /// the cells list `view` at its left and right (LLP 1138). Nonfinite or
    /// negative lengths are refused.
    pub fn set_list_margins(
        &mut self,
        view: ViewId,
        margins: Option<[f32; 2]>,
    ) -> Result<(), KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        if !margins
            .unwrap_or([0.0; 2])
            .iter()
            .all(|l| l.is_finite() && *l >= 0.0)
        {
            return Err(LayoutError::InvalidIntrinsicSize(view).into());
        }
        if self.arena.list_margins(slot) == margins {
            return Ok(());
        }
        self.arena.set_list_margins(slot, margins);
        if let Some(r) = &mut self.region {
            r.intrinsic(slot);
            r.within(&self.arena, slot);
        }
        if let Some(layout) = self.layout.as_deref_mut() {
            for &row in self.arena.children(slot) {
                crate::grouped::restyle(&self.arena, layout, row);
            }
        }
        Ok(())
    }

    /// Set or clear (`None`) the margins the platform's list gives the
    /// grouped list `view`. Nonfinite or negative lengths are refused.
    pub fn set_grouped_margins(
        &mut self,
        view: ViewId,
        margins: Option<crate::grouped::GroupedMargins>,
    ) -> Result<(), KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        let lengths = margins.map_or([0.0; 4], |m| [m.card[0], m.card[1], m.cell[0], m.cell[1]]);
        if !lengths.iter().all(|l| l.is_finite() && *l >= 0.0) {
            return Err(LayoutError::InvalidIntrinsicSize(view).into());
        }
        if self.arena.grouped_margins(slot) == margins {
            return Ok(());
        }
        self.arena.set_grouped_margins(slot, margins);
        if let Some(r) = &mut self.region {
            r.intrinsic(slot);
        }
        if let Some(layout) = self.layout.as_deref_mut() {
            for &section in self.arena.children(slot) {
                crate::grouped::restyle(&self.arena, layout, section);
            }
        }
        Ok(())
    }
}

/// A cover's part of a box's engine style (@ref LLP 1075.003 §3.5): a
/// container's bars add to the padding of the route they cover, a box a bar
/// replaces takes no space, and a scroller the platform insets ends its
/// content box that much sooner (LLP 1116.000 D1.2): its end padding takes
/// both insets, so its children keep their start and their scroll extent's
/// end is where the platform's own inset begins.
pub(crate) fn apply(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    // A popover is the host's to lift, out of flow, open or closed (`style::top_layer`).
    crate::style::top_layer::apply(arena, slot, s);
    let (style, env) = (arena.effective_style(slot), arena.env());
    match arena.cover(slot) {
        Some(HostCover::Whole) => s.display = taffy::style::Display::None,
        Some(HostCover::Edges(_)) => {
            let [top, right, bottom, left] = applied_padding(arena, slot);
            // The authored padding as layout reads it (whole cells under a
            // terminal's space rule), plus the host's own inset.
            let (down, across) = (crate::style::cells::ROW, crate::style::cells::COLUMN);
            s.padding = taffy::geometry::Rect {
                top: style.padding_top.spaced(env, down).floor_lp(env, top),
                right: style.padding_right.spaced(env, across).floor_lp(env, right),
                bottom: style.padding_bottom.spaced(env, down).floor_lp(env, bottom),
                left: style.padding_left.spaced(env, across).floor_lp(env, left),
            };
        }
        Some(HostCover::Inset(_)) => {
            let [_, _, bottom, _] = added_padding(arena, slot);
            let spaced = style.padding_bottom.spaced(env, crate::style::cells::ROW);
            s.padding.bottom = spaced.floor_lp(env, bottom);
        }
        Some(HostCover::Gutter([right, bottom])) => {
            s.scrollbar_width = right.max(bottom);
            // Taffy reserves its scalar width on Scroll axes. Hidden keeps
            // the same scroll-container layout without reserving that axis.
            if right == 0.0 && s.overflow.y == taffy::style::Overflow::Scroll {
                s.overflow.y = taffy::style::Overflow::Hidden;
            }
            if bottom == 0.0 && s.overflow.x == taffy::style::Overflow::Scroll {
                s.overflow.x = taffy::style::Overflow::Hidden;
            }
        }
        None => {}
    }
    // An absolutely positioned child is placed against its parent's padding
    // box (CSS), which the parent's cover padding does not shrink: a footer
    // at `bottom: 0` would sit under the tab bar. The bars cover the
    // parent's edges, so an inset from a covered edge starts where the bar
    // ends, as on the web the footer rests above the tablist in flow and a
    // hand-built screen pins it to the safe area. A percentage inset stays
    // as written. A subtree in its own block has no covered parent edge.
    if s.position == taffy::style::Position::Absolute && !arena.blocks.holds(slot) {
        if let Some(parent) = arena.parent(slot) {
            if matches!(arena.cover(parent), Some(HostCover::Edges(_))) {
                let [top, right, bottom, left] = added_padding(arena, parent);
                let past = |inset: &mut taffy::style::LengthPercentageAuto, cover: f32| {
                    let raw = inset.into_raw();
                    if cover > 0.0 && raw.tag() == taffy::style::CompactLength::LENGTH_TAG {
                        *inset = taffy::style::LengthPercentageAuto::length(raw.value() + cover);
                    }
                };
                past(&mut s.inset.top, top);
                past(&mut s.inset.right, right);
                past(&mut s.inset.bottom, bottom);
                past(&mut s.inset.left, left);
            }
        }
    }
}

/// Whether the scroller at `slot`'s bottom edge is one the platform insets
/// there (`Inset`), the bar's own clearance for the box's bottom. It is
/// found as the host finds it: the last in-flow child, or that child's last,
/// down to the first scroller (a sheet component's root column holding its
/// header and then its scroll). A marked scroller off that chain counts for
/// nothing: the box keeps its padding (an inset twice at worst, never the
/// scroller lifted above the bar for good).
fn scroller_insets_bottom(arena: &NodeArena, slot: u32) -> bool {
    let mut at = slot;
    loop {
        let flowing = |&&c: &&u32| {
            let s = arena.style(c);
            s.display != crate::Display::None
                // A popover is out of flow, open or closed (`style::top_layer`).
                && crate::style::top_layer::layout_position(arena, c) != crate::PositionType::Absolute
                && arena.cover(c) != Some(HostCover::Whole)
        };
        let Some(&last) = arena.children(at).iter().rev().find(flowing) else {
            return false;
        };
        match arena.cover(last) {
            Some(HostCover::Inset([_, bottom])) => return bottom > 0.0,
            // Another covered box decides its own bottom: what is inset
            // below it is its scroller, not this box's.
            Some(HostCover::Edges(_)) => return false,
            _ => {}
        }
        if scrolls(arena, last) {
            return false;
        }
        at = last;
    }
}

/// Whether `slot` scrolls vertically, as its engine style says (`style.rs`).
fn scrolls(arena: &NodeArena, slot: u32) -> bool {
    let s = arena.style(slot);
    let y = if !s.mask.has(crate::StyleId::OverflowY) && arena.node_type(slot).scrolls_by_default()
    {
        crate::Overflow::Auto
    } else {
        s.overflow_y
    };
    // As the engine computes it: `visible` beside a clipping axis scrolls.
    matches!(
        y.computed(s.overflow_x),
        crate::Overflow::Auto | crate::Overflow::Scroll
    )
}

/// The nearest box above `slot` a container's bars cover (`Edges`), whose
/// bottom padding a scroller at its edge decides (`scroller_insets_bottom`):
/// none when a scroller lies between, where that search stops.
fn covering(arena: &NodeArena, slot: u32) -> Option<u32> {
    let mut at = arena.parent(slot);
    while let Some(p) = at {
        if matches!(arena.cover(p), Some(HostCover::Edges(_))) {
            return Some(p);
        }
        if scrolls(arena, p) {
            return None;
        }
        at = arena.parent(p);
    }
    None
}

/// `slot`'s style changed (`txn::style_changed`, a layout row): a replaced
/// header's padding is part of its route's top cover, and its `display`,
/// `position` or `overflow-y` its place on the bottom edge's chain.
pub(crate) fn style_changed(
    arena: &NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    mask: &crate::StyleMask,
) {
    use crate::StyleId;
    if [
        StyleId::Display,
        StyleId::PositionType,
        StyleId::OverflowX,
        StyleId::OverflowY,
    ]
    .iter()
    .any(|&id| mask.has(id))
    {
        chain_changed(arena, layout, slot);
    }
    if arena.cover(slot) == Some(HostCover::Whole) {
        if let Some(p) = arena.parent(slot) {
            // And its absolute children, placed past that cover.
            for s in std::iter::once(p).chain(positioned_children(arena, p)) {
                if let Some(node) = arena.taffy(s) {
                    layout.restyle(arena, s, node);
                    layout.mark_dirty(node);
                }
            }
        }
    }
}

/// `slot`'s place on the bottom edge's chain may have changed (its
/// `display`, `position` or `overflow-y`): the covered box above it is
/// derived again (`scroller_insets_bottom`).
pub(crate) fn chain_changed(arena: &NodeArena, layout: &mut dyn LayoutMirror, slot: u32) {
    if layout.tree_ref().is_none() {
        return;
    }
    if let Some((above, node)) = covering(arena, slot).and_then(|a| Some((a, arena.taffy(a)?))) {
        layout.restyle(arena, above, node);
        layout.mark_dirty(node);
    }
}

/// The absolutely positioned children of `slot`, which `apply` places past
/// its cover: derived again whenever the cover's points change (a cover
/// cleared included, so the caller says when).
pub(crate) fn positioned_children(arena: &NodeArena, slot: u32) -> Vec<u32> {
    (arena.children(slot).iter().copied())
        .filter(|&c| positioned(arena, c))
        .collect()
}

/// Whether `slot` lays out absolute: authored so, or a popover (`style::top_layer`).
fn positioned(arena: &NodeArena, slot: u32) -> bool {
    crate::style::top_layer::layout_position(arena, slot) == crate::PositionType::Absolute
}

/// The points a cover adds to the box's padding as layout pads it (`apply`):
/// `added_padding`, but nothing at the bottom where the scroller at the
/// box's bottom edge takes the bar's clearance as its platform inset (LLP
/// 1116.000 D1.2), so it reaches under the bar. An absolutely positioned
/// child is still placed past the whole cover (`added_padding`).
pub(crate) fn applied_padding(arena: &NodeArena, slot: u32) -> [f32; 4] {
    let [top, right, bottom, left] = added_padding(arena, slot);
    let edges = matches!(arena.cover(slot), Some(HostCover::Edges(_)));
    let bottom = if edges && scroller_insets_bottom(arena, slot) {
        0.0
    } else {
        bottom
    };
    [top, right, bottom, left]
}

/// The points a cover adds to the box's padding for layout (`apply`), top,
/// right, bottom and left: the host's sizing, not the box's CSS padding.
pub(crate) fn added_padding(arena: &NodeArena, slot: u32) -> [f32; 4] {
    match arena.cover(slot) {
        Some(HostCover::Edges([top, right, bottom, left])) => {
            [top + header_inset(arena, slot, top), right, bottom, left]
        }
        Some(HostCover::Inset([top, bottom])) => [0.0, 0.0, top + bottom, 0.0],
        _ => [0.0; 4],
    }
}

/// The scroll extent a scroller publishes: what the engine measured, less
/// the insets the platform adds back (`Inset`).
pub(crate) fn published_extent(arena: &NodeArena, slot: u32, extent: (f32, f32)) -> (f32, f32) {
    match arena.cover(slot) {
        Some(HostCover::Inset([top, bottom])) => (extent.0, (extent.1 - top - bottom).max(0.0)),
        _ => extent,
    }
}

/// Whether the platform insets this scroller (`Inset`).
pub(crate) fn inset(arena: &NodeArena, slot: u32) -> bool {
    matches!(arena.cover(slot), Some(HostCover::Inset(_)))
}

/// `s` as authored at the end the platform's inset took (`apply`).
pub(crate) fn without_inset(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    if inset(arena, slot) {
        let (style, env) = (arena.effective_style(slot), arena.env());
        let spaced = style.padding_bottom.spaced(env, crate::style::cells::ROW);
        s.padding.bottom = spaced.floor_lp(env, 0.0);
    }
}

/// A box's children changed: a covered box's top cover may hold its first
/// child's inset (`header_inset`), so its engine style is derived again.
pub(crate) fn children_changed(
    arena: &NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    touched: &mut Vec<crate::NodeKey>,
) {
    if let (Some(HostCover::Edges(_)), Some(node)) = (arena.cover(slot), arena.taffy(slot)) {
        layout.restyle(arena, slot, node);
        layout.mark_dirty(node);
    }
    if layout.tree_ref().is_some() {
        // An absolutely positioned child placed past its parent's cover, or
        // no longer (`apply`): a child attached, or moved here from a
        // covered box, is derived again.
        for &c in arena.children(slot) {
            if positioned(arena, c) {
                if let Some(node) = arena.taffy(c) {
                    layout.restyle(arena, c, node);
                }
            }
        }
        // And the scroller at the covered box's bottom edge may be another
        // (`scroller_insets_bottom`), however deep the change, unless the
        // box is that covered box itself (above) or a scroller, where the
        // search stops.
        if !matches!(arena.cover(slot), Some(HostCover::Edges(_))) && !scrolls(arena, slot) {
            chain_changed(arena, layout, slot);
        }
    }
    crate::rows::children_changed(arena, layout, slot, touched);
}

/// The top inset a box's replaced first child cleared for itself: that
/// child is covered whole and pads its top by `env(safe-area-inset-top)`
/// or the corner-adapted safe area's, which never lies above it (LLP 1123
/// D10), or a `calc()` or comparison that reads one (LLP 1001 §2).
/// It counts only while a bar covers the box's top (`top > 0`); a scroller
/// that goes under the bar is inset by the platform instead.
pub(crate) fn header_inset(arena: &NodeArena, slot: u32, top: f32) -> f32 {
    let Some(&first) = arena.children(slot).first() else {
        return 0.0;
    };
    match (
        top > 0.0,
        arena.cover(first),
        arena.style(first).padding_top,
    ) {
        (true, Some(HostCover::Whole), Dimension::Env(Edge::Top, _))
        | (
            true,
            Some(HostCover::Whole),
            Dimension::Region(Region::SafeAreaCorners, Edge::Top, _),
        ) => arena.env().inset(Edge::Top),
        // `max(env(safe-area-inset-top), 12px)` clears the inset too.
        (true, Some(HostCover::Whole), Dimension::Compare(c))
            if c.reads(Edge::Top, arena.env()) =>
        {
            arena.env().inset(Edge::Top)
        }
        _ => 0.0,
    }
}
