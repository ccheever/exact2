//! The native ABI: what the presenter `dlopen`s (LLP 1009 D2).
//!
//! One thread-local module; calls on the main thread. Strings are UTF-8
//! bytes with lengths. The `unsafe` this module owns: the platform target
//! handoff (a `CAMetalLayer` pointer the presenter registered, valid until
//! it calls `destroy`) and the byte ranges a host hands over — each checked
//! for a null pointer before it becomes a slice, every count checked for
//! overflow in the module, so a confused host gets a refusal by name, never
//! undefined behaviour (LLP 1009 D2: the ABI is the one `unsafe` boundary).

use crate::{json, Frame, Module, Registry};
use std::cell::RefCell;
use std::ffi::c_void;

thread_local! {
    static MODULE: RefCell<Option<Module>> = const { RefCell::new(None) };
    static ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn with<T>(f: impl FnOnce(&mut Module) -> T) -> Option<T> {
    MODULE.with(|m| m.borrow_mut().as_mut().map(f))
}

/// Record a refusal made by the ABI itself, before the module was reached.
pub fn refuse(why: &str) {
    ERROR.with(|s| *s.borrow_mut() = why.to_string());
}

/// A host's byte range as a slice — `None`, with the refusal recorded, for
/// a null pointer.
///
/// # Safety
/// `ptr`, when non-null, is `len` readable bytes that outlive the call.
pub unsafe fn bytes<'a>(what: &str, ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if ptr.is_null() {
        refuse(&format!("{what}: a null pointer for {len} bytes"));
        return None;
    }
    // SAFETY: the caller's contract, the pointer checked.
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// A host's writable byte range as a slice — `None`, with the refusal
/// recorded, for a null pointer.
///
/// # Safety
/// `ptr`, when non-null, is `len` writable bytes that outlive the call.
pub unsafe fn bytes_mut<'a>(what: &str, ptr: *mut u8, len: usize) -> Option<&'a mut [u8]> {
    if ptr.is_null() {
        refuse(&format!("{what}: a null pointer for {len} bytes"));
        return None;
    }
    // SAFETY: the caller's contract, the pointer checked.
    Some(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
}

/// Create the device and the module. Returns 0 on success, 1 on failure
/// (see [`error`]).
pub fn load(registry: &'static Registry) -> u32 {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    match crate::block_on(crate::load_gpu(instance, None)) {
        Ok(gpu) => {
            let mut module = Module::new(registry);
            module.set_gpu(gpu);
            MODULE.with(|m| *m.borrow_mut() = Some(module));
            0
        }
        Err(e) => {
            ERROR.with(|s| *s.borrow_mut() = e);
            1
        }
    }
}

/// Create a canvas's surface on a `CAMetalLayer`. Returns the canvas id,
/// or 0 on failure.
///
/// # Safety
/// `layer` must be a live `CAMetalLayer` that outlives the canvas.
pub unsafe fn create(name: &str, layer: *mut c_void, width: u32, height: u32) -> u32 {
    let created = with(|m| {
        let gpu = m.gpu()?;
        // SAFETY: the caller's contract — the presenter's layer, alive until destroy.
        let target = unsafe {
            gpu.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer))
        };
        match target {
            Ok(t) => m.create(name, t, width, height),
            Err(e) => {
                ERROR.with(|s| *s.borrow_mut() = format!("{e}"));
                None
            }
        }
    })
    .flatten();
    created.unwrap_or(0)
}

/// Bind inputs (a JSON array of values). Returns 0 on success.
pub fn bind(id: u32, values: &str) -> u32 {
    let values = match json::parse_values(values) {
        Ok(v) => v,
        Err(e) => {
            ERROR.with(|s| *s.borrow_mut() = e);
            return 1;
        }
    };
    match with(|m| m.bind(id, &values)) {
        Some(true) => 0,
        _ => 1,
    }
}

/// Render one frame. Returns 1 when the surface wants another frame, 0
/// otherwise, 2 on failure.
pub fn render(id: u32, width: f32, height: f32, scale: f32, now_ms: f64) -> u32 {
    let frame = Frame {
        width,
        height,
        scale,
        now_ms,
        children_generation: 0,
    };
    match with(|m| m.render(id, &frame)).flatten() {
        Some(true) => 1,
        Some(false) => 0,
        None => 2,
    }
}

/// Whether a canvas's surface wants each child as its own texture (LLP
/// 1014 D5).
pub fn wants_children_each(id: u32) -> bool {
    with(|m| m.wants_children_each(id)).unwrap_or(false)
}

/// The `index`th direct child of a canvas as pixels with its frame (LLP
/// 1014 D5). 0 on success.
pub fn child(id: u32, index: u32, frame: [f32; 4], width: u32, height: u32, bytes: &[u8]) -> u32 {
    match with(|m| m.child(id, index as usize, frame, width, height, bytes)) {
        Some(true) => 0,
        _ => 1,
    }
}

/// How many direct children a canvas has now (LLP 1014 D5). 0 on success.
pub fn children_count(id: u32, count: u32) -> u32 {
    match with(|m| m.children_count(id, count as usize)) {
        Some(true) => 0,
        _ => 1,
    }
}

/// Where the surface put a child: the homography (nine floats, row major)
/// then the depth; `None` when it is the kernel's frame (LLP 1014 D5).
pub fn placement(id: u32, index: u32) -> Option<[f32; 10]> {
    let p = with(|m| m.placement(id, index as usize)).flatten()?;
    let mut out = [0.0; 10];
    out[..9].copy_from_slice(&p.homography);
    out[9] = p.depth;
    Some(out)
}

/// Whether a canvas's surface samples its children (LLP 1014 D2).
pub fn wants_children(id: u32) -> bool {
    with(|m| m.wants_children(id)).unwrap_or(false)
}

/// The canvas's children as pixels (LLP 1014 D3) (`width`×`height` premultiplied
/// RGBA). 0 on success.
pub fn texture(id: u32, width: u32, height: u32, bytes: &[u8]) -> u32 {
    match with(|m| m.texture(id, width, height, bytes)) {
        Some(true) => 0,
        _ => 1,
    }
}

/// A canvas's picture as pixels into `out` — `width`×`height` points at
/// `scale`, RGBA rows top-down, `out` at least the pixel count × 4 (LLP
/// 1014, nested canvases). 0 on success, 2 on success when the surface wants
/// another frame, 1 on failure.
pub fn readback(id: u32, width: f32, height: f32, scale: f32, now_ms: f64, out: &mut [u8]) -> u32 {
    let frame = Frame {
        width,
        height,
        scale,
        now_ms,
        children_generation: 0,
    };
    match with(|m| m.readback(id, &frame)).flatten() {
        Some((px, wants)) if out.len() >= px.data.len() => {
            out[..px.data.len()].copy_from_slice(&px.data);
            if wants {
                2
            } else {
                0
            }
        }
        Some((px, _)) => {
            ERROR.with(|s| {
                *s.borrow_mut() = format!(
                    "readback: {} bytes for {} pixels",
                    out.len(),
                    px.data.len() / 4
                )
            });
            1
        }
        None => 1,
    }
}

/// Whether a canvas has inputs it has not rendered.
pub fn dirty(id: u32) -> bool {
    with(|m| m.dirty(id)).unwrap_or(false)
}

/// Drop a canvas's surface.
pub fn destroy(id: u32) {
    with(|m| m.destroy(id));
}

/// The last failure's text: the ABI's own refusal when there is one
/// (reported once), else the module's.
pub fn error() -> String {
    let own = ERROR.with(|s| std::mem::take(&mut *s.borrow_mut()));
    if !own.is_empty() {
        return own;
    }
    with(|m| m.error().to_string()).unwrap_or_default()
}

/// The exports for one app's registry, C ABI (see LLP 1009 D2).
#[macro_export]
macro_rules! module {
    ($registry:expr) => {
        thread_local! {
            static EXACT_GPU_OUT: ::std::cell::RefCell<Vec<u8>> = const { ::std::cell::RefCell::new(Vec::new()) };
        }

        /// Create the device. 0 on success.
        #[no_mangle]
        pub extern "C" fn gpu_load() -> u32 {
            $crate::native::load(&$registry)
        }

        /// Create a canvas's surface on a CAMetalLayer. The canvas id, or 0.
        ///
        /// # Safety
        /// `name` is `len` bytes of UTF-8; `layer` a live CAMetalLayer that
        /// outlives the canvas.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_create(name: *const u8, len: usize, layer: *mut ::std::ffi::c_void, width: u32, height: u32) -> u32 {
            let Some(name) = (unsafe { $crate::native::bytes("gpu_create", name, len) }) else { return 0 };
            let Ok(name) = ::std::str::from_utf8(name) else { $crate::native::refuse("gpu_create: the name is not UTF-8"); return 0 };
            if layer.is_null() { $crate::native::refuse("gpu_create: a null layer"); return 0 }
            unsafe { $crate::native::create(name, layer, width, height) }
        }

        /// Bind inputs: `len` bytes of a JSON array. 0 on success.
        ///
        /// # Safety
        /// `values` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_bind(id: u32, values: *const u8, len: usize) -> u32 {
            let Some(text) = (unsafe { $crate::native::bytes("gpu_bind", values, len) }) else { return 1 };
            let Ok(text) = ::std::str::from_utf8(text) else { $crate::native::refuse("gpu_bind: the values are not UTF-8"); return 1 };
            $crate::native::bind(id, text)
        }

        /// Render one frame: 1 = wants another, 0 = done, 2 = failed.
        #[no_mangle]
        pub extern "C" fn gpu_render(id: u32, width: f32, height: f32, scale: f32, now_ms: f64) -> u32 {
            $crate::native::render(id, width, height, scale, now_ms)
        }

        /// Whether a canvas's surface samples its children (LLP 1014 D2): 1 or 0.
        #[no_mangle]
        pub extern "C" fn gpu_wants_children(id: u32) -> u32 {
            u32::from($crate::native::wants_children(id))
        }

        /// Whether a canvas's surface wants each child as its own texture
        /// (LLP 1014 D5): 1 or 0.
        #[no_mangle]
        pub extern "C" fn gpu_wants_children_each(id: u32) -> u32 {
            u32::from($crate::native::wants_children_each(id))
        }

        /// The `index`th direct child of a canvas: its frame in points and
        /// `len` bytes of premultiplied RGBA, `width`×`height` (LLP 1014 D5).
        /// 0 on success.
        ///
        /// # Safety
        /// `bytes` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_child(id: u32, index: u32, x: f32, y: f32, w: f32, h: f32, width: u32, height: u32, bytes: *const u8, len: usize) -> u32 {
            let Some(bytes) = (unsafe { $crate::native::bytes("gpu_child", bytes, len) }) else { return 1 };
            $crate::native::child(id, index, [x, y, w, h], width, height, bytes)
        }

        /// How many direct children a canvas has now (LLP 1014 D5). 0 on success.
        #[no_mangle]
        pub extern "C" fn gpu_children_count(id: u32, count: u32) -> u32 {
            $crate::native::children_count(id, count)
        }

        /// Where the surface put a child (LLP 1014 D5): ten floats into
        /// `out`, which is `len` floats long (at least ten) — the homography,
        /// row major, then the depth; 1 when placed, 0 when it is the
        /// kernel's frame or `out` cannot hold it.
        ///
        /// # Safety
        /// `out`, when non-null, points at `len` writable floats, at any
        /// alignment.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_placement(id: u32, index: u32, out: *mut f32, len: usize) -> u32 {
            if out.is_null() || len < 10 { $crate::native::refuse("gpu_placement: out is null or shorter than ten floats"); return 0 }
            match $crate::native::placement(id, index) {
                Some(p) => {
                    for (i, v) in p.iter().enumerate() {
                        // SAFETY: the caller's contract — `len` ≥ 10 floats at `out`, checked above.
                        unsafe { out.add(i).write_unaligned(*v) }
                    }
                    1
                }
                None => 0,
            }
        }

        /// The canvas's children, painted (LLP 1014 D3): `len` bytes of premultiplied
        /// RGBA, `width`×`height`, rows top-down. 0 on success.
        ///
        /// # Safety
        /// `bytes` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_texture(id: u32, width: u32, height: u32, bytes: *const u8, len: usize) -> u32 {
            let Some(bytes) = (unsafe { $crate::native::bytes("gpu_texture", bytes, len) }) else { return 1 };
            $crate::native::texture(id, width, height, bytes)
        }

        /// A canvas's picture as pixels into `out`, `len` bytes (LLP 1014,
        /// nested canvases). 0 on success, 2 when the surface also wants
        /// another frame, 1 on failure.
        ///
        /// # Safety
        /// `out` is `len` writable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_readback(id: u32, width: f32, height: f32, scale: f32, now_ms: f64, out: *mut u8, len: usize) -> u32 {
            let Some(out) = (unsafe { $crate::native::bytes_mut("gpu_readback", out, len) }) else { return 1 };
            $crate::native::readback(id, width, height, scale, now_ms, out)
        }

        /// Whether a canvas has unrendered inputs.
        #[no_mangle]
        pub extern "C" fn gpu_dirty(id: u32) -> u32 {
            u32::from($crate::native::dirty(id))
        }

        /// Drop a canvas's surface.
        #[no_mangle]
        pub extern "C" fn gpu_destroy(id: u32) {
            $crate::native::destroy(id)
        }

        /// The last failure's text: writes it to a module-owned buffer and
        /// returns its length; `gpu_error_ptr` returns the buffer.
        #[no_mangle]
        pub extern "C" fn gpu_error() -> u32 {
            let text = $crate::native::error();
            EXACT_GPU_OUT.with(|b| { *b.borrow_mut() = text.into_bytes(); b.borrow().len() as u32 })
        }

        /// The error buffer's address (valid until the next `gpu_error`).
        #[no_mangle]
        pub extern "C" fn gpu_error_ptr() -> *const u8 {
            EXACT_GPU_OUT.with(|b| b.borrow().as_ptr())
        }
    };
}
