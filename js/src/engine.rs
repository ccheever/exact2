//! The engine behind the executor: the lean Hermes VM through the shim, or
//! a stub that refuses when this binary links no engine (see `build.rs`).

#[cfg(exact_js_engine)]
mod real {
    use std::ffi::{c_char, c_void, CStr, CString};

    /// The one door from the module into Rust: `(ctx, op, a, b, out)`; 0
    /// with `out` null is `undefined`, 0 with a string is that string, and
    /// anything else throws the string in the module.
    pub type HostFn = unsafe extern "C" fn(
        *mut c_void,
        u32,
        *const c_char,
        *const c_char,
        *mut *mut c_char,
    ) -> i32;

    /// The door for byte input (`__exact_bytes`, LLP 1069.005 D1):
    /// `(ctx, op, a, bytes, len, out)`, with the host door's result rules.
    pub type BytesFn = unsafe extern "C" fn(
        *mut c_void,
        u32,
        *const c_char,
        *const u8,
        usize,
        *mut *mut c_char,
    ) -> i32;

    extern "C" {
        fn exact_js_create(
            max_heap_bytes: u32,
            host: HostFn,
            bytes: BytesFn,
            ctx: *mut c_void,
        ) -> *mut c_void;
        fn exact_js_load(h: *mut c_void, data: *const u8, len: usize, out: *mut *mut c_char)
            -> i32;
        fn exact_js_string(h: *mut c_void, name: *const c_char, out: *mut *mut c_char) -> i32;
        fn exact_js_call(
            h: *mut c_void,
            name: *const c_char,
            a: *const c_char,
            b: *const c_char,
            c: *const c_char,
            out: *mut *mut c_char,
        ) -> i32;
        fn exact_js_capture_count(h: *mut c_void) -> usize;
        fn exact_js_capture(
            h: *mut c_void,
            index: usize,
            path: *mut *const c_char,
            path_len: *mut usize,
            value: *mut *const u16,
            value_len: *mut usize,
        ) -> bool;
        fn exact_js_clear_captures(h: *mut c_void);
        fn exact_js_install_storage(
            h: *mut c_void,
            queue: *const c_void,
            grants: *const c_void,
            sqlite: *const u8,
            sqlite_len: usize,
            harden: *const u8,
            harden_len: usize,
            out: *mut *mut c_char,
        ) -> i32;
        fn exact_js_deliver_storage_one(
            h: *mut c_void,
            delivered: *mut bool,
            out: *mut *mut c_char,
        ) -> i32;
        fn exact_js_drain(h: *mut c_void, out: *mut *mut c_char) -> i32;
        fn exact_js_interrupt(h: *mut c_void);
        fn exact_js_take_log(h: *mut c_void, out: *mut *mut c_char);
        fn exact_js_free(p: *mut c_char);
        fn exact_js_destroy(h: *mut c_void);
    }

    /// One runtime with one module evaluated into it.
    pub struct Engine(*mut c_void, Vec<(Vec<String>, String)>);

    /// A runtime as another thread may reach it: only to interrupt it, and
    /// only while its owner keeps it alive (`Module`'s watch).
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Raw(*mut c_void);

    // SAFETY: the one thing another thread does with it is interrupt, which
    // Hermes allows from any thread; the watch's lock keeps the runtime alive.
    unsafe impl Send for Raw {}

    impl Raw {
        /// Stop the runtime's running execution, or its next.
        pub fn interrupt(self) {
            // SAFETY: the caller holds the watch's lock, so the runtime is
            // alive; `asyncTriggerTimeout` may be called on any thread.
            unsafe { exact_js_interrupt(self.0) }
        }
    }

    fn take(out: *mut c_char) -> String {
        if out.is_null() {
            return String::new();
        }
        // SAFETY: the shim malloc'd a NUL-terminated string and handed us
        // ownership; it is freed exactly once, here.
        let s = unsafe { CStr::from_ptr(out) }
            .to_string_lossy()
            .into_owned();
        unsafe { exact_js_free(out) };
        s
    }

    fn c(s: &str) -> Result<CString, String> {
        CString::new(s).map_err(|_| "a NUL byte in a string crossing the seam".to_string())
    }

    impl Engine {
        pub fn new(
            max_heap_bytes: u32,
            host: HostFn,
            bytes: BytesFn,
            ctx: *mut c_void,
        ) -> Result<Engine, String> {
            // SAFETY: the shim returns null or a pointer we own until destroy;
            // `ctx` must outlive the engine, which `Module` guarantees by
            // boxing it for its own lifetime.
            let h = unsafe { exact_js_create(max_heap_bytes, host, bytes, ctx) };
            if h.is_null() {
                return Err("the Hermes runtime could not be created".into());
            }
            Ok(Engine(h, Vec::new()))
        }

        /// Install only during trusted initialization, after the prelude and
        /// before app code. The caller keeps `context` alive until Engine drops.
        pub fn install_storage(
            &mut self,
            context: &ibex2::bindings::Context,
        ) -> Result<(), String> {
            const SQLITE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/storage-sqlite.hbc"));
            const HARDEN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/storage-harden.hbc"));
            let mut out = std::ptr::null_mut();
            // SAFETY: Module owns the context beyond this engine's lifetime;
            // the shim retains bytecode buffers and borrows the Arc-backed state.
            let status = unsafe {
                exact_js_install_storage(
                    self.0,
                    context.state_ptr(),
                    context.grants_ptr(),
                    SQLITE.as_ptr(),
                    SQLITE.len(),
                    HARDEN.as_ptr(),
                    HARDEN.len(),
                    &mut out,
                )
            };
            let text = take(out);
            if status == 0 {
                Ok(())
            } else {
                Err(text)
            }
        }

        /// Deliver at most one completion without running any microtasks.
        pub fn deliver_storage_one(&mut self) -> Result<bool, String> {
            let mut delivered = false;
            let mut out = std::ptr::null_mut();
            // SAFETY: the engine is live; both output pointers outlive the call.
            let status = unsafe { exact_js_deliver_storage_one(self.0, &mut delivered, &mut out) };
            let text = take(out);
            if status == 0 {
                Ok(delivered)
            } else {
                Err(text)
            }
        }

        /// This runtime, for an interrupt from another thread.
        pub fn raw(&self) -> Raw {
            Raw(self.0)
        }

        pub fn load(&mut self, bytecode: &[u8]) -> Result<(), String> {
            let mut out: *mut c_char = std::ptr::null_mut();
            // SAFETY: the slice outlives the call (the shim copies it); `out`
            // receives an owned string or stays null.
            let status =
                unsafe { exact_js_load(self.0, bytecode.as_ptr(), bytecode.len(), &mut out) };
            let text = take(out);
            if status == 0 {
                Ok(())
            } else {
                Err(text)
            }
        }

        pub fn string(&self, name: &str) -> Result<String, String> {
            let name = c(name)?;
            let mut out: *mut c_char = std::ptr::null_mut();
            // SAFETY: `name` outlives the call; `out` receives an owned string.
            let status = unsafe { exact_js_string(self.0, name.as_ptr(), &mut out) };
            let text = take(out);
            if status == 0 {
                Ok(text)
            } else {
                Err(text)
            }
        }

        pub fn call(&mut self, name: &str, args: [&str; 3]) -> Result<String, String> {
            self.clear_reply();
            let name = c(name)?;
            let a = c(args[0])?;
            let b = c(args[1])?;
            let cc = c(args[2])?;
            let mut out: *mut c_char = std::ptr::null_mut();
            // SAFETY: every CString outlives the call; `out` receives an owned string.
            let status = unsafe {
                exact_js_call(
                    self.0,
                    name.as_ptr(),
                    a.as_ptr(),
                    b.as_ptr(),
                    cc.as_ptr(),
                    &mut out,
                )
            };
            let text = take(out);
            if status == 0 {
                self.collect_reply()?;
                Ok(text)
            } else {
                Err(text)
            }
        }

        // Collection remains inside Engine::call and therefore inside the
        // executor's budget. No engine pointers survive this conversion.
        fn collect_reply(&mut self) -> Result<(), String> {
            let result = (|| {
                // SAFETY: only this engine owns the live handle. Reading the
                // count does not mutate its captures or borrowed allocations.
                let count = unsafe { exact_js_capture_count(self.0) };
                let mut captures = Vec::with_capacity(count);
                for index in 0..count {
                    let (mut path, mut value) = (std::ptr::null(), std::ptr::null());
                    let (mut path_len, mut value_len) = (0, 0);
                    // SAFETY: outputs live through the call; returned buffers
                    // remain valid until clear below. No JS runs while borrowed.
                    let found = unsafe {
                        exact_js_capture(
                            self.0,
                            index,
                            &mut path,
                            &mut path_len,
                            &mut value,
                            &mut value_len,
                        )
                    };
                    if !found || path.is_null() || value.is_null() {
                        return Err("invalid native result capture".to_string());
                    }
                    // SAFETY: the shim supplies correctly aligned allocations
                    // and their element counts, retained until all decoding ends.
                    let path = unsafe { std::slice::from_raw_parts(path.cast::<u8>(), path_len) };
                    let value = unsafe { std::slice::from_raw_parts(value, value_len) };
                    let path: Vec<String> = serde_json::from_slice(path)
                        .map_err(|e| format!("invalid result string path: {e}"))?;
                    let value = String::from_utf16(value)
                        .map_err(|_| "result string contains a lone surrogate".to_string())?;
                    captures.push((path, value));
                }
                Ok(captures)
            })();
            // SAFETY: decoding has ended, including on error; no borrowed slice
            // escapes the closure. The C++ allocations are released exactly once.
            unsafe { exact_js_clear_captures(self.0) };
            self.1 = result?;
            Ok(())
        }

        pub fn clear_reply(&mut self) {
            self.1.clear();
            // SAFETY: this engine uniquely owns the live handle.
            unsafe { exact_js_clear_captures(self.0) };
        }

        pub fn has_reply_strings(&self) -> bool {
            !self.1.is_empty()
        }

        /// Consume this call's strings into their exact JSON locations before
        /// the existing shape check. A user property can never act as a marker.
        pub fn restore_reply(&mut self, reply: &mut serde_json::Value) -> Result<(), String> {
            for (path, value) in std::mem::take(&mut self.1) {
                let mut target = &mut *reply;
                for part in path {
                    target = match target {
                        serde_json::Value::Object(object) => object.get_mut(&part),
                        serde_json::Value::Array(array) => part
                            .parse::<usize>()
                            .ok()
                            .filter(|index| index.to_string() == part)
                            .and_then(|index| array.get_mut(index)),
                        _ => None,
                    }
                    .ok_or("result string path is absent")?;
                }
                if target.as_str() != Some("") {
                    return Err("result string placeholder is not empty".into());
                }
                *target = serde_json::Value::String(value);
            }
            Ok(())
        }

        pub fn drain(&mut self) -> Result<(), String> {
            let mut out: *mut c_char = std::ptr::null_mut();
            // SAFETY: `out` receives an owned string or stays null.
            let status = unsafe { exact_js_drain(self.0, &mut out) };
            let text = take(out);
            if status == 0 {
                Ok(())
            } else {
                Err(text)
            }
        }

        pub fn take_log(&mut self) -> Vec<String> {
            let mut out: *mut c_char = std::ptr::null_mut();
            // SAFETY: `out` receives an owned string.
            unsafe { exact_js_take_log(self.0, &mut out) };
            let text = take(out);
            if text.is_empty() {
                Vec::new()
            } else {
                text.lines().map(str::to_string).collect()
            }
        }
    }

    impl Drop for Engine {
        fn drop(&mut self) {
            // SAFETY: the handle came from `exact_js_create` and is destroyed once.
            unsafe { exact_js_destroy(self.0) }
        }
    }
}

#[cfg(not(exact_js_engine))]
mod real {
    use std::ffi::{c_char, c_void};

    /// The host door's signature, kept identical so `Module` is one type.
    pub type HostFn = unsafe extern "C" fn(
        *mut c_void,
        u32,
        *const c_char,
        *const c_char,
        *mut *mut c_char,
    ) -> i32;

    /// The bytes door's signature, likewise.
    pub type BytesFn = unsafe extern "C" fn(
        *mut c_void,
        u32,
        *const c_char,
        *const u8,
        usize,
        *mut *mut c_char,
    ) -> i32;

    /// No engine in this binary: every operation refuses by name.
    pub struct Engine(());

    /// Nothing runs, so nothing is interrupted.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Raw;

    impl Raw {
        pub fn interrupt(self) {}
    }

    const NONE: &str = "this binary links no engine (built without a lean Hermes; see js/build.rs)";

    impl Engine {
        pub fn new(
            _max_heap_bytes: u32,
            _host: HostFn,
            _bytes: BytesFn,
            _ctx: *mut c_void,
        ) -> Result<Engine, String> {
            Err(NONE.into())
        }
        pub fn raw(&self) -> Raw {
            Raw
        }
        pub fn install_storage(
            &mut self,
            _context: &ibex2::bindings::Context,
        ) -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn deliver_storage_one(&mut self) -> Result<bool, String> {
            Err(NONE.into())
        }
        pub fn drain(&mut self) -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn load(&mut self, _bytecode: &[u8]) -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn string(&self, _name: &str) -> Result<String, String> {
            Err(NONE.into())
        }
        pub fn call(&mut self, _name: &str, _args: [&str; 3]) -> Result<String, String> {
            Err(NONE.into())
        }
        pub fn clear_reply(&mut self) {}
        pub fn has_reply_strings(&self) -> bool {
            false
        }
        pub fn restore_reply(&mut self, _reply: &mut serde_json::Value) -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn take_log(&mut self) -> Vec<String> {
            Vec::new()
        }
    }
}

pub(crate) use real::{BytesFn, Engine, HostFn, Raw};

/// Whether this binary links an engine at all.
pub const ENGINE_LINKED: bool = cfg!(exact_js_engine);
