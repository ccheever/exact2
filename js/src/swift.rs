//! An app's native module written in Swift (LLP 1058): the C seam
//! `js/native/ExactNative.swift` exports, as a [`NativeModule`]. An app's
//! crate names it with [`swift_native_module!`], which declares the four
//! symbols there — so nothing references them unless the app links Swift —
//! and its build script compiles the Swift with `exact_js_bake::swift_native`.

use crate::native::{Changed, LaterHandler, NativeModule, NativeReply};
use serde_json::Value;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::Arc;

/// `exact_native_done(context, reply, failed)`: the Swift side's one reply.
pub type Done = extern "C" fn(*mut c_void, *const c_char, i32);

/// `announce(context, topic)`: `ExactNative.changed`, from any thread.
pub type Announce = extern "C" fn(*mut c_void, *const c_char);

/// The entry points `ExactNative.swift` exports, as the app's crate links them.
#[derive(Clone, Copy)]
pub struct SwiftSymbols {
    /// `exact_native_configure(data, cache, temporary)`.
    pub configure: unsafe extern "C" fn(*const c_char, *const c_char, *const c_char),
    /// `exact_native_call(request, &failed) -> reply`.
    pub call: unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_char,
    /// `exact_native_later(request, context, done)`.
    pub later: unsafe extern "C" fn(*const c_char, *mut c_void, Done),
    /// `exact_native_free(reply)`.
    pub free: unsafe extern "C" fn(*mut c_char),
    /// `exact_native_listen(context, announce)`.
    pub listen: unsafe extern "C" fn(*mut c_void, Announce),
}

/// The Swift module behind [`SwiftSymbols`].
pub struct SwiftModule(pub SwiftSymbols);

fn c_path(path: &std::path::Path) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
}

/// Take a reply the Swift side allocated, and free it with its own `free`.
///
/// # Safety
/// `text` is null or a NUL-terminated string from the Swift side's `strdup`.
unsafe fn take(symbols: &SwiftSymbols, text: *mut c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    let owned = CStr::from_ptr(text).to_string_lossy().into_owned();
    (symbols.free)(text);
    owned
}

fn parsed(text: &str, failed: bool) -> Result<Value, String> {
    if failed {
        return Err(text.to_owned());
    }
    serde_json::from_str(text).map_err(|e| format!("the Swift reply was not JSON: {e}"))
}

extern "C" fn announce(context: *mut c_void, topic: *const c_char) {
    // SAFETY: `context` is the `Changed` `changes` leaked for the Swift side;
    // `topic` is valid for this call.
    let changed = unsafe { &*(context as *const Changed) };
    changed(&unsafe { CStr::from_ptr(topic) }.to_string_lossy());
}

extern "C" fn done(context: *mut c_void, reply: *const c_char, failed: i32) {
    // SAFETY: `context` is the reply `later` boxed below, handed back once;
    // `reply` is valid for this call (the Swift side frees it after).
    let reply_to = unsafe { Box::from_raw(context as *mut NativeReply) };
    let text = if reply.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(reply) }
            .to_string_lossy()
            .into_owned()
    };
    reply_to.send(parsed(&text, failed != 0));
}

impl NativeModule for SwiftModule {
    fn configure_storage(
        &mut self,
        data: PathBuf,
        cache: PathBuf,
        temporary: PathBuf,
    ) -> Result<(), String> {
        let (data, cache, temporary) = (c_path(&data)?, c_path(&cache)?, c_path(&temporary)?);
        // SAFETY: three valid NUL-terminated paths; the Swift side copies them.
        unsafe { (self.0.configure)(data.as_ptr(), cache.as_ptr(), temporary.as_ptr()) };
        Ok(())
    }

    fn call(&mut self, request: &Value) -> Result<Value, String> {
        let request = CString::new(request.to_string()).map_err(|e| e.to_string())?;
        let mut failed = 0;
        // SAFETY: a valid request string and flag; the reply is taken once.
        let text = unsafe { take(&self.0, (self.0.call)(request.as_ptr(), &mut failed)) };
        parsed(&text, failed != 0)
    }

    fn changes(&mut self, changed: Changed) {
        // Kept for the process: the Swift side may announce from any thread
        // at any time, and a module is configured once per activation.
        let context = Box::into_raw(Box::new(changed)) as *mut c_void;
        // SAFETY: `context` stays valid forever; `announce` reads it as such.
        unsafe { (self.0.listen)(context, announce) };
    }

    fn later(&mut self) -> Option<LaterHandler> {
        let symbols = self.0;
        Some(Arc::new(move |request: Value, reply: NativeReply| {
            let request = match CString::new(request.to_string()) {
                Ok(request) => request,
                Err(e) => return reply.send(Err(e.to_string())),
            };
            let context = Box::into_raw(Box::new(reply)) as *mut c_void;
            // SAFETY: the Swift side calls `done` exactly once with `context`.
            unsafe { (symbols.later)(request.as_ptr(), context, done) };
        }))
    }
}

/// Link the app's Swift native module: declares the symbols
/// `ExactNative.swift` exports and defines `native_module`, the factory
/// `exact_js::Module::with_native` takes.
#[macro_export]
macro_rules! swift_native_module {
    () => {
        extern "C" {
            fn exact_native_configure(
                data: *const ::std::ffi::c_char,
                cache: *const ::std::ffi::c_char,
                temporary: *const ::std::ffi::c_char,
            );
            fn exact_native_call(
                request: *const ::std::ffi::c_char,
                failed: *mut i32,
            ) -> *mut ::std::ffi::c_char;
            fn exact_native_later(
                request: *const ::std::ffi::c_char,
                context: *mut ::std::ffi::c_void,
                done: $crate::swift::Done,
            );
            fn exact_native_free(pointer: *mut ::std::ffi::c_char);
            fn exact_native_listen(
                context: *mut ::std::ffi::c_void,
                announce: $crate::swift::Announce,
            );
        }

        /// The app's Swift native module, for `Module::with_native`.
        pub fn native_module(_grants: &str) -> Box<dyn $crate::NativeModule> {
            Box::new($crate::swift::SwiftModule($crate::swift::SwiftSymbols {
                configure: exact_native_configure,
                call: exact_native_call,
                later: exact_native_later,
                free: exact_native_free,
                listen: exact_native_listen,
            }))
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    // A stand-in for the Swift side, with its conventions: strdup'd replies,
    // `failed` set for a refusal, `done` called once from another thread.
    unsafe extern "C" fn configure(_: *const c_char, _: *const c_char, _: *const c_char) {}
    unsafe extern "C" fn call(request: *const c_char, failed: *mut i32) -> *mut c_char {
        let text = CStr::from_ptr(request).to_string_lossy().into_owned();
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["op"] == "refuse" {
            *failed = 1;
            return CString::new("refused by Swift").unwrap().into_raw();
        }
        *failed = 0;
        CString::new(format!("{{\"echo\":{text}}}"))
            .unwrap()
            .into_raw()
    }
    unsafe extern "C" fn later(request: *const c_char, context: *mut c_void, done: Done) {
        let text = CStr::from_ptr(request).to_string_lossy().into_owned();
        let context = context as usize;
        std::thread::spawn(move || {
            let reply = CString::new(format!("{{\"later\":{text}}}")).unwrap();
            done(context as *mut c_void, reply.as_ptr(), 0);
        });
    }
    unsafe extern "C" fn free(text: *mut c_char) {
        drop(CString::from_raw(text));
    }
    unsafe extern "C" fn listen(context: *mut c_void, announce: Announce) {
        let topic = CString::new("meter").unwrap();
        announce(context, topic.as_ptr());
    }

    fn module() -> SwiftModule {
        SwiftModule(SwiftSymbols {
            configure,
            call,
            later,
            free,
            listen,
        })
    }

    #[test]
    fn a_swift_module_answers_now_refuses_and_answers_later_from_its_own_thread() {
        let mut m = module();
        assert_eq!(
            m.call(&serde_json::json!({"op": "echo"})).unwrap(),
            serde_json::json!({"echo": {"op": "echo"}})
        );
        assert_eq!(
            m.call(&serde_json::json!({"op": "refuse"})).unwrap_err(),
            "refused by Swift"
        );
        let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let into = heard.clone();
        m.changes(std::sync::Arc::new(move |t: &str| {
            into.lock().unwrap().push(t.into())
        }));
        assert_eq!(
            *heard.lock().unwrap(),
            ["meter"],
            "announced through the seam"
        );
        let handler = m.later().unwrap();
        let (tx, rx) = mpsc::channel();
        handler(
            serde_json::json!({"op": "slow"}),
            NativeReply::new(exact_runner::Reply::new(move |o| tx.send(o).unwrap())),
        );
        match rx.recv().unwrap() {
            exact_runner::Outcome::Response(r) => {
                assert_eq!(r.status, 200);
                assert_eq!(
                    serde_json::from_slice::<Value>(&r.body).unwrap(),
                    serde_json::json!({"later": {"op": "slow"}})
                );
            }
            other => panic!("{other:?}"),
        }
    }
}
