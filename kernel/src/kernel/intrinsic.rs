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
            // The same size again, perhaps now as another kind: withdrawn from
            // the kind it was reported as, and reported as the kind it is now.
            if let Some(size) = size {
                let now = crate::control::mark_key(&self.arena, slot);
                if self.arena.mark_reports.get(&slot).copied() != now {
                    self.mark_withdrawn(slot);
                    self.note_mark_report(slot, Some(size));
                    if now.is_some() {
                        self.mark_reported(slot, size);
                    }
                }
            }
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
        if size.is_none() {
            self.mark_withdrawn(slot);
        }
        self.note_mark_report(slot, size);
        if size.is_some_and(|size| self.mark_reported(slot, size)) {
            self.arena.set_intrinsic(slot, size);
            return Ok(());
        }
        self.arena.set_intrinsic(slot, size);
        self.intrinsic_changed(slot);
        Ok(())
    }

    /// A host reported `size` for `slot`. A mark with no size of its own yet
    /// that already shows its kind's last report keeps its layout. Otherwise
    /// the report is its kind's, and every mark still showing the kind's
    /// last one is laid out again with it. Whether `slot`'s layout stands.
    pub(crate) fn mark_reported(&mut self, slot: u32, size: (f32, f32)) -> bool {
        let Some(key) = crate::control::mark_key(&self.arena, slot) else {
            return false;
        };
        let shown = crate::control::shared_size(&self.arena, slot);
        if self.arena.intrinsic(slot).is_none() && shown == Some(size) {
            return true;
        }
        if shown == Some(size) {
            return false;
        }
        self.arena.mark_sizes.retain(|(k, _)| *k != key);
        self.arena.mark_sizes.push((key, size));
        let reading: Vec<u32> = (self.arena.iter_live())
            .filter(|&s| {
                s != slot
                    && self.arena.intrinsic(s).is_none()
                    && crate::control::mark_key(&self.arena, s) == Some(key)
            })
            .collect();
        for s in reading {
            self.intrinsic_changed(s);
        }
        false
    }

    /// The key a mark's report is made under, kept while it stands; none for
    /// a withdrawn report or one of a node that is no mark.
    fn note_mark_report(&mut self, slot: u32, size: Option<(f32, f32)>) {
        // (the key only for a report: a host's repeated withdrawals resolve no font size)
        match size.and_then(|_| crate::control::mark_key(&self.arena, slot)) {
            Some(key) => self.arena.mark_reports.insert(slot, key),
            None => self.arena.mark_reports.remove(&slot),
        };
    }

    /// A host withdrew `slot`'s report. When that report is the size its kind's
    /// marks share and no other mark of the kind still reports it, the host
    /// sizes them no more (Linux's Material and neutral looks leave a checkbox
    /// to the kernel, after a report it took back): the shared size goes with
    /// it, and the marks still reading it are laid out again at the kind's own.
    fn mark_withdrawn(&mut self, slot: u32) {
        // Judged by the key its report was made under (a switch that became a
        // checkbox withdraws a switch's report), not by what the node is now.
        let Some(&key) = self.arena.mark_reports.get(&slot) else {
            return;
        };
        let own = self.arena.intrinsic(slot);
        let shared = (self.arena.mark_sizes.iter())
            .find(|(k, _)| *k == key)
            .map(|&(_, size)| size);
        if own.is_none() || shared != own {
            return;
        }
        // Another mark's report under that key, of that size, still holds it
        // for the marks with none (two equal reports).
        let held = (self.arena.mark_reports.iter())
            .any(|(&s, &k)| s != slot && k == key && self.arena.intrinsic(s) == own);
        if held {
            return;
        }
        self.arena.mark_sizes.retain(|(k, _)| *k != key);
        let reading: Vec<u32> = (self.arena.iter_live())
            .filter(|&s| {
                s != slot
                    && self.arena.intrinsic(s).is_none()
                    && crate::control::mark_key(&self.arena, s) == Some(key)
            })
            .collect();
        for s in reading {
            self.intrinsic_changed(s);
        }
    }

    /// `slot`'s natural size changed: lay it out again.
    pub(crate) fn intrinsic_changed(&mut self, slot: u32) {
        // A native face's image has no layout node; its button owns measurement.
        let button = self
            .arena
            .parent(slot)
            .filter(|&p| self.arena.is_native_button(p))
            .and_then(|p| self.arena.taffy(p));
        if let (Some(button), Some(layout)) = (button, self.layout.as_deref_mut()) {
            layout.mark_dirty(button);
        }
        if let Some(r) = &mut self.region {
            r.intrinsic(slot);
        }
        if let (Some(node), Some(layout)) = (self.arena.taffy(slot), self.layout.as_deref_mut()) {
            layout.restyle(&self.arena, slot, node);
            layout.mark_dirty(node);
        }
        self.inline_image_resized(slot);
    }

    /// An image in a paragraph's text took a new natural size, or a size
    /// the environment resolves: its box is the paragraph's measurement
    /// (LLP 1011.002).
    pub(super) fn inline_image_resized(&mut self, slot: u32) {
        if !self.arena.is_inline_image(slot) {
            return;
        }
        let owner = self.arena.measure_owner(slot);
        self.arena.revise_text(owner, true);
        self.arena
            .flags_mut(owner)
            .insert(crate::NodeFlags::TEXT_DIRTY);
        if let (Some(node), Some(layout)) = (self.arena.taffy(owner), self.layout.as_deref_mut()) {
            layout.mark_dirty(node);
        }
    }

    /// Whether a symbol's natural size is the one the kernel measured: for
    /// its role and font, under the traits its measurer measures under now.
    /// A host's report for it is not its size.
    fn measures_symbol(&self, slot: u32) -> bool {
        self.arena.measured_symbols.contains(slot)
            && self.symbol_revision == Some(self.measurer.measure_revision())
    }

    /// Whether the next layout would size a symbol first (`measure_symbols`
    /// with `laid_out_only`): a new measurer or traits, or one owed outside
    /// a box covered whole.
    pub(crate) fn symbols_pending(&self) -> bool {
        Some(self.measurer.measure_revision()) != self.symbol_revision
            || (self.arena.symbols_owed.iter()).any(|slot| !covered_whole(&self.arena, slot))
    }

    /// Measure the symbols owed a measurement. A system symbol's natural size
    /// depends only on its role, its font and the traits its host measures
    /// under, so the kernel asks the host's measurer for it before laying out
    /// rather than wait for a view to report it: the first layout has its box
    /// and its ratio. A host's report does not replace a measured size while
    /// those inputs stand. A symbol the measurer cannot size has the size its
    /// host reports. With `laid_out_only`, a symbol in a box covered whole
    /// (`display: none`: a closed popover or dialog, LLP 1021 §5) stays owed
    /// until a layout places it, so one a UIKit menu shows is never measured.
    pub(crate) fn measure_symbols(&mut self, laid_out_only: bool) {
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
        for slot in owed {
            if laid_out_only && covered_whole(&self.arena, slot) {
                continue;
            }
            self.arena.symbols_owed.remove(slot);
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
            self.inline_image_resized(slot);
        }
    }
}

/// Whether `slot` or a box above it is covered whole, so not laid out.
fn covered_whole(arena: &NodeArena, slot: u32) -> bool {
    std::iter::successors(Some(slot), |&s| arena.parent(s))
        .any(|s| arena.cover(s) == Some(super::HostCover::Whole))
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
