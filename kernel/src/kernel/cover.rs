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
//! A replaced header that cleared the status bar itself (the web's
//! `padding-top: env(safe-area-inset-top)` under `viewport-fit=cover`)
//! hands that inset to its route's top cover, so the route's content starts
//! at the bar's bottom whichever of the two padded it (LLP 1075.003 §9.10).

use super::Kernel;
use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::id::ViewId;
use crate::style::{Dimension, Edge};

/// A host container's claim on one box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HostCover {
    /// Points the container's bars cover at the top, right, bottom and left
    /// edges, added to the box's padding.
    Edges([f32; 4]),
    /// Replaced by a native control: the box takes no space.
    Whole,
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
        if let Some(HostCover::Edges(edges)) = cover {
            if !edges.iter().all(|e| e.is_finite() && *e >= 0.0) {
                return Err(LayoutError::InvalidIntrinsicSize(view).into());
            }
        }
        let cover = cover.filter(|c| *c != HostCover::Edges([0.0; 4]));
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

/// The top inset a box's replaced first child cleared for itself: that
/// child is covered whole and pads its top by `env(safe-area-inset-top)`.
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
        _ => 0.0,
    }
}
