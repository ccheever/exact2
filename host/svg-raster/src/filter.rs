//! Filter primitives (LLP 1055.000 D14; Filter Effects 1 §9, §15): a
//! resolved chain run over an island that covers the filter region.
//!
//! Working images are premultiplied RGBA in `f32`, each tagged with its
//! colour space; an input is converted to a primitive's
//! `color-interpolation-filters` space when it is read, and the last result
//! back to sRGB. Pixels outside a primitive's subregion are transparent.

use exact_kernel::svg::filter::{CompositeOp, Filter, Input, Op, Primitive, Transfer};

mod blend;
mod light;
mod noise;

/// A working image.
#[derive(Clone)]
pub(crate) struct Img {
    pub(crate) w: usize,
    pub(crate) h: usize,
    /// Premultiplied RGBA, 0–1.
    pub(crate) px: Vec<f32>,
    /// linearRGB (else sRGB).
    pub(crate) linear: bool,
}

/// A pixel rectangle: x0, y0, x1, y1 (exclusive).
pub(crate) type Rect = (usize, usize, usize, usize);

impl Img {
    pub(crate) fn clear(w: usize, h: usize, linear: bool) -> Img {
        Img {
            w,
            h,
            px: vec![0.0; w * h * 4],
            linear,
        }
    }

    /// The image in the other colour space, when it is not already there.
    fn to_space(&self, linear: bool) -> Img {
        if self.linear == linear {
            return self.clone();
        }
        let mut out = self.clone();
        out.linear = linear;
        for p in out.px.chunks_exact_mut(4) {
            let a = p[3];
            if a <= 0.0 {
                continue;
            }
            for c in &mut p[..3] {
                let v = (*c / a).clamp(0.0, 1.0);
                *c = if linear { to_linear(v) } else { to_srgb(v) } * a;
            }
        }
        out
    }

    /// Transparent outside `r`.
    fn keep(&mut self, r: Rect) {
        for y in 0..self.h {
            for x in 0..self.w {
                if x < r.0 || x >= r.2 || y < r.1 || y >= r.3 {
                    let i = (y * self.w + x) * 4;
                    self.px[i..i + 4].fill(0.0);
                }
            }
        }
    }
}

/// sRGB to linear light (one channel, 0–1).
pub(crate) fn to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light to sRGB.
pub(crate) fn to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Where the island is: the filter region's origin in user units, and
/// pixels per user unit on each axis.
#[derive(Debug, Clone, Copy)]
pub struct Space {
    /// The region's x, y in user units: the island's (0, 0).
    pub origin: (f32, f32),
    /// Pixels per user unit.
    pub scale: (f32, f32),
}

/// Run `filter` over `pixels` (premultiplied sRGB RGBA8, `w × h`, covering
/// the filter region), in place: the element as drawn in, the filtered
/// element out.
pub fn run(filter: &Filter, pixels: &mut [u8], w: usize, h: usize, space: Space) {
    if pixels.len() != w * h * 4 {
        return;
    }
    let mut source = Img::clear(w, h, false);
    for (d, s) in source.px.iter_mut().zip(pixels.iter()) {
        *d = *s as f32 / 255.0;
    }
    let mut results: Vec<(Img, Rect)> = Vec::with_capacity(filter.primitives.len());
    for p in &filter.primitives {
        let rect = subregion(p, space, w, h);
        let mut out = primitive(p, &source, &results, rect, space);
        out.keep(rect);
        results.push((out, rect));
    }
    let last = results.pop().map(|r| r.0.to_space(false));
    for (i, d) in pixels.iter_mut().enumerate() {
        *d = last
            .as_ref()
            .map_or(0, |l| (l.px[i].clamp(0.0, 1.0) * 255.0).round() as u8);
    }
}

/// A primitive's subregion in pixels, within the island.
fn subregion(p: &Primitive, s: Space, w: usize, h: usize) -> Rect {
    let (x, y, sw, sh) = p.subregion;
    let px =
        |v: f32, o: f32, k: f32, max: usize| ((v - o) * k).round().clamp(0.0, max as f32) as usize;
    (
        px(x, s.origin.0, s.scale.0, w),
        px(y, s.origin.1, s.scale.1, h),
        px(x + sw, s.origin.0, s.scale.0, w),
        px(y + sh, s.origin.1, s.scale.1, h),
    )
}

fn input(i: Input, source: &Img, results: &[(Img, Rect)], linear: bool) -> Img {
    match i {
        Input::SourceGraphic => source.to_space(linear),
        Input::SourceAlpha => {
            let mut a = Img::clear(source.w, source.h, linear);
            for (d, s) in a.px.chunks_exact_mut(4).zip(source.px.chunks_exact(4)) {
                d[3] = s[3];
            }
            a
        }
        Input::Result(n) => match results.get(n as usize) {
            Some((img, _)) => img.to_space(linear),
            None => Img::clear(source.w, source.h, linear),
        },
        Input::None => Img::clear(source.w, source.h, linear),
    }
}

fn primitive(p: &Primitive, source: &Img, results: &[(Img, Rect)], rect: Rect, s: Space) -> Img {
    let lin = p.linear;
    let a = || input(p.inputs[0], source, results, lin);
    let b = || input(p.inputs[1], source, results, lin);
    let (w, h) = (source.w, source.h);
    match &p.op {
        Op::Blur(sx, sy) => blur(a(), sx * s.scale.0, sy * s.scale.1),
        Op::Offset(dx, dy) => offset(&a(), dx * s.scale.0, dy * s.scale.1),
        Op::Flood(c) => flood(w, h, *c, lin, rect),
        Op::Composite(op, k) => composite(&a(), &b(), *op, *k),
        Op::Merge(inputs) => {
            let mut out = Img::clear(w, h, lin);
            for i in inputs {
                out = composite(
                    &input(*i, source, results, lin),
                    &out,
                    CompositeOp::Over,
                    [0.0; 4],
                );
            }
            out
        }
        Op::ColorMatrix(m) => unpremultiplied(a(), |c| {
            let mut o = [0.0; 4];
            for (r, v) in o.iter_mut().enumerate() {
                let row = &m[r * 5..r * 5 + 5];
                *v = row[0] * c[0] + row[1] * c[1] + row[2] * c[2] + row[3] * c[3] + row[4];
            }
            o
        }),
        Op::DropShadow(sx, sy, dx, dy, color) => {
            let src = a();
            let mut alpha = src.clone();
            for px in alpha.px.chunks_exact_mut(4) {
                px[..3].fill(0.0);
            }
            let blurred = blur(alpha, sx * s.scale.0, sy * s.scale.1);
            let moved = offset(&blurred, dx * s.scale.0, dy * s.scale.1);
            let tint = flood(w, h, *color, lin, (0, 0, w, h));
            let shadow = composite(&tint, &moved, CompositeOp::In, [0.0; 4]);
            composite(&src, &shadow, CompositeOp::Over, [0.0; 4])
        }
        Op::Blend(mode) => blend::blend(&a(), &b(), *mode),
        Op::Morphology(dilate, rx, ry) => morphology(a(), *dilate, rx * s.scale.0, ry * s.scale.1),
        Op::ComponentTransfer(funcs) => unpremultiplied(a(), |c| {
            let mut o = c;
            for (v, f) in o.iter_mut().zip(funcs.iter()) {
                *v = transfer(f, *v);
            }
            o
        }),
        Op::Tile => {
            let src = a();
            let from = match p.inputs[0] {
                Input::Result(n) => results.get(n as usize).map_or((0, 0, w, h), |r| r.1),
                _ => (0, 0, w, h),
            };
            tile(&src, from, rect)
        }
        Op::Turbulence(bx, by, octaves, seed, fractal, stitch) => noise::turbulence(
            w,
            h,
            lin,
            rect,
            s,
            noise::Params {
                base: (*bx, *by),
                octaves: *octaves,
                seed: *seed,
                fractal: *fractal,
                stitch: *stitch,
                tile: p.subregion,
            },
        ),
        Op::Displacement(scale, xc, yc) => {
            displace(&a(), &b(), scale * s.scale.0, scale * s.scale.1, *xc, *yc)
        }
        Op::Convolve(c) => light::convolve(&a(), c),
        Op::Lighting(l) => light::lighting(&a(), l, lin, s),
    }
}

/// A σ per axis in pixels, as Filter Effects 1 §15.6 says: three box blurs
/// when σ ≥ 2, else a true Gaussian kernel.
fn blur(mut img: Img, sx: f32, sy: f32) -> Img {
    if sx < 0.0 || sy < 0.0 || !(sx.is_finite() && sy.is_finite()) {
        return Img::clear(img.w, img.h, img.linear);
    }
    let (w, h) = (img.w, img.h);
    if sx > 0.0 {
        pass(&mut img.px, w, h, 4, w * 4, sx);
    }
    if sy > 0.0 {
        pass(&mut img.px, h, w, w * 4, 4, sy);
    }
    img
}

/// Blur `lines` lines of `len` pixels, `step` floats apart along a line and
/// `stride` floats between lines' starts.
fn pass(px: &mut [f32], len: usize, lines: usize, step: usize, stride: usize, sigma: f32) {
    let mut line = vec![0.0f32; len * 4];
    let mut tmp = vec![0.0f32; len * 4];
    for l in 0..lines {
        let base = l * stride;
        for i in 0..len {
            line[i * 4..i * 4 + 4].copy_from_slice(&px[base + i * step..base + i * step + 4]);
        }
        if sigma >= 2.0 {
            let d =
                (sigma * 3.0 * (2.0 * std::f32::consts::PI).sqrt() / 4.0 + 0.5).floor() as usize;
            let d = d.max(1);
            if d % 2 == 1 {
                let r = (d - 1) / 2;
                for _ in 0..3 {
                    boxed(&line, &mut tmp, len, r, r);
                    std::mem::swap(&mut line, &mut tmp);
                }
            } else {
                boxed(&line, &mut tmp, len, d / 2, d / 2 - 1);
                boxed(&tmp, &mut line, len, d / 2 - 1, d / 2);
                boxed(&line, &mut tmp, len, d / 2, d / 2);
                std::mem::swap(&mut line, &mut tmp);
            }
        } else {
            let r = (sigma * 3.0).ceil() as isize;
            let weights: Vec<f32> = (-r..=r)
                .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
                .collect();
            let total: f32 = weights.iter().sum();
            for i in 0..len as isize {
                let mut acc = [0.0f32; 4];
                for (k, wgt) in weights.iter().enumerate() {
                    let j = i + k as isize - r;
                    if j < 0 || j >= len as isize {
                        continue;
                    }
                    for c in 0..4 {
                        acc[c] += line[j as usize * 4 + c] * wgt;
                    }
                }
                for c in 0..4 {
                    tmp[i as usize * 4 + c] = acc[c] / total;
                }
            }
            std::mem::swap(&mut line, &mut tmp);
        }
        for i in 0..len {
            px[base + i * step..base + i * step + 4].copy_from_slice(&line[i * 4..i * 4 + 4]);
        }
    }
}

/// A box blur over `[i − left, i + right]`, transparent past the ends.
fn boxed(src: &[f32], dst: &mut [f32], len: usize, left: usize, right: usize) {
    let size = (left + right + 1) as f32;
    let mut acc = [0.0f32; 4];
    // The window for i = 0 is [−left, right].
    for j in 0..=right.min(len.saturating_sub(1)) {
        for c in 0..4 {
            acc[c] += src[j * 4 + c];
        }
    }
    for i in 0..len {
        for c in 0..4 {
            dst[i * 4 + c] = acc[c] / size;
        }
        let add = i + right + 1;
        if add < len {
            for c in 0..4 {
                acc[c] += src[add * 4 + c];
            }
        }
        if i >= left {
            let sub = i - left;
            for c in 0..4 {
                acc[c] -= src[sub * 4 + c];
            }
        }
    }
}

fn offset(img: &Img, dx: f32, dy: f32) -> Img {
    let (dx, dy) = (dx.round() as isize, dy.round() as isize);
    let mut out = Img::clear(img.w, img.h, img.linear);
    for y in 0..img.h as isize {
        let sy = y - dy;
        if sy < 0 || sy >= img.h as isize {
            continue;
        }
        for x in 0..img.w as isize {
            let sx = x - dx;
            if sx < 0 || sx >= img.w as isize {
                continue;
            }
            let (d, s) = (
                ((y as usize) * img.w + x as usize) * 4,
                ((sy as usize) * img.w + sx as usize) * 4,
            );
            out.px[d..d + 4].copy_from_slice(&img.px[s..s + 4]);
        }
    }
    out
}

/// A straight sRGB colour over `rect`, in the primitive's space.
fn flood(w: usize, h: usize, c: [f32; 4], linear: bool, rect: Rect) -> Img {
    let mut out = Img::clear(w, h, linear);
    let conv = |v: f32| if linear { to_linear(v) } else { v };
    let a = c[3].clamp(0.0, 1.0);
    let px = [conv(c[0]) * a, conv(c[1]) * a, conv(c[2]) * a, a];
    for y in rect.1..rect.3 {
        for x in rect.0..rect.2 {
            let i = (y * w + x) * 4;
            out.px[i..i + 4].copy_from_slice(&px);
        }
    }
    out
}

/// `a` (in) composited with `b` (in2).
pub(crate) fn composite(a: &Img, b: &Img, op: CompositeOp, k: [f32; 4]) -> Img {
    let mut out = Img::clear(a.w, a.h, a.linear);
    for ((o, s), d) in out
        .px
        .chunks_exact_mut(4)
        .zip(a.px.chunks_exact(4))
        .zip(b.px.chunks_exact(4))
    {
        let (sa, da) = (s[3], d[3]);
        for c in 0..4 {
            o[c] = match op {
                CompositeOp::Over => s[c] + d[c] * (1.0 - sa),
                CompositeOp::In => s[c] * da,
                CompositeOp::Out => s[c] * (1.0 - da),
                CompositeOp::Atop => s[c] * da + d[c] * (1.0 - sa),
                CompositeOp::Xor => s[c] * (1.0 - da) + d[c] * (1.0 - sa),
                CompositeOp::Lighter => (s[c] + d[c]).min(1.0),
                CompositeOp::Arithmetic => {
                    (k[0] * s[c] * d[c] + k[1] * s[c] + k[2] * d[c] + k[3]).clamp(0.0, 1.0)
                }
            };
        }
        if op == CompositeOp::Arithmetic {
            for c in 0..3 {
                o[c] = o[c].min(o[3]);
            }
        }
    }
    out
}

/// `f` over each pixel's unpremultiplied colour; the result clamped and
/// premultiplied again.
fn unpremultiplied(mut img: Img, f: impl Fn([f32; 4]) -> [f32; 4]) -> Img {
    for p in img.px.chunks_exact_mut(4) {
        let a = p[3];
        let c = if a > 0.0 {
            [p[0] / a, p[1] / a, p[2] / a, a]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        };
        let o = f(c).map(|v| v.clamp(0.0, 1.0));
        p.copy_from_slice(&[o[0] * o[3], o[1] * o[3], o[2] * o[3], o[3]]);
    }
    img
}

fn transfer(f: &Transfer, c: f32) -> f32 {
    match f {
        Transfer::Identity => c,
        Transfer::Table(v) if v.len() > 1 => {
            let n = v.len() - 1;
            let k = ((c * n as f32).floor() as usize).min(n - 1);
            let t = c * n as f32 - k as f32;
            v[k] + (v[k + 1] - v[k]) * t
        }
        Transfer::Table(v) | Transfer::Discrete(v) if v.len() == 1 => v[0],
        Transfer::Table(_) => c,
        Transfer::Discrete(v) if v.is_empty() => c,
        Transfer::Discrete(v) => {
            let n = v.len();
            v[((c * n as f32).floor() as usize).min(n - 1)]
        }
        Transfer::Linear(slope, intercept) => slope * c + intercept,
        Transfer::Gamma(amp, exp, off) => amp * c.powf(*exp) + off,
    }
}

/// Erode (min) or dilate (max) over a box of radius `rx × ry` pixels.
fn morphology(img: Img, dilate: bool, rx: f32, ry: f32) -> Img {
    if rx < 0.0 || ry < 0.0 {
        return Img::clear(img.w, img.h, img.linear);
    }
    let (rx, ry) = (rx.round() as isize, ry.round() as isize);
    let (w, h) = (img.w as isize, img.h as isize);
    let pick = |a: f32, b: f32| if dilate { a.max(b) } else { a.min(b) };
    let start = if dilate { 0.0 } else { 1.0 };
    let mut mid = Img::clear(img.w, img.h, img.linear);
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let mut v = start;
                for k in -rx..=rx {
                    let sx = x + k;
                    let s = if (0..w).contains(&sx) {
                        img.px[((y * w + sx) * 4 + c) as usize]
                    } else {
                        0.0
                    };
                    v = pick(v, s);
                }
                mid.px[((y * w + x) * 4 + c) as usize] = v;
            }
        }
    }
    let mut out = Img::clear(img.w, img.h, img.linear);
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let mut v = start;
                for k in -ry..=ry {
                    let sy = y + k;
                    let s = if (0..h).contains(&sy) {
                        mid.px[((sy * w + x) * 4 + c) as usize]
                    } else {
                        0.0
                    };
                    v = pick(v, s);
                }
                out.px[((y * w + x) * 4 + c) as usize] = v;
            }
        }
    }
    out
}

/// `src`'s `from` rectangle repeated over `to`, from `from`'s corner.
fn tile(src: &Img, from: Rect, to: Rect) -> Img {
    let mut out = Img::clear(src.w, src.h, src.linear);
    let (fw, fh) = (from.2.saturating_sub(from.0), from.3.saturating_sub(from.1));
    if fw == 0 || fh == 0 {
        return out;
    }
    for y in to.1..to.3 {
        let sy = from.1 + (y + fh - from.1 % fh) % fh;
        for x in to.0..to.2 {
            let sx = from.0 + (x + fw - from.0 % fw) % fw;
            let (d, s) = ((y * src.w + x) * 4, (sy * src.w + sx) * 4);
            out.px[d..d + 4].copy_from_slice(&src.px[s..s + 4]);
        }
    }
    out
}

/// `feDisplacementMap`: `img` sampled at each pixel moved by `map`'s
/// unpremultiplied channels, `scale` pixels per unit of displacement.
fn displace(img: &Img, map: &Img, kx: f32, ky: f32, xc: u8, yc: u8) -> Img {
    let mut out = Img::clear(img.w, img.h, img.linear);
    for y in 0..img.h {
        for x in 0..img.w {
            let i = (y * img.w + x) * 4;
            let a = map.px[i + 3];
            let ch = |c: u8| {
                if c == 3 {
                    a
                } else if a > 0.0 {
                    map.px[i + c as usize] / a
                } else {
                    0.0
                }
            };
            let sx = (x as f32 + kx * (ch(xc) - 0.5)).round() as isize;
            let sy = (y as f32 + ky * (ch(yc) - 0.5)).round() as isize;
            if sx < 0 || sy < 0 || sx >= img.w as isize || sy >= img.h as isize {
                continue;
            }
            let s = (sy as usize * img.w + sx as usize) * 4;
            out.px[i..i + 4].copy_from_slice(&img.px[s..s + 4]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::svg::filter::{Light, Lighting};

    fn one(op: Op, linear: bool) -> Filter {
        Filter {
            region: (0.0, 0.0, 20.0, 20.0),
            primitives: vec![Primitive {
                op,
                inputs: [Input::SourceGraphic, Input::None],
                subregion: (0.0, 0.0, 20.0, 20.0),
                linear,
            }],
        }
    }

    fn square() -> Vec<u8> {
        let mut px = vec![0u8; 20 * 20 * 4];
        for y in 5..15 {
            for x in 5..15 {
                px[(y * 20 + x) * 4..(y * 20 + x) * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        px
    }

    const SPACE: Space = Space {
        origin: (0.0, 0.0),
        scale: (1.0, 1.0),
    };

    /// The wire form round-trips every op.
    #[test]
    fn the_chain_crosses_as_numbers() {
        let ops = vec![
            Op::Blur(2.0, 1.0),
            Op::Offset(1.0, -2.0),
            Op::Flood([0.1, 0.2, 0.3, 0.5]),
            Op::Composite(CompositeOp::Arithmetic, [0.0, 0.5, 0.5, 0.0]),
            Op::Merge(vec![Input::SourceAlpha, Input::Result(0)]),
            Op::ColorMatrix([0.5; 20]),
            Op::Blend(3),
            Op::Morphology(true, 1.0, 2.0),
            Op::ComponentTransfer(Box::new([
                Transfer::Identity,
                Transfer::Table(vec![0.0, 1.0]),
                Transfer::Linear(2.0, 0.1),
                Transfer::Gamma(1.0, 2.0, 0.0),
            ])),
            Op::Tile,
            Op::Turbulence(0.05, 0.1, 2, 3.0, true, false),
            Op::Displacement(4.0, 0, 1),
            Op::Lighting(Box::new(Lighting {
                specular: true,
                surface_scale: 2.0,
                constant: 1.0,
                exponent: 8.0,
                color: [1.0, 1.0, 1.0],
                light: Light::Spot([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 1.0, None),
            })),
        ];
        let f = Filter {
            region: (1.0, 2.0, 3.0, 4.0),
            primitives: ops
                .into_iter()
                .map(|op| Primitive {
                    op,
                    inputs: [Input::SourceGraphic, Input::Result(0)],
                    subregion: (1.0, 2.0, 3.0, 4.0),
                    linear: true,
                })
                .collect(),
        };
        assert_eq!(Filter::decode(&f.encode()), Some(f));
        assert_eq!(Filter::decode(&[0.0, 0.0, 1.0, 1.0, 1.0, 99.0]), None);
    }

    /// A blur keeps the total coverage and spreads it; an offset moves it.
    #[test]
    fn blur_spreads_and_offset_moves() {
        let mut px = square();
        run(&one(Op::Blur(2.0, 2.0), false), &mut px, 20, 20, SPACE);
        let alpha: u32 = px.chunks_exact(4).map(|p| p[3] as u32).sum();
        assert!(
            (alpha as f32 / (100.0 * 255.0) - 1.0).abs() < 0.02,
            "{alpha}"
        );
        assert!(px[(10 * 20 + 3) * 4 + 3] > 0, "spread past the edge");
        let mut px = square();
        run(&one(Op::Offset(3.0, 0.0), false), &mut px, 20, 20, SPACE);
        assert_eq!(px[(10 * 20 + 6) * 4 + 3], 0);
        assert_eq!(px[(10 * 20 + 17) * 4 + 3], 255);
    }

    /// A colour matrix works on unpremultiplied sRGB; linearRGB round-trips.
    #[test]
    fn color_matrix_and_spaces() {
        let mut m = [0.0; 20];
        m[2] = 1.0; // R ← B
        m[5] = 1.0; // G ← R
        m[18] = 1.0;
        for linear in [false, true] {
            let mut px = square();
            run(&one(Op::ColorMatrix(m), linear), &mut px, 20, 20, SPACE);
            let i = (10 * 20 + 10) * 4;
            assert_eq!(&px[i..i + 4], &[0, 255, 0, 255]);
        }
    }
}
