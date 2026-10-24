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

    /// Scroll `id` by `dy` points, clamped to its travel; whether it moved.
    pub(crate) fn scroll_by(&mut self, id: ViewId, dy: f32) -> bool {
        self.restore_scroll_anchors();
        let kernel = self.host.kernel();
        let Some(node) = kernel.node(id) else {
            return false;
        };
        let bounds = self.display.bounds(kernel, id).unwrap_or_else(|| {
            let limits = self.collection_scroll_limits();
            self.brush.scroll_bounds(
                kernel,
                self.host.content_region(),
                &node,
                limits.get(&id).copied(),
            )
        });
        let off = self.scroll.get(&id).copied().unwrap_or((0.0, 0.0));
        let ny = (off.1 + dy).clamp(0.0, bounds.max.1.max(0.0));
        let moved = self.anchor_input(id, off, dy, (off.0, ny));
        if ny == off.1 && moved.is_none_or(|dy| dy == 0.) {
            return false;
        }
        self.scroll.insert(id, (off.0, ny));
        self.scroll_end_input(id, off, Completion::Instant);
        self.dirty = true;
        self.collection_scroll_input(id, off);
        if let Some(error) = self.refresh_transform_geometry() {
            self.host.log(error);
        }
        true
    }
}
