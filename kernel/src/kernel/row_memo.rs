//! A list's rows laid out by replaying rows like them (Taffy patch 29).

use super::Kernel;
use crate::layout::LayoutMirror;

impl Kernel {
    /// Lay out each list row by replaying a row like it where one was laid
    /// out before (the default), or by its own algorithm every time. The
    /// frames are the same either way; a host turns it off to compare.
    pub fn set_row_layout_memo(&mut self, on: bool) {
        if let Some(tree) = self.layout.as_deref_mut().and_then(LayoutMirror::tree) {
            tree.set_row_memo(on);
        }
    }

    /// List rows laid out by a replay, and those computed and recorded,
    /// since the engine tree was made.
    pub fn row_layout_memo_counts(&self) -> (usize, usize) {
        self.layout
            .as_deref()
            .and_then(LayoutMirror::tree_ref)
            .map_or((0, 0), |tree| tree.row_memo_counts())
    }
}
