//! CSS `backdrop-filter: blur(σ)` on the CPU painter (LLP 1053.000 D2): the
//! pixels already painted under the border box — in the current layer, so
//! an opacity group is its own backdrop root, as CSS's is — blurred by the
//! shared island rasterizer's Gaussian and put back inside the box, under
//! the current clip.

use super::{rounded_rect, Raster};
use crate::paint::Shape;
use tiny_skia::{
    BlendMode, FillRule, FilterQuality, IntSize, Paint, Pattern, Pixmap, Point, SpreadMode,
    Transform,
};

impl Raster {
    pub(super) fn blur_backdrop(&mut self, shape: &Shape, sigma: f32, ts: Transform) {
        let Some(path) = rounded_rect(shape) else {
            return;
        };
        let dev = self.device(ts);
        let Some(inverse) = dev.invert() else {
            return;
        };
        let b = path.bounds();
        let mut corners = [
            Point::from_xy(b.left(), b.top()),
            Point::from_xy(b.right(), b.top()),
            Point::from_xy(b.right(), b.bottom()),
            Point::from_xy(b.left(), b.bottom()),
        ];
        dev.map_points(&mut corners);
        let (width, height) = (self.width as f32, self.height as f32);
        let x0 = corners
            .iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.0);
        let y0 = corners
            .iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.0);
        let x1 = corners
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(width);
        let y1 = corners
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(height);
        if !(x0 < x1 && y0 < y1) {
            return;
        }
        let (x0, y0, w, h) = (
            x0 as usize,
            y0 as usize,
            (x1 - x0) as usize,
            (y1 - y0) as usize,
        );
        let Some(target) = self.target.as_mut() else {
            return;
        };
        let stride = target.width() as usize * 4;
        let mut pixels = Vec::with_capacity(w * h * 4);
        for row in y0..y0 + h {
            let at = row * stride + x0 * 4;
            pixels.extend_from_slice(&target.data()[at..at + w * 4]);
        }
        // σ in device pixels: the transform's area scale.
        let k = (dev.sx * dev.sy - dev.kx * dev.ky).abs().sqrt();
        exact_svg_raster::backdrop_blur(&mut pixels, w, h, sigma * k);
        let Some(patch) =
            IntSize::from_wh(w as u32, h as u32).and_then(|s| Pixmap::from_vec(pixels, s))
        else {
            return;
        };
        let paint = Paint {
            shader: Pattern::new(
                patch.as_ref(),
                SpreadMode::Pad,
                FilterQuality::Nearest,
                1.0,
                inverse.pre_concat(Transform::from_translate(x0 as f32, y0 as f32)),
            ),
            blend_mode: BlendMode::Source,
            anti_alias: true,
            ..Paint::default()
        };
        let mask = self.clips.last().cloned();
        target.fill_path(&path, &paint, FillRule::Winding, dev, mask.as_deref());
    }
}
