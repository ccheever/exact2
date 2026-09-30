//! Canvas 2D on the GPU with vello: LLP 1056's recorded lists replayed into
//! a vello scene and rendered into the host's IOSurfaces, as a module the
//! Apple host loads on demand (the GPU bake-off's vello candidate; ABI v2,
//! `host/apple/Sources/ExactKit/Canvas2DGpu.swift`).
//!
//! - `replay.rs`: the records, with the Core Graphics replayer's semantics.
//! - `paint.rs`: styles, global alpha, operators, shadows.
//! - `gpu.rs`: one device and renderer per process, one canvas's textures.
//! - `surface.rs`: IOSurfaces as wgpu textures.
//! - `kernels.rs`: every shader, compiled to Metal AIR at build time.
//!
//! Every export is safe to call from several threads at once: the GPU
//! context is behind one lock, and each canvas behind its own.
#![cfg(target_vendor = "apple")]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod gpu;
pub mod kernels;
pub mod paint;
pub mod replay;
pub mod surface;

use replay::Host;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString};
use std::sync::{Arc, Mutex, OnceLock};
use surface::IOSurfaceRef;
use vello::kurbo::BezPath;
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

/// A text run's glyphs (ABI `EcgRun`).
#[repr(C)]
pub struct EcgRun {
    pub font: *const c_void,
    pub glyphs: *const u16,
    pub positions: *const f64,
    pub count: usize,
}

/// The host's callbacks (ABI `EcgHost`).
#[repr(C)]
pub struct EcgHost {
    pub ctx: *mut c_void,
    pub text_path: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            font: *const f64,
            font_n: usize,
            text: *const u32,
            text_n: usize,
            rtl: u32,
            len: *mut usize,
        ) -> *const f32,
    >,
    pub text_runs: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            font: *const f64,
            font_n: usize,
            text: *const u32,
            text_n: usize,
            rtl: u32,
            count: *mut usize,
        ) -> *const EcgRun,
    >,
    pub image: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            src: *const u8,
            src_len: usize,
            w: *mut u32,
            h: *mut u32,
        ) -> *const u8,
    >,
}

/// Decoded images, by the host's (pointer, width, height): the same handle
/// is the same `ImageData`, so vello's atlas keeps it between replays.
type Images = HashMap<(usize, u32, u32), ImageData>;

fn images() -> &'static Mutex<Images> {
    static IMAGES: OnceLock<Mutex<Images>> = OnceLock::new();
    IMAGES.get_or_init(Default::default)
}

/// The host's callbacks for one replay.
struct AbiHost<'a> {
    host: Option<&'a EcgHost>,
}

impl Host for AbiHost<'_> {
    fn text(&mut self, font: &[f64], text: &[u32], rtl: bool) -> Option<BezPath> {
        let h = self.host?;
        let f = h.text_path?;
        let mut len = 0usize;
        // SAFETY: the host's callback contract (ABI.md).
        let p = unsafe {
            f(
                h.ctx,
                font.as_ptr(),
                font.len(),
                text.as_ptr(),
                text.len(),
                u32::from(rtl),
                &mut len,
            )
        };
        if p.is_null() {
            return None;
        }
        // SAFETY: `len` floats, valid until the next callback.
        Some(outline(unsafe { std::slice::from_raw_parts(p, len) }))
    }

    fn image(&mut self, src: &str) -> Option<ImageData> {
        let h = self.host?;
        let f = h.image?;
        let (mut w, mut hh) = (0u32, 0u32);
        // SAFETY: the host's callback contract.
        let p = unsafe { f(h.ctx, src.as_ptr(), src.len(), &mut w, &mut hh) };
        if p.is_null() || w == 0 || hh == 0 {
            return None;
        }
        let mut cache = images().lock().unwrap_or_else(|e| e.into_inner());
        let key = (p as usize, w, hh);
        if let Some(i) = cache.get(&key) {
            return Some(i.clone());
        }
        // SAFETY: premultiplied RGBA8, tightly packed, valid for the replay.
        let px = unsafe { std::slice::from_raw_parts(p, (w * hh * 4) as usize) }.to_vec();
        let image = ImageData {
            data: Blob::new(Arc::new(px)),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::AlphaPremultiplied,
            width: w,
            height: hh,
        };
        cache.insert(key, image.clone());
        Some(image)
    }
}

/// The host's glyph outlines (`[verb, …]` f32s: 0 move, 1 line, 2 quad,
/// 3 cubic, 4 close) as a path.
pub fn outline(buf: &[f32]) -> BezPath {
    let mut p = BezPath::new();
    let mut i = 0;
    let at = |j: usize| (f64::from(buf[j]), f64::from(buf[j + 1]));
    while i < buf.len() {
        let verb = buf[i] as u32;
        i += 1;
        let need = [2, 2, 4, 6, 0]
            .get(verb as usize)
            .copied()
            .unwrap_or(usize::MAX);
        if i + need > buf.len() {
            break;
        }
        match verb {
            0 => p.move_to(at(i)),
            1 => p.line_to(at(i)),
            2 => p.quad_to(at(i), at(i + 2)),
            3 => p.curve_to(at(i), at(i + 2), at(i + 4)),
            _ => p.close_path(),
        }
        i += need;
    }
    p
}

/// A canvas behind its own lock, with the string `unsupported` returns.
pub struct Handle {
    canvas: Mutex<gpu::Canvas>,
    unsupported: Mutex<Option<CString>>,
}

/// The ABI's version.
#[no_mangle]
pub extern "C" fn ecg_abi() -> u32 {
    // A build without the Metal toolchain has no kernels: not a module.
    if cfg!(exact_no_metal_toolchain) {
        0
    } else {
        2
    }
}

/// "vello".
#[no_mangle]
pub extern "C" fn ecg_name() -> *const c_char {
    c"vello".as_ptr()
}

/// A canvas `w`×`h` device pixels at `scale`; null when there is no GPU.
#[no_mangle]
pub extern "C" fn ecg_canvas_new(w: u32, h: u32, scale: f64) -> *mut c_void {
    if let Err(e) = gpu::gpu() {
        eprintln!("exact canvas gpu: {e}");
        return std::ptr::null_mut();
    }
    let h = Handle {
        canvas: Mutex::new(gpu::Canvas::new(w, h, scale)),
        unsupported: Mutex::new(None),
    };
    Box::into_raw(Box::new(h)).cast()
}

/// Free a canvas and release its surfaces.
///
/// # Safety
/// `c` from `ecg_canvas_new`, not used after.
#[no_mangle]
pub unsafe extern "C" fn ecg_canvas_free(c: *mut c_void) {
    if !c.is_null() {
        // SAFETY: the caller's contract.
        let h = unsafe { Box::from_raw(c.cast::<Handle>()) };
        // Textures are released under the context's lock.
        let _g = gpu::made();
        drop(h);
    }
}

/// Replay `n` lists over `previous` into `target`; returns when the GPU has
/// written it. 0 ok; 1 a list was unreadable (the rest applied); 2 GPU
/// failure (target untouched).
///
/// # Safety
/// The ABI's contract: `c` from `ecg_canvas_new`; `target` (and `previous`
/// unless null) live IOSurfaceRefs; `lists`/`lens` `n` entries; `host` null
/// or valid for the call.
#[no_mangle]
pub unsafe extern "C" fn ecg_canvas_replay(
    c: *mut c_void,
    target: *mut c_void,
    previous: *mut c_void,
    lists: *const *const u8,
    lens: *const usize,
    n: usize,
    host: *const EcgHost,
) -> u32 {
    if c.is_null() || target.is_null() {
        return 2;
    }
    // SAFETY: the caller's contract.
    let h = unsafe { &*c.cast::<Handle>() };
    let lists: Vec<&[u8]> = (0..n)
        // SAFETY: the caller's contract.
        .map(|i| unsafe {
            let p = *lists.add(i);
            if p.is_null() {
                &[][..]
            } else {
                std::slice::from_raw_parts(p, *lens.add(i))
            }
        })
        .collect();
    // SAFETY: null or valid for the call.
    let mut abi = AbiHost {
        host: unsafe { host.as_ref() },
    };
    let mut canvas = h.canvas.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the caller's contract.
    let result = unsafe {
        canvas.replay(
            IOSurfaceRef(target),
            IOSurfaceRef(previous),
            &lists,
            &mut abi,
        )
    };
    match result {
        Ok(()) => 0,
        Err(gpu::Failed::List) => 1,
        Err(gpu::Failed::Gpu(e)) => {
            eprintln!("exact canvas gpu: {e}");
            2
        }
    }
}

/// The last replay's milliseconds into `out[3]`: CPU encode, GPU, wall.
///
/// # Safety
/// `c` from `ecg_canvas_new`; `out` three writable doubles.
#[no_mangle]
pub unsafe extern "C" fn ecg_canvas_stats(c: *mut c_void, out: *mut f64) {
    if c.is_null() || out.is_null() {
        return;
    }
    // SAFETY: the caller's contract.
    let h = unsafe { &*c.cast::<Handle>() };
    let s = h.canvas.lock().unwrap_or_else(|e| e.into_inner()).stats;
    // SAFETY: the caller's contract.
    unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), out, 3) };
}

/// The op names the replays so far could not draw, comma-separated, or
/// null; valid until the next call on this canvas.
///
/// # Safety
/// `c` from `ecg_canvas_new`.
#[no_mangle]
pub unsafe extern "C" fn ecg_canvas_unsupported(c: *mut c_void) -> *const c_char {
    if c.is_null() {
        return std::ptr::null();
    }
    // SAFETY: the caller's contract.
    let h = unsafe { &*c.cast::<Handle>() };
    let names = h
        .canvas
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .replayer
        .unsupported
        .join(",");
    let mut slot = h.unsupported.lock().unwrap_or_else(|e| e.into_inner());
    if names.is_empty() {
        *slot = None;
        return std::ptr::null();
    }
    *slot = CString::new(names).ok();
    slot.as_ref().map_or(std::ptr::null(), |s| s.as_ptr())
}

/// Bytes the module holds now, process-wide: vello's pooled buffers and
/// kept textures, and the decoded images it copied.
#[no_mangle]
pub extern "C" fn ecg_memory() -> u64 {
    let gpu = gpu::made().map_or(0, |g| g.renderer.memory());
    let images: u64 = images()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .map(|i| u64::from(i.width) * u64::from(i.height) * 4)
        .sum();
    gpu + images
}

/// Drop every cache that can be rebuilt.
#[no_mangle]
pub extern "C" fn ecg_trim() {
    images().lock().unwrap_or_else(|e| e.into_inner()).clear();
    if let Some(mut g) = gpu::made() {
        g.renderer.trim();
    }
}
