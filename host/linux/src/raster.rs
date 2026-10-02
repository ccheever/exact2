//! The CPU backend: tiny-skia. The fallback where no GPU adapter exists
//! (a fleet box), and the deterministic oracle for pixel fixtures — the
//! same bytes on every machine.
//!
//! @ref LLP 1015 §2

mod backdrop;
mod mask;

use crate::image::Bitmap;
use crate::paint::border::{BorderFill, PathOp};
use crate::paint::GradientPaint;
use crate::paint::{Backend, Rect4, Shape, POINTER};
use crate::text::{Paragraph, RunPaint, TextEngine};
use exact_kernel::gradient::{premultiplied_ramp, Geometry};
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::{
    Color, FillRule, FilterQuality, GradientStop, LinearGradient, Mask, Paint, Path, PathBuilder,
    Pixmap, PixmapPaint, Point, RadialGradient, Rect, SpreadMode, Stroke, Transform,
};

// One optional CPU coverage mask, never a source, picture or node owner.
const CLIP_CACHE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ClipKey {
    width: u32,
    height: u32,
    scale: u32,
    shape: [u32; 12],
    transform: [u32; 6],
}

/// The tiny-skia backend.
#[derive(Default)]
pub struct Raster {
    target: Option<Pixmap>,
    transparent: bool,
    scale: f32,
    width: u32,
    height: u32,
    clips: Vec<Rc<Mask>>,
    text_clips: Vec<Rect4>,
    rectangular_damage: Option<(usize, Rect)>,
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

    pub(crate) fn transparent() -> Self {
        Self {
            transparent: true,
            ..Self::default()
        }
    }

    fn device(&self, ts: Transform) -> Transform {
        Transform::from_scale(self.scale, self.scale).pre_concat(ts)
    }

    fn reset(&mut self, width: f32, height: f32, scale: f32) {
        self.scale = scale;
        self.width = ((width * scale).round() as u32).max(1);
        self.height = ((height * scale).round() as u32).max(1);
        self.clips.clear();
        self.text_clips.clear();
        self.rectangular_damage = None;
        self.layers.clear();
        self.first_clip_used = false;
        if self.cached_clip.as_ref().is_some_and(|(key, _)| {
            key.width != self.width || key.height != self.height || key.scale != scale.to_bits()
        }) {
            self.cached_clip = None;
        }
        self.target = None;
    }

    // A rounded box has two solid central strips. If the complete binary
    // damage rectangle lies inside either strip, no curved edge is painted.
    fn covered_damage(&self, shape: &Shape, dev: Transform) -> Option<Rect> {
        let (depth, damage) = self.rectangular_damage?;
        let (x, y, w, h) = shape.rect;
        if depth != self.clips.len()
            || self.width > 8191
            || self.height > 8191
            || !dev.is_finite()
            || dev.kx != 0.0
            || dev.ky != 0.0
            || dev.sx == 0.0
            || dev.sy == 0.0
            || ![x, y, w, h].iter().all(|v| v.is_finite())
            || !shape.radii.iter().all(|r| {
                r.0.is_finite()
                    && r.1.is_finite()
                    && r.0 >= 0.0
                    && r.1 >= 0.0
                    && r.0 <= w / 2.0
                    && r.1 <= h / 2.0
            })
        {
            return None;
        }
        let bounds = |[left, top, right, bottom]: [f32; 4]| {
            let mut corners = [Point::from_xy(left, top), Point::from_xy(right, bottom)];
            dev.map_points(&mut corners);
            [
                corners[0].x.min(corners[1].x),
                corners[0].y.min(corners[1].y),
                corners[0].x.max(corners[1].x),
                corners[0].y.max(corners[1].y),
            ]
        };
        if bounds([x, y, x + w, y + h])
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 8191.0)
        {
            return None;
        }
        let [tl, tr, br, bl] = shape.radii;
        for strip in [
            [x + tl.0.max(bl.0), y, x + w - tr.0.max(br.0), y + h],
            [x, y + tl.1.max(tr.1), x + w, y + h - bl.1.max(br.1)],
        ] {
            let [left, top, right, bottom] = bounds(strip);
            // Keep two device pixels away from scan-conversion and AA edges.
            if damage.left() >= left + 2.0
                && damage.top() >= top + 2.0
                && damage.right() <= right - 2.0
                && damage.bottom() <= bottom - 2.0
            {
                return Some(damage);
            }
        }
        None
    }

    fn clip_key(&self, shape: &Shape, ts: Transform) -> Option<ClipKey> {
        let bytes = (self.width as usize).checked_mul(self.height as usize)?;
        let shape = [
            shape.rect.0,
            shape.rect.1,
            shape.rect.2,
            shape.rect.3,
            shape.radii[0].0,
            shape.radii[0].1,
            shape.radii[1].0,
            shape.radii[1].1,
            shape.radii[2].0,
            shape.radii[2].1,
            shape.radii[3].0,
            shape.radii[3].1,
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
        let mut m = match self.clips.last() {
            Some(c) => {
                if let Some((depth, rect)) = self.rectangular_damage {
                    if depth == self.clips.len() {
                        let mut paths = shapes.iter().filter_map(rounded_rect);
                        if let Some(first) = paths.next() {
                            // A proven integer damage rectangle has only 0/255
                            // coverage. Build the child once and clear outside it.
                            let mut mask = self.new_clip_mask()?;
                            mask.fill_path(&first, FillRule::Winding, true, dev);
                            let width = self.width as usize;
                            let (left, top, right, bottom) = (
                                rect.left() as usize,
                                rect.top() as usize,
                                rect.right() as usize,
                                rect.bottom() as usize,
                            );
                            let data = mask.data_mut();
                            data[..top * width].fill(0);
                            data[bottom * width..].fill(0);
                            for row in data[top * width..bottom * width].chunks_exact_mut(width) {
                                row[..left].fill(0);
                                row[right..].fill(0);
                            }
                            for path in paths {
                                mask.intersect_path(&path, FillRule::Winding, true, dev);
                            }
                            return Some(mask);
                        }
                    }
                }
                (**c).clone()
            }
            None => {
                let mut m = self.new_clip_mask()?;
                let first = rounded_rect(shapes.first()?)?;
                m.fill_path(&first, FillRule::Winding, true, dev);
                return Some(
                    shapes[1..]
                        .iter()
                        .filter_map(rounded_rect)
                        .fold(m, |mut m, p| {
                            m.intersect_path(&p, FillRule::Winding, true, dev);
                            m
                        }),
                );
            }
        };
        for p in shapes.iter().filter_map(rounded_rect) {
            m.intersect_path(&p, FillRule::Winding, true, dev);
        }
        Some(m)
    }
}

// A new path mask is zero outside its control bounds. Include AA slack;
// tiny-skia tiles above 8191 pixels, so retain full-mask work beyond that range.
/// `mask` times `other`, coverage by coverage: an intersection.
fn multiply(mask: &mut Mask, other: &Mask) {
    for (a, b) in mask.data_mut().iter_mut().zip(other.data()) {
        *a = ((*a as u16 * *b as u16 + 127) / 255) as u8;
    }
}

fn intersect_mask(mask: &mut Mask, parent: &Mask, path: &Path, dev: Transform) {
    let (width, height) = (mask.width() as usize, mask.height() as usize);
    let bounds = if width <= 8191 && height <= 8191 {
        path.clone().transform(dev).and_then(|p| {
            let b = p.bounds();
            let edges = [b.left(), b.top(), b.right(), b.bottom()];
            edges
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 8191.0)
                .then(|| {
                    [
                        (b.left().floor() - 2.0).clamp(0.0, width as f32) as usize,
                        (b.top().floor() - 2.0).clamp(0.0, height as f32) as usize,
                        (b.right().ceil() + 2.0).clamp(0.0, width as f32) as usize,
                        (b.bottom().ceil() + 2.0).clamp(0.0, height as f32) as usize,
                    ]
                })
        })
    } else {
        None
    };
    let [left, top, right, bottom] = bounds.unwrap_or([0, 0, width, height]);
    for y in top..bottom {
        let span = y * width + left..y * width + right;
        for (a, b) in mask.data_mut()[span.clone()]
            .iter_mut()
            .zip(&parent.data()[span])
        {
            *a = (u16::from(*a) * u16::from(*b) / 255) as u8;
        }
    }
}

fn solid(c: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(c[0], c[1], c[2], c[3]));
    p.anti_alias = true;
    p
}

/// A border part's path for tiny-skia.
fn tiny_path(ops: &[PathOp]) -> Option<Path> {
    let mut b = PathBuilder::new();
    for op in ops {
        match *op {
            PathOp::Move(x, y) => b.move_to(x, y),
            PathOp::Line(x, y) => b.line_to(x, y),
            PathOp::Cubic(a, c, d, e, f, g) => b.cubic_to(a, c, d, e, f, g),
            PathOp::Close => b.close(),
        }
    }
    b.finish()
}

/// A shape as a path: a rectangle, or rounded corners as cubic arcs.
pub fn rounded_rect(shape: &Shape) -> Option<Path> {
    let (x, y, w, h) = shape.rect;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    if !shape.rounded() {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let mut ops = Vec::new();
    crate::paint::border::shape_path(&mut ops, shape);
    tiny_path(&ops)
}

/// The largest frame side, in device pixels, either painter draws.
pub(crate) const MAX_FRAME_SIDE: u32 = 16384;

impl Backend for Raster {
    fn name(&self) -> &'static str {
        "cpu"
    }

    fn begin(&mut self, width: f32, height: f32, scale: f32) {
        self.reset(width, height, scale);
        // A frame past the largest side is never allocated (420×860 points at
        // scale 1000 is 1.4 TB); drawing into none is a no-op and `finish`
        // refuses the frame by name.
        let fits = self.width <= MAX_FRAME_SIDE && self.height <= MAX_FRAME_SIDE;
        self.target = fits
            .then(|| Pixmap::new(self.width, self.height))
            .flatten()
            .map(|mut pixmap| {
                if !self.transparent {
                    pixmap.fill(Color::WHITE);
                }
                pixmap
            });
    }

    fn begin_damage(
        &mut self,
        width: f32,
        height: f32,
        scale: f32,
        previous: &Pixmap,
        rects: &[Rect4],
    ) -> bool {
        self.reset(width, height, scale);
        if self.damage(previous, rects) {
            true
        } else {
            self.begin(width, height, scale);
            false
        }
    }

    fn damage(&mut self, previous: &Pixmap, rects: &[Rect4]) -> bool {
        if previous.width() != self.width || previous.height() != self.height {
            return false;
        }
        let Some(mut mask) = Mask::new(self.width, self.height) else {
            return false;
        };
        let dev = self.device(Transform::identity());
        // The damage mask has binary coverage. Paint those same paths opaquely
        // while building it, avoiding a second masked pass over the whole frame.
        // Retain the original clear where viewport edges or tiling are uncertain.
        let direct_clear = self.scale.is_finite()
            && self.scale > 0.0
            && self.width <= 8191
            && self.height <= 8191
            && (self.width as f32 / self.scale) * self.scale == self.width as f32
            && (self.height as f32 / self.scale) * self.scale == self.height as f32
            && rects.iter().all(|&(x, y, w, h)| {
                [x, y, x + w, y + h]
                    .iter()
                    .all(|v| (v * self.scale).is_finite() && (v * self.scale).abs() <= 8191.0)
            });
        let mut target = previous.clone();
        let mut clear = solid([255; 4]);
        clear.anti_alias = false;
        let mut bounds = (f32::INFINITY, f32::INFINITY, 0.0_f32, 0.0_f32);
        for &(x, y, w, h) in rects {
            let Some(rect) = Rect::from_xywh(x, y, w, h) else {
                continue;
            };
            let path = PathBuilder::from_rect(rect);
            mask.fill_path(&path, FillRule::Winding, false, dev);
            if direct_clear {
                target.fill_path(&path, &clear, FillRule::Winding, dev, None);
            }
            bounds.0 = bounds.0.min(x);
            bounds.1 = bounds.1.min(y);
            bounds.2 = bounds.2.max(x + w);
            bounds.3 = bounds.3.max(y + h);
        }
        self.target = Some(target);
        self.clips.push(Rc::new(mask));
        // A containing input rectangle proves the binary union is rectangular.
        // Integer device edges allow opaque integer fills to use that rectangle
        // directly, without changing antialiasing or alpha rounding at an edge.
        self.rectangular_damage = None;
        let edges = [bounds.0, bounds.1, bounds.2, bounds.3].map(|v| v * self.scale);
        if direct_clear
            && edges.iter().all(|v| v.fract() == 0.0)
            && rects.iter().any(|&(x, y, w, h)| {
                x == bounds.0 && y == bounds.1 && x + w == bounds.2 && y + h == bounds.3
            })
        {
            self.rectangular_damage = Rect::from_ltrb(
                edges[0].max(0.0),
                edges[1].max(0.0),
                edges[2].min(self.width as f32),
                edges[3].min(self.height as f32),
            )
            .map(|rect| (self.clips.len(), rect));
        }
        self.text_clips.push((
            bounds.0 * self.scale,
            bounds.1 * self.scale,
            (bounds.2 - bounds.0) * self.scale,
            (bounds.3 - bounds.1) * self.scale,
        ));
        if !direct_clear {
            self.fill(
                &Shape::rect((
                    0.,
                    0.,
                    self.width as f32 / self.scale,
                    self.height as f32 / self.scale,
                )),
                [255; 4],
                Transform::identity(),
            );
        }
        true
    }

    fn fill(&mut self, shape: &Shape, color: [u8; 4], ts: Transform) {
        let Some(mut path) = rounded_rect(shape) else {
            return;
        };
        let mut dev = self.device(ts);
        if shape.rounded() && color[3] == 255 {
            if let Some(rect) = self.covered_damage(shape, dev) {
                if let Some(target) = self.target.as_mut() {
                    target.fill_path(
                        &PathBuilder::from_rect(rect),
                        &solid(color),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
                return;
            }
        }
        // Avoid shading a large background outside the active mask. Keep the
        // original fractional edges; only introduce integer edges beyond the
        // conservative clip bounds, where mask coverage is already zero.
        if !shape.rounded()
            && dev.kx == 0.0
            && dev.ky == 0.0
            && self.width <= 8191
            && self.height <= 8191
        {
            if let Some(&(x, y, w, h)) = self.text_clips.last() {
                let b = path.bounds();
                let mut corners = [
                    Point::from_xy(b.left(), b.top()),
                    Point::from_xy(b.right(), b.bottom()),
                ];
                dev.map_points(&mut corners);
                let original = [
                    corners[0].x.min(corners[1].x),
                    corners[0].y.min(corners[1].y),
                    corners[0].x.max(corners[1].x),
                    corners[0].y.max(corners[1].y),
                ];
                let edges = [
                    original[0],
                    original[1],
                    original[2],
                    original[3],
                    x,
                    y,
                    x + w,
                    y + h,
                ];
                if dev.is_finite() && edges.iter().all(|v| v.is_finite() && v.abs() <= 8191.0) {
                    if color[3] == 255 && original.iter().all(|v| v.fract() == 0.0) {
                        if let Some((depth, damage)) = self.rectangular_damage {
                            if depth == self.clips.len() {
                                if let Some(rect) = Rect::from_ltrb(
                                    original[0].max(damage.left()),
                                    original[1].max(damage.top()),
                                    original[2].min(damage.right()),
                                    original[3].min(damage.bottom()),
                                ) {
                                    if let Some(target) = self.target.as_mut() {
                                        target.fill_path(
                                            &PathBuilder::from_rect(rect),
                                            &solid(color),
                                            FillRule::Winding,
                                            Transform::identity(),
                                            None,
                                        );
                                    }
                                }
                                return;
                            }
                        }
                    }
                    let left = original[0].max(x.floor() - 2.0);
                    let top = original[1].max(y.floor() - 2.0);
                    let right = original[2].min((x + w).ceil() + 2.0);
                    let bottom = original[3].min((y + h).ceil() + 2.0);
                    if left >= right || top >= bottom {
                        return;
                    }
                    if [left, top, right, bottom] != original {
                        let mut builder = PathBuilder::new();
                        builder.move_to(left, top);
                        builder.line_to(right, top);
                        builder.line_to(right, bottom);
                        builder.line_to(left, bottom);
                        builder.close();
                        path = builder.finish().expect("a finite nonempty rectangle");
                        dev = Transform::identity();
                    }
                }
            }
        }
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

    // @ref LLP 1055 D4 — an SVG shape: fill under stroke, clipped as boxes are.
    fn svg_path(&mut self, s: &crate::paint::SvgPaint<'_>, ts: Transform) {
        let mut b = PathBuilder::new();
        for seg in &s.path.0 {
            match *seg {
                exact_kernel::svg::Seg::Move(x, y) => b.move_to(x, y),
                exact_kernel::svg::Seg::Line(x, y) => b.line_to(x, y),
                exact_kernel::svg::Seg::Cubic(a, c, d, e, x, y) => b.cubic_to(a, c, d, e, x, y),
                exact_kernel::svg::Seg::Close => b.close(),
            }
        }
        let Some(path) = b.finish() else {
            return;
        };
        let dev = self.device(ts);
        let mask = self.clips.last().cloned();
        let Some(t) = self.target.as_mut() else {
            return;
        };
        // @ref LLP 1055.000 D7 — a gradient is a tiny-skia shader in the
        // path's space (two circles, as SVG's focal radial).
        fn paint<'a>(ink: &'a crate::paint::Ink<'_>) -> Option<tiny_skia::Paint<'a>> {
            match ink {
                crate::paint::Ink::Solid(c) => Some(solid(*c)),
                // @ref LLP 1055.000 D7 — a pattern's tile, repeated.
                crate::paint::Ink::Pattern {
                    tile,
                    transform: m,
                    opacity,
                } => Some(tiny_skia::Paint {
                    shader: tiny_skia::Pattern::new(
                        tile.as_ref().as_ref(),
                        tiny_skia::SpreadMode::Repeat,
                        tiny_skia::FilterQuality::Bilinear,
                        *opacity,
                        Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5]),
                    ),
                    anti_alias: true,
                    ..Default::default()
                }),
                crate::paint::Ink::Gradient {
                    server,
                    stops,
                    transform,
                } => {
                    use exact_kernel::svg::server::{ServerKind, Spread};
                    use tiny_skia::{GradientStop, Point, SpreadMode};
                    let stops: Vec<GradientStop> = stops
                        .iter()
                        .map(|(o, c)| {
                            GradientStop::new(*o, Color::from_rgba8(c[0], c[1], c[2], c[3]))
                        })
                        .collect();
                    let mode = match server.spread {
                        Spread::Pad => SpreadMode::Pad,
                        Spread::Reflect => SpreadMode::Reflect,
                        Spread::Repeat => SpreadMode::Repeat,
                    };
                    let m = transform;
                    let tr = Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5]);
                    let shader = match server.kind {
                        ServerKind::Linear { x1, y1, x2, y2 } => tiny_skia::LinearGradient::new(
                            Point::from_xy(x1, y1),
                            Point::from_xy(x2, y2),
                            stops,
                            mode,
                            tr,
                        ),
                        ServerKind::Radial {
                            cx,
                            cy,
                            r,
                            fx,
                            fy,
                            fr,
                        } => tiny_skia::RadialGradient::new(
                            Point::from_xy(fx, fy),
                            fr,
                            Point::from_xy(cx, cy),
                            r,
                            stops,
                            mode,
                            tr,
                        ),
                    }?;
                    Some(tiny_skia::Paint {
                        shader,
                        anti_alias: true,
                        ..Default::default()
                    })
                }
            }
        }
        for part in s.order {
            match part {
                0 => {
                    if let Some(p) = s.fill.as_ref().and_then(paint) {
                        let rule = if s.even_odd {
                            FillRule::EvenOdd
                        } else {
                            FillRule::Winding
                        };
                        t.fill_path(&path, &p, rule, dev, mask.as_deref());
                    }
                }
                1 => {
                    let Some(p) = s.stroke.as_ref().and_then(paint).filter(|_| s.width > 0.0)
                    else {
                        continue;
                    };
                    let stroke = Stroke {
                        width: s.width,
                        miter_limit: s.miter,
                        line_cap: [
                            tiny_skia::LineCap::Butt,
                            tiny_skia::LineCap::Round,
                            tiny_skia::LineCap::Square,
                        ][s.cap.min(2) as usize],
                        line_join: [
                            tiny_skia::LineJoin::Miter,
                            tiny_skia::LineJoin::Round,
                            tiny_skia::LineJoin::Bevel,
                        ][s.join.min(2) as usize],
                        dash: (!s.dash.is_empty())
                            .then(|| tiny_skia::StrokeDash::new(s.dash.clone(), s.phase))
                            .flatten(),
                    };
                    t.stroke_path(&path, &p, &stroke, dev, mask.as_deref());
                }
                _ => {}
            }
        }
    }

    fn fill_gradient(&mut self, shape: &Shape, gradient: &GradientPaint, ts: Transform) {
        let Some(path) = rounded_rect(shape) else {
            return;
        };
        // tiny-skia mixes stops unpremultiplied; CSS mixes premultiplied.
        let stops = premultiplied_ramp(&gradient.stops)
            .into_iter()
            .map(|(at, c)| GradientStop::new(at, Color::from_rgba8(c.r(), c.g(), c.b(), c.a())))
            .collect();
        let shader = match gradient.geometry {
            Geometry::Linear { start, end } => LinearGradient::new(
                Point::from_xy(start.0, start.1),
                Point::from_xy(end.0, end.1),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            ),
            // The unit circle, scaled to the ellipse.
            Geometry::Radial { center, radii } => RadialGradient::new(
                Point::zero(),
                0.0,
                Point::zero(),
                1.0,
                stops,
                SpreadMode::Pad,
                Transform::from_row(radii.0, 0.0, 0.0, radii.1, center.0, center.1),
            ),
        };
        let Some(shader) = shader else {
            return;
        };
        let paint = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        let dev = self.device(ts);
        let mask = self.clips.last().cloned();
        if let Some(t) = self.target.as_mut() {
            t.fill_path(&path, &paint, FillRule::Winding, dev, mask.as_deref());
        }
    }

    fn fill_border(&mut self, part: &BorderFill, ts: Transform) {
        let Some(region) = tiny_path(&part.region) else {
            return;
        };
        let dev = self.device(ts);
        let parent = self.clips.last().cloned();
        // A clipped part: its quadrilaterals as a mask, under whatever clip
        // is already in force. Only rounded multicolour borders have one.
        let clipped = match &part.clip {
            None => None,
            Some(clip) => {
                let Some(clip) = tiny_path(clip) else {
                    return;
                };
                let Some(mut mask) = Mask::new(self.width, self.height) else {
                    return;
                };
                mask.fill_path(&clip, FillRule::Winding, true, dev);
                if let Some(parent) = &parent {
                    intersect_mask(&mut mask, parent, &clip, dev);
                }
                Some(Rc::new(mask))
            }
        };
        let mask = clipped.or(parent);
        if let Some(t) = self.target.as_mut() {
            t.fill_path(
                &region,
                &solid(part.color),
                FillRule::EvenOdd,
                dev,
                mask.as_deref(),
            );
        }
    }

    fn image(
        &mut self,
        image: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        tint: Option<[u8; 4]>,
    ) {
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
        // Tint in an isolated paint layer, never by copying the decoded asset.
        let layers = self.layers.len();
        if tint.is_some() {
            self.push_opacity(1.0);
            if self.layers.len() == layers {
                return;
            }
        }
        if let Some(t) = self.target.as_mut() {
            t.draw_pixmap(0, 0, image.pixels(), &paint, dev, mask.as_ref());
            if let Some(tint) = tint {
                let mut ink = solid(tint);
                ink.blend_mode = tiny_skia::BlendMode::SourceIn;
                let rect = Rect::from_xywh(0., 0., t.width() as f32, t.height() as f32).unwrap();
                t.fill_rect(rect, &ink, Transform::identity(), None);
            }
        }
        if tint.is_some() {
            self.pop_opacity();
        }
    }

    // @ref LLP 1055.000 D14 — an island in its element's user space.
    fn island_image(&mut self, image: Arc<Pixmap>, dst: Rect4, ts: Transform, mode: u8) {
        let (nw, nh) = (image.width() as f32, image.height() as f32);
        if nw <= 0.0 || nh <= 0.0 || dst.2 <= 0.0 || dst.3 <= 0.0 {
            return;
        }
        let mask = self.clips.last().cloned();
        let dev = self
            .device(ts)
            .pre_concat(Transform::from_translate(dst.0, dst.1).pre_scale(dst.2 / nw, dst.3 / nh));
        // @ref LLP 1055.000 D19 — `mix-blend-mode`, in CSS's order.
        use tiny_skia::BlendMode as B;
        let blend_mode = [
            B::SourceOver,
            B::Multiply,
            B::Screen,
            B::Overlay,
            B::Darken,
            B::Lighten,
            B::ColorDodge,
            B::ColorBurn,
            B::HardLight,
            B::SoftLight,
            B::Difference,
            B::Exclusion,
            B::Hue,
            B::Saturation,
            B::Color,
            B::Luminosity,
        ][mode.min(15) as usize];
        let paint = PixmapPaint {
            quality: FilterQuality::Bilinear,
            blend_mode,
            ..PixmapPaint::default()
        };
        if let Some(t) = self.target.as_mut() {
            t.draw_pixmap(0, 0, image.as_ref().as_ref(), &paint, dev, mask.as_deref());
        }
    }

    fn surface_image(&mut self, image: Arc<Pixmap>, dst: Rect4) {
        self.canvas(&image, dst, &[], Transform::identity());
    }

    fn canvas(&mut self, image: &Arc<Pixmap>, dst: Rect4, clips: &[Shape], ts: Transform) {
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

    fn backdrop_blur(&mut self, shape: &Shape, sigma: f32, ts: Transform) {
        self.blur_backdrop(shape, sigma, ts);
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

    fn push_css_clip(&mut self, css: &exact_kernel::clip::ClipPath, ts: Transform) -> bool {
        let mut b = PathBuilder::new();
        for (op, v) in css.commands() {
            match op {
                'M' => b.move_to(v[0], v[1]),
                'L' => b.line_to(v[0], v[1]),
                'Q' => b.quad_to(v[0], v[1], v[2], v[3]),
                'C' => b.cubic_to(v[0], v[1], v[2], v[3], v[4], v[5]),
                'Z' => b.close(),
                _ => unreachable!("validated CSS path"),
            }
        }
        let Some(path) = b.finish() else {
            return false;
        };
        let Some(mut mask) = Mask::new(self.width, self.height) else {
            return false;
        };
        let rule = match css.rule() {
            exact_kernel::FillRule::Evenodd => FillRule::EvenOdd,
            exact_kernel::FillRule::Nonzero => FillRule::Winding,
        };
        mask.fill_path(&path, rule, true, self.device(ts));
        if let Some(parent) = self.clips.last() {
            intersect_mask(&mut mask, parent, &path, self.device(ts));
        }
        self.clips.push(Rc::new(mask));
        self.text_clips
            .push(self.text_clips.last().copied().unwrap_or((
                0.,
                0.,
                self.width as f32,
                self.height as f32,
            )));
        true
    }

    // @ref LLP 1055.000 D10 — an SVG clip: the union of its shapes as one
    // coverage mask, multiplied by its own clip's and the one in force.
    fn push_svg_clip(&mut self, clip: &exact_kernel::svg::scene::Clip, ts: Transform) -> usize {
        fn union(
            r: &Raster,
            clip: &exact_kernel::svg::scene::Clip,
            dev: Transform,
        ) -> Option<Mask> {
            let mut mask = Mask::new(r.width, r.height)?;
            for shape in &clip.shapes {
                let mut b = PathBuilder::new();
                for seg in &shape.path.0 {
                    match *seg {
                        exact_kernel::svg::Seg::Move(x, y) => b.move_to(x, y),
                        exact_kernel::svg::Seg::Line(x, y) => b.line_to(x, y),
                        exact_kernel::svg::Seg::Cubic(a, c, d, e, x, y) => {
                            b.cubic_to(a, c, d, e, x, y)
                        }
                        exact_kernel::svg::Seg::Close => b.close(),
                    }
                }
                if let Some(path) = b.finish() {
                    let rule = if shape.even_odd {
                        FillRule::EvenOdd
                    } else {
                        FillRule::Winding
                    };
                    mask.fill_path(&path, rule, true, dev);
                }
            }
            if let Some(then) = &clip.then {
                let inner = union(r, then, dev)?;
                multiply(&mut mask, &inner);
            }
            Some(mask)
        }
        let dev = self.device(ts);
        let Some(mut mask) = union(self, clip, dev) else {
            return 0;
        };
        if let Some(parent) = self.clips.last() {
            multiply(&mut mask, parent);
        }
        self.clips.push(Rc::new(mask));
        self.text_clips
            .push(self.text_clips.last().copied().unwrap_or((
                0.,
                0.,
                self.width as f32,
                self.height as f32,
            )));
        1
    }

    fn pop_clip(&mut self) {
        if self
            .rectangular_damage
            .is_some_and(|(depth, _)| depth == self.clips.len())
        {
            self.rectangular_damage = None;
        }
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

    fn push_mask(&mut self, _shape: &Shape, _ts: Transform) {
        self.push_opacity(1.0);
    }

    fn pop_mask(&mut self, shape: &Shape, mask: &Result<GradientPaint, [u8; 4]>, ts: Transform) {
        self.pop_masked(shape, mask, ts);
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
        let (width, height) = (self.width, self.height);
        self.target.take().ok_or_else(|| {
            if width > MAX_FRAME_SIDE || height > MAX_FRAME_SIDE {
                format!("a {width}×{height} frame is past {MAX_FRAME_SIDE} device pixels a side")
            } else {
                "no frame begun".to_string()
            }
        })
    }
}

#[test]
fn a_frame_past_the_largest_side_is_refused_not_allocated() {
    let mut raster = Raster::new();
    // 420×860 points at scale 1000: 1.4 TB were it allocated.
    raster.begin(420.0, 860.0, 1000.0);
    let error = raster.finish().unwrap_err();
    assert!(
        error.contains("420000×860000") && error.contains("16384"),
        "{error}"
    );
    raster.begin(420.0, 860.0, 2.0);
    assert_eq!(
        raster.finish().map(|p| (p.width(), p.height())),
        Ok((840, 1720))
    );
}

#[test]
fn rounded_damage_proof_rejects_invalid_radii() {
    let mut raster = Raster::new();
    raster.begin(96.0, 80.0, 1.0);
    let previous = Pixmap::new(96, 80).unwrap();
    assert!(raster.damage(&previous, &[(24.0, 8.0, 40.0, 64.0)]));
    let mut shape = Shape {
        rect: (0.0, 0.0, 96.0, 80.0),
        radii: [(4.0, 4.0); 4],
        corners: None,
    };
    assert!(raster
        .covered_damage(&shape, Transform::identity())
        .is_some());
    for radius in [f32::NAN, f32::INFINITY, -1.0, 41.0] {
        shape.radii[1] = (radius, radius);
        assert!(raster
            .covered_damage(&shape, Transform::identity())
            .is_none());
    }
}

#[test]
fn css_clip_intersection_matches_full_mask_for_curves_transforms_and_tiling() {
    let mut curve = PathBuilder::new();
    curve.move_to(-12.25, 7.75);
    curve.cubic_to(110.5, -18.25, -35.0, 88.5, 75.25, 64.75);
    curve.quad_to(12.5, 110.25, -12.25, 7.75);
    curve.close();
    let paths = [
        PathBuilder::from_rect(Rect::from_xywh(8.25, 9.5, 27.75, 31.25).unwrap()),
        curve.finish().unwrap(),
    ];
    let transforms = [
        Transform::identity(),
        Transform::from_translate(-35.5, 19.25),
        Transform::from_scale(0.25, 1.75),
        Transform::from_row(-1.0, 0.3, 0.6, 1.1, 50.0, 30.0),
        Transform::from_rotate(37.0),
        Transform::from_translate(9000.0, 0.0),
        Transform::from_scale(0.0, 0.0),
        Transform::from_scale(f32::INFINITY, 1.0),
    ];
    let mut changed = 0;
    for (width, height) in [(64, 51), (137, 93), (8192, 3), (3, 8192)] {
        for path in &paths {
            for dev in transforms {
                let mut original = Mask::new(width, height).unwrap();
                original.fill_path(path, FillRule::Winding, true, dev);
                for kind in 0..3 {
                    let mut parent = Mask::new(width, height).unwrap();
                    for (i, byte) in parent.data_mut().iter_mut().enumerate() {
                        *byte = match kind {
                            0 => 255,
                            1 => (i.wrapping_mul(37) % 256) as u8,
                            _ => {
                                if i % 7 == 0 {
                                    255
                                } else {
                                    0
                                }
                            }
                        };
                    }
                    let mut expected = original.clone();
                    for (a, b) in expected.data_mut().iter_mut().zip(parent.data()) {
                        *a = (u16::from(*a) * u16::from(*b) / 255) as u8;
                    }
                    changed += usize::from(expected.data() != original.data());
                    let mut actual = original.clone();
                    intersect_mask(&mut actual, &parent, path, dev);
                    assert_eq!(actual.data(), expected.data(), "{width}x{height} {dev:?}");
                }
            }
        }
    }
    assert!(changed > 40, "parents must change real nonempty coverage");
}

#[test]
fn nested_shape_masks_match_original_intersections() {
    fn original(raster: &Raster, shapes: &[Shape], ts: Transform) -> Option<Mask> {
        let dev = raster.device(ts);
        let (mut mask, remaining) = if let Some(parent) = raster.clips.last() {
            ((**parent).clone(), shapes)
        } else {
            let mut mask = Mask::new(raster.width, raster.height)?;
            let first = rounded_rect(shapes.first()?)?;
            mask.fill_path(&first, FillRule::Winding, true, dev);
            (mask, &shapes[1..])
        };
        for path in remaining.iter().filter_map(rounded_rect) {
            mask.intersect_path(&path, FillRule::Winding, true, dev);
        }
        Some(mask)
    }
    let invalid = Shape::rect((0.0, 0.0, -1.0, 8.0));
    let first = Shape::new((8.25, 9.5, 47.75, 39.25), [3.25, 7.0, 1.5, 9.0]);
    let second = Shape::new((-3.5, 17.25, 48.0, 55.75), [5.5; 4]);
    let sets: &[&[Shape]] = &[
        &[],
        &[invalid],
        &[invalid, first],
        &[first],
        &[first, second],
        &[first, invalid, second],
    ];
    let mut fast_cases = 0;
    for (width, height) in [(96, 80), (8192, 3), (3, 8192)] {
        for scale in [0.75, 1.0, 2.0] {
            for ts in [
                Transform::identity(),
                Transform::from_translate(-35.5, 19.25),
                Transform::from_row(-1.0, 0.3, 0.6, 1.1, 50.0, 30.0),
                Transform::from_rotate(37.0),
                Transform::from_translate(9000.0, 0.0),
                Transform::from_scale(0.0, 0.0),
                Transform::from_scale(f32::INFINITY, 1.0),
            ] {
                for kind in 0..8 {
                    let mut raster = Raster::new();
                    raster.width = width;
                    raster.height = height;
                    raster.scale = scale;
                    if kind != 0 && kind < 4 {
                        let mut parent = Mask::new(width, height).unwrap();
                        for (i, byte) in parent.data_mut().iter_mut().enumerate() {
                            *byte = match kind {
                                1 => 255,
                                2 => (i.wrapping_mul(37) % 256) as u8,
                                _ => {
                                    if i % 7 == 0 {
                                        255
                                    } else {
                                        0
                                    }
                                }
                            };
                        }
                        raster.clips.push(Rc::new(parent));
                    }
                    if kind >= 4 {
                        let (x, y, w, h) = match kind {
                            4 => (0.0, 0.0, width as f32, height as f32),
                            5 | 7 => (1.0, 1.0, width as f32 - 2.0, height as f32 - 2.0),
                            _ => (
                                -10.0,
                                -5.0,
                                (width / 2) as f32 + 10.0,
                                (height / 2) as f32 + 5.0,
                            ),
                        };
                        let previous = Pixmap::new(width, height).unwrap();
                        assert!(raster
                            .damage(&previous, &[(x / scale, y / scale, w / scale, h / scale)]));
                        if kind == 7 {
                            raster.push_clip(&second, ts);
                        }
                    }
                    let parent_bytes = raster.clips.last().map(|p| p.data().to_vec());
                    for shapes in sets {
                        let expected = original(&raster, shapes, ts);
                        let allocations = raster.clip_allocations.get();
                        let actual = raster.mask_with(shapes, ts);
                        fast_cases +=
                            usize::from(raster.clip_allocations.get() > allocations && kind >= 4);
                        assert_eq!(
                            actual.as_ref().map(Mask::data),
                            expected.as_ref().map(Mask::data),
                            "{width}x{height} scale={scale} {ts:?} parent={kind} shapes={}",
                            shapes.len()
                        );
                        assert_eq!(
                            raster.clips.last().map(|p| p.data()),
                            parent_bytes.as_deref()
                        );
                    }
                }
            }
        }
    }
    assert!(
        fast_cases > 100,
        "rectangular damage must exercise the shortcut"
    );
}
