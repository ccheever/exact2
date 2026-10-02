//! CSS `background-clip: text` (LLP 1076 D6): the node's background colour
//! and gradients, painted over its border box into a CPU island, kept only
//! where its paragraph's glyphs cover, and placed under the glyphs by either
//! backend. The box itself painted no background (`background_shape`).

use super::{gradient, BoxPaint, Painter, Rect4};
use crate::text::{Paragraph, RunPaint};
use exact_kernel::{BackgroundClip, Kernel};
use std::sync::Arc;
use tiny_skia::Transform;

impl Painter {
    /// The background through `paragraph`'s glyphs at `origin`, when the
    /// node clips its background to its text; `rect` is its border box.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn text_clip(
        &mut self,
        node: &exact_kernel::NodeRef<'_>,
        kernel: &Kernel,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        rect: Rect4,
        ts: Transform,
    ) {
        if node.style.background_clip != BackgroundClip::Text || rect.2 <= 0.0 || rect.3 <= 0.0 {
            return;
        }
        let paint = BoxPaint::capture(node, kernel, self.dark, rect.2);
        let geometry = paint.geometry(rect);
        let Some(mut fill) = self.island(rect, |p, t| {
            if paint.background[3] > 0 {
                p.backend.fill(&geometry.outer, paint.background, t);
            }
            for g in paint.gradients.iter().rev() {
                gradient::paint(
                    g,
                    &geometry.outer,
                    &geometry.outer,
                    paint.widths,
                    &mut *p.backend,
                    t,
                );
            }
        }) else {
            return;
        };
        let opaque: Vec<RunPaint> = palette
            .iter()
            .map(|r| RunPaint {
                color: [255; 4],
                source: r.source,
            })
            .collect();
        let Some(glyphs) = self.island(rect, |p, t| {
            let text = p.text.clone();
            let mut engine = text.borrow_mut();
            p.backend.text(&mut engine, paragraph, &opaque, origin, t);
        }) else {
            return;
        };
        exact_svg_raster::apply_coverage(fill.data_mut(), glyphs.data(), 4);
        self.backend.island_image(Arc::new(fill), rect, ts, 0);
    }
}
