//! What a native container covers of a box (LLP 1075.003 §3.5): the iOS
//! navigation and tab containers report their bars over a route, and the
//! header a bar replaces; one layout for all of a turn's reports. And the
//! containing block a native container gives a subtree it shows in a view
//! of its own (LLP 1115.002 D8.2): a split's pane, a sheet.

use super::Host;
use exact_kernel::{HostBlock, HostCover, ViewId};
use exact_runner::DataSource;

impl<D: DataSource> Host<D> {
    /// Several boxes' covers (`None` clears one) under one layout. A refusal
    /// for one view (it is gone) leaves the others set and is the batch's
    /// error.
    pub fn set_covers(&mut self, covers: &[(ViewId, Option<HostCover>)]) -> String {
        self.set_claims(covers, &[])
    }

    /// Several subtrees' containing blocks (`None` takes one back) under one
    /// layout: each root's frame comes back in its block's space, with the
    /// block's used size.
    pub fn set_blocks(&mut self, blocks: &[(ViewId, Option<HostBlock>)]) -> String {
        self.set_claims(&[], blocks)
    }

    /// Covers and containing blocks under one layout.
    pub fn set_claims(
        &mut self,
        covers: &[(ViewId, Option<HostCover>)],
        blocks: &[(ViewId, Option<HostBlock>)],
    ) -> String {
        self.height_targets_dirty = true;
        let mut batch = crate::batch::Batch::new();
        let mut error = None;
        for &(view, cover) in covers {
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
        if let Err(e) = self.layout(&mut batch) {
            error.get_or_insert(e);
        }
        self.finish(batch, error)
    }
}
