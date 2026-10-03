//! Host measurements enter layout without changing authored rows.

use super::Kernel;
use crate::error::{KernelError, LayoutError};
use crate::generated::PropId;
use crate::id::ViewId;

impl Kernel {
    /// The host intrinsic size a node holds, if any.
    pub fn intrinsic_size(&self, view: ViewId) -> Option<(f32, f32)> {
        self.arena
            .slot_of(view)
            .and_then(|slot| self.arena.intrinsic(slot))
    }

    /// Host intrinsic size (`None` to forget it). Replaced elements keep
    /// their natural ratio; a projected tablist uses the height as its automatic
    /// minimum (LLP 1059); a form control takes it as its size, with no ratio
    /// (LLP 1069.001 D3), as does a native module (LLP 1024 D4). Other nodes and nonpositive/nonfinite sizes are refused.
    pub fn set_intrinsic_size(
        &mut self,
        view: ViewId,
        size: Option<(f32, f32)>,
    ) -> Result<(), KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        if self.arena.intrinsic(slot) == size {
            return Ok(());
        }
        let tablist = self
            .arena
            .props(slot)
            .get(PropId::AccessibilityRole)
            .and_then(|v| v.as_str())
            == Some("tablist");
        let clearing_projection = size.is_none() && self.arena.intrinsic(slot).is_some();
        let control = matches!(
            self.arena.node_type(slot),
            crate::NodeType::Control | crate::NodeType::NativeView
        );
        if !self.arena.node_type(slot).is_replaced() && !control && !tablist && !clearing_projection
        {
            return Err(LayoutError::NotAnImage(view).into());
        }
        if let Some((w, h)) = size {
            if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
                return Err(LayoutError::InvalidIntrinsicSize(view).into());
            }
        }
        self.arena.set_intrinsic(slot, size);
        if let Some(r) = &mut self.region {
            r.intrinsic(slot);
        }
        if let (Some(node), Some(layout)) = (self.arena.taffy(slot), self.layout.as_deref_mut()) {
            layout.restyle(&self.arena, slot, node);
            layout.mark_dirty(node);
        }
        Ok(())
    }
}
