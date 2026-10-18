//! One session's boots from the input buffer: prepare, then commit.

use super::{Bridge, Hooks};
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// Prepare and commit a single session's plan replacement.
    pub fn boot_plan(&mut self, len: usize, data: D, hooks: Hooks, width: f32, height: f32) -> u32 {
        let len = self.prepare_plan(len, data, hooks, width, height);
        if self.prepared.is_some() {
            self.commit_plan()
        } else {
            len
        }
    }
}
