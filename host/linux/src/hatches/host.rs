//! The host's part of a hatch's overlay (@ref LLP 1075.003.000.001 §2.2.1):
//! its recording replayed whole by the replayer the 2D canvases use, into a
//! bitmap the painter composites over the node.
use super::Host;
use exact_runner::DataSource;
use std::sync::Arc;

impl<D: DataSource> Host<D> {
    /// `lists` replayed into a fresh bitmap of `size` points at `scale`;
    /// `None` for a box with no area. An image or a text run draws only
    /// where the app's canvases have brought their cache and engine.
    pub(crate) fn replay_overlay(
        &self,
        lists: &[Vec<u8>],
        size: (f32, f32),
        scale: f32,
    ) -> Result<Option<Arc<tiny_skia::Pixmap>>, String> {
        let env = crate::canvas2d::Env {
            images: &self.canvas2d.images,
            text: self.canvas2d.text.as_deref(),
        };
        crate::canvas2d::replay(lists, size, scale, &env)
    }
}
