//! `feTurbulence`: Filter Effects 1 §15.23's reference algorithm, as the
//! spec gives it in C, evaluated at each pixel's user-space point.

use super::{Img, Rect, Space};

const B: usize = 0x100;
const BM: i64 = 0xff;
const PERLIN_N: f64 = 4096.0;
const RAND_M: i64 = 2_147_483_647;
const RAND_A: i64 = 16807;
const RAND_Q: i64 = 127_773;
const RAND_R: i64 = 2836;

/// The primitive's parameters.
pub(crate) struct Params {
    pub(crate) base: (f32, f32),
    pub(crate) octaves: u32,
    pub(crate) seed: f32,
    pub(crate) fractal: bool,
    pub(crate) stitch: bool,
    /// The subregion in user units, the stitching tile.
    pub(crate) tile: (f32, f32, f32, f32),
}

struct Lattice {
    selector: [usize; B + B + 2],
    gradient: Box<[[[f64; 2]; B + B + 2]; 4]>,
}

fn random(seed: i64) -> i64 {
    let r = RAND_A * (seed % RAND_Q) - RAND_R * (seed / RAND_Q);
    if r <= 0 {
        r + RAND_M
    } else {
        r
    }
}

impl Lattice {
    fn new(seed: f32) -> Lattice {
        // SVG: the seed is rounded (Chrome rounds half away from zero).
        let mut seed = seed.round() as i64;
        if seed <= 0 {
            seed = -(seed % (RAND_M - 1)) + 1;
        }
        if seed > RAND_M - 1 {
            seed = RAND_M - 1;
        }
        let mut selector = [0usize; B + B + 2];
        let mut gradient = Box::new([[[0.0f64; 2]; B + B + 2]; 4]);
        for g in gradient.iter_mut() {
            for (i, cell) in g.iter_mut().enumerate().take(B) {
                selector[i] = i;
                for v in cell.iter_mut() {
                    seed = random(seed);
                    *v = ((seed % (B as i64 * 2)) - B as i64) as f64 / B as f64;
                }
                let s = (cell[0] * cell[0] + cell[1] * cell[1]).sqrt();
                if s > 0.0 {
                    cell[0] /= s;
                    cell[1] /= s;
                }
            }
        }
        for i in (1..B).rev() {
            seed = random(seed);
            let j = (seed % B as i64) as usize;
            selector.swap(i, j);
        }
        for i in 0..B + 2 {
            selector[B + i] = selector[i];
            for g in gradient.iter_mut() {
                g[B + i] = g[i];
            }
        }
        Lattice { selector, gradient }
    }

    fn noise2(&self, channel: usize, v: [f64; 2], stitch: Option<&Stitch>) -> f64 {
        let t = v[0] + PERLIN_N;
        let (mut bx0, rx0) = ((t as i64) & BM, t - (t as i64) as f64);
        let mut bx1 = (bx0 + 1) & BM;
        let rx1 = rx0 - 1.0;
        let t = v[1] + PERLIN_N;
        let (mut by0, ry0) = ((t as i64) & BM, t - (t as i64) as f64);
        let mut by1 = (by0 + 1) & BM;
        let ry1 = ry0 - 1.0;
        if let Some(s) = stitch {
            if bx0 >= s.wrap_x {
                bx0 -= s.width;
            }
            if bx1 >= s.wrap_x {
                bx1 -= s.width;
            }
            if by0 >= s.wrap_y {
                by0 -= s.height;
            }
            if by1 >= s.wrap_y {
                by1 -= s.height;
            }
        }
        let (bx0, bx1, by0, by1) = (
            (bx0 & BM) as usize,
            (bx1 & BM) as usize,
            (by0 & BM) as usize,
            (by1 & BM) as usize,
        );
        let i = self.selector[bx0];
        let j = self.selector[bx1];
        let b00 = self.selector[i + by0];
        let b10 = self.selector[j + by0];
        let b01 = self.selector[i + by1];
        let b11 = self.selector[j + by1];
        let curve = |t: f64| t * t * (3.0 - 2.0 * t);
        let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
        let (sx, sy) = (curve(rx0), curve(ry0));
        let g = &self.gradient[channel];
        let u = rx0 * g[b00][0] + ry0 * g[b00][1];
        let v = rx1 * g[b10][0] + ry0 * g[b10][1];
        let a = lerp(sx, u, v);
        let u = rx0 * g[b01][0] + ry1 * g[b01][1];
        let v = rx1 * g[b11][0] + ry1 * g[b11][1];
        let b = lerp(sx, u, v);
        lerp(sy, a, b)
    }
}

#[derive(Clone, Copy)]
struct Stitch {
    width: i64,
    height: i64,
    wrap_x: i64,
    wrap_y: i64,
}

/// The turbulence image over `rect`, in the primitive's colour space.
pub(crate) fn turbulence(w: usize, h: usize, linear: bool, rect: Rect, s: Space, p: Params) -> Img {
    let mut out = Img::clear(w, h, linear);
    let lattice = Lattice::new(p.seed);
    let (mut fx, mut fy) = (p.base.0 as f64, p.base.1 as f64);
    if fx < 0.0 || fy < 0.0 {
        return out;
    }
    let (tx, ty, tw, th) = (
        p.tile.0 as f64,
        p.tile.1 as f64,
        p.tile.2 as f64,
        p.tile.3 as f64,
    );
    let mut stitch = None;
    if p.stitch {
        let adjust = |f: f64, size: f64| {
            if f == 0.0 || size <= 0.0 {
                return f;
            }
            let lo = (size * f).floor() / size;
            let hi = (size * f).ceil() / size;
            if lo > 0.0 && f / lo < hi / f {
                lo
            } else {
                hi
            }
        };
        fx = adjust(fx, tw);
        fy = adjust(fy, th);
        let width = (tw * fx + 0.5) as i64;
        let height = (th * fy + 0.5) as i64;
        stitch = Some(Stitch {
            width,
            height,
            wrap_x: (tx * fx + PERLIN_N) as i64 + width,
            wrap_y: (ty * fy + PERLIN_N) as i64 + height,
        });
    }
    for y in rect.1..rect.3 {
        for x in rect.0..rect.2 {
            let point = [
                s.origin.0 as f64 + x as f64 / s.scale.0 as f64,
                s.origin.1 as f64 + y as f64 / s.scale.1 as f64,
            ];
            let mut c = [0.0f32; 4];
            for (ch, v) in c.iter_mut().enumerate() {
                let mut st = stitch;
                let mut vec = [point[0] * fx, point[1] * fy];
                let (mut sum, mut ratio) = (0.0f64, 1.0f64);
                for _ in 0..p.octaves {
                    let n = lattice.noise2(ch, vec, st.as_ref());
                    sum += if p.fractal { n } else { n.abs() } / ratio;
                    vec = [vec[0] * 2.0, vec[1] * 2.0];
                    ratio *= 2.0;
                    if let Some(s) = st.as_mut() {
                        s.width *= 2;
                        s.wrap_x = 2 * s.wrap_x - PERLIN_N as i64;
                        s.height *= 2;
                        s.wrap_y = 2 * s.wrap_y - PERLIN_N as i64;
                    }
                }
                let value = if p.fractal { (sum + 1.0) / 2.0 } else { sum };
                *v = value.clamp(0.0, 1.0) as f32;
            }
            let i = (y * w + x) * 4;
            let a = c[3];
            out.px[i..i + 4].copy_from_slice(&[c[0] * a, c[1] * a, c[2] * a, a]);
        }
    }
    out
}
