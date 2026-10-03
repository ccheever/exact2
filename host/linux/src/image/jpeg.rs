//! JPEG: the header read here (the frame's SOF marker, no decoder), the pixels
//! by the platform's decoder where there is one — Android's `AImageDecoder`
//! (libjpeg-turbo, sampled to the plan's size while decoding, as the
//! platform's own image views decode). Other targets refuse JPEG as before.
//!
//! @ref LLP 1076 §3.4 (the Rust painter on Android)
use super::png_decode::{DecodePlan, Header};
use exact_raster::{Metadata, PixelSize, Refusal, MAX_HEADER_BYTES, MAX_SOURCE_PIXELS};
use std::io::{Read, Seek, SeekFrom};
use tiny_skia::Pixmap;

/// Whether these leading bytes are a JPEG's SOI.
pub(super) fn is_jpeg(head: &[u8]) -> bool {
    head.len() >= 3 && head[0] == 0xFF && head[1] == 0xD8 && head[2] == 0xFF
}

/// The natural size from the first frame header, reading at most
/// `MAX_HEADER_BYTES`.
pub(super) fn inspect(
    input: &mut (impl Read + Seek),
    encoded_bytes: u64,
) -> Result<Header, Refusal> {
    input
        .seek(SeekFrom::Start(2))
        .map_err(|_| Refusal::DecodeFailed)?;
    let mut at = 2u64;
    loop {
        if at + 4 > MAX_HEADER_BYTES || at + 4 > encoded_bytes {
            return Err(Refusal::HeaderLimit);
        }
        let mut marker = [0u8; 2];
        input
            .read_exact(&mut marker)
            .map_err(|_| Refusal::DecodeFailed)?;
        at += 2;
        if marker[0] != 0xFF {
            return Err(Refusal::DecodeFailed);
        }
        let m = marker[1];
        if m == 0xFF {
            // Fill byte: the marker follows.
            input
                .seek(SeekFrom::Current(-1))
                .map_err(|_| Refusal::DecodeFailed)?;
            at -= 1;
            continue;
        }
        if m == 0xD8 || (0xD0..=0xD7).contains(&m) || m == 0x01 {
            continue;
        }
        if m == 0xD9 || m == 0xDA {
            return Err(Refusal::DecodeFailed);
        }
        let mut len = [0u8; 2];
        input
            .read_exact(&mut len)
            .map_err(|_| Refusal::DecodeFailed)?;
        let len = u64::from(u16::from_be_bytes(len));
        if len < 2 {
            return Err(Refusal::DecodeFailed);
        }
        let sof = (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC;
        if sof {
            let mut f = [0u8; 6];
            input
                .read_exact(&mut f)
                .map_err(|_| Refusal::DecodeFailed)?;
            let height = u32::from(u16::from_be_bytes([f[1], f[2]]));
            let width = u32::from(u16::from_be_bytes([f[3], f[4]]));
            if width == 0 || height == 0 {
                return Err(Refusal::InvalidDimensions);
            }
            if u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
                return Err(Refusal::SourcePixels);
            }
            return Ok(Header::jpeg(Metadata {
                natural: PixelSize { width, height },
                encoded_bytes,
                header_bytes: at + len,
            }));
        }
        at += len;
        input
            .seek(SeekFrom::Start(at))
            .map_err(|_| Refusal::DecodeFailed)?;
    }
}

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod platform {
    use std::ffi::c_void;
    #[link(name = "jnigraphics")]
    extern "C" {
        pub fn AImageDecoder_createFromBuffer(
            buffer: *const c_void,
            length: usize,
            out: *mut *mut c_void,
        ) -> i32;
        pub fn AImageDecoder_setAndroidBitmapFormat(decoder: *mut c_void, format: i32) -> i32;
        pub fn AImageDecoder_setTargetSize(decoder: *mut c_void, width: i32, height: i32) -> i32;
        pub fn AImageDecoder_computeSampledSize(
            decoder: *mut c_void,
            sample_size: i32,
            width: *mut i32,
            height: *mut i32,
        ) -> i32;
        pub fn AImageDecoder_decodeImage(
            decoder: *mut c_void,
            pixels: *mut c_void,
            stride: usize,
            size: usize,
        ) -> i32;
        pub fn AImageDecoder_delete(decoder: *mut c_void);
    }
}

#[cfg(target_os = "android")]
thread_local! {
    /// The decoding thread's buffer for a reduction it resamples.
    static SAMPLED: std::cell::Cell<Vec<u8>> = const { std::cell::Cell::new(Vec::new()) };
}

/// Decode at the plan's size. The whole file is read (it is bounded by
/// `MAX_ENCODED_BYTES`) because the platform decoder takes a buffer.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
pub(super) fn decode<R: Read + Seek>(
    mut input: R,
    plan: &DecodePlan,
    cancelled: impl Fn() -> bool,
) -> Result<Pixmap, Refusal> {
    use platform::*;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| Refusal::DecodeFailed)?;
    let mut bytes = Vec::with_capacity(plan.header.metadata.encoded_bytes as usize);
    input
        .take(plan.header.metadata.encoded_bytes)
        .read_to_end(&mut bytes)
        .map_err(|_| Refusal::DecodeFailed)?;
    if cancelled() {
        return Err(Refusal::Stale);
    }
    let (w, h) = (plan.pixels.width, plan.pixels.height);
    crate::android::section_begin(c"exact jpeg decode");
    struct End;
    impl Drop for End {
        fn drop(&mut self) {
            crate::android::section_end();
        }
    }
    let _end = End;
    let mut decoder = std::ptr::null_mut();
    // SAFETY: the buffer outlives the decoder, which is deleted on every path;
    // the output holds `stride × height` bytes, as decodeImage is told.
    let decoded = unsafe {
        if AImageDecoder_createFromBuffer(bytes.as_ptr().cast(), bytes.len(), &mut decoder) != 0 {
            return Err(Refusal::DecodeFailed);
        }
        // libjpeg-turbo reduces by a power of two while it decodes, for
        // nothing; any other size is that decode and a resample after it.
        // Decode at the smallest such reduction still covering the plan
        // and resample here (`resample`), not in the platform's scaler.
        let natural = plan.header.metadata.natural;
        let mut sampled = (natural.width as i32, natural.height as i32);
        let mut sample = 2;
        loop {
            let (mut sw, mut sh) = (0, 0);
            if AImageDecoder_computeSampledSize(decoder, sample, &mut sw, &mut sh) != 0
                || sw < w as i32
                || sh < h as i32
            {
                break;
            }
            sampled = (sw, sh);
            sample *= 2;
        }
        let (sw, sh) = (sampled.0 as u32, sampled.1 as u32);
        let stride = sw as usize * 4;
        let direct = (sw, sh) == (w, h);
        // A decode to resample goes through this thread's one buffer, kept
        // between decodes: a fresh one per decode stays mapped once freed.
        let mut out = if direct {
            vec![0u8; stride * sh as usize]
        } else {
            SAMPLED.take()
        };
        out.resize(stride * sh as usize, 0);
        let s = if AImageDecoder_setAndroidBitmapFormat(decoder, 1) != 0
            || AImageDecoder_setTargetSize(decoder, sampled.0, sampled.1) != 0
        {
            -1
        } else {
            AImageDecoder_decodeImage(decoder, out.as_mut_ptr().cast(), stride, out.len())
        };
        AImageDecoder_delete(decoder);
        (s == 0).then_some((out, sw, sh))
    };
    let Some((out, sw, sh)) = decoded else {
        return Err(Refusal::DecodeFailed);
    };
    let out = if (sw, sh) == (w, h) {
        out
    } else {
        crate::android::section_begin(c"exact jpeg resample");
        let r = resample(&out, (sw, sh), (w, h));
        crate::android::section_end();
        SAMPLED.set(out);
        r
    };
    tiny_skia::IntSize::from_wh(w, h)
        .and_then(|size| Pixmap::from_vec(out, size))
        .ok_or(Refusal::DecodeFailed)
}

/// RGBA `src` (`from`) resampled to `to`, at most 2× smaller on each axis:
/// bilinear, rows then columns (the filter the platform's scaler uses), two
/// channels per 32-bit lane, holding only the two source rows a row needs.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(super) fn resample(src: &[u8], from: (u32, u32), to: (u32, u32)) -> Vec<u8> {
    // Each output coordinate's two source taps and the second's weight (/256).
    fn taps(from: u32, to: u32) -> Vec<(usize, usize, u32)> {
        let scale = from as f32 / to as f32;
        (0..to)
            .map(|i| {
                let x = ((i as f32 + 0.5) * scale - 0.5).max(0.);
                let x0 = (x as u32).min(from - 1);
                let x1 = (x0 + 1).min(from - 1);
                let f = ((x - x0 as f32) * 256.).round().min(256.) as u32;
                (x0 as usize, x1 as usize, f)
            })
            .collect()
    }
    // a·(256−f) + b·f over 256, for the 8-bit channels at bits 0 and 16.
    fn mix(a: u32, b: u32, f: u32) -> u32 {
        ((a & 0x00FF_00FF) * (256 - f) + (b & 0x00FF_00FF) * f) >> 8 & 0x00FF_00FF
    }
    fn lerp(a: u32, b: u32, f: u32) -> u32 {
        mix(a, b, f) | mix(a >> 8, b >> 8, f) << 8
    }
    let (xs, ys) = (taps(from.0, to.0), taps(from.1, to.1));
    let pixels = |y: usize| {
        src[y * from.0 as usize * 4..(y + 1) * from.0 as usize * 4]
            .chunks_exact(4)
            .map(|p| u32::from_le_bytes([p[0], p[1], p[2], p[3]]))
    };
    let mut line = Vec::with_capacity(from.0 as usize);
    let mut across = |y: usize, row: &mut Vec<u32>| {
        line.clear();
        line.extend(pixels(y));
        row.clear();
        row.extend(xs.iter().map(|&(x0, x1, f)| lerp(line[x0], line[x1], f)));
    };
    let (mut upper, mut lower) = (Vec::new(), Vec::new());
    let mut held = (usize::MAX, usize::MAX);
    let mut out = Vec::with_capacity(to.0 as usize * to.1 as usize * 4);
    for &(y0, y1, f) in &ys {
        if held.0 != y0 {
            if held.1 == y0 {
                std::mem::swap(&mut upper, &mut lower);
            } else {
                across(y0, &mut upper);
            }
            held.0 = y0;
        }
        if held.1 != y1 {
            across(y1, &mut lower);
            held.1 = y1;
        }
        out.extend(
            upper
                .iter()
                .zip(&lower)
                .flat_map(|(&a, &b)| lerp(a, b, f).to_le_bytes()),
        );
    }
    out
}

/// No platform decoder on this target: JPEG is refused, as before.
#[cfg(not(target_os = "android"))]
pub(super) fn decode<R: Read + Seek>(
    _input: R,
    _plan: &DecodePlan,
    _cancelled: impl Fn() -> bool,
) -> Result<Pixmap, Refusal> {
    Err(Refusal::DecodeFailed)
}

#[cfg(test)]
mod tests {
    #[test]
    fn resample_keeps_flat_colour_and_averages_pairs() {
        let flat = [10u8, 20, 30, 255].repeat(7 * 5);
        let out = super::resample(&flat, (7, 5), (4, 3));
        assert!(out.chunks(4).all(|p| p == [10, 20, 30, 255]));
        // Columns 0,255,0,255 halved: each output pixel sits between a pair.
        let row: Vec<u8> = (0..4)
            .flat_map(|x| [if x % 2 == 0 { 0 } else { 255 }; 4])
            .collect();
        let out = super::resample(&row.repeat(2), (4, 2), (2, 1));
        assert!(out.iter().all(|&v| (127..=128).contains(&v)), "{out:?}");
    }
}
