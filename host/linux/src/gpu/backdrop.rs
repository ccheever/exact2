//! CSS `backdrop-filter` on the GPU painter (LLP 1053.000 D2).
//!
//! Vello draws a frame in one pass and cannot read what it has drawn, so a
//! backdrop flushes: the scene so far is rendered and read back, the patch
//! under the border box is filtered on the CPU with the CPU painter's own
//! blur and saturation (`exact_svg_raster`), and the scene starts again
//! from that frame as an image, with the open clip and opacity layers
//! pushed again and the patch drawn inside the box. One extra render and
//! readback per backdrop node per frame — declared in LLP 1053.000 §3, as is
//! that an opacity group's content before the backdrop is composited early.

use super::{shape, Gpu};
use crate::paint::Shape;
use exact_kernel::style::{BackdropFilter, BackdropOp};
use std::sync::Arc;
use tiny_skia::{IntSize, Pixmap, Transform};
use vello::kurbo::{Affine, BezPath, Rect};
use vello::peniko::{Fill, ImageAlphaType, ImageBrush, ImageData, ImageFormat, Mix};

/// A layer open in the scene, as pushed, so a flush can push it again.
pub(super) enum Layer {
    Clip(Fill, Affine, BezPath),
    Opacity(f32),
    /// An isolated group clipped to a path: a mask's (LLP 1077 D2).
    Group(Affine, BezPath),
}

struct Pixels(Arc<Pixmap>);
impl AsRef<[u8]> for Pixels {
    fn as_ref(&self) -> &[u8] {
        self.0.data()
    }
}

fn brush(p: Pixmap) -> ImageBrush {
    let (width, height) = (p.width(), p.height());
    ImageBrush::new(ImageData {
        data: vello::peniko::Blob::new(Arc::new(Pixels(Arc::new(p)))),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::AlphaPremultiplied,
        width,
        height,
    })
}

impl Gpu {
    pub(super) fn push_recorded(&mut self, layer: Layer) {
        self.push_layer(&layer);
        self.layers.push(layer);
    }

    fn push_layer(&mut self, layer: &Layer) {
        match layer {
            Layer::Clip(fill, a, path) => self.scene.push_clip_layer(*fill, *a, path),
            Layer::Group(a, path) => {
                self.scene
                    .push_layer(Fill::NonZero, Mix::Normal, 1.0, *a, path)
            }
            Layer::Opacity(alpha) => {
                let whole = Rect::new(
                    0.0,
                    0.0,
                    (self.width * self.scale) as f64,
                    (self.height * self.scale) as f64,
                );
                self.scene
                    .push_layer(Fill::NonZero, Mix::Normal, *alpha, Affine::IDENTITY, &whole);
            }
        }
    }

    pub(super) fn pop_recorded(&mut self) {
        self.layers.pop();
        self.scene.pop_layer();
    }

    pub(super) fn filter_backdrop(&mut self, s: &Shape, filter: &BackdropFilter, ts: Transform) {
        if s.rect.2 <= 0.0 || s.rect.3 <= 0.0 || self.image_refused {
            return;
        }
        let mut snapshot = self.scene.clone();
        for _ in &self.layers {
            snapshot.pop_layer();
        }
        let Ok(frame) = self.render_scene(&snapshot) else {
            return;
        };
        let a = self.affine(ts);
        let outline = shape(s);
        let b = a.transform_rect_bbox(Rect::new(
            s.rect.0 as f64,
            s.rect.1 as f64,
            (s.rect.0 + s.rect.2) as f64,
            (s.rect.1 + s.rect.3) as f64,
        ));
        let (fw, fh) = (frame.width() as f64, frame.height() as f64);
        let (x0, y0) = (b.x0.floor().max(0.0), b.y0.floor().max(0.0));
        let (x1, y1) = (b.x1.ceil().min(fw), b.y1.ceil().min(fh));
        // The scene starts again from the frame so far, whatever the patch.
        self.scene.reset();
        let full = brush(frame.clone());
        self.scene.draw_image(&full, Affine::IDENTITY);
        let open = std::mem::take(&mut self.layers);
        for layer in &open {
            self.push_layer(layer);
        }
        self.layers = open;
        if !(x0 < x1 && y0 < y1) {
            return;
        }
        let (px, py, w, h) = (
            x0 as usize,
            y0 as usize,
            (x1 - x0) as usize,
            (y1 - y0) as usize,
        );
        let stride = frame.width() as usize * 4;
        let mut pixels = Vec::with_capacity(w * h * 4);
        for row in py..py + h {
            let at = row * stride + px * 4;
            pixels.extend_from_slice(&frame.data()[at..at + w * 4]);
        }
        let k = a.determinant().abs().sqrt() as f32;
        for op in &filter.0 {
            match op {
                BackdropOp::Blur(sigma) => {
                    exact_svg_raster::backdrop_blur(&mut pixels, w, h, sigma * k)
                }
                BackdropOp::Saturate(amount) => {
                    exact_svg_raster::backdrop_saturate(&mut pixels, *amount)
                }
            }
        }
        let Some(patch) =
            IntSize::from_wh(w as u32, h as u32).and_then(|s| Pixmap::from_vec(pixels, s))
        else {
            return;
        };
        self.scene.push_clip_layer(Fill::NonZero, a, &outline);
        self.scene
            .draw_image(&brush(patch), Affine::translate((x0, y0)));
        self.scene.pop_layer();
    }
}
