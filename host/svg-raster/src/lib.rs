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

#[cfg(test)]
mod tests {
    use super::*;

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
