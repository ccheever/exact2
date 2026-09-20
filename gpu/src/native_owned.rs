//! Ownership-only native exports. No device load, layer, render or texture ABI.
//! Linux retains its existing ownership and placement symbols.
#![allow(missing_docs)]

const BINARY_LIMIT: usize = 256 * 1024 * 1024;

pub fn binary_length(len: usize) -> bool {
    if len > BINARY_LIMIT {
        crate::native::refuse("surface carry/restore limit (256 MiB)");
        false
    } else {
        true
    }
}

/// # Safety
/// A range within the admission limit must be readable for the duration of the call.
pub unsafe fn text<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if len > 16_384 {
        crate::native::refuse("surface request exceeds 16384 bytes");
        return None;
    }
    // SAFETY: caller supplies the readable range, admitted before constructing a slice.
    let bytes = unsafe { crate::native::bytes("surface request", ptr, len) }?;
    match std::str::from_utf8(bytes)
        .map_err(|_| "surface request is not UTF-8".to_owned())
        .and_then(|text| crate::binding::admit(text).map(|()| text))
    {
        Ok(text) => Some(text),
        Err(error) => {
            crate::native::refuse(&error);
            None
        }
    }
}

pub fn output(text: String) -> Option<Vec<u8>> {
    if text.len() > 65_536 {
        crate::native::refuse("surface returned text limit (65536 bytes)");
        None
    } else {
        Some(text.into_bytes())
    }
}

pub fn error(mut text: String) -> String {
    if text.len() > 4096 {
        let mut end = 4096;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

// Bound retained delivery after every owned callback, including repeated undrained ticks.
pub(crate) fn bound_pending(module: &mut crate::Module) {
    for inst in module.instances.values_mut() {
        let bytes = inst
            .messages
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s.len()));
        if inst.messages.len() > 1024 || bytes.is_none_or(|n| n > 65_536) {
            inst.messages.clear();
            crate::native::refuse("surface messages limit (1024 / 65536 bytes)");
        }
        if inst.published.as_ref().is_some_and(|s| s.len() > 65_536) {
            inst.published = None;
            crate::native::refuse("surface returned text limit (65536 bytes)");
        }
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! native_owned_module {
    ($registry:expr) => {
        thread_local! {
            static EXACT_GPU_OUT: ::std::cell::RefCell<Vec<u8>> = const { ::std::cell::RefCell::new(Vec::new()) };
        }
        fn exact_owned_output(text: String, refused: u32) -> u32 {
            let Some(bytes) = $crate::native_owned::output(text) else {
                EXACT_GPU_OUT.with(|out| out.borrow_mut().clear());
                return refused;
            };
            let len = u32::try_from(bytes.len()).expect("admitted owned text length");
            EXACT_GPU_OUT.with(|out| *out.borrow_mut() = bytes);
            len
        }

        /// Recover the device; JSON outcome in gpu_out_ptr, returning its length.
        #[no_mangle]
        pub extern "C" fn gpu_recover() -> u32 {
            exact_owned_output($crate::native::recover(), 0)
        }

        /// Release all instances and module TLS before unloading the library.
        #[no_mangle]
        pub extern "C" fn gpu_unload() { $crate::native::unload(); }

        /// Advance owned state without rendering or agent inspection.
        #[no_mangle]
        pub extern "C" fn gpu_advance(id: u32, now_ms: f64) -> bool { $crate::native::advance(id, now_ms) }

        /// Load ownership without a GPU.
        #[no_mangle]
        pub extern "C" fn gpu_load_headless() {
            $crate::native::load_owned(&$registry);
            if ::std::env::var_os("EXACT_WORLD_TIMING").is_some() {
                eprintln!("exact-world-device: {{\"registry_id\":{}}}", $crate::native::device_registry_id());
            }
        }

        /// Create ownership without a presentation target. Zero means refusal.
        /// # Safety
        /// `name` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_create_headless(name: *const u8, len: usize) -> u32 {
            let Some(name) = (unsafe { $crate::native_owned::text(name, len) }) else { return 0 };
            $crate::native::create_headless(name)
        }

        /// Bind inputs: `len` bytes of a JSON array. 0 on success.
        ///
        /// # Safety
        /// `values` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_bind(id: u32, values: *const u8, len: usize) -> u32 {
            let Some(text) = (unsafe { $crate::native_owned::text(values, len) }) else { return 1 };
            $crate::native::bind(id, text)
        }

        /// Bind inputs at the host commit clock. 0 on success.
        ///
        /// # Safety
        /// `values` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_bind_at(id: u32, values: *const u8, len: usize, at_ms: f64) -> u32 {
            let Some(text) = (unsafe { $crate::native_owned::text(values, len) }) else { return 1 };
            $crate::native::bind_at(id, text, Some(at_ms))
        }

        /// Child composition: overlay=0, composite=1, composite/history=2, each=3.
        #[no_mangle]
        pub extern "C" fn gpu_children_mode(id: u32) -> u32 {
            $crate::native::children_mode(id)
        }

        /// The `index`th direct child of a canvas: its frame in points and
        /// `len` bytes of premultiplied RGBA, `width`×`height` (LLP 1014 D5).
        /// 0 on success.
        ///
        /// # Safety
        /// `name` and `bytes` are readable for their corresponding byte lengths.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_child_view(id: u32, index: u32, name: *const u8, name_len: usize, x: f32, y: f32, w: f32, h: f32, width: u32, height: u32, bytes: *const u8, len: usize) -> u32 {
            let Some(name) = (unsafe { $crate::native_owned::text(name, name_len) }) else { return 1 };
            let Some(bytes) = (unsafe { $crate::native::bytes("gpu_child_view", bytes, len) }) else { return 1 };
            $crate::native::child(id, index, name, [x, y, w, h], [width, height], bytes)
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
                    if len >= 16 {
                        for (i, v) in p.clip_depth.iter().flatten().enumerate() {
                            // SAFETY: this optional extension checks all sixteen output floats.
                            unsafe { out.add(10 + i).write_unaligned(*v) }
                        }
                    }
                    1
                }
                None => 0,
            }
        }

        /// Whether a canvas wants raw input.
        #[no_mangle]
        pub extern "C" fn gpu_wants_input(id: u32) -> u32 { u32::from($crate::native::wants_input(id)) }

        /// Deliver one JSON event; 0 on success, 1 on refusal.
        /// # Safety
        /// `text` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_input(id: u32, text: *const u8, len: usize) -> u32 {
            let Some(text) = (unsafe { $crate::native_owned::text(text, len) }) else { return 1 };
            u32::from(!$crate::native::input(id, text))
        }

        /// Requested paths as JSON in the output buffer.
        #[no_mangle]
        pub extern "C" fn gpu_assets(id: u32) -> u32 {
            let text = $crate::native::assets(id);
            exact_owned_output(text, 0)
        }
        /// Deliver requested bytes; null data with zero length means missing. True on success.
        /// # Safety
        /// name and non-null data point to readable ranges of the supplied lengths.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_asset(id: u32, name: *const u8, name_len: usize, data: *const u8, len: usize) -> bool {
            let Some(name) = (unsafe { $crate::native_owned::text(name, name_len) }) else { return false };
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
            let Some(name) = (unsafe { $crate::native_owned::text(name, name_len) }) else { return false };
            let Some(reason) = (unsafe { $crate::native_owned::text(reason, reason_len) }) else { return false };
            $crate::native::asset_failed(id, name, reason)
        }


        /// Carry in the output buffer; MAX means nothing, MAX-1 a refusal; zero is empty.
        #[no_mangle]
        pub extern "C" fn gpu_carry(id: u32) -> u32 {
            EXACT_GPU_OUT.with(|b| b.borrow_mut().clear());
            match $crate::native::carry(id) {
                Ok(Some(bytes)) => {
                    if !$crate::native_owned::binary_length(bytes.len()) { return u32::MAX - 1 }
                    let Some(len) = $crate::native::carry_length(bytes.len()) else { return u32::MAX - 1 };
                    EXACT_GPU_OUT.with(|b| *b.borrow_mut() = bytes);
                    len
                },
                Ok(None) => u32::MAX,
                Err(error) => { $crate::native::refuse(&error.0); u32::MAX - 1 },
            }
        }

        /// Restore state; true on success, false with gpu_error on refusal.
        /// # Safety
        /// `data` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_restore(id: u32, data: *const u8, len: usize, mode: u32) -> bool {
            if !$crate::native_owned::binary_length(len) { return false }
            if mode > 1 { $crate::native::refuse("invalid restore mode"); return false }
            let Some(bytes) = (unsafe { $crate::native::bytes("gpu_restore", data, len) }) else { return false };
            $crate::native::restore(id, bytes, mode)
        }

        /// Changed public record in the output buffer; u32::MAX means unchanged.
        #[no_mangle]
        pub extern "C" fn gpu_published(id: u32) -> u32 {
            match $crate::native::published(id) {
                Some(text) => exact_owned_output(text, u32::MAX),
                None => u32::MAX,
            }
        }

        /// Drain messages into the output buffer; u32::MAX means no messages.
        #[no_mangle]
        pub extern "C" fn gpu_messages(id: u32) -> u32 {
            match $crate::native::messages(id) {
                Some(text) => exact_owned_output(text, u32::MAX),
                None => u32::MAX,
            }
        }

        /// Ask the surface; returns the output byte length, zero for no answer.
        /// # Safety
        /// `text` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_agent(id: u32, text: *const u8, len: usize) -> u32 {
            EXACT_GPU_OUT.with(|b| b.borrow_mut().clear());
            let Some(text) = (unsafe { $crate::native_owned::text(text, len) }) else { return 0 };
            let text = $crate::native::agent(id, text);
            exact_owned_output(text, 0)
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
            let text = $crate::native_owned::error($crate::native::error());
            exact_owned_output(text, 0)
        }

        /// The shared output address (valid until the next carry, published, agent, messages or error call).
        #[no_mangle]
        pub extern "C" fn gpu_error_ptr() -> *const u8 {
            EXACT_GPU_OUT.with(|b| b.borrow().as_ptr())
        }
    };
}
