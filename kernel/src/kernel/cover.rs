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

use super::Kernel;
use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::id::ViewId;
use crate::layout::LayoutMirror;
use crate::style::{Dimension, Edge};

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
}

impl Kernel {
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
            Some(HostCover::Inset(e)) => e,
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
        // takes from it (`header_inset`).
        let whole = |c: Option<HostCover>| c == Some(HostCover::Whole);
        let parent = (whole(before) != whole(cover))
            .then(|| self.arena.parent(slot))
            .flatten();
        for slot in std::iter::once(slot).chain(parent) {
            if let (Some(node), Some(layout)) = (self.arena.taffy(slot), self.layout.as_deref_mut())
            {
                layout.restyle(&self.arena, slot, node);
                layout.mark_dirty(node);
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
    let (style, env) = (arena.style(slot), arena.env());
    match arena.cover(slot) {
        Some(HostCover::Whole) => s.display = taffy::style::Display::None,
        Some(HostCover::Edges([top, right, bottom, left])) => {
            let top = top + header_inset(arena, slot, top);
            s.padding = taffy::geometry::Rect {
                top: style.padding_top.plus(env, top).to_lp(env),
                right: style.padding_right.plus(env, right).to_lp(env),
                bottom: style.padding_bottom.plus(env, bottom).to_lp(env),
                left: style.padding_left.plus(env, left).to_lp(env),
            };
        }
        Some(HostCover::Inset([top, bottom])) => {
            s.padding.bottom = style.padding_bottom.plus(env, top + bottom).to_lp(env);
        }
        None => {}
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
        let (style, env) = (arena.style(slot), arena.env());
        s.padding.bottom = style.padding_bottom.plus(env, 0.0).to_lp(env);
    }
}

/// A box's children changed: a covered box's top cover may hold its first
/// child's inset (`header_inset`), so its engine style is derived again.
pub(crate) fn children_changed(arena: &NodeArena, layout: &mut dyn LayoutMirror, slot: u32) {
    if let (Some(HostCover::Edges(_)), Some(node)) = (arena.cover(slot), arena.taffy(slot)) {
        layout.restyle(arena, slot, node);
        layout.mark_dirty(node);
    }
}

/// The top inset a box's replaced first child cleared for itself: that
/// child is covered whole and pads its top by `env(safe-area-inset-top)`
/// (or a comparison that reads it, LLP 1001 §2).
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
        (true, Some(HostCover::Whole), Dimension::Env(Edge::Top, _)) => {
            arena.env().inset(Edge::Top)
        }
        // `max(env(safe-area-inset-top), 12px)` clears the inset too.
        (true, Some(HostCover::Whole), Dimension::Compare(c)) if c.reads(Edge::Top) => {
            arena.env().inset(Edge::Top)
        }
        _ => 0.0,
    }
}
