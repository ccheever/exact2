//! `feConvolveMatrix` and the lighting primitives (Filter Effects 1
//! §15.10, §15.14, §15.20, §15.22): neighbourhoods over the input.

use super::{to_linear, Img, Space};
use exact_svg_filter::{Convolve, Light, Lighting};

/// `feConvolveMatrix`: the kernel, turned by 180° as the spec writes it,
/// over premultiplied colour (or unpremultiplied colour with alpha kept).
pub(crate) fn convolve(img: &Img, c: &Convolve) -> Img {
    let (w, h) = (img.w as isize, img.h as isize);
    let (ox, oy) = (c.order.0 as isize, c.order.1 as isize);
    let mut out = Img::clear(img.w, img.h, img.linear);
    if c.kernel.len() != (ox * oy) as usize || c.divisor == 0.0 {
        return out;
    }
    let sample = |x: isize, y: isize| -> Option<[f32; 4]> {
        let (x, y) = match c.edge {
            0 => (x.clamp(0, w - 1), y.clamp(0, h - 1)),
            1 => (x.rem_euclid(w), y.rem_euclid(h)),
            _ if x < 0 || y < 0 || x >= w || y >= h => return None,
            _ => (x, y),
        };
        let i = ((y * w + x) * 4) as usize;
        let p = &img.px[i..i + 4];
        Some(if c.preserve_alpha {
            let a = p[3];
            if a > 0.0 {
                [p[0] / a, p[1] / a, p[2] / a, a]
            } else {
                [0.0; 4]
            }
        } else {
            [p[0], p[1], p[2], p[3]]
        })
    };
    let (tx, ty) = (c.target.0 as isize, c.target.1 as isize);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            for i in 0..oy {
                for j in 0..ox {
                    let Some(s) = sample(x - tx + j, y - ty + i) else {
                        continue;
                    };
                    let k = c.kernel[((oy - 1 - i) * ox + (ox - 1 - j)) as usize];
                    for ch in 0..4 {
                        acc[ch] += s[ch] * k;
                    }
                }
            }
            let o = ((y * w + x) * 4) as usize;
            let a0 = img.px[o + 3];
            if c.preserve_alpha {
                let a = a0;
                for (d, v) in out.px[o..o + 3].iter_mut().zip(acc) {
                    *d = ((v / c.divisor + c.bias).clamp(0.0, 1.0)) * a;
                }
                out.px[o + 3] = a;
            } else {
                let a = (acc[3] / c.divisor + c.bias).clamp(0.0, 1.0);
                for (d, v) in out.px[o..o + 3].iter_mut().zip(acc) {
                    *d = (v / c.divisor + c.bias * a).clamp(0.0, a);
                }
                out.px[o + 3] = a;
            }
        }
    }
    out
}

/// Diffuse or specular lighting of the input's alpha as a height map.
pub(crate) fn lighting(img: &Img, l: &Lighting, linear: bool, s: Space) -> Img {
    let (w, h) = (img.w as isize, img.h as isize);
    let mut out = Img::clear(img.w, img.h, linear);
    let alpha =
        |x: isize, y: isize| img.px[((y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) * 4 + 3) as usize];
    let color = if linear {
        l.color.map(to_linear)
    } else {
        l.color
    };
    let zk = (s.scale.0 * s.scale.1).sqrt();
    let at = |x: f32, y: f32, z: f32| {
        [
            (x - s.origin.0) * s.scale.0,
            (y - s.origin.1) * s.scale.1,
            z * zk,
        ]
    };
    let norm = |v: [f32; 3]| {
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if n > 0.0 {
            v.map(|c| c / n)
        } else {
            v
        }
    };
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let ss = l.surface_scale;
    for y in 0..h {
        for x in 0..w {
            // Sobel over the alpha, the spec's interior kernels, the edges
            // duplicated; slopes and heights per user unit, so the surface
            // is the one a pixel per unit would light, at any scale.
            let nx = -ss * s.scale.0 / 4.0
                * ((alpha(x + 1, y - 1) + 2.0 * alpha(x + 1, y) + alpha(x + 1, y + 1))
                    - (alpha(x - 1, y - 1) + 2.0 * alpha(x - 1, y) + alpha(x - 1, y + 1)));
            let ny = -ss * s.scale.1 / 4.0
                * ((alpha(x - 1, y + 1) + 2.0 * alpha(x, y + 1) + alpha(x + 1, y + 1))
                    - (alpha(x - 1, y - 1) + 2.0 * alpha(x, y - 1) + alpha(x + 1, y - 1)));
            let n = norm([nx, ny, 1.0]);
            let z = ss * alpha(x, y) * zk;
            let (lv, lc) = match l.light {
                Light::Distant(az, el) => {
                    let (az, el) = (az.to_radians(), el.to_radians());
                    ([az.cos() * el.cos(), az.sin() * el.cos(), el.sin()], color)
                }
                Light::Point(lx, ly, lz) => {
                    let p = at(lx, ly, lz);
                    (norm([p[0] - x as f32, p[1] - y as f32, p[2] - z]), color)
                }
                Light::Spot(p, exp, cone) => {
                    let from = at(p[0], p[1], p[2]);
                    let to = at(p[3], p[4], p[5]);
                    let lv = norm([from[0] - x as f32, from[1] - y as f32, from[2] - z]);
                    let sd = norm([to[0] - from[0], to[1] - from[1], to[2] - from[2]]);
                    let minus = -dot(lv, sd);
                    let lit = cone.is_none_or(|c| minus >= c.to_radians().cos());
                    let k = if lit && minus > 0.0 {
                        minus.powf(exp)
                    } else {
                        0.0
                    };
                    (lv, color.map(|c| c * k))
                }
            };
            let o = ((y * w + x) * 4) as usize;
            if l.specular {
                let hv = norm([lv[0], lv[1], lv[2] + 1.0]);
                let k = l.constant * dot(n, hv).max(0.0).powf(l.exponent);
                let c = lc.map(|c| (c * k).clamp(0.0, 1.0));
                let a = c[0].max(c[1]).max(c[2]);
                out.px[o..o + 4].copy_from_slice(&[c[0], c[1], c[2], a]);
            } else {
                let k = l.constant * dot(n, lv).max(0.0);
                let c = lc.map(|c| (c * k).clamp(0.0, 1.0));
                out.px[o..o + 4].copy_from_slice(&[c[0], c[1], c[2], 1.0]);
            }
        }
    }
    out
}
