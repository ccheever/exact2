//! What a native container covers of a box (LLP 1075.003 §3.5): the iOS
//! navigation and tab containers report their bars over a route, and the
//! header a bar replaces; one layout for all of a turn's reports. And the
//! containing block a native container gives a subtree it shows in a view
//! of its own (LLP 1115.002 D8.2): a split's pane, a sheet, and where it
//! shows that subtree's root when it does not lift it to the top layer (a
//! stack's route, LLP 1123 D2), which only the runner's geometry reads.
//! And the margins UIKit's own list gives a grouped list's cards and cells.

use super::Host;
use exact_kernel::{GroupedMargins, HostBlock, HostCover, ViewId};
use exact_runner::DataSource;

impl<D: DataSource> Host<D> {
    /// Several boxes' covers (`None` clears one) under one layout. A refusal
    /// for one view (it is gone) leaves the others set and is the batch's
    /// error.
    pub fn set_covers(&mut self, covers: &[(ViewId, Option<HostCover>)]) -> String {
        self.set_claims(covers, &[], &[], &[], &[])
    }

    /// Several subtrees' containing blocks (`None` takes one back) under one
    /// layout: each root's frame comes back in its block's space, with the
    /// block's used size.
    pub fn set_blocks(&mut self, blocks: &[(ViewId, Option<HostBlock>)]) -> String {
        self.set_claims(&[], blocks, &[], &[], &[])
    }

    /// Covers, containing blocks and the blocks' placements under one
    /// layout, and the canvases' work after it: a canvas in a box given
    /// back is laid out and drawn in this batch.
    pub fn set_claims(
        &mut self,
        covers: &[(ViewId, Option<HostCover>)],
        blocks: &[(ViewId, Option<HostBlock>)],
        placements: &[(ViewId, Option<exact_runner::BlockPlacement>)],
        margins: &[(ViewId, Option<GroupedMargins>)],
        list_margins: &[(ViewId, Option<[f32; 2]>)],
    ) -> String {
        let needs_layout = !(covers.is_empty()
            && blocks.is_empty()
            && margins.is_empty()
            && list_margins.is_empty());
        if needs_layout {
            self.height_targets_dirty = true;
        }
        let mut batch = crate::batch::Batch::new();
        let mut error = None;
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
        if needs_layout {
            if let Err(e) = self.layout(&mut batch) {
                error.get_or_insert(e);
            }
            self.canvas_turn(&mut batch);
        }
        self.finish(batch, error)
    }
}
