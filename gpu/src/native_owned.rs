//! Ownership-only native exports. No device load, layer, render or texture ABI.
//! Linux retains its existing ownership and placement symbols.

#[doc(hidden)]
#[macro_export]
macro_rules! native_owned_module {
    ($registry:expr) => {
        thread_local! {
            static EXACT_GPU_OUT: ::std::cell::RefCell<Vec<u8>> = const { ::std::cell::RefCell::new(Vec::new()) };
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

        /// Advance owned state without rendering or agent inspection.
        #[no_mangle]
        pub extern "C" fn gpu_advance(id: u32, now_ms: f64) -> bool { $crate::native::advance(id, now_ms) }

        /// Load ownership without a GPU.
        #[no_mangle]
        pub extern "C" fn gpu_load_headless() {
            $crate::native::load_headless(&$registry);
            if ::std::env::var_os("EXACT_WORLD_TIMING").is_some() {
                eprintln!("exact-world-device: {{\"registry_id\":{}}}", $crate::native::device_registry_id());
            }
        }

        /// Create ownership without a presentation target. Zero means refusal.
        /// # Safety
        /// `name` is `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn gpu_create_headless(name: *const u8, len: usize) -> u32 {
            let Some(name) = (unsafe { $crate::native::bytes("gpu_create_headless", name, len) }) else { return 0 };
            let Ok(name) = ::std::str::from_utf8(name) else { $crate::native::refuse("gpu_create_headless: invalid UTF-8"); return 0 };
            $crate::native::create_headless(name)
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
            let Some(name) = (unsafe { $crate::native::bytes("gpu_child_view", name, name_len) }) else { return 1 };
            let Ok(name) = ::std::str::from_utf8(name) else { $crate::native::refuse("gpu_child_view: invalid UTF-8"); return 1 };
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


        /// Carry in the output buffer; MAX means nothing, MAX-1 a refusal; zero is empty.
        #[no_mangle]
        pub extern "C" fn gpu_carry(id: u32) -> u32 {
            EXACT_GPU_OUT.with(|b| b.borrow_mut().clear());
            match $crate::native::carry(id) {
                Ok(Some(bytes)) => {
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
