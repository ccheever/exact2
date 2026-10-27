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
//! The margins a platform's own list gives a grouped list's cards and cells
//! are a claim of the same kind (`grouped::GroupedMargins`), set here.

use super::Kernel;
use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::id::ViewId;
use crate::layout::LayoutMirror;
use crate::{AlignItems, AlignSelf, FlexDirection};

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

/// What host containers cover, by the box's slot, and the absolutely
/// positioned boxes placed past a covered box's bottom-edge chain
/// (`settle_chain`), each with the bottom clearance its inset starts past: a
/// few route nodes at most, so lists by slot rather than a column (and not
/// maps, whose code every web core would carry).
#[derive(Clone, Debug, Default)]
pub(crate) struct Covers {
    boxes: Vec<(u32, HostCover)>,
    chain: Vec<(u32, f32)>,
}

impl Covers {
    pub(crate) fn get(&self, slot: u32) -> Option<HostCover> {
        self.boxes.iter().find(|(s, _)| *s == slot).map(|(_, c)| *c)
    }

    pub(crate) fn set(&mut self, slot: u32, cover: Option<HostCover>) {
        self.boxes.retain(|(s, _)| *s != slot);
        self.boxes.extend(cover.map(|c| (slot, c)));
    }

    /// A freed slot's: its cover, and its place past a chain.
    pub(crate) fn remove(&mut self, slot: u32) {
        self.boxes.retain(|(s, _)| *s != slot);
        self.chain.retain(|(s, _)| *s != slot);
    }

    /// The bottom clearance the absolutely positioned `slot`'s inset starts
    /// past, its parent a box of a covered box's bottom-edge chain, as last
    /// settled.
    pub(crate) fn chain_past(&self, slot: u32) -> f32 {
        self.chain
            .iter()
            .find(|(s, _)| *s == slot)
            .map_or(0.0, |(_, p)| *p)
    }
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
        // A scroller the platform insets at the bottom edge, or stops
        // insetting, changes what the covered box it ends pads there
        // (`scroller_insets_bottom`), however deep.
        let whole = |c: Option<HostCover>| c == Some(HostCover::Whole);
        let bottom = |c: Option<HostCover>| matches!(c, Some(HostCover::Inset([_, b])) if b > 0.0);
        // So does one inset at its top, or no longer, as the covered box's
        // first child in flow (`scroller_insets_top`).
        let top = |c: Option<HostCover>| matches!(c, Some(HostCover::Inset([t, _])) if t > 0.0);
        // A box replaced or given back also leaves the chain or joins it.
        // So does a box that comes to be covered, or stops: the walk stops
        // at it (`scroller_insets_bottom`).
        // And one inset at its top alone, where the walk stops too.
        let edges = |c: Option<HostCover>| matches!(c, Some(HostCover::Edges(_)));
        let inset = |c: Option<HostCover>| matches!(c, Some(HostCover::Inset(_)));
        let chain = (self.layout.is_some()
            && (bottom(before) != bottom(cover)
                || top(before) != top(cover)
                || inset(before) != inset(cover)
                || whole(before) != whole(cover)
                || edges(before) != edges(cover)))
        .then(|| covering(&self.arena, slot))
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
        let mut positioned = if edges(before) || edges(cover) {
            positioned_children(&self.arena, slot)
        } else {
            Vec::new()
        };
        // And a scroller that reaches past its sides, as far as they are covered (`side_reach`).
        if edges(before) || edges(cover) {
            positioned.extend(reaching(&self.arena, slot));
        }
        for slot in std::iter::once(slot)
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
        // And the absolute boxes a box of its bottom-edge chain holds.
        settle_chain(&mut self.arena, self.layout.as_deref_mut());
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
    let (style, env) = (arena.effective_style(slot), arena.env_for(slot));
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
    // A scroller under the covered box's side controls, its content where it was.
    let [right, left] = side_reach(arena, slot);
    let across = crate::style::cells::COLUMN;
    if right > 0.0 {
        s.margin.right = taffy::style::LengthPercentageAuto::length(-right);
        s.padding.right = style.padding_right.spaced(env, across).floor_lp(env, right);
    }
    if left > 0.0 {
        s.margin.left = taffy::style::LengthPercentageAuto::length(-left);
        s.padding.left = style.padding_left.spaced(env, across).floor_lp(env, left);
    }
    // An absolutely positioned child is placed against its parent's padding
    // box (CSS), which the parent's cover padding does not shrink: a footer
    // at `bottom: 0` would sit under the tab bar. The bars cover the
    // parent's edges, so an inset from a covered edge starts where the bar
    // ends, as on the web the footer rests above the tablist in flow and a
    // hand-built screen pins it to the safe area. So at the top, where the
    // box pads nothing while its first scroller reaches under a collapsing
    // title's bar (`scroller_insets_top`): a child at `top: 16px` rests 16
    // below the bar, expanded or collapsed. A box of the bottom-edge chain
    // reaches under the bar (`applied_padding`), and its absolute children
    // start past the clearance it reaches over (`settle_chain`), as a view
    // pinned to the safe-area guide of UIKit's lower split view stays above
    // the tab bar. A percentage inset stays as written. A subtree in its own
    // block has no covered parent edge.
    if s.position == taffy::style::Position::Absolute && !arena.blocks.holds(slot) {
        if let Some(parent) = arena.parent(slot) {
            let [top, right, bottom, left] = match arena.cover(parent) {
                Some(HostCover::Edges(_)) => added_padding(arena, parent),
                _ => [0.0, 0.0, arena.covers.chain_past(slot), 0.0],
            };
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
        let Some(last) = last_flowing(arena, at) else {
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

/// Whether `slot`'s first child in flow is one the platform insets at its
/// top (`Inset`): the scroller a title collapses with, the first box in
/// flow below the header the bar replaces (`NavigationHost.collapse`),
/// which goes under the bar. The covered box then pads nothing at its top
/// (`applied_padding`), as at the bottom (`scroller_insets_bottom`), while
/// its absolutely positioned children are placed past the whole cover
/// (`apply`). The host reports no scroller inset at its top in a route it
/// pads at the top, so the first child in flow is inset there only while it
/// is the title's scroller (`NavigationHost.reportCovers`).
fn scroller_insets_top(arena: &NodeArena, slot: u32) -> bool {
    let first = arena
        .children(slot)
        .iter()
        .copied()
        .find(|&c| flowing(arena, c));
    matches!(first.and_then(|c| arena.cover(c)), Some(HostCover::Inset([top, _])) if top > 0.0)
}

/// `slot`'s last child in flow, where its bottom-edge chain goes on
/// (`scroller_insets_bottom`).
fn last_flowing(arena: &NodeArena, slot: u32) -> Option<u32> {
    arena
        .children(slot)
        .iter()
        .rev()
        .copied()
        .find(|&c| flowing(arena, c))
}

/// Whether `c` is in flow for the edges a covered box's scrollers take:
/// shown, not absolutely positioned and not replaced by a native control.
fn flowing(arena: &NodeArena, c: u32) -> bool {
    arena.style(c).display != crate::Display::None
        // A popover is out of flow, open or closed (`style::top_layer`).
        && crate::style::top_layer::layout_position(arena, c) != crate::PositionType::Absolute
        && arena.cover(c) != Some(HostCover::Whole)
}

/// Settle which absolutely positioned boxes start past a covered box's
/// bottom clearance, and by how much (`apply`): the children of each box of
/// a bottom-edge chain whose scroller the platform insets there
/// (`scroller_insets_bottom`), from the covered box down to that scroller,
/// as those boxes reach under the bar with it. It runs after each batch and
/// each cover change, and derives again each box whose distance changed.
/// `apply` reads the settled distance rather than walking the chain, so a
/// box restyled partway through a batch is corrected here, and one that
/// leaves the chain, however it left, gives the distance back.
pub(crate) fn settle_chain<'a>(
    arena: &mut NodeArena,
    layout: Option<&mut (dyn LayoutMirror + 'a)>,
) {
    let mut now = Vec::new();
    for &(covered, cover) in &arena.covers.boxes {
        let HostCover::Edges([_, _, bottom, _]) = cover else {
            continue;
        };
        if bottom <= 0.0 || !arena.is_live(covered) || !scroller_insets_bottom(arena, covered) {
            continue;
        }
        let mut at = last_flowing(arena, covered);
        let on = |&c: &u32| {
            !matches!(
                arena.cover(c),
                Some(HostCover::Inset(_) | HostCover::Edges(_))
            ) && !scrolls(arena, c)
        };
        while let Some(chain) = at.filter(on) {
            now.extend(
                positioned_children(arena, chain)
                    .into_iter()
                    .map(|c| (c, bottom)),
            );
            at = last_flowing(arena, chain);
        }
    }
    if now == arena.covers.chain {
        return;
    }
    let before = std::mem::replace(&mut arena.covers.chain, now);
    let Some(layout) = layout else {
        return;
    };
    let arena = &*arena;
    let changed = |&&(slot, past): &&(u32, f32)| arena.covers.chain_past(slot) != past;
    let gained = |&&(slot, _): &&(u32, f32)| before.iter().all(|(s, _)| *s != slot);
    let now = &arena.covers.chain;
    for &(slot, _) in before
        .iter()
        .filter(changed)
        .chain(now.iter().filter(gained))
    {
        if let Some(node) = arena.is_live(slot).then(|| arena.taffy(slot)).flatten() {
            layout.restyle(arena, slot, node);
            layout.mark_dirty(node);
        }
    }
}

/// How far a `scroll` reaches past the sides of the box a container's bars
/// cover (`Edges`), right and left: that box's
/// side cover, on each side where the scroller lies on the box's content box
/// (LLP 1123 D3, "A scroller reaches an edge"). It lies there when it and
/// every box between them fill their parents across, in flow and with no
/// width or maximum width, and have no margin on that side, and the boxes above it no
/// padding or border there, the covered box included. The scroller is laid
/// out that much wider, and its content that much further in (`apply`), so
/// the content keeps its place while the scroller's box, and the scroll
/// indicator UIKit draws at its edge, run under the side controls (the
/// iPhone Duo's side strip), as a hand-built screen's scroll view does. A
/// list is left out: the platform's own list lays its rows out, and a
/// covered box's own scroller is it.
pub(crate) fn side_reach(arena: &NodeArena, slot: u32) -> [f32; 2] {
    use crate::style::Dimension::{Auto, Points};
    if arena.node_type(slot) != crate::NodeType::ScrollView
        || matches!(arena.cover(slot), Some(HostCover::Edges(_)))
    {
        return [0.0; 2];
    }
    let (mut right, mut left) = (true, true);
    let mut at = slot;
    while let Some(p) = arena.parent(at) {
        let (s, parent) = (arena.style(at), arena.style(p));
        let fills = match parent.display {
            crate::Display::Block => true,
            crate::Display::Flex => {
                matches!(
                    parent.flex_direction,
                    FlexDirection::Column | FlexDirection::ColumnReverse
                ) && match s.align_self {
                    AlignSelf::Auto => {
                        matches!(parent.align_items, AlignItems::Normal | AlignItems::Stretch)
                    }
                    AlignSelf::Stretch => true,
                    _ => false,
                }
            }
            _ => false,
        };
        if !fills || positioned(arena, at) || s.width != Auto || s.max_width != Auto {
            return [0.0; 2];
        }
        let [_, border_right, _, border_left] = parent.border_widths();
        right &= s.margin_right == Points(0.0)
            && parent.padding_right == Points(0.0)
            && border_right == 0.0;
        left &= s.margin_left == Points(0.0)
            && parent.padding_left == Points(0.0)
            && border_left == 0.0;
        if matches!(arena.cover(p), Some(HostCover::Edges(_))) {
            let [_, cover_right, _, cover_left] = added_padding(arena, p);
            return [
                if right { cover_right } else { 0.0 },
                if left { cover_left } else { 0.0 },
            ];
        }
        if scrolls(arena, p) {
            return [0.0; 2];
        }
        at = p;
    }
    [0.0; 2]
}

/// The scrollers that may reach past `slot`'s sides when it is covered
/// (`side_reach`): the first `scroll` on each path down from it, which
/// `apply` derives again when the cover changes.
fn reaching(arena: &NodeArena, slot: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack: Vec<u32> = arena.children(slot).to_vec();
    while let Some(c) = stack.pop() {
        if positioned(arena, c) {
            continue;
        }
        if arena.node_type(c) == crate::NodeType::ScrollView {
            out.push(c);
        } else if !scrolls(arena, c) {
            stack.extend(arena.children(c).iter().copied());
        }
    }
    out
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

/// `slot`'s style changed (`txn::style_changed`, a layout row): its
/// `display`, `position` or `overflow-y` decide its place on the bottom
/// edge's chain or first in flow at the top, and its fill, width, margins,
/// padding and borders how far a scroller reaches past a covered box's
/// sides (`side_reach`).
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
    if [
        StyleId::Display,
        StyleId::FlexDirection,
        StyleId::AlignItems,
        StyleId::AlignSelf,
        StyleId::PositionType,
        StyleId::Width,
        StyleId::MaxWidth,
        StyleId::MarginLeft,
        StyleId::MarginRight,
        StyleId::PaddingLeft,
        StyleId::PaddingRight,
        StyleId::BorderWidthLeft,
        StyleId::BorderWidthRight,
        StyleId::BorderStyleLeft,
        StyleId::BorderStyleRight,
        StyleId::OverflowX,
        StyleId::OverflowY,
    ]
    .iter()
    .any(|&id| mask.has(id))
    {
        reach_changed(arena, layout, slot);
    }
}

/// `slot`'s place on the bottom edge's chain, or first in flow at the top,
/// may have changed (its `display`, `position` or `overflow-y`): the covered
/// box above it is derived again (`scroller_insets_bottom`,
/// `scroller_insets_top`).
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
/// `added_padding`, but nothing at the top or the bottom where the scroller
/// at that edge takes the bar's clearance as its platform inset (LLP
/// 1116.000 D1.2), so it reaches under the bar. An absolutely positioned
/// child is still placed past the whole cover (`added_padding`).
pub(crate) fn applied_padding(arena: &NodeArena, slot: u32) -> [f32; 4] {
    let [top, right, bottom, left] = added_padding(arena, slot);
    if !matches!(arena.cover(slot), Some(HostCover::Edges(_))) {
        return [top, right, bottom, left];
    }
    let top = if scroller_insets_top(arena, slot) {
        0.0
    } else {
        top
    };
    let bottom = if scroller_insets_bottom(arena, slot) {
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
        Some(HostCover::Edges(edges)) => edges,
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
        let (style, env) = (arena.effective_style(slot), arena.env_for(slot));
        let spaced = style.padding_bottom.spaced(env, crate::style::cells::ROW);
        s.padding.bottom = spaced.floor_lp(env, 0.0);
    }
}

/// A box's children changed: a covered box pads its top and bottom edges by
/// what the scrollers there leave (`scroller_insets_top`,
/// `scroller_insets_bottom`), so its engine style is derived again.
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
        reach_changed(arena, layout, slot);
    }
    crate::rows::children_changed(arena, layout, slot, touched);
}

/// A box at or under a covered one changed what decides how far a scroller
/// reaches past the covered box's sides (`side_reach`): its fill, width,
/// margins, padding, borders or children. The scrollers that may reach
/// (`reaching`) are derived again.
fn reach_changed(arena: &NodeArena, layout: &mut dyn LayoutMirror, slot: u32) {
    let mut at = Some(slot);
    while let Some(s) = at {
        if matches!(arena.cover(s), Some(HostCover::Edges(_))) {
            for r in reaching(arena, s) {
                if let Some(node) = arena.taffy(r) {
                    layout.restyle(arena, r, node);
                    layout.mark_dirty(node);
                }
            }
            return;
        }
        at = arena.parent(s);
    }
}
