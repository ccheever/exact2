//! What a native container covers of a box (LLP 1075.003 §3.5): the iOS
//! navigation and tab containers report their bars over a route, and the
//! header a bar replaces; one layout for all of a turn's reports. And the
//! containing block a native container gives a subtree it shows in a view
//! of its own (LLP 1115.002 D8.2): a split's pane, a sheet, and where it
//! shows that subtree's root when it does not lift it to the top layer (a
//! stack's route, LLP 1123 D2), which only the runner's geometry reads.
//! And the margins UIKit's own list gives a grouped list's cards and cells,
//! which split placeholders a pane shows (LLP 1116.006 D2), and a content
//! owner's values (LLP 1123 D9).

use super::Host;
use exact_kernel::{
    BlockMeasure, BlockShown, GroupedMargins, HostBlock, HostCover, OwnerValues, ViewId,
};
use exact_runner::DataSource;

/// What a host claims of boxes in one report, each `None` taking one back.
#[derive(Default)]
pub struct Claims<'a> {
    /// What a native container covers of a box.
    pub covers: &'a [(ViewId, Option<HostCover>)],
    /// The containing block a container gives a subtree.
    pub blocks: &'a [(ViewId, Option<HostBlock>)],
    /// Where a block's root is shown.
    pub placements: &'a [(ViewId, Option<exact_runner::BlockPlacement>)],
    /// A grouped list's card and cell margins.
    pub margins: &'a [(ViewId, Option<GroupedMargins>)],
    /// A cells list's rows' cell margin.
    pub list_margins: &'a [(ViewId, Option<[f32; 2]>)],
    /// Whether a split's own placeholder shows (LLP 1116.006 D2).
    pub shows: &'a [(ViewId, bool)],
    /// A content owner's safe area, regions, segments and size.
    pub owners: &'a [(ViewId, Option<OwnerValues>)],
    /// How a block is shown against its stack's top (a split's columns).
    pub shown: &'a [(ViewId, BlockShown)],
}

impl<D: DataSource> Host<D> {
    /// Several boxes' covers (`None` clears one) under one layout. A refusal
    /// for one view (it is gone) leaves the others set and is the batch's
    /// error.
    pub fn set_covers(&mut self, covers: &[(ViewId, Option<HostCover>)]) -> String {
        self.set_claims(&Claims {
            covers,
            ..Claims::default()
        })
    }

    /// What UIKit covers of list `view`'s port at its start and end
    /// (`Runner::set_list_covered`). Nothing is laid out or sent.
    pub fn set_list_covered(&mut self, view: ViewId, covered: [f64; 2]) {
        self.runner.set_list_covered(view, covered);
    }

    /// The kernel's layout stamp folded to one number (`Kernel::layout_stamp`):
    /// a measure kept while it stands is the one the kernel would answer.
    pub fn layout_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.runner.kernel().layout_stamp().hash(&mut h);
        h.finish()
    }

    /// What `view`'s subtree would measure in `block` (`Kernel::measure_block`):
    /// no layout of the page, no batch; `None` when refused.
    pub fn measure_block(&mut self, view: ViewId, block: HostBlock) -> Option<BlockMeasure> {
        self.runner.count_kernel_layout();
        self.runner.kernel_mut().measure_block(view, block).ok()
    }

    /// Several subtrees' containing blocks (`None` takes one back) under one
    /// layout: each root's frame comes back in its block's space, with the
    /// block's used size.
    pub fn set_blocks(&mut self, blocks: &[(ViewId, Option<HostBlock>)]) -> String {
        self.set_claims(&Claims {
            blocks,
            ..Claims::default()
        })
    }

    /// The views at or under `root` whose style reads a viewport or
    /// environment length, an owner's value included.
    fn env_readers(&self, root: ViewId) -> Vec<ViewId> {
        let kernel = self.runner.kernel();
        let (mut out, mut stack) = (Vec::new(), vec![root]);
        while let Some(id) = stack.pop() {
            let Some(node) = kernel.node(id) else {
                continue;
            };
            if exact_kernel::uses_env(node.style) {
                out.push(id);
            }
            stack.extend(node.children());
        }
        out
    }

    /// Every claim under one layout, and the canvases' work after it: a
    /// canvas in a box given back is laid out and drawn in this batch.
    pub fn set_claims(&mut self, claims: &Claims<'_>) -> String {
        let Claims {
            covers,
            blocks,
            placements,
            margins,
            list_margins,
            shows,
            owners,
            shown,
        } = *claims;
        let (mut error, mut read) = (None, false);
        let mut batch = crate::batch::Batch::new();
        // An owner's values lay out again only where a row reads them, and
        // those rows' native styles are sent again as the owner resolves them.
        for (view, values) in owners {
            match self
                .runner
                .kernel_mut()
                .set_owner_values(*view, values.clone())
            {
                Ok(true) => {
                    read = true;
                    for id in self.env_readers(*view) {
                        self.update(id, &mut batch);
                    }
                }
                Ok(false) => {}
                Err(e) => drop(error.get_or_insert(format!("owner values: {e:?}"))),
            }
        }
        // A run that reads them has no view: its paragraph is sent again
        if read {
            self.emit_paragraphs(&mut batch);
        }
        let needs_layout = read
            || !(covers.is_empty()
                && blocks.is_empty()
                && margins.is_empty()
                && list_margins.is_empty());
        if needs_layout {
            self.height_targets_dirty = true;
        }
        // A placeholder's style changes as a commit does, so the presenter
        // hears its rows; it is laid out with the claims below, once.
        let mut receipts = Vec::new();
        for &(view, shown) in shows {
            match self.runner.show_split_placeholder(view, shown) {
                Ok(Some(receipt)) => receipts.push(exact_runner::Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Ok(None) => {}
                Err(e) => drop(error.get_or_insert(format!("placeholder: {e:?}"))),
            }
        }
        for (view, cover) in self.top_layer_claims(covers) {
            if let Err(e) = self.runner.kernel_mut().set_host_cover(view, cover) {
                error.get_or_insert(format!("cover: {e:?}"));
            }
        }
        for &(view, block) in blocks {
            match self.runner.kernel_mut().set_host_block(view, block) {
                // Its frame is in another space now: sent whatever it is.
                Ok(()) => self.queue_layout(view),
                Err(e) => drop(error.get_or_insert(format!("block: {e:?}"))),
            }
        }
        self.runner.place_blocks(placements);
        // Liveness alone: the runner reads it, layout does not.
        for &(view, how) in shown {
            self.runner.kernel_mut().set_host_block_shown(view, how);
        }
        for &(view, list) in margins {
            if let Err(e) = self.runner.kernel_mut().set_grouped_margins(view, list) {
                error.get_or_insert(format!("grouped margins: {e:?}"));
            }
        }
        // @ref LLP 1138 — a cells list's rows, the platform's cell margin.
        for &(view, list) in list_margins {
            if let Err(e) = self.runner.kernel_mut().set_list_margins(view, list) {
                error.get_or_insert(format!("list margins: {e:?}"));
            }
        }
        if !receipts.is_empty() {
            return self.commit_into(&receipts, error, batch);
        }
        if needs_layout {
            if let Err(e) = self.layout(&mut batch) {
                error.get_or_insert(e);
            }
            self.canvas_turn(&mut batch);
        }
        self.finish(batch, error)
    }
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod owner_tests;
