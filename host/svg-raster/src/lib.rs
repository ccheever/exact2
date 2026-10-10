//! SVG islands (LLP 1055.000 D2, §3, §8 ruling 4): the pixel work native
//! hosts cannot hand to their compositor, as pure functions over
//! premultiplied RGBA8, row-major and tightly packed.
//!
//! Each host rasterizes an island's source with the rasterizer it already
//! has (Core Animation's own `render(in:)` on Apple, tiny-skia on Linux), so
//! the source antialiases as the rest of that host's SVG does and text comes
//! along. This crate does what comes after: turning a mask's content into
//! coverage, and (stage 9) running a filter's primitives. On Apple it is a
//! separately loaded module (`abi`); on Linux a plain dependency.

pub mod abi;
pub mod document;
pub mod filter;

/// CSS Masking 1 §7.1's luminance coefficients (sRGB), applied once to
/// premultiplied colour, so a translucent white masks by its alpha.
const LUMINANCE: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// A mask's rendered content turned into coverage, in place: every channel
/// of each pixel becomes the coverage (so the image is an opaque-white mask
/// scaled by coverage, premultiplied, usable as an alpha mask). `luminance`
/// takes the luminance of the premultiplied colour (`mask-type:
/// luminance`); otherwise the alpha (`mask-type: alpha`).
pub fn mask_coverage(pixels: &mut [u8], luminance: bool) {
    for px in pixels.chunks_exact_mut(4) {
        let c = if luminance {
            let l = LUMINANCE[0] * px[0] as f32
                + LUMINANCE[1] * px[1] as f32
                + LUMINANCE[2] * px[2] as f32;
            (l.round() as u32).min(px[3] as u32) as u8
        } else {
            px[3]
        };
        px.copy_from_slice(&[c, c, c, c]);
    }
}

/// `target` (premultiplied RGBA) scaled by `coverage` (one byte per pixel,
/// or the alpha of a [`mask_coverage`] image when `stride` is 4), in place.
pub fn apply_coverage(target: &mut [u8], coverage: &[u8], stride: usize) {
    for (px, c) in target
        .chunks_exact_mut(4)
        .zip(coverage.chunks_exact(stride))
    {
        let k = c[stride - 1] as u32;
        for v in px.iter_mut() {
            *v = ((*v as u32 * k + 127) / 255) as u8;
        }
    }
}

/// CSS `backdrop-filter: blur(σ)` over a backdrop (LLP 1053.000 D2), in
/// place: premultiplied RGBA8, `w` × `h`, σ in pixels. The Gaussian is
/// feFilter's three box blurs (Filter Effects 1 §15.6), over encoded sRGB
/// as Chrome blurs, and the backdrop's edges mirror outward, as Chrome's
/// backdrop filter reads past them, so nothing fades in from outside.
pub fn backdrop_blur(pixels: &mut [u8], w: usize, h: usize, sigma: f32) {
    if !(sigma.is_finite() && sigma > 0.0) || w == 0 || h == 0 || pixels.len() < w * h * 4 {
        return;
    }
    let pad = (3.0 * sigma).ceil() as usize + 2;
    let (pw, ph) = (w + 2 * pad, h + 2 * pad);
    // A mirror index: …2 1 0 | 0 1 2 … n-1 | n-1 n-2…
    let mirror = |i: isize, n: usize| -> usize {
        let n = n as isize;
        let period = 2 * n;
        let m = i.rem_euclid(period);
        (if m < n { m } else { period - 1 - m }) as usize
    };
    let mut img = filter::Img::clear(pw, ph, false);
    for y in 0..ph {
        let sy = mirror(y as isize - pad as isize, h);
        for x in 0..pw {
            let sx = mirror(x as isize - pad as isize, w);
            let from = (sy * w + sx) * 4;
            let to = (y * pw + x) * 4;
            for c in 0..4 {
                img.px[to + c] = pixels[from + c] as f32 / 255.0;
            }
        }
    }
    let img = filter::blur(img, sigma, sigma);
    for y in 0..h {
        for x in 0..w {
            let from = ((y + pad) * pw + x + pad) * 4;
            let to = (y * w + x) * 4;
            let a = img.px[from + 3].clamp(0.0, 1.0);
            for c in 0..4 {
                // Premultiplied stays premultiplied: no channel above alpha.
                let v = img.px[from + c].clamp(0.0, if c == 3 { 1.0 } else { a });
                pixels[to + c] = (v * 255.0 + 0.5) as u8;
            }
        }
    }
}

/// CSS `saturate()` (Filter Effects 1 §9.6) in encoded sRGB. The matrix is
/// linear, so it can act directly on premultiplied RGB; clamp to alpha after
/// each operation, leaving alpha unchanged. Amounts above one remain allowed.
pub fn backdrop_saturate(pixels: &mut [u8], amount: f32) {
    if !amount.is_finite() || amount < 0.0 || amount == 1.0 {
        return;
    }
    for px in pixels.chunks_exact_mut(4) {
        let gray = 0.213 * px[0] as f64 + 0.715 * px[1] as f64 + 0.072 * px[2] as f64;
        let alpha = px[3] as f64;
        for channel in &mut px[..3] {
            *channel = (gray + amount as f64 * (*channel as f64 - gray))
                .clamp(0.0, alpha)
                .round() as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backdrop_blur_spreads_an_edge_and_keeps_a_flat_field() {
        // Black | white, 40 × 4: the edge spreads, the flat ends stay put
        // (mirrored edges bring nothing in from outside).
        let (w, h) = (40, 4);
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let v = if x < w / 2 { 0 } else { 255 };
                px[(y * w + x) * 4..][..4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        backdrop_blur(&mut px, w, h, 3.0);
        let at = |x: usize| px[(w + x) * 4];
        assert_eq!(at(0), 0);
        assert_eq!(at(w - 1), 255);
        // Symmetric about the edge between pixels 19 and 20.
        assert!((250..=260).contains(&(at(w / 2 - 1) as u32 + at(w / 2) as u32)));
        assert!(at(w / 2 - 3) > 0 && at(w / 2 + 2) < 255);
        assert!(px.chunks_exact(4).all(|p| p[3] == 255));
    }

    #[test]
    fn backdrop_saturation_preserves_alpha_and_clamps_each_result() {
        let original = [255, 0, 0, 255, 0, 128, 0, 128, 40, 80, 120, 255];
        let mut px = original;
        backdrop_saturate(&mut px, 0.0);
        assert_eq!(px, [54, 54, 54, 255, 92, 92, 92, 128, 74, 74, 74, 255]);
        let mut px = original;
        backdrop_saturate(&mut px, 1.0);
        assert_eq!(px, original);
        backdrop_saturate(&mut px, 2.0);
        assert_eq!(px, [255, 0, 0, 255, 0, 128, 0, 128, 6, 86, 166, 255]);
    }

    #[test]
    fn luminance_and_alpha_coverage() {
        // Opaque white, opaque black, half-transparent white, opaque green.
        let mut px = vec![
            255, 255, 255, 255, 0, 0, 0, 255, 128, 128, 128, 128, 0, 255, 0, 255,
        ];
        let mut alpha = px.clone();
        mask_coverage(&mut px, true);
        assert_eq!(
            px.iter().step_by(4).copied().collect::<Vec<_>>(),
            vec![255, 0, 128, 182]
        );
        mask_coverage(&mut alpha, false);
        assert_eq!(
            alpha.iter().step_by(4).copied().collect::<Vec<_>>(),
            vec![255, 255, 128, 255]
        );
        let mut target = vec![200, 100, 50, 255];
        apply_coverage(&mut target, &[128, 128, 128, 128], 4);
        assert_eq!(target, vec![100, 50, 25, 128]);
    }
}
