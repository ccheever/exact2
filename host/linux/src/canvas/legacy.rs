//! What the reader (`host/android`) asks of the Rust on Android releases
//! that lack a platform call it otherwise makes; its Kotlin half is
//! `Legacy.kt` and `LegacyGlyphs.kt`. A newer release never calls in here.
//!
//! - **A glyph by its id, below Android 12** (API 31, `Canvas.drawGlyphs`):
//!   [`glyph`], its outline in ems, which the reader fills as a `Path` with
//!   the run's paint, or a colour glyph's image (an emoji's bitmap). The
//!   reader asks once per face and glyph and keeps the answer. It is a JNI
//!   call rather than an op in the stream: the reader holds the face (its
//!   `Font`'s mapped file), so the answer needs nothing of the host's state
//!   and can be asked from whichever thread records the run, and the stream,
//!   which every release parses, stays as it is.
//! - **JPEG, GIF and WebP below Android 11** (API 30, `AImageDecoder`):
//!   [`decode`], by the reader's `Legacy.decode`, which is `ImageDecoder`
//!   (API 28), the same platform decoder's Java face, asked the same things:
//!   the target size, the crop, software pixels.
//!
//! The outlines are swash's, whose scaler the host already links (Canvas 2D
//! text, glyph ink).
//!
//! @ref LLP 1076 §3.3 (Exact plus Android Canvas)

use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Command, PathData};

/// The size a COLR glyph's image is rendered at, pixels an em; a bitmap
/// glyph's is its largest strike's.
const COLOR_PPEM: f32 = 128.0;

/// Glyph `id` of face `index` in `data` at the variation settings `axes`
/// (tag, value in the axis's units), as the reader's words:
///
/// - `1 n (verb coords…)×n` — its outline in ems from the pen, y down:
///   verb 0 move (x y), 1 line (x y), 2 quad (x1 y1 x y), 3 cubic (x1 y1 x2
///   y2 x y), 4 close; coordinates as `f32` bits. No verbs: nothing to draw
///   (a space).
/// - `2 ppem left top width height argb×(width × height)` — a colour
///   glyph's image at `ppem` (`f32` bits) pixels an em, its left edge and
///   top `left` and `top` pixels right of and above the pen, straight ARGB
///   rows.
///
/// Colour first, as Skia draws a face's colour glyph where it has one.
/// `None` when the face cannot be read or has no such glyph.
pub(crate) fn glyph_words(
    data: &[u8],
    index: u32,
    axes: &[(u32, f32)],
    id: u16,
) -> Option<Vec<i32>> {
    static SCALE: std::sync::Mutex<Option<ScaleContext>> = std::sync::Mutex::new(None);
    let font = swash::FontRef::from_index(data, index as usize)?;
    let settings = || {
        axes.iter()
            .map(|&(tag, value)| swash::Setting { tag, value })
    };
    let mut scale = SCALE.lock().unwrap_or_else(|e| e.into_inner());
    let context = scale.get_or_insert_with(ScaleContext::new);
    let ppem = font
        .color_strikes()
        .find_by_largest_ppem(id)
        .map_or(COLOR_PPEM, |s| f32::from(s.ppem()));
    let mut scaler = context
        .builder(font)
        .size(ppem)
        .variations(settings())
        .build();
    if scaler.has_color_outlines() || scaler.has_color_bitmaps() {
        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::LargestSize),
        ])
        .render(&mut scaler, id);
        if let Some(image) = image.filter(|i| i.content == Content::Color) {
            let p = image.placement;
            let mut out = vec![
                2,
                ppem.to_bits() as i32,
                p.left,
                p.top,
                p.width as i32,
                p.height as i32,
            ];
            out.extend(
                image
                    .data
                    .chunks_exact(4)
                    .map(|c| i32::from_be_bytes([c[3], c[0], c[1], c[2]])),
            );
            return Some(out);
        }
    }
    let mut scaler = context
        .builder(font)
        .size(1.0)
        .variations(settings())
        .build();
    let outline = scaler.scale_outline(id)?;
    let mut out = vec![1, 0];
    let mut put = |verb: i32, points: &[swash::zeno::Vector]| {
        out.push(verb);
        for p in points {
            out.extend([p.x.to_bits() as i32, (-p.y).to_bits() as i32]);
        }
    };
    let mut verbs = 0;
    for command in outline.path().commands() {
        match command {
            Command::MoveTo(p) => put(0, &[p]),
            Command::LineTo(p) => put(1, &[p]),
            Command::QuadTo(a, p) => put(2, &[a, p]),
            Command::CurveTo(a, b, p) => put(3, &[a, b, p]),
            Command::Close => put(4, &[]),
        }
        verbs += 1;
    }
    out[1] = verbs;
    Some(out)
}

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod jni {
    use jni_sys::{
        jclass, jint, jintArray, jmethodID, jobject, jvalue, JNIEnv, JavaVM, JNI_OK,
        JNI_VERSION_1_6,
    };
    use std::ffi::c_void;
    use std::sync::OnceLock;

    /// `LegacyGlyphs.glyph(font, index, axes, id)`: [`super::glyph_words`]
    /// for the face whose file `font` maps (a direct `ByteBuffer`, the
    /// `Font`'s own), at `axes` (tag, value bits pairs), or null.
    ///
    /// # Safety
    /// `env`, `font` and `axes` are the JNI call's.
    pub unsafe fn glyph(
        env: *mut JNIEnv,
        font: jobject,
        index: jint,
        axes: jintArray,
        id: jint,
    ) -> jintArray {
        let at = ((**env).GetDirectBufferAddress.expect("jni"))(env, font);
        let len = ((**env).GetDirectBufferCapacity.expect("jni"))(env, font);
        if at.is_null() || len <= 0 {
            return std::ptr::null_mut();
        }
        // SAFETY: the reader's `Font` keeps its buffer mapped for the call.
        let data = std::slice::from_raw_parts(at.cast::<u8>(), len as usize);
        let n = ((**env).GetArrayLength.expect("jni"))(env, axes).max(0);
        let mut words = vec![0i32; n as usize];
        ((**env).GetIntArrayRegion.expect("jni"))(env, axes, 0, n, words.as_mut_ptr());
        let axes: Vec<(u32, f32)> = words
            .chunks_exact(2)
            .map(|w| (w[0] as u32, f32::from_bits(w[1] as u32)))
            .collect();
        let Some(out) = super::glyph_words(data, index.max(0) as u32, &axes, id as u16) else {
            return std::ptr::null_mut();
        };
        let array = ((**env).NewIntArray.expect("jni"))(env, out.len() as jint);
        if !array.is_null() {
            ((**env).SetIntArrayRegion.expect("jni"))(
                env,
                array,
                0,
                out.len() as jint,
                out.as_ptr(),
            );
        }
        array
    }

    /// The reader's `Legacy.decode`, found once where `AImageDecoder` is not.
    struct Decoder {
        vm: *mut JavaVM,
        class: jclass,
        decode: jmethodID,
    }
    // SAFETY: a JavaVM pointer, a global reference and a method id are the
    // process's, valid on every thread.
    unsafe impl Send for Decoder {}
    unsafe impl Sync for Decoder {}

    static DECODER: OnceLock<Option<Decoder>> = OnceLock::new();

    /// `Legacy.decode(source, width, height, x, y, w, h)`.
    const DECODE: (&std::ffi::CStr, &std::ffi::CStr) = (
        c"decode",
        c"(Ljava/nio/ByteBuffer;IIIIII)Landroid/graphics/Bitmap;",
    );

    /// The reader boots the host: below API 30 its decoder is found by the
    /// class loader of the call (a thread the host makes has only the
    /// system's).
    ///
    /// # Safety
    /// `env` is the JNI call's.
    pub unsafe fn attach(env: *mut JNIEnv) {
        if crate::image::platform_decoder() {
            return;
        }
        DECODER.get_or_init(|| {
            let mut vm: *mut JavaVM = std::ptr::null_mut();
            if ((**env).GetJavaVM.expect("jni"))(env, &mut vm) != JNI_OK {
                return None;
            }
            let local = ((**env).FindClass.expect("jni"))(env, c"dev/exact/canvas/Legacy".as_ptr());
            let decode = if local.is_null() {
                std::ptr::null_mut()
            } else {
                ((**env).GetStaticMethodID.expect("jni"))(
                    env,
                    local,
                    DECODE.0.as_ptr(),
                    DECODE.1.as_ptr(),
                )
            };
            if decode.is_null() {
                ((**env).ExceptionClear.expect("jni"))(env);
                return None;
            }
            let class = ((**env).NewGlobalRef.expect("jni"))(env, local);
            Some(Decoder { vm, class, decode })
        });
    }

    /// `AndroidBitmapInfo`.
    #[repr(C)]
    struct BitmapInfo {
        width: u32,
        height: u32,
        stride: u32,
        format: i32,
        flags: u32,
    }

    #[link(name = "jnigraphics")]
    extern "C" {
        fn AndroidBitmap_getInfo(env: *mut JNIEnv, bitmap: jobject, info: *mut BitmapInfo) -> i32;
        fn AndroidBitmap_lockPixels(
            env: *mut JNIEnv,
            bitmap: jobject,
            pixels: *mut *mut c_void,
        ) -> i32;
        fn AndroidBitmap_unlockPixels(env: *mut JNIEnv, bitmap: jobject) -> i32;
    }

    /// `bytes` decoded at `full` and cropped to `crop` (x, y, width, height)
    /// into `pixels`, RGBA premultiplied, rows `stride` apart; whether every
    /// row was written. False with no reader to decode in.
    ///
    /// # Safety
    /// `pixels` is writable for `len` bytes, `len` ≥ `stride × crop height`,
    /// `stride` ≥ `4 × crop width`.
    pub unsafe fn decode(
        bytes: &[u8],
        full: (u32, u32),
        crop: [u32; 4],
        pixels: *mut c_void,
        stride: usize,
        len: usize,
    ) -> bool {
        let Some(Some(d)) = DECODER.get() else {
            return false;
        };
        let vm = d.vm;
        let mut env: *mut JNIEnv = std::ptr::null_mut();
        let out = (&mut env as *mut *mut JNIEnv).cast::<*mut c_void>();
        // Decode workers are the host's threads: attached for the call.
        let mut attached = false;
        if ((**vm).GetEnv.expect("jni"))(vm, out, JNI_VERSION_1_6) != JNI_OK {
            if ((**vm).AttachCurrentThread.expect("jni"))(vm, out, std::ptr::null_mut()) != JNI_OK {
                return false;
            }
            attached = true;
        }
        // The decoder reads `bytes` in place during the call.
        let source = ((**env).NewDirectByteBuffer.expect("jni"))(
            env,
            bytes.as_ptr() as *mut c_void,
            bytes.len() as i64,
        );
        let args =
            [full.0, full.1, crop[0], crop[1], crop[2], crop[3]].map(|v| jvalue { i: v as jint });
        let args = [
            jvalue { l: source },
            args[0],
            args[1],
            args[2],
            args[3],
            args[4],
            args[5],
        ];
        let bitmap = if source.is_null() {
            std::ptr::null_mut()
        } else {
            ((**env).CallStaticObjectMethodA.expect("jni"))(env, d.class, d.decode, args.as_ptr())
        };
        if ((**env).ExceptionCheck.expect("jni"))(env) != 0 {
            ((**env).ExceptionClear.expect("jni"))(env);
        }
        let mut ok = false;
        if !bitmap.is_null() {
            let mut src: *mut c_void = std::ptr::null_mut();
            let mut info = BitmapInfo {
                width: 0,
                height: 0,
                stride: 0,
                format: 0,
                flags: 0,
            };
            let (w, h) = (crop[2] as usize, crop[3] as usize);
            // ANDROID_BITMAP_FORMAT_RGBA_8888 (1), at the crop's size.
            if AndroidBitmap_getInfo(env, bitmap, &mut info) == 0
                && (info.format, info.width as usize, info.height as usize) == (1, w, h)
                && stride * h <= len
                && AndroidBitmap_lockPixels(env, bitmap, &mut src) == 0
            {
                for y in 0..h {
                    let from = src.cast::<u8>().add(y * info.stride as usize);
                    std::ptr::copy_nonoverlapping(from, pixels.cast::<u8>().add(y * stride), w * 4);
                }
                AndroidBitmap_unlockPixels(env, bitmap);
                ok = true;
            }
            let class = ((**env).GetObjectClass.expect("jni"))(env, bitmap);
            let recycle = ((**env).GetMethodID.expect("jni"))(
                env,
                class,
                c"recycle".as_ptr(),
                c"()V".as_ptr(),
            );
            if !recycle.is_null() {
                ((**env).CallVoidMethodA.expect("jni"))(env, bitmap, recycle, std::ptr::null());
            }
            ((**env).DeleteLocalRef.expect("jni"))(env, class);
            ((**env).DeleteLocalRef.expect("jni"))(env, bitmap);
        }
        if !source.is_null() {
            ((**env).DeleteLocalRef.expect("jni"))(env, source);
        }
        if attached {
            ((**vm).DetachCurrentThread.expect("jni"))(vm);
        }
        ok
    }
}
#[cfg(target_os = "android")]
pub use jni::{attach, decode, glyph};

#[cfg(test)]
mod tests {
    use super::glyph_words;

    fn arabic() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fonts/NotoSansArabic.ttf"
        ))
        .unwrap()
    }

    /// Glyphs no character maps to (contextual forms, ligatures): what the
    /// reader drew nothing for below API 31 when it drew characters.
    fn unmapped(data: &[u8]) -> Vec<u16> {
        use skrifa::MetadataProvider;
        let font = skrifa::FontRef::new(data).unwrap();
        let mapped: std::collections::HashSet<u32> =
            font.charmap().mappings().map(|(_, g)| g.to_u32()).collect();
        let count = skrifa::raw::TableProvider::maxp(&font)
            .unwrap()
            .num_glyphs();
        (1..count)
            .filter(|g| !mapped.contains(&u32::from(*g)))
            .collect()
    }

    #[test]
    fn a_glyph_no_character_maps_to_has_its_outline_in_ems() {
        let data = arabic();
        let ids = unmapped(&data);
        assert!(
            ids.len() > 100,
            "NotoSansArabic's contextual forms: {}",
            ids.len()
        );
        let mut drawn = 0;
        for id in ids {
            let w = glyph_words(&data, 0, &[], id).expect("a glyph of the face");
            assert_eq!(w[0], 1, "an outline");
            if w[1] == 0 {
                continue;
            }
            drawn += 1;
            // Every coordinate in ems: within a few of the pen.
            let mut i = 2;
            for _ in 0..w[1] {
                let n = [2, 2, 4, 6, 0][w[i] as usize];
                for k in 0..n {
                    let v = f32::from_bits(w[i + 1 + k] as u32);
                    assert!(v.is_finite() && v.abs() < 4.0, "glyph {id}: {v}");
                }
                i += 1 + n;
            }
            assert_eq!(i, w.len());
        }
        assert!(drawn > 100);
    }

    #[test]
    fn an_axis_moves_the_outline() {
        let data = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../apple/tests/fonts/InterVariable.ttf"
        ))
        .unwrap();
        let id = {
            use skrifa::MetadataProvider;
            skrifa::FontRef::new(&data)
                .unwrap()
                .charmap()
                .map('H')
                .unwrap()
                .to_u32() as u16
        };
        let wght = u32::from_be_bytes(*b"wght");
        let light = glyph_words(&data, 0, &[(wght, 100.0)], id).unwrap();
        let bold = glyph_words(&data, 0, &[(wght, 900.0)], id).unwrap();
        assert_ne!(light, bold);
    }
}
