//! Host measurements enter layout without changing authored rows.

use super::Kernel;
use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::generated::PropId;
use crate::id::ViewId;
use crate::NodeType;
use taffy::prelude::length;

impl Kernel {
    /// Host intrinsic size (`None` to forget it). Replaced elements keep
    /// their natural ratio; a projected tablist or radio group (a segmented
    /// control, LLP 1116.004 D5.2) uses the height as its automatic
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
        let tablist = matches!(
            self.arena
                .props(slot)
                .get(PropId::AccessibilityRole)
                .and_then(|v| v.as_str()),
            Some("tablist" | "radiogroup")
        );
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
        if self.measures_symbol(slot) {
            return Ok(());
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

    /// Whether a symbol's natural size is the one the kernel measured: for
    /// its role and font, under the traits its measurer measures under now.
    /// A host's report for it is not its size.
    fn measures_symbol(&self, slot: u32) -> bool {
        self.arena.measured_symbols.contains(slot)
            && self.symbol_revision == Some(self.measurer.measure_revision())
    }

    /// Measure the symbols owed a measurement. A system symbol's natural size
    /// depends only on its role, its font and the traits its host measures
    /// under, so the kernel asks the host's measurer for it before laying out
    /// rather than wait for a view to report it: the first layout has its box
    /// and its ratio. A host's report does not replace a measured size while
    /// those inputs stand. A symbol the measurer cannot size has the size its
    /// host reports.
    pub(crate) fn measure_symbols(&mut self) {
        // Ask about every symbol again, one the measurer declined too, under a new measurer or new traits (a content size, a scale)
        let revision = Some(self.measurer.measure_revision());
        if revision != self.symbol_revision {
            self.symbol_revision = revision;
            self.arena.measured_symbols.clear();
            let symbols: Vec<u32> = (self.arena.iter_live())
                .filter(|&slot| crate::replaced::symbol_role(&self.arena, slot).is_some())
                .collect();
            for slot in symbols {
                self.arena.symbols_owed.insert(slot);
            }
        }
        if self.arena.symbols_owed.is_empty() {
            return;
        }
        let owed: Vec<u32> = self.arena.symbols_owed.iter().collect();
        self.arena.symbols_owed.clear();
        for slot in owed {
            let measured = crate::replaced::symbol_role(&self.arena, slot)
                .and_then(|role| {
                    let font = self.arena.computed_style(slot, crate::StyleMask::TEXT);
                    self.measurer.symbol_size(&crate::SymbolRequest {
                        role,
                        font_size: font.font_size,
                        font_weight: font.font_weight,
                    })
                })
                .filter(|(w, h)| w.is_finite() && h.is_finite() && *w >= 0.0 && *h >= 0.0);
            let Some(size) = measured else {
                // The host's report stands, and the style already has its ratio; a region relays the change
                if let Some(r) = &mut self.region {
                    r.intrinsic(slot);
                }
                continue;
            };
            self.arena.measured_symbols.insert(slot);
            if self.arena.intrinsic(slot) == Some(size) {
                continue;
            }
            self.arena.set_intrinsic(slot, Some(size));
            if let Some(r) = &mut self.region {
                r.intrinsic(slot);
            }
            if let (Some(node), Some(layout)) = (self.arena.taffy(slot), self.layout.as_deref_mut())
            {
                layout.restyle(&self.arena, slot, node);
                layout.mark_dirty(node);
            }
        }
    }
}

/// A projected tablist's or radio group's box. A native tab bar fills the
/// tablist's box, a segmented control its content box. Its measured height
/// supplies the automatic minimum, so even a short authored row reserves the
/// control. Its width is the narrowest the control can be, the automatic
/// minimum width in place of the hidden tabs' own (LLP 1116.004 D2): UIKit's
/// fits its segments to any box, so a grouped row's tablist shrinks to the
/// row; AppKit's keeps its natural width. An explicit CSS min-height or
/// min-width still owns that constraint; no natural ratio or preferred width
/// is inferred from this container measurement. A radio group is never a
/// tab bar but always the segmented control (LLP 1116.004 D5.2), whose height
/// is its own: its automatic height is the measured one, so the hidden
/// radios, whose text follows Dynamic Type where the control's does not, no
/// longer make it taller (LLP 1116.019 D1).
pub(crate) fn apply_projection(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    if arena.node_type(slot).is_replaced()
        || matches!(
            arena.node_type(slot),
            NodeType::Control | NodeType::NativeView
        )
    {
        return;
    }
    let Some((width, height)) = arena.intrinsic(slot) else {
        return;
    };
    if s.min_size.height.is_auto() {
        s.min_size.height = length(height);
    }
    let radios = arena
        .props(slot)
        .get(PropId::AccessibilityRole)
        .and_then(|v| v.as_str())
        == Some("radiogroup");
    if radios && s.size.height.is_auto() {
        s.size.height = length(height);
    }
    if s.min_size.width.is_auto() {
        s.min_size.width = length(width);
    }
}
