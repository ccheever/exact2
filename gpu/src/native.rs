//! The native ABI: what the presenter `dlopen`s (LLP 1009 D2).
//!
//! One thread-local module; calls on the main thread. Strings are UTF-8
//! bytes with lengths. The one `unsafe` this module owns is the platform
//! target handoff: a `CAMetalLayer` pointer the presenter registered, valid
//! until it calls `destroy`.

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
    };
    match with(|m| m.render(id, &frame)).flatten() {
        Some(true) => 1,
        Some(false) => 0,
        None => 2,
    }
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

/// Whether a canvas has inputs it has not rendered.
pub fn dirty(id: u32) -> bool {
    with(|m| m.dirty(id)).unwrap_or(false)
}

/// Drop a canvas's surface.
pub fn destroy(id: u32) {
    with(|m| m.destroy(id));
}

/// The last failure's text.
pub fn error() -> String {
    let own = ERROR.with(|s| s.borrow().clone());
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
            let name = unsafe { ::std::slice::from_raw_parts(name, len) };
            let Ok(name) = ::std::str::from_utf8(name) else { return 0 };
            unsafe { $crate::native::create(name, layer, width, height) }
        }

        /// Bind inputs: `len` bytes of a JSON array. 0 on success.
        ///
        /// # Safety
        /// `values` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_bind(id: u32, values: *const u8, len: usize) -> u32 {
            let text = unsafe { ::std::slice::from_raw_parts(values, len) };
            let Ok(text) = ::std::str::from_utf8(text) else { return 1 };
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

        /// The canvas's children, painted (LLP 1014 D3): `len` bytes of premultiplied
        /// RGBA, `width`×`height`, rows top-down. 0 on success.
        ///
        /// # Safety
        /// `bytes` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_texture(id: u32, width: u32, height: u32, bytes: *const u8, len: usize) -> u32 {
            let bytes = unsafe { ::std::slice::from_raw_parts(bytes, len) };
            $crate::native::texture(id, width, height, bytes)
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
