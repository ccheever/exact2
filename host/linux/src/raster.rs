//! The CPU backend: tiny-skia. The fallback where no GPU adapter exists
//! (a fleet box), and the deterministic oracle for pixel fixtures — the
//! same bytes on every machine.
//!
//! @ref LLP 1015 §2

use crate::paint::{Backend, Rect4, Shape, POINTER};
use crate::text::{Paragraph, TextEngine};
use std::rc::Rc;
use tiny_skia::{
    Color, FillRule, FilterQuality, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect,
    Stroke, Transform,
};

/// The tiny-skia backend.
#[derive(Default)]
pub struct Raster {
    target: Option<Pixmap>,
    scale: f32,
    width: u32,
    height: u32,
    clips: Vec<Rc<Mask>>,
    layers: Vec<(Pixmap, f32)>,
}

impl Raster {
    /// A backend with nothing painted.
    pub fn new() -> Raster {
        Raster::default()
    }

    fn device(&self, ts: Transform) -> Transform {
        Transform::from_scale(self.scale, self.scale).pre_concat(ts)
    }

    /// The current clip intersected with more shapes, as a mask of its own.
    fn mask_with(&self, shapes: &[Shape], ts: Transform) -> Option<Mask> {
        let dev = self.device(ts);
        let mut m =
            match self.clips.last() {
                Some(c) => (**c).clone(),
                None => {
                    let mut m = Mask::new(self.width, self.height)?;
                    let first = rounded_rect(shapes.first()?)?;
                    m.fill_path(&first, FillRule::Winding, true, dev);
                    return Some(shapes[1..].iter().filter_map(rounded_rect).fold(
                        m,
                        |mut m, p| {
                            m.intersect_path(&p, FillRule::Winding, true, dev);
                            m
                        },
                    ));
                }
            };
        for p in shapes.iter().filter_map(rounded_rect) {
            m.intersect_path(&p, FillRule::Winding, true, dev);
        }
        Some(m)
    }
}

fn solid(c: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(c[0], c[1], c[2], c[3]));
    p.anti_alias = true;
    p
}

/// A shape as a path: a rectangle, or rounded corners as cubic arcs.
pub fn rounded_rect(shape: &Shape) -> Option<Path> {
    let (x, y, w, h) = shape.rect;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let [tl, tr, br, bl] = shape.radii;
    if !shape.rounded() {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    const K: f32 = 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x + tl, y);
    pb.line_to(x + w - tr, y);
    if tr > 0.0 {
        pb.cubic_to(
            x + w - tr + tr * K,
            y,
            x + w,
            y + tr - tr * K,
            x + w,
            y + tr,
        );
    }
    pb.line_to(x + w, y + h - br);
    if br > 0.0 {
        pb.cubic_to(
            x + w,
            y + h - br + br * K,
            x + w - br + br * K,
            y + h,
            x + w - br,
            y + h,
        );
    }
    pb.line_to(x + bl, y + h);
    if bl > 0.0 {
        pb.cubic_to(
            x + bl - bl * K,
            y + h,
            x,
            y + h - bl + bl * K,
            x,
            y + h - bl,
        );
    }
    pb.line_to(x, y + tl);
    if tl > 0.0 {
        pb.cubic_to(x, y + tl - tl * K, x + tl - tl * K, y, x + tl, y);
    }
    pb.close();
    pb.finish()
}

impl Backend for Raster {
    fn name(&self) -> &'static str {
        "cpu"
    }

    fn begin(&mut self, width: f32, height: f32, scale: f32) {
        self.scale = scale;
        self.width = ((width * scale).round() as u32).max(1);
        self.height = ((height * scale).round() as u32).max(1);
        let mut pixmap = Pixmap::new(self.width, self.height).expect("a viewport has pixels");
        pixmap.fill(Color::WHITE);
        self.target = Some(pixmap);
        self.clips.clear();
        self.layers.clear();
    }

    fn fill(&mut self, shape: &Shape, color: [u8; 4], ts: Transform) {
        let Some(path) = rounded_rect(shape) else {
            return;
        };
        let dev = self.device(ts);
        let mask = self.clips.last().cloned();
        if let Some(t) = self.target.as_mut() {
            t.fill_path(
                &path,
                &solid(color),
                FillRule::Winding,
                dev,
                mask.as_deref(),
            );
        }
    }

    fn stroke(&mut self, shape: &Shape, width: f32, color: [u8; 4], ts: Transform) {
        let Some(path) = rounded_rect(shape) else {
            return;
        };
        let dev = self.device(ts);
        let stroke = Stroke {
            width,
            ..Stroke::default()
        };
        let mask = self.clips.last().cloned();
        if let Some(t) = self.target.as_mut() {
            t.stroke_path(&path, &solid(color), &stroke, dev, mask.as_deref());
        }
    }

    fn image(&mut self, image: &Rc<Pixmap>, dst: Rect4, clips: &[Shape], ts: Transform) {
        let (nw, nh) = (image.width() as f32, image.height() as f32);
        if nw <= 0.0 || nh <= 0.0 || dst.2 <= 0.0 || dst.3 <= 0.0 {
            return;
        }
        let mask = self.mask_with(clips, ts);
        let dev = self
            .device(ts)
            .pre_concat(Transform::from_translate(dst.0, dst.1).pre_scale(dst.2 / nw, dst.3 / nh));
        let paint = PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        };
        if let Some(t) = self.target.as_mut() {
            t.draw_pixmap(0, 0, image.as_ref().as_ref(), &paint, dev, mask.as_ref());
        }
    }

    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        color: [u8; 4],
        origin: (f32, f32),
        ts: Transform,
    ) {
        let dev = self.device(ts);
        let scale = self.scale;
        let mask = self.clips.last().cloned();
        if let Some(t) = self.target.as_mut() {
            text.paint(t, paragraph, color, origin, scale, dev, mask.as_deref());
        }
    }

    fn push_clip(&mut self, shape: &Shape, ts: Transform) {
        match self.mask_with(&[*shape], ts) {
            Some(m) => self.clips.push(Rc::new(m)),
            None => {
                // Nothing can show inside an empty box; an empty mask says so.
                if let Some(m) = Mask::new(self.width, self.height) {
                    self.clips.push(Rc::new(m));
                }
            }
        }
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
    }

    fn push_opacity(&mut self, alpha: f32) {
        let Some(fresh) = Pixmap::new(self.width, self.height) else {
            return;
        };
        if let Some(old) = self.target.replace(fresh) {
            self.layers.push((old, alpha));
        }
    }

    fn pop_opacity(&mut self) {
        let Some((mut below, alpha)) = self.layers.pop() else {
            return;
        };
        if let Some(layer) = self.target.take() {
            let paint = PixmapPaint {
                opacity: alpha,
                ..PixmapPaint::default()
            };
            below.draw_pixmap(0, 0, layer.as_ref(), &paint, Transform::identity(), None);
        }
        self.target = Some(below);
    }

    fn pointer(&mut self, x: f32, y: f32) {
        let mut pb = PathBuilder::new();
        for (i, (px, py)) in POINTER.iter().enumerate() {
            if i == 0 {
                pb.move_to(*px, *py);
            } else {
                pb.line_to(*px, *py);
            }
        }
        pb.close();
        let Some(path) = pb.finish() else { return };
        let ts = self.device(Transform::from_translate(x, y));
        let stroke = Stroke {
            width: 1.0,
            ..Stroke::default()
        };
        if let Some(t) = self.target.as_mut() {
            t.fill_path(
                &path,
                &solid([255, 255, 255, 255]),
                FillRule::Winding,
                ts,
                None,
            );
            t.stroke_path(&path, &solid([0, 0, 0, 255]), &stroke, ts, None);
        }
    }

    fn finish(&mut self) -> Result<Pixmap, String> {
        // An unbalanced opacity layer would leave the frame in a layer.
        while !self.layers.is_empty() {
            self.pop_opacity();
        }
        self.target
            .take()
            .ok_or_else(|| "no frame begun".to_string())
    }
}
