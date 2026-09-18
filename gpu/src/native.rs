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

/// Drain requested asset paths as JSON.
pub fn assets(id: u32) -> String {
    with(|m| json::strings(&m.take_assets(id))).unwrap_or_else(|| "[]".into())
}
/// Deliver named bytes, including a missing file, without requiring a device.
pub fn asset(id: u32, name: &str, bytes: Option<&[u8]>) -> bool {
    with(|m| m.asset(id, name, bytes.ok_or(crate::AssetError::Missing))).unwrap_or(false)
}

/// Deliver a terminal transport failure.
pub fn asset_failed(id: u32, name: &str, reason: &str) -> bool {
    with(|m| m.asset(id, name, Err(crate::AssetError::Failed(reason.into())))).unwrap_or(false)
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
    if len == 0 {
        return Some(&[]);
    }
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
    if len == 0 {
        return Some(&mut []);
    }
    if ptr.is_null() {
        refuse(&format!("{what}: a null pointer for {len} bytes"));
        return None;
    }
    // SAFETY: the caller's contract, the pointer checked.
    Some(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
}

/// u32::MAX is reserved for no carry; never truncate a host-sized length.
pub fn carry_length(len: usize) -> Option<u32> {
    match u32::try_from(len) {
        Ok(n) if n != u32::MAX => Some(n),
        _ => {
            refuse("gpu_carry: carry exceeds ABI byte limit");
            None
        }
    }
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

/// Recover the loaded module without replacing its surface table. JSON outcome.
pub fn recover() -> String {
    with(|m| crate::block_on(m.recover()))
        .unwrap_or_else(|| Err("GPU module not loaded".into()))
        .unwrap_or_else(|error| {
            refuse(&error);
            let quoted = json::strings(&[error]);
            format!(
                "{{\"status\":\"failed\",\"error\":{}}}",
                &quoted[1..quoted.len() - 1]
            )
        })
}

/// Load surface ownership only; no adapter is requested.
pub fn load_headless(registry: &'static Registry) {
    let mut module = Module::new(registry);
    module.set_seekable(true);
    MODULE.with(|m| *m.borrow_mut() = Some(module));
}

/// Create a surface without presentation; zero means refusal.
pub fn create_headless(name: &str) -> u32 {
    with(|m| m.create_headless(name)).flatten().unwrap_or(0)
}

/// Drop the device and every canvas; [`load`] may run again. A thread that
/// loaded a module must not leave it for thread-local teardown: wgpu's own
/// thread-locals may already be gone by then, and dropping a device without
/// them aborts the process (found by the first test to load one off the main thread).
pub fn unload() {
    let module = MODULE.with(|m| m.borrow_mut().take());
    drop(module);
}

/// Create a canvas's surface on a `CAMetalLayer`. Returns the canvas id,
/// or 0 on failure.
///
/// # Safety
/// `layer` must be a live `CAMetalLayer` that outlives the canvas.
#[cfg(any(target_os = "macos", target_os = "ios"))]
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

/// Off Apple there is no layer to present to yet (LLP 1015 §7): the module
/// still loads, and a surface runs through [`crate::fixture`]; a canvas on a
/// platform target is refused by name.
///
/// # Safety
/// None: `layer` is never read.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub unsafe fn create(name: &str, _layer: *mut c_void, _width: u32, _height: u32) -> u32 {
    refuse(&format!(
        "gpu_create `{name}`: this platform has no presentable target yet"
    ));
    0
}

/// Register the text of shader `name` (LLP 1030 D8): validated, its
/// interface checked against the one this module's Rust binds. Returns 0 on
/// success, 1 on a refusal (see [`error`]). The module must be loaded.
pub fn shader(name: &str, text: &str) -> u32 {
    match with(|m| m.set_shader(name, text.to_string())) {
        Some(true) => 0,
        Some(false) => 1,
        None => {
            refuse("gpu_shader: the module is not loaded (gpu_load first)");
            1
        }
    }
}

/// Validate without replacing a live shader, during app generation preparation.
pub fn validate_shader(name: &str, text: &str) -> u32 {
    let result = with(|m| {
        let digest = m
            .expected_digest(name)
            .ok_or_else(|| format!("no shader named `{name}` in this module"))?;
        crate::shaders::validate_shader(name, text, Some(digest)).map(|_| ())
    })
    .unwrap_or_else(|| Err("GPU module not loaded".into()));
    match result {
        Ok(()) => 0,
        Err(error) => {
            refuse(&error);
            1
        }
    }
}

/// Bind inputs (a JSON array of values). Returns 0 on success.
pub fn bind(id: u32, values: &str) -> u32 {
    bind_at(id, values, None)
}

/// Bind inputs at an optional host commit clock.
pub fn bind_at(id: u32, values: &str, at_ms: Option<f64>) -> u32 {
    let values = match json::parse_values(values) {
        Ok(v) => v,
        Err(e) => {
            ERROR.with(|s| *s.borrow_mut() = e);
            return 1;
        }
    };
    match with(|m| m.bind(id, &values, at_ms)) {
        Some(true) => 0,
        _ => 1,
    }
}

/// Render one frame. Returns 1 when the surface wants another frame, 0
/// otherwise, 2 on failure, 3 when presentation has no device.
pub fn render(id: u32, width: f32, height: f32, scale: f32, now_ms: f64) -> u32 {
    let frame = Frame {
        width,
        height,
        scale,
        now_ms,
        children_generation: 0,
        seekable: false,
        period_ms: 0.0,
        shader_generation: 0,
    };
    match with(|m| m.render(id, &frame)).flatten() {
        Some(true) => 1,
        Some(false) => 0,
        None if with(|m| m.instances.contains_key(&id) && !m.has_device(id)).unwrap_or(false) => 3,
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
pub fn placement(id: u32, index: u32) -> Option<crate::Placement> {
    with(|m| m.placement(id, index as usize)).flatten()
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

/// The module's GPU work complete (LLP 1008 §9). 0 on success.
pub fn sync() -> u32 {
    match with(|m| m.sync()) {
        Some(true) => 0,
        _ => 1,
    }
}

/// The canvas's children as a Metal texture the host rendered, imported as
/// it is (LLP 1008 §9). 0 on success.
///
/// # Safety
/// `raw` is a live `MTLTexture` of `width`×`height`, `rgba8Unorm`, kept
/// alive by the host while the canvas lives.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub unsafe fn texture_from_metal(id: u32, width: u32, height: u32, raw: *mut c_void) -> u32 {
    // SAFETY: the caller's contract, passed on.
    match with(|m| unsafe { m.texture_from_metal(id, width, height, raw) }) {
        Some(true) => 0,
        _ => 1,
    }
}

/// A canvas's picture as pixels into `out` — `width`×`height` points at
/// `scale`, RGBA rows top-down, `out` at least the pixel count × 4 (LLP
/// 1014, nested canvases). 0 on success, 2 on success when the surface wants
/// another frame, 1 on failure, 3 when the device is unavailable.
pub fn readback(id: u32, width: f32, height: f32, scale: f32, now_ms: f64, out: &mut [u8]) -> u32 {
    let frame = Frame {
        width,
        height,
        scale,
        now_ms,
        children_generation: 0,
        seekable: false,
        period_ms: 0.0,
        shader_generation: 0,
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
        None if with(|m| m.instances.contains_key(&id) && m.gpu().is_none()).unwrap_or(false) => 3,
        None => 1,
    }
}

/// Whether a canvas wants raw input.
pub fn wants_input(id: u32) -> bool {
    with(|m| m.wants_input(id)).unwrap_or(false)
}

/// Deliver a JSON device event. True on success.
pub fn input(id: u32, event: &str) -> bool {
    with(|m| m.input_json(id, event)).unwrap_or(false)
}

/// Capture state; None is distinct from a zero-byte carry.
pub fn carry(id: u32) -> Option<Vec<u8>> {
    with(|m| m.carry(id)).flatten()
}

/// Restore state. False leaves the surface unchanged; error explains why.
pub fn restore(id: u32, bytes: &[u8], mode: u32) -> bool {
    let Ok(mode) = crate::Restore::from_code(mode) else {
        return false;
    };
    with(|m| m.restore(id, bytes, mode)).unwrap_or(false)
}

/// Take the latest changed public record, if any.
pub fn published(id: u32) -> Option<String> {
    with(|m| m.take_published(id)).flatten()
}

/// Drain posted messages as a JSON array.
pub fn messages(id: u32) -> Option<String> {
    let messages = with(|m| m.take_messages(id)).unwrap_or_default();
    (!messages.is_empty()).then(|| json::strings(&messages))
}

/// Ask the surface; an empty string means no answer.
pub fn agent(id: u32, request: &str) -> String {
    with(|m| m.agent(id, request)).flatten().unwrap_or_default()
}

/// Deliver a host lifecycle notification without advancing the surface.
pub fn lifecycle(id: u32, code: u32) {
    with(|m| m.lifecycle(id, code));
}

/// Set the host's clock ownership.
pub fn seekable(on: bool) {
    with(|m| m.set_seekable(on));
}

/// The display's frame period in milliseconds (0 = unknown), for every frame after.
pub fn period(period_ms: f64) {
    with(|m| m.set_period(period_ms));
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
    with(|m| m.take_error()).unwrap_or_default()
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

        /// Recover the device; JSON outcome in gpu_out_ptr, returning its length.
        #[no_mangle]
        pub extern "C" fn gpu_recover() -> u32 {
            let bytes = $crate::native::recover().into_bytes();
            let len = bytes.len() as u32;
            EXACT_GPU_OUT.with(|out| *out.borrow_mut() = bytes);
            len
        }

        /// Release all instances and module TLS before unloading the library.
        #[no_mangle]
        pub extern "C" fn gpu_unload() { $crate::native::unload(); }

        /// Load ownership without a GPU.
        #[no_mangle]
        pub extern "C" fn gpu_load_headless() { $crate::native::load_headless(&$registry); }

        /// Create ownership without a presentation target. Zero means refusal.
        /// # Safety
        /// `name` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_create_headless(name: *const u8, len: usize) -> u32 {
            let Some(name) = (unsafe { $crate::native::bytes("gpu_create_headless", name, len) }) else { return 0 };
            let Ok(name) = ::std::str::from_utf8(name) else { $crate::native::refuse("gpu_create_headless: invalid UTF-8"); return 0 };
            $crate::native::create_headless(name)
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

        /// Clear the previous complete shader namespace after app acceptance.
        #[no_mangle]
        pub extern "C" fn gpu_shaders_clear() { $crate::shaders::clear_shaders(); }
        /// Validate source and interface without changing the current shader.
        /// # Safety
        /// Each pointer is readable for its corresponding byte length.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_shader_validate(name: *const u8, name_len: usize, text: *const u8, text_len: usize) -> u32 {
            let Some(name) = (unsafe { $crate::native::bytes("gpu_shader_validate", name, name_len) }) else { return 1 };
            let Some(text) = (unsafe { $crate::native::bytes("gpu_shader_validate", text, text_len) }) else { return 1 };
            let (Ok(name), Ok(text)) = (::std::str::from_utf8(name), ::std::str::from_utf8(text)) else { return 1 };
            $crate::native::validate_shader(name, text)
        }
        /// Register validated shader text for the next surface frame.
        /// # Safety
        /// Each pointer is readable for its corresponding byte length.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_shader(name: *const u8, name_len: usize, text: *const u8, text_len: usize) -> u32 {
            let Some(name) = (unsafe { $crate::native::bytes("gpu_shader", name, name_len) }) else { return 1 };
            let Ok(name) = ::std::str::from_utf8(name) else { $crate::native::refuse("gpu_shader: the name is not UTF-8"); return 1 };
            let Some(text) = (unsafe { $crate::native::bytes("gpu_shader", text, text_len) }) else { return 1 };
            let Ok(text) = ::std::str::from_utf8(text) else { $crate::native::refuse("gpu_shader: the text is not UTF-8"); return 1 };
            $crate::native::shader(name, text)
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

        /// Bind inputs at the host commit clock. 0 on success.
        ///
        /// # Safety
        /// `values` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_bind_at(id: u32, values: *const u8, len: usize, at_ms: f64) -> u32 {
            let Some(text) = (unsafe { $crate::native::bytes("gpu_bind_at", values, len) }) else { return 1 };
            let Ok(text) = ::std::str::from_utf8(text) else { $crate::native::refuse("gpu_bind_at: the values are not UTF-8"); return 1 };
            $crate::native::bind_at(id, text, Some(at_ms))
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
        /// kernel's frame or `out` cannot hold it; 2 means hidden, with `out` untouched.
        ///
        /// # Safety
        /// `out`, when non-null, points at `len` writable floats, at any
        /// alignment.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_placement(id: u32, index: u32, out: *mut f32, len: usize) -> u32 {
            if out.is_null() || len < 10 { $crate::native::refuse("gpu_placement: out is null or shorter than ten floats"); return 0 }
            match $crate::native::placement(id, index) {
                Some(p) if p.hidden => 2,
                Some(p) => {
                    for (i, v) in p.homography.iter().chain(std::iter::once(&p.depth)).enumerate() {
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

        /// The module's GPU work complete, so a texture it was reading may be
        /// drawn into (LLP 1008 §9). 0 on success.
        #[no_mangle]
        pub extern "C" fn gpu_sync() -> u32 {
            $crate::native::sync()
        }

        /// The canvas's children as a Metal texture the host rendered (LLP
        /// 1008 §9): imported as it is. 0 on success.
        ///
        /// # Safety
        /// `raw` is a live `MTLTexture` of that size, `rgba8Unorm`, alive
        /// while the canvas is.
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        #[no_mangle]
        pub unsafe extern "C" fn gpu_texture_metal(id: u32, width: u32, height: u32, raw: *mut ::std::ffi::c_void) -> u32 {
            unsafe { $crate::native::texture_from_metal(id, width, height, raw) }
        }

        /// A canvas's picture as pixels into `out`, `len` bytes (LLP 1014,
        /// nested canvases). 0 on success, 2 when the surface also wants
        /// another frame, 1 on failure, 3 when the device is unavailable.
        ///
        /// # Safety
        /// `out` is `len` writable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_readback(id: u32, width: f32, height: f32, scale: f32, now_ms: f64, out: *mut u8, len: usize) -> u32 {
            let Some(out) = (unsafe { $crate::native::bytes_mut("gpu_readback", out, len) }) else { return 1 };
            $crate::native::readback(id, width, height, scale, now_ms, out)
        }

        /// Whether a canvas wants raw input.
        #[no_mangle]
        pub extern "C" fn gpu_wants_input(id: u32) -> u32 { u32::from($crate::native::wants_input(id)) }

        /// Deliver one JSON event; 0 on success, 1 on refusal.
        /// # Safety
        /// `text` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_input(id: u32, text: *const u8, len: usize) -> u32 {
            let Some(text) = (unsafe { $crate::native::bytes("gpu_input", text, len) }) else { return 1 };
            let Ok(text) = ::std::str::from_utf8(text) else { $crate::native::refuse("gpu_input: the event is not UTF-8"); return 1 };
            u32::from(!$crate::native::input(id, text))
        }

        /// Requested paths as JSON in the output buffer.
        #[no_mangle]
        pub extern "C" fn gpu_assets(id: u32) -> u32 {
            let text = $crate::native::assets(id);
            EXACT_GPU_OUT.with(|b| { *b.borrow_mut() = text.into_bytes(); b.borrow().len() as u32 })
        }
        /// Deliver requested bytes; null data with zero length means missing. True on success.
        /// # Safety
        /// name and non-null data point to readable ranges of the supplied lengths.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_asset(id: u32, name: *const u8, name_len: usize, data: *const u8, len: usize) -> bool {
            let Some(name) = (unsafe { $crate::native::bytes("gpu_asset name", name, name_len) }) else { return false };
            let Ok(name) = ::std::str::from_utf8(name) else { $crate::native::refuse("gpu_asset: invalid UTF-8 name"); return false };
            let bytes = if data.is_null() && len == 0 { None } else {
                let Some(bytes) = (unsafe { $crate::native::bytes("gpu_asset", data, len) }) else { return false };
                Some(bytes)
            };
            $crate::native::asset(id, name, bytes)
        }
        /// # Safety
        /// Both strings must be readable UTF-8 byte slices for this call.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn gpu_asset_failed(id: u32, name: *const u8, name_len: usize, reason: *const u8, reason_len: usize) -> bool {
            let Some(name) = (unsafe { $crate::native::bytes("asset name", name, name_len) }) else { return false };
            let Some(reason) = (unsafe { $crate::native::bytes("asset reason", reason, reason_len) }) else { return false };
            let (Ok(name), Ok(reason)) = (::std::str::from_utf8(name), ::std::str::from_utf8(reason)) else { return false };
            $crate::native::asset_failed(id, name, reason)
        }


        /// Carry in the output buffer; u32::MAX means nothing, zero is an empty carry.
        #[no_mangle]
        pub extern "C" fn gpu_carry(id: u32) -> u32 {
            EXACT_GPU_OUT.with(|b| b.borrow_mut().clear());
            match $crate::native::carry(id) {
                Some(bytes) => {
                    let Some(len) = $crate::native::carry_length(bytes.len()) else { return u32::MAX };
                    EXACT_GPU_OUT.with(|b| *b.borrow_mut() = bytes);
                    len
                },
                None => u32::MAX,
            }
        }

        /// Restore state; true on success, false with gpu_error on refusal.
        /// # Safety
        /// `data` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_restore(id: u32, data: *const u8, len: usize, mode: u32) -> bool {
            let Some(bytes) = (unsafe { $crate::native::bytes("gpu_restore", data, len) }) else { return false };
            $crate::native::restore(id, bytes, mode)
        }

        /// Changed public record in the output buffer; u32::MAX means unchanged.
        #[no_mangle]
        pub extern "C" fn gpu_published(id: u32) -> u32 {
            match $crate::native::published(id) {
                Some(text) => EXACT_GPU_OUT.with(|b| { *b.borrow_mut() = text.into_bytes(); b.borrow().len() as u32 }),
                None => u32::MAX,
            }
        }

        /// Drain messages into the output buffer; u32::MAX means no messages.
        #[no_mangle]
        pub extern "C" fn gpu_messages(id: u32) -> u32 {
            match $crate::native::messages(id) {
                Some(text) => EXACT_GPU_OUT.with(|b| { *b.borrow_mut() = text.into_bytes(); b.borrow().len() as u32 }),
                None => u32::MAX,
            }
        }

        /// Ask the surface; returns the output byte length, zero for no answer.
        /// # Safety
        /// `text` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_agent(id: u32, text: *const u8, len: usize) -> u32 {
            EXACT_GPU_OUT.with(|b| b.borrow_mut().clear());
            let Some(text) = (unsafe { $crate::native::bytes("gpu_agent", text, len) }) else { return 0 };
            let Ok(text) = ::std::str::from_utf8(text) else { $crate::native::refuse("gpu_agent: the request is not UTF-8"); return 0 };
            let text = $crate::native::agent(id, text);
            EXACT_GPU_OUT.with(|b| { *b.borrow_mut() = text.into_bytes(); b.borrow().len() as u32 })
        }

        /// Host lifecycle code; unknown codes are ignored.
        #[no_mangle]
        pub extern "C" fn gpu_lifecycle(id: u32, code: u32) { $crate::native::lifecycle(id, code); }
        /// Set the host clock ownership.
        #[no_mangle]
        pub extern "C" fn gpu_seekable(on: bool) { $crate::native::seekable(on); }

        /// The display's frame period in milliseconds, 0 while unknown.
        #[no_mangle]
        pub extern "C" fn gpu_period(period_ms: f64) { $crate::native::period(period_ms); }

        /// Output address, valid until the next carry, published, agent, messages or error call.
        #[no_mangle]
        pub extern "C" fn gpu_out_ptr() -> *const u8 { EXACT_GPU_OUT.with(|b| b.borrow().as_ptr()) }

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

        /// The shared output address (valid until the next carry, published, agent, messages or error call).
        #[no_mangle]
        pub extern "C" fn gpu_error_ptr() -> *const u8 {
            EXACT_GPU_OUT.with(|b| b.borrow().as_ptr())
        }
    };
}

#[cfg(test)]
mod placement_abi_tests {
    use super::{bind, child, create_headless, error, load, recover, unload, with};
    use crate::{wgpu, Frame, Placement, Registry, Surface, SurfaceError, Value};
    #[derive(Default)]
    struct Sign {
        frame: [f32; 4],
        preparations: usize,
        retired: bool,
    }
    impl Surface for Sign {
        fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn render(
            &mut self,
            _: &Frame,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &wgpu::TextureView,
            _: wgpu::TextureFormat,
        ) -> bool {
            false
        }
        fn wants_children_each(&self) -> bool {
            !self.retired
        }
        fn prepare_assets(&mut self, _: &wgpu::Device, _: &wgpu::Queue, _: wgpu::TextureFormat) {
            self.preparations += 1;
        }
        fn agent(&mut self, request: &str) -> Option<String> {
            self.retired = request == "retire";
            Some(format!("{{\"preparations\":{}}}", self.preparations))
        }
        fn child(&mut self, _: usize, _: Option<&wgpu::TextureView>, frame: [f32; 4]) {
            self.frame = frame;
        }
        fn placement(&self, index: usize) -> Option<Placement> {
            (index != 0).then_some(Placement {
                hidden: index == 2,
                homography: [self.frame[2]; 9],
                depth: -3.,
            })
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("sign", 0, || Box::<Sign>::default())],
        shaders: &[],
    };
    crate::module!(REGISTRY);
    #[test]
    fn retiring_each_releases_textures_and_zero_frame_releases_a_capture() {
        assert_eq!(load(&REGISTRY), 0);
        let id = create_headless("sign");
        assert_eq!(bind(id, "[]"), 0);
        for _ in 0..3 {
            with(|m| m.agent(id, "active"));
            assert_eq!(child(id, 0, [0., 0., 20., 20.], 1, 1, &[255; 4]), 0);
            assert_eq!(
                with(|m| m.instances[&id].each.iter().filter(|t| t.is_some()).count()),
                Some(1)
            );
            assert_eq!(child(id, 0, [0.; 4], 0, 0, &[]), 0);
            assert_eq!(
                with(|m| m.instances[&id].each.iter().filter(|t| t.is_some()).count()),
                Some(0)
            );
            assert_eq!(child(id, 0, [0., 0., 20., 20.], 1, 1, &[255; 4]), 0);
            with(|m| m.agent(id, "retire"));
            assert_eq!(gpu_wants_children_each(id), 0);
            assert_eq!(with(|m| m.instances[&id].each.len()), Some(0));
            assert_eq!(
                with(|m| m.placement(id, 1).unwrap().homography),
                Some([0.; 9])
            );
        }
        unload();
    }

    #[test]
    fn destroyed_device_recovers_without_replacing_the_surface_table() {
        if load(&REGISTRY) != 0 {
            eprintln!("SKIP native recovery: {}", error());
            return;
        }
        let id = create_headless("sign");
        assert_ne!(id, 0);
        assert_eq!(bind(id, "[]"), 0);
        assert_eq!(child(id, 0, [10., 20., 100., 50.], 0, 0, &[]), 0);
        with(|m| {
            m.instances
                .get_mut(&id)
                .unwrap()
                .messages
                .push("pending".into())
        });
        with(|m| m.gpu().unwrap().device.destroy());
        let result = recover();
        assert!(result.contains("recovered"), "{result}");
        assert!(result.contains("\"preparations\":1"), "{result}");
        assert_eq!(
            with(|m| m.instances.get(&id).unwrap().messages.clone()).unwrap(),
            ["pending"]
        );
        assert!(with(|m| m.instances.get(&id).unwrap().bound).unwrap());
        assert_eq!(
            with(|m| m.placement(id, 1).unwrap().homography).unwrap(),
            [100.; 9]
        );
        assert!(with(|m| m.gpu().is_some()).unwrap());
        unload();
    }

    #[test]
    fn frame_only_child_and_explicit_hidden_out_do_not_need_a_device() {
        gpu_load_headless();
        let report = recover();
        assert!(report.contains("no device"), "{report}");
        // SAFETY: name bytes and output arrays live across these synchronous ABI calls.
        unsafe {
            let id = gpu_create_headless(b"sign".as_ptr(), 4);
            assert_ne!(id, 0);
            assert_eq!(gpu_wants_children_each(id), 1);
            assert_eq!(
                gpu_child(id, 0, 10., 20., 100., 50., 0, 0, std::ptr::null(), 0),
                0
            );
            let mut out = [77.; 10];
            assert_eq!(gpu_placement(id, 0, out.as_mut_ptr(), out.len()), 0);
            assert_eq!(out, [77.; 10]);
            assert_eq!(gpu_placement(id, 2, out.as_mut_ptr(), out.len()), 2);
            assert_eq!(out, [77.; 10]);
            assert_eq!(gpu_placement(id, 1, out.as_mut_ptr(), out.len()), 1);
            assert_eq!(out[..9], [100.; 9]);
            assert_eq!(out[9], -3.);
        }
        gpu_unload();
    }
}

#[cfg(test)]
mod device_loss_tests {
    use crate::*;
    struct Probe;
    impl Surface for Probe {
        fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn render(
            &mut self,
            _: &Frame,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &wgpu::TextureView,
            _: wgpu::TextureFormat,
        ) -> bool {
            panic!("lost device rendered")
        }
        fn agent(&mut self, _: &str) -> Option<String> {
            Some("alive".into())
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("probe", 0, || Box::new(Probe))],
        shaders: &[],
    };
    #[test]
    fn device_loss_guards_each_gpu_entry_before_any_other_call() {
        for operation in 0..7 {
            let Ok(gpu) = fixture::device() else { return };
            let mut m = Module::new(&REGISTRY);
            m.set_gpu(gpu);
            let id = m.create_headless("probe").unwrap();
            m.bind(id, &[], None);
            // Model the asynchronous callback, without calling lose_device first.
            m.device_lost.store(true, Ordering::Release);
            match operation {
                0 => assert!(!m.child(id, 0, [0., 0., 1., 1.], 1, 1, &[0; 4])),
                1 => assert!(!m.texture(id, 1, 1, &[0; 4])),
                2 => assert!(m
                    .readback(
                        id,
                        &Frame {
                            width: 1.,
                            height: 1.,
                            scale: 1.,
                            now_ms: 0.,
                            seekable: true,
                            period_ms: 0.,
                            children_generation: 0,
                            shader_generation: 0
                        }
                    )
                    .is_none()),
                3 => assert!(!m.dirty(id)),
                4 => assert!(!m.sync()),
                5 => assert!(m.gpu().is_none()),
                _ => {
                    #[cfg(any(target_os = "macos", target_os = "ios"))]
                    // A sentinel must never be retained/imported after loss.
                    assert!(!unsafe { m.texture_from_metal(id, 1, 1, std::ptr::dangling_mut()) });
                }
            }
            assert_eq!(m.agent(id, "state").as_deref(), Some("alive"));
            assert!(!m.dirty(id));
        }
    }
}
