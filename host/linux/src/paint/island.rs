//! SVG islands on Linux (LLP 1055.000 D2, D7, D10): what vello and a clip
//! stack cannot express is painted by a child painter over tiny-skia into
//! pixels, worked on by `exact-svg-raster`, and composited back through
//! [`Backend::surface_image`]: the same path on the CPU and GPU painters.
//! tiny-skia is already this host's; the module is linked in-process here
//! (§8 ruling 4 keeps it out of the Apple core only).

use super::{Painter, Rect4, Shape};
use exact_kernel::svg::scene::{Item, Mask, ShapePaint};
use exact_kernel::svg::transform as tf;
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

/// The largest island side, in device pixels.
const MAX_SIDE: f32 = 4096.0;

/// `r` (x, y, w, h) mapped by `ts`: its bounds.
fn bounds(ts: Transform, r: (f32, f32, f32, f32)) -> Rect4 {
    let mut pts = [
        tiny_skia::Point::from_xy(r.0, r.1),
        tiny_skia::Point::from_xy(r.0 + r.2, r.1),
        tiny_skia::Point::from_xy(r.0, r.1 + r.3),
        tiny_skia::Point::from_xy(r.0 + r.2, r.1 + r.3),
    ];
    ts.map_points(&mut pts);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in pts {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

impl Painter {
    /// Paint into a transparent island covering `rect` (points in the
    /// frame's space, whole device pixels), `paint` given the shift that
    /// maps frame points to island points. `None` when it has no area.
    pub(super) fn island(
        &mut self,
        rect: Rect4,
        paint: impl FnOnce(&mut Painter, Transform),
    ) -> Option<Pixmap> {
        let scale = self.scale;
        let mut child = Painter::new(
            self.text.clone(),
            scale,
            Box::new(crate::raster::Raster::transparent()),
        );
        child.dark = self.dark;
        child.viewport = (rect.2, rect.3);
        child.backend.begin(rect.2, rect.3, scale);
        paint(&mut child, Transform::from_translate(-rect.0, -rect.1));
        child.backend.finish().ok()
    }

    /// `rect` snapped outward to device pixels and limited to the frame.
    fn island_rect(&self, rect: Rect4) -> Option<Rect4> {
        let s = self.scale;
        let (vw, vh) = self.viewport;
        let x0 = (rect.0 * s).floor().max(0.0);
        let y0 = (rect.1 * s).floor().max(0.0);
        let x1 = ((rect.0 + rect.2) * s).ceil().min(vw * s);
        let y1 = ((rect.1 + rect.3) * s).ceil().min(vh * s);
        (x1 > x0 && y1 > y0 && x1 - x0 <= MAX_SIDE && y1 - y0 <= MAX_SIDE)
            .then(|| (x0 / s, y0 / s, (x1 - x0) / s, (y1 - y0) / s))
    }

    /// A masked item (LLP 1055.000 D10): the element and the mask's content
    /// each rendered to an island over the mask region, the content turned
    /// into coverage by luminance or alpha, the element scaled by it.
    pub(super) fn svg_masked(
        &mut self,
        item: &Item,
        mask: &Mask,
        own: Transform,
        origin: Transform,
    ) {
        let Some(rect) = self.island_rect(bounds(own, mask.region)) else {
            return;
        };
        let Some(mut element) = self.island(rect, |p, shift| {
            p.svg_kind(item, shift.pre_concat(own), shift.pre_concat(origin));
        }) else {
            return;
        };
        let Some(mut content) = self.island(rect, |p, shift| {
            let at = shift.pre_concat(own);
            p.backend.push_clip(&Shape::rect(mask.region), at);
            for m in &mask.items {
                p.svg_item(m, at, shift.pre_concat(origin));
            }
            p.backend.pop_clip();
        }) else {
            return;
        };
        exact_svg_raster::mask_coverage(content.data_mut(), mask.luminance);
        exact_svg_raster::apply_coverage(element.data_mut(), content.data(), 4);
        self.backend.surface_image(Arc::new(element), rect);
    }

    /// A pattern paint as an ink (LLP 1055.000 D7): its tile rendered at the
    /// scale it shows at in `space` (points), repeated from the tile's
    /// origin in pattern space.
    pub(super) fn pattern_ink(
        &mut self,
        p: &ShapePaint,
        space: Transform,
        to_path: [f32; 6],
    ) -> Option<super::Ink<'static>> {
        let pat = p.pattern.as_ref()?;
        let (tx, ty, tw, th) = pat.tile;
        // Pattern space to the path's space, then to frame points.
        let m = tf::mul(to_path, pat.transform);
        let d = space.pre_concat(Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5]));
        let k = (d.sx * d.sy - d.kx * d.ky).abs().sqrt() * self.scale;
        if !(k.is_finite() && k > 0.0) {
            return None;
        }
        let pw = (tw * k).ceil().clamp(1.0, MAX_SIDE);
        let ph = (th * k).ceil().clamp(1.0, MAX_SIDE);
        let scale = self.scale;
        let tile = self.island((0.0, 0.0, pw / scale, ph / scale), |child, _| {
            let ts =
                Transform::from_scale(pw / scale / tw, ph / scale / th).pre_translate(-tx, -ty);
            for item in &pat.items {
                child.svg_item(item, ts, ts);
            }
        })?;
        let (w, h) = (tile.width() as f32, tile.height() as f32);
        let pixel = [tw / w, 0.0, 0.0, th / h, tx, ty];
        Some(super::Ink::Pattern {
            tile: Arc::new(tile),
            transform: tf::mul(m, pixel),
            opacity: p.opacity,
        })
    }
}
