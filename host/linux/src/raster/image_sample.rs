//! Axis-aligned image sampling with tiny-skia 0.12's highp arithmetic.
//! Device coordinates stay unchanged; only the repeated horizontal lookup is shared.
use tiny_skia::{Mask, Pixmap, PixmapRef, Transform};

#[derive(Clone, Copy)]
pub(super) struct Sample {
    at: [usize; 2],
    weight: [f32; 2],
}

fn sample(center: f32, extent: u32, nearest: bool) -> Sample {
    let limit = f32::from_bits((extent as f32).to_bits() - 1);
    let index = |at: f32| at.max(0.).min(limit) as usize;
    if nearest {
        return Sample {
            at: [index(center); 2],
            weight: [1., 0.],
        };
    }
    let fraction = (center + 0.5).fract();
    let first = center - 0.5;
    Sample {
        at: [index(first), index(first + 1.)],
        weight: [1. - fraction, fraction],
    }
}

pub(super) fn draw(
    target: &mut Pixmap,
    source: PixmapRef<'_>,
    transform: Transform,
    mask: Option<&Mask>,
    horizontal: &mut Vec<Sample>,
) -> bool {
    if !transform.is_finite()
        || transform.kx != 0.
        || transform.ky != 0.
        || transform.sx <= 0.
        || transform.sy <= 0.
        || target.width() > 8191
        || target.height() > 8191
        || source.width() > 8191
        || source.height() > 8191
        || mask.is_some_and(|m| m.width() != target.width() || m.height() != target.height())
    {
        return false;
    }
    let edges = [
        transform.tx,
        transform.ty,
        source.width() as f32 * transform.sx + transform.tx,
        source.height() as f32 * transform.sy + transform.ty,
    ];
    if !edges
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 8191. && v.fract() == 0.)
    {
        return false;
    }
    let Some(inverse) = transform.invert().filter(Transform::is_finite) else {
        return false;
    };
    let left = edges[0].max(0.).min(target.width() as f32) as usize;
    let top = edges[1].max(0.).min(target.height() as f32) as usize;
    let right = edges[2].max(0.).min(target.width() as f32) as usize;
    let bottom = edges[3].max(0.).min(target.height() as f32) as usize;
    if right <= left || bottom <= top {
        return true;
    }
    let nearest = inverse.is_identity() || inverse.is_translate();
    horizontal.clear();
    horizontal.extend((left..right).map(|x| {
        sample(
            (x as f32 + 0.5) * inverse.sx + inverse.tx,
            source.width(),
            nearest,
        )
    }));
    let stride = target.width() as usize;
    let source_stride = source.width() as usize;
    let source_height = source.height();
    let source = source.data();
    for y in top..bottom {
        let vertical = sample(
            (y as f32 + 0.5) * inverse.sy + inverse.ty,
            source_height,
            nearest,
        );
        let first = vertical.at[0] * source_stride;
        let second = vertical.at[1] * source_stride;
        let row_start = y * stride + left;
        let coverage = mask.map(|m| &m.data()[row_start..row_start + horizontal.len()]);
        let row = &mut target.data_mut()[row_start * 4..(row_start + horizontal.len()) * 4];
        for (x, (out, horizontal)) in row.chunks_exact_mut(4).zip(horizontal.iter()).enumerate() {
            let cover = coverage.map_or(255, |m| m[x]);
            if cover == 0 {
                continue;
            }
            let taps = [
                first + horizontal.at[0],
                first + horizontal.at[1],
                second + horizontal.at[0],
                second + horizontal.at[1],
            ];
            let weights = [
                horizontal.weight[0] * vertical.weight[0],
                horizontal.weight[1] * vertical.weight[0],
                horizontal.weight[0] * vertical.weight[1],
                horizontal.weight[1] * vertical.weight[1],
            ];
            blend(out, source, taps, weights, cover, nearest);
        }
    }
    true
}

fn blend(
    out: &mut [u8],
    source: &[u8],
    taps: [usize; 4],
    weights: [f32; 4],
    coverage: u8,
    nearest: bool,
) {
    use std::arch::x86_64::*;
    // SSE2 is baseline on x86-64. Pixel loads use checked four-byte slices;
    // all SIMD values stay in registers, with no pointer loads or stores.
    #[allow(unsafe_code)]
    unsafe {
        let zero = _mm_setzero_si128();
        let factor = _mm_set1_ps(1. / 255.);
        let load = |pixel: &[u8]| {
            let rgba = _mm_cvtsi32_si128(i32::from_ne_bytes(pixel.try_into().expect("RGBA")));
            let rgba = _mm_unpacklo_epi16(_mm_unpacklo_epi8(rgba, zero), zero);
            _mm_mul_ps(_mm_cvtepi32_ps(rgba), factor)
        };
        let mut color = _mm_setzero_ps();
        for i in 0..if nearest { 1 } else { 4 } {
            let pixel = load(&source[taps[i] * 4..taps[i] * 4 + 4]);
            color = _mm_add_ps(_mm_mul_ps(_mm_set1_ps(weights[i]), pixel), color);
        }
        color = _mm_mul_ps(color, _mm_set1_ps(coverage as f32 / 255.));
        let inverse = _mm_sub_ps(_mm_set1_ps(1.), _mm_shuffle_ps::<255>(color, color));
        let result = _mm_add_ps(_mm_mul_ps(load(out), inverse), color);
        let result = _mm_min_ps(_mm_max_ps(result, _mm_setzero_ps()), _mm_set1_ps(1.));
        let result = _mm_cvtps_epi32(_mm_mul_ps(result, _mm_set1_ps(255.)));
        let result = _mm_packus_epi16(_mm_packs_epi32(result, zero), zero);
        out.copy_from_slice(&_mm_cvtsi128_si32(result).to_ne_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{Color, FilterQuality, PixmapPaint};

    fn pixels(width: u32, height: u32, seed: u32) -> Pixmap {
        let mut image = Pixmap::new(width, height).unwrap();
        let mut random = seed;
        for pixel in image.pixels_mut() {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            let [r, g, b, a] = random.to_le_bytes();
            *pixel = Color::from_rgba8(r, g, b, a).premultiply().to_color_u8();
        }
        image
    }

    #[test]
    fn sampled_images_match_reference_with_alpha_clips_and_device_positions() {
        let source = pixels(96, 64, 1);
        let mut samples = Vec::new();
        for (width, height) in [(96., 64.), (48., 32.), (80., 48.), (144., 96.), (120., 80.)] {
            for (x, y) in [(0., 0.), (7., 13.), (-9., -15.), (40., 70.), (64., -65.)] {
                for clipped in [false, true] {
                    let mut actual = pixels(160, 120, 2);
                    let mut expected = actual.clone();
                    let mut mask = Mask::new(160, 120).unwrap();
                    for (i, pixel) in mask.data_mut().iter_mut().enumerate() {
                        *pixel = (i % 256) as u8;
                    }
                    let mask = clipped.then_some(&mask);
                    let transform =
                        Transform::from_translate(x, y).pre_scale(width / 96., height / 64.);
                    assert!(draw(
                        &mut actual,
                        source.as_ref(),
                        transform,
                        mask,
                        &mut samples
                    ));
                    expected.draw_pixmap(
                        0,
                        0,
                        source.as_ref(),
                        &PixmapPaint {
                            quality: FilterQuality::Bilinear,
                            ..Default::default()
                        },
                        transform,
                        mask,
                    );
                    let differences: Vec<_> = actual
                        .pixels()
                        .iter()
                        .zip(expected.pixels())
                        .enumerate()
                        .filter(|(_, (a, b))| a != b)
                        .take(5)
                        .collect();
                    assert!(
                        differences.is_empty(),
                        "size={width}x{height} origin={x},{y} clipped={clipped}: {differences:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn fractional_or_turned_images_fall_back_without_writing() {
        let source = pixels(32, 24, 1);
        let target = pixels(48, 48, 2);
        let mut samples = Vec::new();
        for transform in [
            Transform::from_translate(0.25, 1.),
            Transform::from_rotate(15.),
            Transform::from_scale(-1., 1.),
            Transform::from_scale(0., 1.),
        ] {
            let mut actual = target.clone();
            assert!(!draw(
                &mut actual,
                source.as_ref(),
                transform,
                None,
                &mut samples
            ));
            assert_eq!(actual.data(), target.data());
        }
    }
}
