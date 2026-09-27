//! `feBlend` and `mix-blend-mode`'s arithmetic (Compositing and Blending 1
//! §5, §10): the source over the backdrop with a blend function, on
//! premultiplied colour.

use super::Img;

/// A separable mode's B(cb, cs), unpremultiplied, 0–1.
fn separable(mode: u8, cb: f32, cs: f32) -> f32 {
    let hard = |cb: f32, cs: f32| {
        if cs <= 0.5 {
            cb * 2.0 * cs
        } else {
            let s = 2.0 * cs - 1.0;
            cb + s - cb * s
        }
    };
    match mode {
        1 => cb * cs,
        2 => cb + cs - cb * cs,
        3 => hard(cs, cb),
        4 => cb.min(cs),
        5 => cb.max(cs),
        6 => {
            if cb == 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        }
        7 => {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        }
        8 => hard(cb, cs),
        9 => {
            if cs <= 0.5 {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            } else {
                let d = if cb <= 0.25 {
                    ((16.0 * cb - 12.0) * cb + 4.0) * cb
                } else {
                    cb.sqrt()
                };
                cb + (2.0 * cs - 1.0) * (d - cb)
            }
        }
        10 => (cb - cs).abs(),
        11 => cb + cs - 2.0 * cb * cs,
        _ => cs,
    }
}

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut c = c;
    if n < 0.0 {
        c = c.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 {
        c = c.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    c
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip(c.map(|v| v + d))
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let (mut idx, mut out) = ([0usize, 1, 2], [0.0f32; 3]);
    idx.sort_by(|a, b| c[*a].total_cmp(&c[*b]));
    let (lo, mid, hi) = (idx[0], idx[1], idx[2]);
    if c[hi] > c[lo] {
        out[mid] = (c[mid] - c[lo]) * s / (c[hi] - c[lo]);
        out[hi] = s;
    }
    out[lo] = 0.0;
    out
}

/// A non-separable mode's B(cb, cs).
fn non_separable(mode: u8, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    match mode {
        12 => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        13 => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        14 => set_lum(cs, lum(cb)),
        _ => set_lum(cb, lum(cs)),
    }
}

/// `src` (the source, `in`) blended over `dst` (the backdrop, `in2`) with
/// mode `mode` (an index into the kernel's `BLEND_MODES`).
pub(crate) fn blend(src: &Img, dst: &Img, mode: u8) -> Img {
    let mut out = Img::clear(src.w, src.h, src.linear);
    for ((o, s), d) in out
        .px
        .chunks_exact_mut(4)
        .zip(src.px.chunks_exact(4))
        .zip(dst.px.chunks_exact(4))
    {
        blend_pixel(o, s, d, mode);
    }
    out
}

/// One pixel of [`blend`], premultiplied in and out.
pub(crate) fn blend_pixel(o: &mut [f32], s: &[f32], d: &[f32], mode: u8) {
    let (sa, da) = (s[3], d[3]);
    let un = |p: &[f32], a: f32| {
        if a > 0.0 {
            [p[0] / a, p[1] / a, p[2] / a]
        } else {
            [0.0; 3]
        }
    };
    let (cs, cb) = (un(s, sa), un(d, da));
    let mixed = if mode >= 12 {
        non_separable(mode, cb, cs)
    } else {
        [0, 1, 2].map(|c| separable(mode, cb[c], cs[c]))
    };
    for c in 0..3 {
        o[c] = (1.0 - da) * s[c] + (1.0 - sa) * d[c] + sa * da * mixed[c].clamp(0.0, 1.0);
    }
    o[3] = sa + da - sa * da;
}
