//! JPEG, GIF and WebP: the header read here (the frame's SOF marker, the GIF
//! screen descriptor, the WebP frame header; no decoder), the pixels by the
//! platform's decoder where there is one — Android's `AImageDecoder`
//! (libjpeg-turbo, giflib, libwebp; sampled to the plan's size while
//! decoding, as the platform's own image views decode; an animation's first
//! frame). Other targets refuse them as before. An animated GIF or WebP plays
//! in the platform's own animated drawable, which a reader may draw in the
//! still's place ([`crate::image::Bitmap::animation_file`]).
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

/// A GIF's or WebP's header from its first 30 bytes, or `None` for neither.
pub(super) fn inspect_animated(head: &[u8], encoded_bytes: u64) -> Result<Option<Header>, Refusal> {
    let le16 = |i: usize| u32::from(u16::from_le_bytes([head[i], head[i + 1]]));
    let le24 = |i: usize| u32::from_le_bytes([head[i], head[i + 1], head[i + 2], 0]);
    let (width, height) =
        if head.len() >= 10 && (&head[..6] == b"GIF87a" || &head[..6] == b"GIF89a") {
            (le16(6), le16(8))
        } else if head.len() >= 30 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
            match &head[12..16] {
                b"VP8 " => (le16(26) & 0x3FFF, le16(28) & 0x3FFF),
                b"VP8L" => {
                    let bits = u32::from_le_bytes([head[21], head[22], head[23], head[24]]);
                    ((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1)
                }
                b"VP8X" => (le24(24) + 1, le24(27) + 1),
                _ => return Err(Refusal::DecodeFailed),
            }
        } else {
            return Ok(None);
        };
    if width == 0 || height == 0 {
        return Err(Refusal::InvalidDimensions);
    }
    if u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
        return Err(Refusal::SourcePixels);
    }
    Ok(Some(Header::jpeg(Metadata {
        natural: PixelSize { width, height },
        encoded_bytes,
        header_bytes: 30.min(encoded_bytes),
    })))
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
        pub fn AImageDecoder_decodeImage(
            decoder: *mut c_void,
            pixels: *mut c_void,
            stride: usize,
            size: usize,
        ) -> i32;
        pub fn AImageDecoder_delete(decoder: *mut c_void);
    }
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
    crate::android::section_begin(c"exact platform decode");
    struct End;
    impl Drop for End {
        fn drop(&mut self) {
            crate::android::section_end();
        }
    }
    let _end = End;
    let stride = w as usize * 4;
    let mut out = vec![0u8; stride * h as usize];
    let mut decoder = std::ptr::null_mut();
    // SAFETY: the buffer outlives the decoder, which is deleted on every path;
    // the output holds `stride × h` bytes, as decodeImage is told.
    let status = unsafe {
        if AImageDecoder_createFromBuffer(bytes.as_ptr().cast(), bytes.len(), &mut decoder) != 0 {
            return Err(Refusal::DecodeFailed);
        }
        let s = if AImageDecoder_setAndroidBitmapFormat(decoder, 1) != 0
            || AImageDecoder_setTargetSize(decoder, w as i32, h as i32) != 0
        {
            -1
        } else {
            AImageDecoder_decodeImage(decoder, out.as_mut_ptr().cast(), stride, out.len())
        };
        AImageDecoder_delete(decoder);
        s
    };
    if status != 0 {
        return Err(Refusal::DecodeFailed);
    }
    tiny_skia::IntSize::from_wh(w, h)
        .and_then(|size| Pixmap::from_vec(out, size))
        .ok_or(Refusal::DecodeFailed)
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
