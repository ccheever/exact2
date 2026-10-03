//! `-webkit-text-stroke` (LLP 1077 D7): a stroke centred on the glyphs'
//! outlines, painted over their fill. The painters draw glyphs as coverage,
//! not outlines, so the stroke is a band: the paragraph in the stroke's
//! colour dilated by half the width, less the same eroded by half. It is a
//! CPU island either backend places over the glyphs, which paint as usual.

use super::{Painter, Rect4};
use crate::text::{Paragraph, RunPaint};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

impl Painter {
    /// The stroke of the runs `stroked` paints (the rest transparent), as
    /// one band island of `width` over their glyphs (`text_paint` orders it).
    pub(super) fn stroke_pass(
        &mut self,
        paragraph: &Paragraph,
        stroked: &[RunPaint],
        width: f32,
        origin: (f32, f32),
        ts: Transform,
    ) {
        let reach = width + 1.0;
        let rect: Rect4 = (
            origin.0 - reach,
            origin.1 - reach,
            paragraph.width + 2.0 * reach,
            paragraph.height + 2.0 * reach,
        );
        let radius = width / 2.0 * self.scale;
        let Some(mut band) = self.island(rect, |p, t| {
            let text = p.text.clone();
            let mut engine = text.borrow_mut();
            p.backend.text(&mut engine, paragraph, stroked, origin, t);
        }) else {
            return;
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
    }
}

/// Dilate (max) or erode (min) every channel over a disc of `radius` device
/// pixels. The disc's edge is soft — a neighbour counts by how much of its
/// pixel the radius reaches past the source pixel's own half-pixel edge — so
/// a fractional width (1.5px at 1×) draws as wide as it is, not rounded to
/// whole pixels.
fn morphology(pixels: &mut Pixmap, radius: f32, dilate: bool) {
    if radius.is_nan() || radius <= 0.0 {
        return;
    }
    let reach = radius.ceil() as i32;
    let (w, h) = (pixels.width() as i32, pixels.height() as i32);
    let src = pixels.data().to_vec();
    let out = pixels.data_mut();
    for y in 0..h {
        for x in 0..w {
            let mut v = if dilate { [0.0f32; 4] } else { [255.0f32; 4] };
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let weight =
                        (radius + 1.0 - ((dx * dx + dy * dy) as f32).sqrt()).clamp(0.0, 1.0);
                    if weight == 0.0 {
                        continue;
                    }
                    let (sx, sy) = (x + dx, y + dy);
                    let s = if (0..w).contains(&sx) && (0..h).contains(&sy) {
                        let i = ((sy * w + sx) * 4) as usize;
                        [src[i], src[i + 1], src[i + 2], src[i + 3]]
                    } else {
                        [0; 4]
                    };
                    for c in 0..4 {
                        let s = s[c] as f32;
                        v[c] = if dilate {
                            v[c].max(s * weight)
                        } else {
                            v[c].min(255.0 - (255.0 - s) * weight)
                        };
                    }
                }
            }
            let i = ((y * w + x) * 4) as usize;
            for c in 0..4 {
                out[i + c] = v[c].round().clamp(0.0, 255.0) as u8;
            }
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
        assert_eq!(p.data().chunks(4).filter(|c| c[3] > 128).count(), 1);
        // Half a pixel more reach is half-covered pixels, not nothing.
        let mut q = Pixmap::new(9, 9).unwrap();
        q.data_mut()[i..i + 4].copy_from_slice(&[255; 4]);
        morphology(&mut q, 1.5, true);
        let half = q
            .data()
            .chunks(4)
            .filter(|c| c[3] > 0 && c[3] < 255)
            .count();
        assert!(half > 0, "a soft edge");
    }
}
