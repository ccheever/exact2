//! The module's C ABI (LLP 1055.000 §8 ruling 4, after LLP 1009 D2): what
//! the Apple host `dlsym`s from `libexact_svg.dylib`. Pixels cross as a
//! pointer and a length, premultiplied RGBA8, tightly packed; nothing else
//! crosses. The host checks `exact_svg_raster_abi` before any other call.
//!
//! This is the module's one `unsafe` boundary: the core hosts never
//! dereference a pointer for it.
#![allow(unsafe_code)]

/// The ABI this module speaks. A host that expects another refuses the
/// module by name, as a missing symbol is refused.
pub const ABI: u32 = 2;

/// Read the natural viewport of a bounded SVG image document.
///
/// # Safety
/// `bytes` is valid for `len` reads; `width` and `height` each for one write.
#[no_mangle]
pub unsafe extern "C" fn exact_svg_document_size(
    bytes: *const u8,
    len: usize,
    width: *mut u32,
    height: *mut u32,
) -> i32 {
    if bytes.is_null() || width.is_null() || height.is_null() || len > crate::document::SOURCE_LIMIT
    {
        return 2;
    }
    // SAFETY: the caller supplies all three buffers for this call.
    match crate::document::size(unsafe { std::slice::from_raw_parts(bytes, len) }) {
        Ok((w, h)) => {
            unsafe {
                *width = w;
                *height = h;
            }
            0
        }
        Err(error) => error as i32,
    }
}

/// Render an SVG document into the host's reserved BGRA8 output.
///
/// # Safety
/// `bytes` is valid for `len` reads; `pixels` for `stride * height` writes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn exact_svg_document_render(
    bytes: *const u8,
    len: usize,
    pixels: *mut u8,
    width: u32,
    height: u32,
    stride: usize,
    natural_width: u32,
    natural_height: u32,
) -> i32 {
    let Some(output) = stride.checked_mul(height as usize) else {
        return 2;
    };
    if bytes.is_null()
        || pixels.is_null()
        || len > crate::document::SOURCE_LIMIT
        || output > 32 * 1024 * 1024
    {
        return 2;
    }
    // SAFETY: both slices are bounded above and owned by the caller.
    let (bytes, pixels) = unsafe {
        (
            std::slice::from_raw_parts(bytes, len),
            std::slice::from_raw_parts_mut(pixels, output),
        )
    };
    crate::document::render(
        bytes,
        pixels,
        width,
        height,
        stride,
        (natural_width, natural_height),
    )
    .map_or_else(|error| error as i32, |()| 0)
}

/// The ABI version.
#[no_mangle]
pub extern "C" fn exact_svg_raster_abi() -> u32 {
    ABI
}

/// [`crate::filter::run`] over `w × h` premultiplied pixels covering the
/// filter region, whose origin is `(ox, oy)` in user units at `(sx, sy)`
/// pixels per unit; the chain is `Filter::encode`'s `n` numbers. 0 when it
/// ran; 1 when the chain did not decode (the pixels are left alone).
///
/// # Safety
/// `program` must be valid for `n` reads and `pixels` for `w · h · 4`
/// reads and writes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn exact_svg_raster_filter(
    program: *const f32,
    n: usize,
    pixels: *mut u8,
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
) -> i32 {
    if program.is_null() || pixels.is_null() {
        return 1;
    }
    // SAFETY: the caller owns both buffers for this call.
    let (program, px) = unsafe {
        (
            std::slice::from_raw_parts(program, n),
            std::slice::from_raw_parts_mut(pixels, w * h * 4),
        )
    };
    let Some(filter) = exact_svg_filter::Filter::decode(program) else {
        return 1;
    };
    crate::filter::run(
        &filter,
        px,
        w,
        h,
        crate::filter::Space {
            origin: (ox, oy),
            scale: (sx, sy),
        },
    );
    0
}

/// [`crate::mask_coverage`] over `len` bytes at `pixels`.
///
/// # Safety
/// `pixels` must be valid for reads and writes of `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn exact_svg_raster_mask(pixels: *mut u8, len: usize, luminance: u8) {
    if pixels.is_null() || !len.is_multiple_of(4) {
        return;
    }
    // SAFETY: the caller owns `len` bytes at `pixels` for this call.
    let px = unsafe { std::slice::from_raw_parts_mut(pixels, len) };
    crate::mask_coverage(px, luminance != 0);
}
