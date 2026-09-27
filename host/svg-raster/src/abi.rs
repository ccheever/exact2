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
pub const ABI: u32 = 1;

/// The ABI version.
#[no_mangle]
pub extern "C" fn exact_svg_raster_abi() -> u32 {
    ABI
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
