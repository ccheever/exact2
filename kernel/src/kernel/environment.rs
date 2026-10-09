//! Host environment invalidation. @ref LLP 1104 D4.
use super::*;

impl Kernel {
    /// The page's environment: what `env(safe-area-inset-*)` and
    /// `env(viewport-segment-*)` lengths resolve to (LLP 1001 §2; LLP 1078 D3).
    pub fn env(&self) -> Env {
        self.arena.env().clone()
    }

    /// Layouts published by the kernel using any provisional native field or button measure.
    /// Hosts count presented batches separately; relayouts can remain silent.
    pub fn provisional_layouts(&self) -> u64 {
        self.provisional_layouts
    }

    /// Set the insets and control text styles of the environment — the safe-area insets the host
    /// reports with the viewport (a rotation changes them); the segment grid
    /// `env` carries is ignored, [`Kernel::set_segments`] being its twin.
    /// Every node whose style holds an `env()` length gets its engine style
    /// re-derived and is marked dirty; returns whether any did (a layout is
    /// owed then). Changed control fonts immediately re-resolve `em` rows and
    /// invalidate the controls that read them. Non-finite/nonpositive font sizes
    /// and non-finite insets are refused. A `reset` keeps the
    /// environment: it is the host's.
    pub fn set_env(&mut self, env: Env) -> Result<bool, KernelError> {
        if !env.is_finite() {
            return Err(LayoutError::InvalidEnv.into());
        }
        let next = self
            .arena
            .env()
            .with_insets(env.top, env.right, env.bottom, env.left);
        let next = Env {
            control_text_styles: env.control_text_styles,
            button_fonts: env.button_fonts,
            screen: env.screen,
            ..next
        };
        self.replace_env(next)
    }

    /// Set the viewport segments (LLP 1078 D3): `cols × rows` rects,
    /// row-major, in the layout viewport's points — none for one segment,
    /// where CSS defines no segment variable. Refuses a zero count, a count
    /// that is not `cols × rows` (any rect on a 1 × 1 grid), and a
    /// non-finite rect. Re-derives and dirties exactly the nodes whose style
    /// reads the environment, as `set_env` does, and says whether any did.
    pub fn set_segments(
        &mut self,
        cols: u8,
        rows: u8,
        segments: Vec<Rect>,
    ) -> Result<bool, KernelError> {
        let next = self.arena.env().with_segments(cols, rows, segments);
        if !next.segments_consistent() || !next.is_finite() {
            return Err(LayoutError::InvalidSegments.into());
        }
        self.replace_env(next)
    }

    /// Lay borders out as a terminal does: a drawn side is one cell (LLP
    /// 1101.001 P13). The terminal host sets it on its own kernel before
    /// the tree is built; it is this kernel's alone.
    pub fn set_cell_borders(&mut self, on: bool) {
        let next = self.arena.env().with_cell_borders(on);
        let _ = self.replace_env(next);
    }

    pub(super) fn replace_env(&mut self, env: Env) -> Result<bool, KernelError> {
        if *self.arena.env() == env {
            return Ok(false);
        }
        let control_changed = self.arena.env().control_text_styles != env.control_text_styles
            || self.arena.env().button_fonts != env.button_fonts;
        let before: Vec<_> = if control_changed {
            self.arena
                .iter_live()
                .filter(|&s| self.arena.is_native_text_control(s) || self.arena.is_native_button(s))
                .map(|s| {
                    (
                        s,
                        self.arena.computed_inherited(s),
                        self.arena.control_font(s).map(|f| f.family.clone()),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        self.arena.set_env(env);
        if let Some(r) = &mut self.region {
            r.invalidate();
        }
        let users: Vec<u32> = self
            .arena
            .iter_live()
            // A covered box too: its top cover can hold an inset (cover.rs).
            .filter(|s| uses_env(self.arena.style(*s)) || self.arena.cover(*s).is_some())
            .collect();
        for slot in &users {
            if let (Some(node), Some(layout)) =
                (self.arena.taffy(*slot), self.layout.as_deref_mut())
            {
                layout.restyle(&self.arena, *slot, node);
                layout.mark_dirty(node);
            }
            self.arena.flags_mut(*slot).insert(NodeFlags::STYLE_DIRTY);
        }
        let mut controls_dirty = false;
        let mut unmirrored = Unmirrored;
        let layout = self.layout.as_deref_mut().unwrap_or(&mut unmirrored);
        for (slot, old, old_family) in before {
            let changed = old.changed_mask(&self.arena.computed_inherited(slot));
            let family_changed = !self.arena.style(slot).mask.has(StyleId::FontFamily)
                && old_family.as_deref()
                    != self.arena.control_font(slot).map(|f| f.family.as_str());
            if !changed.is_empty() || family_changed {
                txn::control_env_changed(&mut self.arena, layout, slot);
                controls_dirty = true;
            }
        }
        let mut relative_receipt = CommitReceipt::default();
        if control_changed {
            txn::relative::resolve(
                &mut self.arena,
                layout,
                &mut Vec::new(),
                &mut relative_receipt,
            );
        }
        Ok(!users.is_empty() || controls_dirty || relative_receipt.layout_invalidated)
    }
}
