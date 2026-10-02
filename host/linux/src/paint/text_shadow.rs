//! CSS `text-shadow` (LLP 1077 D3): runs sharing shadow geometry are drawn
//! in their shadow colours into one CPU island over the paragraph's reach,
//! blurred (σ is half CSS's radius) and placed under all glyphs.

use super::{Painter, Rect4};
use crate::text::{Paragraph, RunPaint};
use exact_kernel::style::GlyphShadow;
use exact_kernel::{Kernel, StyleId, StyleMask};
use std::sync::Arc;
use tiny_skia::Transform;

impl Painter {
    /// The shadow of `paragraph` painted at `origin`, before its text.
    pub(super) fn text_shadow(
        &mut self,
        kernel: &Kernel,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        let mut groups: Vec<(GlyphShadow, Vec<RunPaint>)> = Vec::new();
        for (i, run) in palette.iter().enumerate() {
            let Some(node) = kernel.node(run.source) else {
                continue;
            };
            let style = node.computed_style(StyleMask::of(StyleId::TextShadow));
            let Some(&shadow) = style.text_shadow.shadow() else {
                continue;
            };
            let group = groups
                .iter()
                .position(|(s, _)| s.offset == shadow.offset && s.blur == shadow.blur)
                .unwrap_or_else(|| {
                    groups.push((
                        shadow,
                        palette
                            .iter()
                            .map(|r| RunPaint {
                                color: [0; 4],
                                ..*r
                            })
                            .collect(),
                    ));
                    groups.len() - 1
                });
            // Resolve `currentcolor` from this run's presented colour.
            groups[group].1[i].color = shadow
                .color
                .map_or(run.color, |c| super::rgba(c.resolve(self.dark)));
        }
        for (GlyphShadow { offset, blur, .. }, shadowed) in groups {
            let sigma = blur / 2.0;
            let reach = 3.0 * sigma + 1.0;
            let rect: Rect4 = (
                origin.0 + offset.x - reach,
                origin.1 + offset.y - reach,
                paragraph.width + 2.0 * reach,
                paragraph.height + 2.0 * reach,
            );
            let at = (origin.0 + offset.x, origin.1 + offset.y);
            let Some(mut pixels) = self.island(rect, |p, t| {
                let text = p.text.clone();
                let mut engine = text.borrow_mut();
                p.backend.text(&mut engine, paragraph, &shadowed, at, t);
            }) else {
                continue;
            };
            let (w, h) = (pixels.width() as usize, pixels.height() as usize);
            exact_svg_raster::backdrop_blur(pixels.data_mut(), w, h, sigma * self.scale);
            self.backend.island_image(Arc::new(pixels), rect, ts, 0);
        }
    }
}
