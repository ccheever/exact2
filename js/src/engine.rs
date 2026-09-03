//! The engine behind the executor: the lean Hermes VM through the shim, or
//! a stub that refuses when this binary links no engine (see `build.rs`).

#[cfg(exact_js_engine)]
mod real {
    use std::ffi::{c_char, c_void, CStr, CString};

    extern "C" {
        fn exact_js_create(max_heap_bytes: u32) -> *mut c_void;
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
        fn exact_js_take_log(h: *mut c_void, out: *mut *mut c_char);
        fn exact_js_free(p: *mut c_char);
        fn exact_js_destroy(h: *mut c_void);
    }

    /// One runtime with one module evaluated into it.
    pub struct Engine(*mut c_void);

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
        pub fn new(max_heap_bytes: u32) -> Result<Engine, String> {
            // SAFETY: the shim returns null or a pointer we own until destroy.
            let h = unsafe { exact_js_create(max_heap_bytes) };
            if h.is_null() {
                return Err("the Hermes runtime could not be created".into());
            }
            Ok(Engine(h))
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
                Ok(text)
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
    /// No engine in this binary: every operation refuses by name.
    pub struct Engine(());

    const NONE: &str = "this binary links no engine (built without a lean Hermes; see js/build.rs)";

    impl Engine {
        pub fn new(_max_heap_bytes: u32) -> Result<Engine, String> {
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
        pub fn take_log(&mut self) -> Vec<String> {
            Vec::new()
        }
    }
}

pub(crate) use real::Engine;

/// Whether this binary links an engine at all.
pub const ENGINE_LINKED: bool = cfg!(exact_js_engine);
