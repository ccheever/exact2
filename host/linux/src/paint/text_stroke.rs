//! `-webkit-text-stroke` (LLP 1076 D7): a stroke centred on the glyphs'
//! outlines, painted over their fill. The painters draw glyphs as coverage,
//! not outlines, so the stroke is a band: the paragraph in the stroke's
//! colour dilated by half the width, less the same eroded by half. It is a
//! CPU island either backend places over the glyphs, which paint as usual.

use super::{Painter, Rect4};
use crate::text::{Paragraph, RunPaint};
use exact_kernel::{StyleId, StyleMask};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

impl Painter {
    /// Paint `paragraph` at `origin` and its stroke over it, when its node
    /// has a stroke; `false` when it has none and the caller paints it.
    pub(super) fn text_stroke(
        &mut self,
        node: &exact_kernel::NodeRef<'_>,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) -> bool {
        let mut mask = StyleMask::of(StyleId::TextStrokeWidth);
        mask.set(StyleId::TextStrokeColor);
        let style = node.computed_style(mask);
        let width = style.text_stroke_width;
        if !(width > 0.0 && width.is_finite()) {
            return false;
        }
        let dark = self.dark;
        // `currentcolor` is each run's own colour.
        let stroked: Vec<RunPaint> = palette
            .iter()
            .map(|r| RunPaint {
                color: style
                    .text_stroke_color
                    .map_or(r.color, |c| super::rgba(c.resolve(dark))),
                source: r.source,
            })
            .collect();
        let reach = width + 1.0;
        let rect: Rect4 = (
            origin.0 - reach,
            origin.1 - reach,
            paragraph.width + 2.0 * reach,
            paragraph.height + 2.0 * reach,
        );
        let radius = width / 2.0 * self.scale;
        {
            let mut engine = self.text.borrow_mut();
            self.backend
                .text(&mut engine, paragraph, palette, origin, ts);
        }
        let Some(mut band) = self.island(rect, |p, t| {
            let text = p.text.clone();
            let mut engine = text.borrow_mut();
            p.backend.text(&mut engine, paragraph, &stroked, origin, t);
        }) else {
            return true;
        };
        let mut inside = band.clone();
        morphology(&mut band, radius, true);
        morphology(&mut inside, radius, false);
        // The band: what the dilation covers and the erosion does not.
        for (px, e) in band
            .data_mut()
            .chunks_exact_mut(4)
            .zip(inside.data().chunks_exact(4))
        {
            let keep = 255 - e[3] as u32;
            for v in px.iter_mut() {
                *v = ((*v as u32 * keep + 127) / 255) as u8;
            }
        }
        self.backend.island_image(Arc::new(band), rect, ts, 0);
        true
    }
}

/// Dilate (max) or erode (min) every channel over a disc of `radius` device
/// pixels, rows then the disc's spans.
fn morphology(pixels: &mut Pixmap, radius: f32, dilate: bool) {
    let r = radius.round() as i32;
    if r <= 0 {
        return;
    }
    let (w, h) = (pixels.width() as i32, pixels.height() as i32);
    let src = pixels.data().to_vec();
    let out = pixels.data_mut();
    let start = if dilate { 0u8 } else { 255u8 };
    for y in 0..h {
        for x in 0..w {
            let mut v = [start; 4];
            for dy in -r..=r {
                let sy = y + dy;
                let span = ((r * r - dy * dy) as f32).sqrt() as i32;
                for dx in -span..=span {
                    let sx = x + dx;
                    let s = if (0..w).contains(&sx) && (0..h).contains(&sy) {
                        let i = ((sy * w + sx) * 4) as usize;
                        [src[i], src[i + 1], src[i + 2], src[i + 3]]
                    } else {
                        [0; 4]
                    };
                    for c in 0..4 {
                        v[c] = if dilate {
                            v[c].max(s[c])
                        } else {
                            v[c].min(s[c])
                        };
                    }
                }
            }
            let i = ((y * w + x) * 4) as usize;
            out[i..i + 4].copy_from_slice(&v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disc_grows_a_dot_and_shrinks_it_back() {
        let mut p = Pixmap::new(9, 9).unwrap();
        let i = (4 * 9 + 4) * 4;
        p.data_mut()[i..i + 4].copy_from_slice(&[255; 4]);
        morphology(&mut p, 2.0, true);
        let lit = p.data().chunks(4).filter(|c| c[3] == 255).count();
        assert_eq!(lit, 13, "a radius-2 disc");
        morphology(&mut p, 2.0, false);
        assert_eq!(p.data().chunks(4).filter(|c| c[3] == 255).count(), 1);
    }
}
