//! Projective composition of a Contract child; the same map drives paint and hit.
use crate::paint::Rect4;
use tiny_skia::{Pixmap, Transform};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Placement {
    Hidden,
    Visible {
        h: [f32; 9],
        depth: f32,
        clip_depth: [[f32; 3]; 2],
        canvas: u32,
    },
}
impl Placement {
    pub(crate) fn depth(self) -> f32 {
        match self {
            Self::Hidden => 0.,
            Self::Visible { depth, .. } => depth,
        }
    }
}
pub(crate) fn map(h: &[f32; 9], x: f32, y: f32) -> (f32, f32) {
    let w = h[6] * x + h[7] * y + h[8];
    (
        (h[0] * x + h[1] * y + h[2]) / w,
        (h[3] * x + h[4] * y + h[5]) / w,
    )
}
pub(crate) fn inverse(h: [f32; 9]) -> Option<[f32; 9]> {
    let [a, b, c, d, e, f, g, i, j] = h;
    let out = [
        e * j - f * i,
        c * i - b * j,
        b * f - c * e,
        f * g - d * j,
        a * j - c * g,
        c * d - a * f,
        d * i - e * g,
        b * g - a * i,
        a * e - b * d,
    ];
    let det = a * out[0] + b * out[3] + c * out[6];
    (det.is_finite() && det.abs() > 1e-12).then(|| out.map(|n| n / det))
}
pub(crate) fn compose(h: [f32; 9], ts: Transform, x: f32, y: f32) -> [f32; 9] {
    let tx = ts.sx * x + ts.kx * y + ts.tx;
    let ty = ts.ky * x + ts.sy * y + ts.ty;
    let mut out = h;
    for c in 0..3 {
        out[c] = ts.sx * h[c] + ts.kx * h[3 + c] + tx * h[6 + c];
        out[3 + c] = ts.ky * h[c] + ts.sy * h[3 + c] + ty * h[6 + c];
    }
    out
}
/// Inverse-map each destination pixel through the full homography, then sample
/// premultiplied RGBA bilinearly. There is no affine/projective approximation.
/// Bounding allocation is clipped to the viewport, independently of perspective.
#[cfg(test)]
pub(crate) fn warp(
    source: &Pixmap,
    h: [f32; 9],
    scale: f32,
    viewport: (f32, f32),
) -> Option<(Pixmap, Rect4)> {
    warp_clipped(source, h, [[0., 0., 1.]; 2], scale, viewport)
}
pub(crate) fn accepts(planes: &[[f32; 3]; 2], x: f32, y: f32) -> bool {
    planes.iter().all(|p| p[0] * x + p[1] * y + p[2] >= 0.)
}
pub(crate) fn clipped_bounds(h: &[f32; 9], planes: [[f32; 3]; 2], r: Rect4) -> Rect4 {
    let mut points = vec![
        (r.0, r.1),
        (r.0 + r.2, r.1),
        (r.0 + r.2, r.1 + r.3),
        (r.0, r.1 + r.3),
    ];
    for p in [planes[0], planes[1], [h[6], h[7], h[8] - 1e-6]] {
        let mut next = Vec::new();
        let Some(mut a) = points.last().copied() else {
            return (0., 0., 0., 0.);
        };
        let d = |v: (f32, f32)| p[0] * v.0 + p[1] * v.1 + p[2];
        for &b in &points {
            let (da, db) = (d(a), d(b));
            if (da >= 0.) != (db >= 0.) {
                let t = da / (da - db);
                next.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            }
            if db >= 0. {
                next.push(b);
            }
            a = b;
        }
        points = next;
    }
    if points.is_empty() {
        return (0., 0., 0., 0.);
    }
    let mut lo = (f32::INFINITY, f32::INFINITY);
    let mut hi = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (x, y) in points {
        let q = map(h, x, y);
        lo = (lo.0.min(q.0), lo.1.min(q.1));
        hi = (hi.0.max(q.0), hi.1.max(q.1));
    }
    (lo.0, lo.1, hi.0 - lo.0, hi.1 - lo.1)
}
pub(crate) fn warp_clipped(
    source: &Pixmap,
    h: [f32; 9],
    planes: [[f32; 3]; 2],
    scale: f32,
    viewport: (f32, f32),
) -> Option<(Pixmap, Rect4)> {
    let inv = inverse(h)?;
    let b = clipped_bounds(
        &h,
        planes,
        (
            0.,
            0.,
            source.width() as f32 / scale,
            source.height() as f32 / scale,
        ),
    );
    if ![b.0, b.1, b.2, b.3].iter().all(|n| n.is_finite()) {
        return None;
    }
    let x0 = (b.0 * scale).floor().max(0.);
    let y0 = (b.1 * scale).floor().max(0.);
    let x1 = ((b.0 + b.2) * scale).ceil().min(viewport.0 * scale);
    let y1 = ((b.1 + b.3) * scale).ceil().min(viewport.1 * scale);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut out = Pixmap::new((x1 - x0) as u32, (y1 - y0) as u32)?;
    let width = out.width();
    for (i, pixel) in out.data_mut().chunks_exact_mut(4).enumerate() {
        let (x, y) = map(
            &inv,
            (x0 + (i as u32 % width) as f32 + 0.5) / scale,
            (y0 + (i as u32 / width) as f32 + 0.5) / scale,
        );
        if !accepts(&planes, x, y) || h[6] * x + h[7] * y + h[8] <= 0. {
            continue;
        }
        let (x, y) = (x * scale, y * scale);
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.
            || y < 0.
            || x >= source.width() as f32
            || y >= source.height() as f32
        {
            continue;
        }
        let (x, y) = (x - 0.5, y - 0.5);
        let (ix, iy) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let mut rgba = [0.; 4];
        for (dx, dy, weight) in [
            (0, 0, (1. - fx) * (1. - fy)),
            (1, 0, fx * (1. - fy)),
            (0, 1, (1. - fx) * fy),
            (1, 1, fx * fy),
        ] {
            let (sx, sy) = (
                (ix + dx).clamp(0, source.width() as i32 - 1),
                (iy + dy).clamp(0, source.height() as i32 - 1),
            );
            let index = (sy as usize * source.width() as usize + sx as usize) * 4;
            for (c, value) in rgba.iter_mut().enumerate() {
                *value += source.data()[index + c] as f32 * weight;
            }
        }
        for c in 0..4 {
            pixel[c] = rgba[c].round() as u8;
        }
    }
    Some((
        out,
        (x0 / scale, y0 / scale, (x1 - x0) / scale, (y1 - y0) / scale),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn near_clipped_nameplate_retains_visible_pixels_and_rejects_clipped_hits() {
        let mut src = Pixmap::new(100, 50).unwrap();
        src.fill(tiny_skia::Color::WHITE);
        let h = [1., 0., 0., 0., 1., 0., 0., 0., 1.];
        let clip = [[1., 0., -50.], [0., 0., 1.]];
        let (out, rect) = warp_clipped(&src, h, clip, 1., (100., 50.)).unwrap();
        assert_eq!(rect, (50., 0., 50., 50.));
        assert!(out.data().chunks_exact(4).all(|p| p == [255; 4]));
        assert_eq!(
            clipped_bounds(&h, clip, (0., 0., 40., 50.)),
            (0., 0., 0., 0.)
        );
        assert!(!accepts(&clip, 49., 25.));
        assert!(accepts(&clip, 51., 25.));
        // An eye crossing is bounded by near before projecting, never by the invalid full quad.
        let h = [1., 0., 0., 0., 1., 0., 0.02, 0., -0.5];
        let (out, rect) =
            warp_clipped(&src, h, [[1., 0., -30.], [0., 0., 1.]], 1., (400., 400.)).unwrap();
        assert!(out.data().chunks_exact(4).any(|p| p[3] != 0));
        assert!(rect.0.is_finite() && rect.2 > 0.);
    }
    #[test]
    fn opaque_texel_scaled_to_two_pixels_has_no_transparent_border() {
        let mut src = Pixmap::new(1, 1).unwrap();
        src.fill(tiny_skia::Color::from_rgba8(200, 50, 10, 255));
        let (out, _) = warp(&src, [2., 0., 0., 0., 2., 0., 0., 0., 1.], 1., (2., 2.)).unwrap();
        assert!(
            out.data().chunks_exact(4).all(|p| p == [200, 50, 10, 255]),
            "{:?}",
            out.data()
        );
    }

    #[test]
    fn projective_quad_preserves_four_distinct_corners() {
        let mut src = Pixmap::new(40, 40).unwrap();
        let colors = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
        ];
        for (i, p) in src.data_mut().chunks_exact_mut(4).enumerate() {
            p.copy_from_slice(&colors[usize::from(i % 40 >= 20) + 2 * usize::from(i / 40 >= 20)]);
        }
        let h = [2., 0.3, 10., 0.2, 2., 10., 0.02, 0.005, 1.];
        let (out, b) = warp(&src, h, 1., (100., 100.)).unwrap();
        for ((x, y), color) in [(2., 2.), (38., 2.), (2., 38.), (38., 38.)]
            .into_iter()
            .zip(colors)
        {
            let (x, y) = map(&h, x, y);
            let p = out.pixel((x - b.0) as u32, (y - b.1) as u32).unwrap();
            assert_eq!([p.red(), p.green(), p.blue(), p.alpha()], color);
        }
    }

    #[test]
    fn perspective_round_trip_and_quad_pixels() {
        let h = [1.2, 0.2, 10., 0.1, 1., 20., 0.004, 0.002, 1.];
        let inv = inverse(h).unwrap();
        for (x, y) in [(0., 0.), (80., 0.), (80., 40.), (0., 40.), (23., 17.)] {
            let p = map(&h, x, y);
            let q = map(&inv, p.0, p.1);
            assert!((q.0 - x).abs() < 0.001 && (q.1 - y).abs() < 0.001);
        }
        let mut src = Pixmap::new(80, 40).unwrap();
        src.fill(tiny_skia::Color::from_rgba8(200, 50, 10, 255));
        let (pixels, b) = warp(&src, h, 1., (150., 100.)).unwrap();
        let p = map(&h, 40., 20.);
        let pixel = pixels
            .pixel((p.0 - b.0) as u32, (p.1 - b.1) as u32)
            .unwrap();
        assert_eq!(
            (pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()),
            (200, 50, 10, 255)
        );
    }
}
