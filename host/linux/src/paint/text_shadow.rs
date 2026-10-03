//! CSS `text-shadow` (LLP 1077 D3): the paragraph's glyphs in the shadow's
//! colour, drawn on the CPU into an island over their reach, blurred (σ is
//! half CSS's radius) and placed under the text by either backend.

use super::{Painter, Rect4};
use crate::text::{Paragraph, RunPaint};
use exact_kernel::style::GlyphShadow;
use exact_kernel::{StyleId, StyleMask};
use std::sync::Arc;
use tiny_skia::Transform;

impl Painter {
    /// The shadow of `paragraph` painted at `origin`, before its text.
    pub(super) fn text_shadow(
        &mut self,
        node: &exact_kernel::NodeRef<'_>,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        let style = node.computed_style(StyleMask::of(StyleId::TextShadow));
        let Some(&GlyphShadow {
            color,
            offset,
            blur,
        }) = style.text_shadow.shadow()
        else {
            return;
        };
        let sigma = blur / 2.0;
        let reach = 3.0 * sigma + 1.0;
        let rect: Rect4 = (
            origin.0 + offset.x - reach,
            origin.1 + offset.y - reach,
            paragraph.width + 2.0 * reach,
            paragraph.height + 2.0 * reach,
        );
        // `currentcolor` is each run's own colour.
        let dark = self.dark;
        let shadowed: Vec<RunPaint> = palette
            .iter()
            .map(|r| RunPaint {
                color: color.map_or(r.color, |c| super::rgba(c.resolve(dark))),
                source: r.source,
            })
            .collect();
        let at = (origin.0 + offset.x, origin.1 + offset.y);
        let Some(mut pixels) = self.island(rect, |p, t| {
            let text = p.text.clone();
            let mut engine = text.borrow_mut();
            p.backend.text(&mut engine, paragraph, &shadowed, at, t);
        }) else {
            return;
        };
        let (w, h) = (pixels.width() as usize, pixels.height() as usize);
        exact_svg_raster::backdrop_blur(pixels.data_mut(), w, h, sigma * self.scale);
        self.backend.island_image(Arc::new(pixels), rect, ts, 0);
    }
}
