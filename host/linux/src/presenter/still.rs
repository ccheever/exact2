//! What a painted frame shows besides its scroll offsets: when nothing of it
//! changed since the last paint and only scrollers moved, a host whose
//! backend keeps rows may move the kept drawing instead of painting again
//! (as a browser's compositor scrolls without the main thread). The paint it
//! owes follows soon after: boxes, hits and picture visibility are those of
//! the last paint until then.
use super::*;

/// Everything but scroll offsets that a paint reads, as far as a scroll
/// frame could change it.
#[derive(Clone, PartialEq)]
pub(crate) struct Still {
    epoch: u64,
    images: Vec<(ViewId, usize)>,
    focus: Option<ViewId>,
    pointer: Option<(u32, u32)>,
    menu: Option<ViewId>,
    page: (u32, u32),
    viewport: (u32, u32),
    dark: bool,
    controls: BTreeMap<ViewId, bool>,
}

impl<D: DataSource> Presenter<D> {
    /// The paint's inputs other than scroll offsets; `None` while anything
    /// moves on its own (motion, a press, a collection's pending correction).
    pub(crate) fn still(&self) -> Option<Still> {
        if self.needs_animation_frame()
            || self.host.wants_frames()
            || self.host.pressing()
            || self.collection_paint_scroll().is_some()
            || self.host.content_region().is_some()
            || !self.brush.canvases.is_empty()
            || !self.brush.placements.is_empty()
        {
            return None;
        }
        let bits = |(a, b): (f32, f32)| (a.to_bits(), b.to_bits());
        Some(Still {
            epoch: self.host.kernel().epoch(),
            images: self
                .images
                .bitmaps
                .iter()
                .map(|(id, b)| (*id, Arc::as_ptr(b) as usize))
                .collect(),
            focus: self.focus,
            pointer: self.pointer.map(bits),
            menu: self.menu,
            page: bits(self.page),
            viewport: bits(self.viewport),
            dark: self.brush.dark,
            controls: self.controls.clone(),
        })
    }

    /// The scroll offsets a paint now would use.
    pub(crate) fn scroll_offsets(&self) -> &BTreeMap<ViewId, (f32, f32)> {
        &self.scroll
    }

    /// The host moved the last paint instead of painting: nothing is owed
    /// until something else changes or the host paints anyway.
    pub(crate) fn moved_without_paint(&mut self) {
        self.dirty = false;
    }
}
