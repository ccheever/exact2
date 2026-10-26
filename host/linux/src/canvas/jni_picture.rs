//! A package's picture from the reader (LLP 1133.019 D11 item 1): the
//! `Bitmap` an extension answered with, copied into the host's pixels.
use jni_sys::{jobject, JNIEnv};
use std::ffi::c_void;

/// `AndroidBitmapInfo`.
#[repr(C)]
struct BitmapInfo {
    width: u32,
    height: u32,
    stride: u32,
    format: i32,
    flags: u32,
}

/// `ANDROID_BITMAP_FORMAT_RGBA_8888`: `Bitmap.Config.ARGB_8888`, whose bytes
/// are R, G, B, A.
const RGBA_8888: i32 = 1;
/// `ANDROID_BITMAP_FLAGS_ALPHA_MASK` and its unpremultiplied value.
const ALPHA_MASK: u32 = 0x3;
const ALPHA_UNPREMUL: u32 = 0x2;

#[link(name = "jnigraphics")]
extern "C" {
    fn AndroidBitmap_getInfo(env: *mut JNIEnv, bitmap: jobject, info: *mut BitmapInfo) -> i32;
    fn AndroidBitmap_lockPixels(env: *mut JNIEnv, bitmap: jobject, pixels: *mut *mut c_void)
        -> i32;
    fn AndroidBitmap_unlockPixels(env: *mut JNIEnv, bitmap: jobject) -> i32;
}

/// `bitmap`'s pixels, RGBA premultiplied as tiny-skia's are, or `None` for
/// one that is not `ARGB_8888` in memory (the reader converts a hardware or
/// other bitmap before it answers: `Extensions.picture`).
///
/// # Safety
/// `env` is the JNI call's and `bitmap` a live `android.graphics.Bitmap`.
pub unsafe fn pixels(env: *mut JNIEnv, bitmap: jobject) -> Option<tiny_skia::Pixmap> {
    let mut info = BitmapInfo {
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
        flags: 0,
    };
    if AndroidBitmap_getInfo(env, bitmap, &mut info) != 0 || info.format != RGBA_8888 {
        return None;
    }
    let mut out = tiny_skia::Pixmap::new(info.width, info.height)?;
    let mut base = std::ptr::null_mut();
    if AndroidBitmap_lockPixels(env, bitmap, &mut base) != 0 || base.is_null() {
        return None;
    }
    let row = info.width as usize * 4;
    let rows = out.data_mut().chunks_exact_mut(row);
    for (y, dst) in rows.enumerate() {
        let src = base.cast::<u8>().add(y * info.stride as usize);
        dst.copy_from_slice(std::slice::from_raw_parts(src, row));
    }
    AndroidBitmap_unlockPixels(env, bitmap);
    if info.flags & ALPHA_MASK == ALPHA_UNPREMUL {
        for px in out.data_mut().chunks_exact_mut(4) {
            let a = u16::from(px[3]);
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
    }
    Some(out)
}
