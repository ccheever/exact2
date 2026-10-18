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

    /// `exact_boot_entry`: boot a snapshot entry's plan (LLP 1126 D2) from the
    /// input buffer, the plan's `len` bytes followed by its kind's `kind_len`
    /// bytes. The headless session's boot: a running host refuses the plan.
    pub fn boot_entry(
        &mut self,
        len: usize,
        kind_len: usize,
        data: D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> u32 {
        let end = (len + kind_len).min(self.input.len());
        let kind = match std::str::from_utf8(&self.input[len.min(end)..end]) {
            Ok(kind) => kind.to_string(),
            Err(_) => return self.refuse_preparation("entry kind is not UTF-8"),
        };
        let len =
            self.prepare_plan_carried(len, data, hooks, width, height, None, None, Some(&kind));
        if self.prepared.is_some() {
            self.commit_plan()
        } else {
            len
        }
    }
}
