//! What a native container covers of a box (LLP 1075.003 §3.5): the iOS
//! navigation and tab containers report their bars over a route, and the
//! header a bar replaces; one layout for all of a turn's reports.

use super::Host;
use exact_kernel::{HostCover, ViewId};
use exact_runner::DataSource;

impl<D: DataSource> Host<D> {
    /// Several boxes' covers (`None` clears one) under one layout. A refusal
    /// for one view (it is gone) leaves the others set and is the batch's
    /// error.
    pub fn set_covers(&mut self, covers: &[(ViewId, Option<HostCover>)]) -> String {
        self.height_targets_dirty = true;
        let mut batch = crate::batch::Batch::new();
        let mut error = None;
        for &(view, cover) in covers {
            if let Err(e) = self.runner.kernel_mut().set_host_cover(view, cover) {
                error.get_or_insert(format!("cover: {e:?}"));
            }
        }
        if let Err(e) = self.layout(&mut batch) {
            error.get_or_insert(e);
        }
        self.finish(batch, error)
    }
}
