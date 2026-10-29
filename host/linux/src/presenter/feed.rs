//! A scroller moved by id (the Canvas host's bench driver, LLP 1076): the
//! feed its first wheel took, moved directly after, as the other stacks'
//! drivers move their feed, not whatever is under the screen's centre (a
//! nested list there would take the wheel).
use super::*;

impl<D: DataSource> Presenter<D> {
    /// The scroller the last wheel moved.
    #[cfg(target_os = "android")]
    pub(crate) fn last_wheel(&self) -> Option<ViewId> {
        self.last_wheel
    }
}
