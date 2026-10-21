//! Siblings' ranks for the walk (LLP 1083.000), kept across paints. The
//! kernel decides every node's place from its own facts and what its
//! subtree lets escape; a kept row that nothing changed since its recording
//! has the same subtree, so its potentials stand in for walking it (the
//! kernel's `paint_order_reusing`) and its descendants keep their ranks.
//! A fling's paint then walks the new rows and what holds them, not every
//! mounted row.
use super::*;

/// Incremental passes between full ones, which drop the ranks of nodes
/// that went.
const FULL_EVERY: u32 = 64;

impl Painter {
    /// The ranks for this paint's kernel epoch.
    pub(super) fn refresh_ranks(&mut self, kernel: &exact_kernel::Kernel) {
        let full = !self.rows.active() || self.rank_increments >= FULL_EVERY;
        let rows = &self.rows;
        let mut known = |view: ViewId| -> Option<exact_kernel::paint_order::Potentials> {
            if full {
                return None;
            }
            rows.order(kernel.node(view)?.key)
        };
        let answers = kernel.paint_order_reusing(&mut known);
        self.fresh_order.clear();
        let ranks = Rc::make_mut(&mut self.ranks);
        if full {
            ranks.clear();
            self.rank_increments = 0;
        } else {
            self.rank_increments += 1;
        }
        for (view, placed, potentials) in answers {
            // In flow (rank 0) is what a missing entry reads as: only the
            // few others are kept.
            if placed.rank != 0 {
                ranks.insert(view, placed.rank);
            } else {
                ranks.remove(&view);
            }
            self.fresh_order.insert(view, potentials);
        }
    }
}
