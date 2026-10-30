//! JPEG, GIF and WebP: the header read here (the frame's SOF marker, the GIF
//! screen descriptor, the WebP frame header; no decoder), the pixels by the
//! platform's decoder where there is one — Android's `AImageDecoder`
//! (libjpeg-turbo, giflib, libwebp; sampled to the plan's size while
//! decoding, as the platform's own image views decode; an animation's first
//! frame). Before Android 11 (API 30, `AImageDecoder`'s) the same decoder
//! through the reader's `ImageDecoder` (`crate::canvas::legacy`). On Android,
//! an animated GIF or WebP plays
//! in the platform's own animated drawable, which a reader may draw in the
//! still's place ([`crate::image::Bitmap::animation_file`]). Windows uses WIC
//! for these formats, including installed WebP codecs.
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
    #[cfg(any(windows, target_os = "android", target_os = "macos"))]
    let mut orientation = 1;
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
        if at + len > MAX_HEADER_BYTES || at + len > encoded_bytes {
            return Err(Refusal::HeaderLimit);
        }
        #[cfg(any(windows, target_os = "android", target_os = "macos"))]
        if m == 0xE1 {
            let mut data = vec![0; (len - 2) as usize];
            input
                .read_exact(&mut data)
                .map_err(|_| Refusal::DecodeFailed)?;
            if let Some(value) = exif_orientation(&data) {
                orientation = value;
            }
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
            let header = Header::jpeg(Metadata {
                natural: PixelSize { width, height },
                encoded_bytes,
                header_bytes: at + len,
            });
            // The picture's size is the upright one: WIC is told to turn it, and Android's decoders
            // turn it themselves, so a target size in the stored grid would squash a turned photo.
            #[cfg(any(windows, target_os = "android", target_os = "macos"))]
            let header = header.oriented(orientation);
            return Ok(header);
        }
        at += len;
        input
            .seek(SeekFrom::Start(at))
            .map_err(|_| Refusal::DecodeFailed)?;
    }
}

#[cfg(any(windows, target_os = "android", target_os = "macos"))]
fn exif_orientation(bytes: &[u8]) -> Option<u8> {
    let tiff = bytes.strip_prefix(b"Exif\0\0")?;
    let little = match tiff.get(..4)? {
        b"II\x2a\0" => true,
        b"MM\0\x2a" => false,
        _ => return None,
    };
    let u16_at = |bytes: &[u8]| {
        let pair = [bytes[0], bytes[1]];
        if little {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        }
    };
    let offset: [u8; 4] = tiff.get(4..8)?.try_into().ok()?;
    let offset = if little {
        u32::from_le_bytes(offset)
    } else {
        u32::from_be_bytes(offset)
    };
    let ifd = tiff.get(usize::try_from(offset).ok()?..)?;
    let count = usize::from(u16_at(ifd.get(..2)?));
    let entries = ifd.get(2..)?.get(..count * 12)?;
    for entry in entries.chunks_exact(12) {
        if u16_at(entry) != 0x112 || u16_at(&entry[2..]) != 3 {
            continue;
        }
        let count: [u8; 4] = entry[4..8].try_into().ok()?;
        let count = if little {
            u32::from_le_bytes(count)
        } else {
            u32::from_be_bytes(count)
        };
        if count != 1 {
            return None;
        }
        let value = u16_at(&entry[8..]);
        return (1..=8).contains(&value).then_some(value as u8);
    }
    None
}

#[cfg(all(test, any(windows, target_os = "android", target_os = "macos")))]
#[test]
fn exif_orientation_checks_endianness_types_offsets_and_truncation() {
    let little = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let big = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0";
    for bytes in [little, big] {
        assert_eq!(exif_orientation(bytes), Some(6));
        for len in 0..28 {
            assert_eq!(exif_orientation(&bytes[..len]), None, "prefix {len}");
        }
    }
    for (at, value) in [(10, 255), (16, 255), (18, 4), (20, 2), (24, 9)] {
        let mut invalid = *little;
        invalid[at] = value;
        assert_eq!(exif_orientation(&invalid), None, "byte {at}");
    }
    for orientation in 1..=8 {
        let mut bytes = *little;
        bytes[24] = orientation;
        assert_eq!(exif_orientation(&bytes), Some(orientation));
    }
}

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod platform {
    use std::ffi::{c_void, CStr};
    use std::sync::OnceLock;
    /// `ARect` (`<android/rect.h>`): left, top, right, bottom.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct ARect(pub i32, pub i32, pub i32, pub i32);

    type Create = unsafe extern "C" fn(*const c_void, usize, *mut *mut c_void) -> i32;
    type SetInt = unsafe extern "C" fn(*mut c_void, i32) -> i32;
    type SetSize = unsafe extern "C" fn(*mut c_void, i32, i32) -> i32;
    type SetCrop = unsafe extern "C" fn(*mut c_void, ARect) -> i32;
    type Decode = unsafe extern "C" fn(*mut c_void, *mut c_void, usize, usize) -> i32;
    type Delete = unsafe extern "C" fn(*mut c_void);

    /// `AImageDecoder`'s functions: `createFromBuffer`, `setAndroidBitmapFormat`,
    /// `setTargetSize`, `setCrop`, `decodeImage`, `delete`.
    pub struct Decoder {
        pub create: Create,
        pub format: SetInt,
        pub target: SetSize,
        pub crop: SetCrop,
        pub decode: Decode,
        pub delete: Delete,
    }

    /// The platform decoder, looked up at run time: `AImageDecoder` came with
    /// API 30, and a library that imported it would not load on Android 10.
    pub fn decoder() -> Option<&'static Decoder> {
        static DECODER: OnceLock<Option<Decoder>> = OnceLock::new();
        // SAFETY: libjnigraphics is a public NDK library; dlopen loads it or
        // takes a reference. Each symbol is cast to its NDK signature.
        DECODER
            .get_or_init(|| unsafe {
                let lib = libc::dlopen(c"libjnigraphics.so".as_ptr(), libc::RTLD_NOW);
                if lib.is_null() {
                    return None;
                }
                let symbol = |name: &CStr| {
                    let at = libc::dlsym(lib, name.as_ptr());
                    (!at.is_null()).then_some(at)
                };
                Some(Decoder {
                    create: std::mem::transmute::<*mut c_void, Create>(symbol(
                        c"AImageDecoder_createFromBuffer",
                    )?),
                    format: std::mem::transmute::<*mut c_void, SetInt>(symbol(
                        c"AImageDecoder_setAndroidBitmapFormat",
                    )?),
                    target: std::mem::transmute::<*mut c_void, SetSize>(symbol(
                        c"AImageDecoder_setTargetSize",
                    )?),
                    crop: std::mem::transmute::<*mut c_void, SetCrop>(symbol(
                        c"AImageDecoder_setCrop",
                    )?),
                    decode: std::mem::transmute::<*mut c_void, Decode>(symbol(
                        c"AImageDecoder_decodeImage",
                    )?),
                    delete: std::mem::transmute::<*mut c_void, Delete>(symbol(
                        c"AImageDecoder_delete",
                    )?),
                })
            })
            .as_ref()
    }
}

/// Whether `AImageDecoder` is the platform's (Android 11, API 30, and later).
#[cfg(target_os = "android")]
pub(crate) fn platform_decoder() -> bool {
    platform::decoder().is_some()
}

/// The whole file (it is bounded by `MAX_ENCODED_BYTES`): the platform
/// decoder takes a buffer.
#[cfg(any(target_os = "android", windows, target_os = "macos"))]
fn read_all<R: Read + Seek>(
    mut input: R,
    plan: &DecodePlan,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>, Refusal> {
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
    Ok(bytes)
}

/// Decode `bytes` at the plan's size into `pixels` (`len` bytes, rows
/// `stride` apart): the whole picture, or the part of it the plan is
/// (`DecodePlan::part`; the decoder scales to the whole's size, then crops,
/// so the part's pixels are the whole's). Whether every row was written.
///
/// The platform decoder samples by a power of two in the DCT and scales the
/// rest of the way itself (bilinear, Skia's vector code), as an image loader
/// asks it to: cheaper than decoding at the power of two and resampling here
/// (heavy's probe: -7% of every thread's instructions).
///
/// # Safety
/// `pixels` is writable for `len` bytes, `len` ≥ `stride × h`, `stride` ≥ `4w`.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
unsafe fn decode_into(
    bytes: &[u8],
    plan: &DecodePlan,
    pixels: *mut std::ffi::c_void,
    stride: usize,
    len: usize,
) -> bool {
    use platform::ARect;
    let (full, part, out) = (plan.full(), plan.crop, plan.pixels);
    let Some(d) = platform::decoder() else {
        let crop = [part.x, part.y, out.width, out.height];
        return crate::canvas::legacy::decode(
            bytes,
            (full.width, full.height),
            crop,
            pixels,
            stride,
            len,
        );
    };
    crate::android::section_begin(c"exact platform decode");
    struct End;
    impl Drop for End {
        fn drop(&mut self) {
            crate::android::section_end();
        }
    }
    let _end = End;
    let mut decoder = std::ptr::null_mut();
    // The buffer outlives the decoder, which is deleted on every path.
    if (d.create)(bytes.as_ptr().cast(), bytes.len(), &mut decoder) != 0 {
        return false;
    }
    let cut = ARect(
        part.x as i32,
        part.y as i32,
        (part.x + out.width) as i32,
        (part.y + out.height) as i32,
    );
    let ok = (d.format)(decoder, 1) == 0
        && (d.target)(decoder, full.width as i32, full.height as i32) == 0
        && (part.whole() || (d.crop)(decoder, cut) == 0)
        && (d.decode)(decoder, pixels, stride, len) == 0;
    (d.delete)(decoder);
    ok
}

/// Decode at the plan's size, onto the heap.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
pub(super) fn decode<R: Read + Seek>(
    input: R,
    plan: &DecodePlan,
    cancelled: impl Fn() -> bool,
) -> Result<Pixmap, Refusal> {
    let bytes = read_all(input, plan, cancelled)?;
    let (w, h) = (plan.pixels.width, plan.pixels.height);
    let stride = w as usize * 4;
    let len = stride * h as usize;
    // Not zeroed first: the decoder writes every byte of it, and zeroing a
    // picture's megabytes cost a fifth of the decode threads' time (heavy).
    let mut out: Vec<u8> = Vec::with_capacity(len);
    // SAFETY: the output's capacity is `stride × h` bytes, as the decoder is
    // told, and its length is set only after it has written all of them.
    unsafe {
        if !decode_into(&bytes, plan, out.as_mut_ptr().cast(), stride, len) {
            return Err(Refusal::DecodeFailed);
        }
        out.set_len(len);
    }
    tiny_skia::IntSize::from_wh(w, h)
        .and_then(|size| Pixmap::from_vec(out, size))
        .ok_or(Refusal::DecodeFailed)
}

/// A picture's pixels for [`super::Bitmap`]: a platform-decoded one straight
/// into a GPU buffer where a host's reader draws from those
/// ([`super::hardware_pictures`]), anything else onto the heap.
pub(super) fn decode_pixels<R: Read + Seek>(
    input: R,
    plan: &DecodePlan,
    cancelled: impl Fn() -> bool,
) -> Result<super::bitmap::Pixels, Refusal> {
    #[cfg(target_os = "android")]
    #[allow(unsafe_code)]
    if plan.platform() && super::hardware::enabled() {
        let bytes = read_all(input, plan, cancelled)?;
        let size = (plan.pixels.width, plan.pixels.height);
        // SAFETY: `filled` hands a buffer of `len` bytes, rows `stride` apart.
        return super::hardware::Hardware::filled(size.0, size.1, |pixels, stride, len| unsafe {
            decode_into(&bytes, plan, pixels, stride, len)
        })
        .map(super::bitmap::Pixels::Hardware)
        .ok_or(Refusal::DecodeFailed);
    }
    super::png_decode::decode_rows(input, plan, cancelled).map(Into::into)
}

#[cfg(windows)]
#[path = "windows.rs"]
mod windows;
#[cfg(windows)]
pub(super) use windows::decode;

/// ImageIO decodes and orients the thumbnail on the raster worker.
#[cfg(target_os = "macos")]
pub(super) fn decode<R: Read + Seek>(
    input: R,
    plan: &DecodePlan,
    cancelled: impl Fn() -> bool,
) -> Result<Pixmap, Refusal> {
    let bytes = read_all(input, plan, &cancelled)?;
    let full = plan.full();
    let pixels = exact_ink::image::apple::decode(
        &bytes,
        [full.width, full.height],
        [
            plan.crop.x,
            plan.crop.y,
            plan.pixels.width,
            plan.pixels.height,
        ],
    )
    .ok_or(Refusal::DecodeFailed)?;
    if cancelled() {
        Err(Refusal::Stale)
    } else {
        Ok(pixels)
    }
}

/// No platform decoder on this target.
#[cfg(not(any(target_os = "android", windows, target_os = "macos")))]
pub(super) fn decode<R: Read + Seek>(
    _input: R,
    _plan: &DecodePlan,
    _cancelled: impl Fn() -> bool,
) -> Result<Pixmap, Refusal> {
    Err(Refusal::DecodeFailed)
}
