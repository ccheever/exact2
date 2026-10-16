//! A host-rendered alert title's generated layout item.
//!
//! Neutral's title is CSS generated content: it participates in the alert's
//! flex layout and scrolling, but is no authored node, wire id or accessibility
//! node. The arena keeps only its presence; layout derives text and metrics
//! from the owning dialog on every refresh.

use super::Kernel;
use crate::error::{KernelError, LayoutError};
use crate::id::{Frame, ViewId};
use crate::layout::LayoutMirror;

impl Kernel {
    /// Set whether `view` has the host-rendered alert title item. Reasserting
    /// presence refreshes its measurement after label or inherited-text edits.
    pub fn set_alert_prefix(&mut self, view: ViewId, present: bool) -> Result<(), KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        self.arena.set_alert_prefix(slot, present);
        if let (Some(owner), Some(tree)) = (
            self.arena.taffy(slot),
            self.layout.as_deref_mut().and_then(LayoutMirror::tree),
        ) {
            tree.sync_alert_prefix(&self.arena, slot, owner);
        }
        Ok(())
    }

    /// The generated title item's live layout frame in the dialog block.
    pub fn alert_prefix_frame(&self, view: ViewId) -> Option<Frame> {
        let slot = self.arena.slot_of(view)?;
        self.layout
            .as_deref()
            .and_then(LayoutMirror::tree_ref)?
            .alert_prefix_frame(&self.arena, slot)
    }
}
