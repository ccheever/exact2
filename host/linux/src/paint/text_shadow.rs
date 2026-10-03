//! A paragraph's glyph paint with CSS `text-shadow` (LLP 1077 D3) and
//! `-webkit-text-stroke` (D7), per inline run. CSS paints each inline box's
//! shadow, then its text, then its stroke, box after box in tree order, so a
//! later run's shadow can fall over an earlier run's glyphs and an earlier
//! run's stroke never covers a later run's fill. Runs that all look alike
//! paint in one pass: the shadow under every glyph, the fill, the stroke.
//! Otherwise each stretch of adjacent runs that look alike paints in turn,
//! the other runs transparent in its passes. A shadow is the runs drawn in
//! its colour into a CPU island over the paragraph's reach, blurred (σ is
//! half CSS's radius) and placed under that stretch's glyphs.

use super::{Painter, Rect4};
use crate::text::{Paragraph, RunPaint};
use exact_kernel::{Kernel, NodeRef, StyleId, StyleMask};
use std::sync::Arc;
use tiny_skia::Transform;

/// One run's shadow (offset, blur, colour) and stroke (width, colour),
/// `currentcolor` resolved to the run's presented colour.
#[derive(Clone, Copy, PartialEq, Default)]
struct RunLook {
    shadow: Option<((f32, f32), f32, [u8; 4])>,
    stroke: Option<(f32, [u8; 4])>,
}

impl Painter {
    fn run_looks(&self, kernel: &Kernel, palette: &[RunPaint]) -> Vec<RunLook> {
        let mut mask = StyleMask::of(StyleId::TextShadow);
        mask.set(StyleId::TextStrokeWidth);
        mask.set(StyleId::TextStrokeColor);
        palette
            .iter()
            .map(|run| {
                let Some(node) = kernel.node(run.source) else {
                    return RunLook::default();
                };
                let style = node.computed_style(mask);
                let shadow = style.text_shadow.shadow().map(|s| {
                    let color = s
                        .color
                        .map_or(run.color, |c| super::rgba(c.resolve(self.dark)));
                    ((s.offset.x, s.offset.y), s.blur, color)
                });
                let width = style.text_stroke_width;
                let stroke = (width > 0.0 && width.is_finite()).then(|| {
                    let color = style
                        .text_stroke_color
                        .map_or(run.color, |c| super::rgba(c.resolve(self.dark)));
                    (width, color)
                });
                RunLook { shadow, stroke }
            })
            .collect()
    }

    /// `paragraph` at `origin`: its shadows, the `background-clip: text`
    /// fill under its glyphs, its glyphs and its strokes.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn text_paint(
        &mut self,
        node: &NodeRef<'_>,
        kernel: &Kernel,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        rect: Rect4,
        ts: Transform,
    ) {
        let looks = self.run_looks(kernel, palette);
        // Stretches of adjacent runs that look alike, in run order.
        let mut steps: Vec<(usize, usize)> = Vec::new();
        for i in 0..looks.len() {
            match steps.last_mut() {
                Some((_, end)) if looks[*end - 1] == looks[i] => *end = i + 1,
                _ => steps.push((i, i + 1)),
            }
        }
        if steps.len() <= 1 {
            // One look: today's single pass.
            let look = looks.first().copied().unwrap_or_default();
            if let Some(shadow) = look.shadow {
                self.shadow_pass(
                    paragraph,
                    &only(palette, 0..palette.len(), shadow.2),
                    shadow,
                    origin,
                    ts,
                );
            }
            self.text_clip(node, kernel, paragraph, palette, origin, rect, ts);
            self.fill_pass(paragraph, palette, origin, ts);
            if let Some((width, color)) = look.stroke {
                self.stroke_pass(
                    paragraph,
                    &only(palette, 0..palette.len(), color),
                    width,
                    origin,
                    ts,
                );
            }
            return;
        }
        // The box's background, clipped to the glyphs, is under all of its
        // inline content, shadows included.
        self.text_clip(node, kernel, paragraph, palette, origin, rect, ts);
        for (start, end) in steps {
            let look = looks[start];
            if let Some(shadow) = look.shadow {
                self.shadow_pass(
                    paragraph,
                    &only(palette, start..end, shadow.2),
                    shadow,
                    origin,
                    ts,
                );
            }
            let fill: Vec<RunPaint> = palette
                .iter()
                .enumerate()
                .map(|(i, r)| RunPaint {
                    color: if (start..end).contains(&i) {
                        r.color
                    } else {
                        [0; 4]
                    },
                    ..*r
                })
                .collect();
            self.fill_pass(paragraph, &fill, origin, ts);
            if let Some((width, color)) = look.stroke {
                self.stroke_pass(
                    paragraph,
                    &only(palette, start..end, color),
                    width,
                    origin,
                    ts,
                );
            }
        }
    }

    fn fill_pass(
        &mut self,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        let mut engine = self.text.borrow_mut();
        self.backend
            .text(&mut engine, paragraph, palette, origin, ts);
    }

    /// The runs `shadowed` paints (the rest transparent), blurred and offset.
    fn shadow_pass(
        &mut self,
        paragraph: &Paragraph,
        shadowed: &[RunPaint],
        ((x, y), blur, _): ((f32, f32), f32, [u8; 4]),
        origin: (f32, f32),
        ts: Transform,
    ) {
        let sigma = blur / 2.0;
        let reach = 3.0 * sigma + 1.0;
        let rect: Rect4 = (
            origin.0 + x - reach,
            origin.1 + y - reach,
            paragraph.width + 2.0 * reach,
            paragraph.height + 2.0 * reach,
        );
        let at = (origin.0 + x, origin.1 + y);
        let Some(mut pixels) = self.island(rect, |p, t| {
            let text = p.text.clone();
            let mut engine = text.borrow_mut();
            p.backend.text(&mut engine, paragraph, shadowed, at, t);
        }) else {
            return;
        };
        let (w, h) = (pixels.width() as usize, pixels.height() as usize);
        exact_svg_raster::backdrop_blur(pixels.data_mut(), w, h, sigma * self.scale);
        self.backend.island_image(Arc::new(pixels), rect, ts, 0);
    }
}

/// `palette` with the runs in `range` in `color` and the rest transparent.
fn only(palette: &[RunPaint], range: std::ops::Range<usize>, color: [u8; 4]) -> Vec<RunPaint> {
    palette
        .iter()
        .enumerate()
        .map(|(i, r)| RunPaint {
            color: if range.contains(&i) { color } else { [0; 4] },
            ..*r
        })
        .collect()
}
