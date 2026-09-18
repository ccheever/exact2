//! Projective composition of a Contract child; the same map drives paint and hit.
use crate::paint::Rect4;
use tiny_skia::{Pixmap, Transform};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Placement {
    Hidden,
    Visible {
        h: [f32; 9],
        depth: f32,
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
pub(crate) fn bounds(h: &[f32; 9], r: Rect4) -> Rect4 {
    let corners = [
        (r.0, r.1),
        (r.0 + r.2, r.1),
        (r.0, r.1 + r.3),
        (r.0 + r.2, r.1 + r.3),
    ]
    .map(|(x, y)| map(h, x, y));
    let lo = corners.iter().fold((f32::INFINITY, f32::INFINITY), |a, p| {
        (a.0.min(p.0), a.1.min(p.1))
    });
    let hi = corners
        .iter()
        .fold((f32::NEG_INFINITY, f32::NEG_INFINITY), |a, p| {
            (a.0.max(p.0), a.1.max(p.1))
        });
    (lo.0, lo.1, hi.0 - lo.0, hi.1 - lo.1)
}
/// Inverse-map each destination pixel through the full homography, then sample
/// premultiplied RGBA bilinearly. There is no affine/projective approximation.
/// Bounding allocation is clipped to the viewport, independently of perspective.
pub(crate) fn warp(
    source: &Pixmap,
    h: [f32; 9],
    scale: f32,
    viewport: (f32, f32),
) -> Option<(Pixmap, Rect4)> {
    let inv = inverse(h)?;
    let b = bounds(
        &h,
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
        let (x, y) = (x * scale - 0.5, y * scale - 0.5);
        if x < -1. || y < -1. || x >= source.width() as f32 || y >= source.height() as f32 {
            continue;
        }
        let (ix, iy) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let mut rgba = [0.; 4];
        for (dx, dy, weight) in [
            (0, 0, (1. - fx) * (1. - fy)),
            (1, 0, fx * (1. - fy)),
            (0, 1, (1. - fx) * fy),
            (1, 1, fx * fy),
        ] {
            let (sx, sy) = (ix + dx, iy + dy);
            if sx < 0 || sy < 0 || sx >= source.width() as i32 || sy >= source.height() as i32 {
                continue;
            }
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
