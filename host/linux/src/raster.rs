//! The CPU backend: tiny-skia. The fallback where no GPU adapter exists
//! (a fleet box), and the deterministic oracle for pixel fixtures — the
//! same bytes on every machine.
//!
//! @ref LLP 1015 §2

use crate::image::Bitmap;
use crate::paint::{Backend, Rect4, Shape, POINTER};
use crate::text::{Paragraph, RunPaint, TextEngine};
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::{
    Color, FillRule, FilterQuality, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect,
    Stroke, Transform,
};

// One optional CPU coverage mask, never a source, picture or node owner.
const CLIP_CACHE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ClipKey {
    width: u32,
    height: u32,
    scale: u32,
    shape: [u32; 8],
    transform: [u32; 6],
}

/// The tiny-skia backend.
#[derive(Default)]
pub struct Raster {
    target: Option<Pixmap>,
    scale: f32,
    width: u32,
    height: u32,
    clips: Vec<Rc<Mask>>,
    text_clips: Vec<Rect4>,
    layers: Vec<(Pixmap, f32)>,
    first_clip_used: bool,
    cached_clip: Option<(ClipKey, Rc<Mask>)>,
    #[cfg(test)]
    pub(crate) clip_allocations: std::cell::Cell<usize>,
    #[cfg(test)]
    pub(crate) clip_watch: Option<std::rc::Weak<Mask>>,
    #[cfg(test)]
    pub(crate) watched_owners_at_allocation: std::cell::Cell<usize>,
}

impl Raster {
    /// A backend with nothing painted.
    pub fn new() -> Raster {
        Raster::default()
    }

    fn device(&self, ts: Transform) -> Transform {
        Transform::from_scale(self.scale, self.scale).pre_concat(ts)
    }

    fn clip_key(&self, shape: &Shape, ts: Transform) -> Option<ClipKey> {
        let bytes = (self.width as usize).checked_mul(self.height as usize)?;
        let shape = [
            shape.rect.0,
            shape.rect.1,
            shape.rect.2,
            shape.rect.3,
            shape.radii[0],
            shape.radii[1],
            shape.radii[2],
            shape.radii[3],
        ];
        let transform = [ts.sx, ts.kx, ts.ky, ts.sy, ts.tx, ts.ty];
        if bytes > CLIP_CACHE_BYTES
            || !self.scale.is_finite()
            || self.scale <= 0.0
            || shape[2] <= 0.0
            || shape[3] <= 0.0
            || !shape.iter().chain(transform.iter()).all(|v| v.is_finite())
            || !self.device(ts).is_finite()
        {
            return None;
        }
        Some(ClipKey {
            width: self.width,
            height: self.height,
            scale: self.scale.to_bits(),
            shape: shape.map(f32::to_bits),
            transform: transform.map(f32::to_bits),
        })
    }

    fn new_clip_mask(&self) -> Option<Mask> {
        // Count this backend's actual Mask::new calls, not tiny-skia's
        // internal intersection scratch or parent-mask clones.
        #[cfg(test)]
        {
            self.clip_allocations.set(self.clip_allocations.get() + 1);
            self.watched_owners_at_allocation.set(
                self.clip_watch
                    .as_ref()
                    .map_or(0, std::rc::Weak::strong_count),
            );
        }
        Mask::new(self.width, self.height)
    }

    #[cfg(test)]
    pub(crate) fn clip_weak(&self) -> std::rc::Weak<Mask> {
        self.clips.last().map(Rc::downgrade).unwrap_or_default()
    }

    // Conservative device bounds of the actual rounded path, including its
    // control points. Mask coverage remains authoritative. An untransformable
    // path keeps the parent bound; uncertainty must never hide text.
    fn text_clip(&self, shape: &Shape, ts: Transform) -> Rect4 {
        let parent = self.text_clips.last().copied().unwrap_or((
            0.0,
            0.0,
            self.width as f32,
            self.height as f32,
        ));
        let Some(path) = rounded_rect(shape) else {
            // mask_with skips invalid shapes when it already has a parent.
            return if self.clips.is_empty() {
                (0.0, 0.0, 0.0, 0.0)
            } else {
                parent
            };
        };
        let Some(path) = path.transform(self.device(ts)) else {
            return parent;
        };
        let b = path.bounds();
        // Beyond the precise integer range, prefer the existing full mask path.
        if [b.left(), b.top(), b.right(), b.bottom()]
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 16_777_216.0)
        {
            return parent;
        }
        let (x, y) = (
            (b.left() - 2.0).max(parent.0),
            (b.top() - 2.0).max(parent.1),
        );
        let (right, bottom) = (
            (b.right() + 2.0).min(parent.0 + parent.2),
            (b.bottom() + 2.0).min(parent.1 + parent.3),
        );
        (x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }

    /// The current clip intersected with more shapes, as a mask of its own.
    fn mask_with(&self, shapes: &[Shape], ts: Transform) -> Option<Mask> {
        let dev = self.device(ts);
        let mut m =
            match self.clips.last() {
                Some(c) => (**c).clone(),
                None => {
                    let mut m = self.new_clip_mask()?;
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
        self.clips.clear();
        self.text_clips.clear();
        self.layers.clear();
        self.first_clip_used = false;
        if self.cached_clip.as_ref().is_some_and(|(key, _)| {
            key.width != self.width || key.height != self.height || key.scale != scale.to_bits()
        }) {
            self.cached_clip = None;
        }
        let mut pixmap = Pixmap::new(self.width, self.height).expect("a viewport has pixels");
        pixmap.fill(Color::WHITE);
        self.target = Some(pixmap);
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

    fn image(&mut self, image: &Arc<Bitmap>, dst: Rect4, clips: &[Shape], ts: Transform) {
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
            t.draw_pixmap(0, 0, image.pixels(), &paint, dev, mask.as_ref());
        }
    }

    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        let dev = self.device(ts);
        let scale = self.scale;
        let mask = self.clips.last().cloned();
        let clip = self.text_clips.last().copied().unwrap_or((
            0.0,
            0.0,
            self.width as f32,
            self.height as f32,
        ));
        if let Some(t) = self.target.as_mut() {
            text.paint_clipped(
                t,
                paragraph,
                palette,
                origin,
                scale,
                dev,
                mask.as_deref(),
                clip,
            );
        }
    }

    fn push_clip(&mut self, shape: &Shape, ts: Transform) {
        let bounds = self.text_clip(shape, ts);
        let first = self.clips.is_empty() && !self.first_clip_used;
        let key = if first {
            // Even an uncacheable first request consumes this frame's slot.
            self.first_clip_used = true;
            let key = self.clip_key(shape, ts);
            if let Some((old, mask)) = self.cached_clip.as_ref() {
                if Some(*old) == key {
                    self.clips.push(mask.clone());
                    self.text_clips.push(bounds);
                    return;
                }
            }
            // No history: retire the old backing before any replacement or
            // refused/fallback allocation. Active parents are absent here.
            self.cached_clip = None;
            key
        } else {
            None
        };
        match self.mask_with(&[*shape], ts) {
            Some(m) => {
                let mask = Rc::new(m);
                if let Some(key) = key {
                    self.cached_clip = Some((key, mask.clone()));
                }
                self.clips.push(mask);
                self.text_clips.push(bounds);
            }
            None => {
                // Nothing can show inside an empty box; an empty mask says so.
                if let Some(m) = self.new_clip_mask() {
                    self.clips.push(Rc::new(m));
                    self.text_clips.push((0.0, 0.0, 0.0, 0.0));
                }
            }
        }
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
        self.text_clips.pop();
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
