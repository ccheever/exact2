//! An app's native module written in Swift (LLP 1067): the C seam
//! `js/native/ExactNative.swift` exports, as a [`NativeModule`]. An app's
//! crate names it with [`swift_native_module!`], which declares the app's
//! symbols there — so nothing references them unless the app links Swift —
//! and its build script compiles the Swift with `exact_js_bake::swift_native`.

use crate::native::{Changed, LaterHandler, NativeModule, NativeReply};
use serde_json::Value;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::Arc;

/// `exact_native_done(context, reply, failed)`: the Swift side's one reply.
pub type Done = extern "C" fn(*mut c_void, *const c_char, i32);

/// Release an unanswered reply; dropping it reports `Aborted` to its host.
pub type Discard = extern "C" fn(*mut c_void);

/// `announce(context, topic)`: `ExactNative.changed`, from any thread.
pub type Announce = extern "C" fn(*mut c_void, *const c_char);

/// The entry points `ExactNative.swift` exports, as the app's crate links them.
#[derive(Clone, Copy)]
pub struct SwiftSymbols {
    /// `create(grants, length) -> instance`.
    pub create: unsafe extern "C" fn(*const u8, usize) -> *mut c_void,
    /// `destroy(instance)`.
    pub destroy: unsafe extern "C" fn(*mut c_void),
    /// `configure(instance, data, cache, temporary)`.
    pub configure: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char),
    /// `call(instance, request, &failed) -> reply`.
    pub call: unsafe extern "C" fn(*mut c_void, *const c_char, *mut i32) -> *mut c_char,
    /// `later(instance, request, context, done, discard)`.
    pub later: unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void, Done, Discard),
    /// `exact_native_free(reply)`.
    pub free: unsafe extern "C" fn(*mut c_char),
    /// `listen(instance, context, announce)`.
    pub listen: unsafe extern "C" fn(*mut c_void, *mut c_void, Announce),
    /// `unlisten(instance)`; waits for any announcement using its context.
    pub unlisten: unsafe extern "C" fn(*mut c_void),
}

/// The Swift module behind [`SwiftSymbols`].
pub struct SwiftModule {
    instance: Arc<Instance>,
    changed: Option<Box<Changed>>,
}

struct Instance {
    symbols: SwiftSymbols,
    handle: NonNull<c_void>,
}

// SAFETY: the Swift bridge serializes every entry to this instance. Its
// retained handle lives until the last in-progress entry returns.
unsafe impl Send for Instance {}
unsafe impl Sync for Instance {}

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: the retained handle is released once, after all entries.
        unsafe { (self.symbols.destroy)(self.handle.as_ptr()) };
    }
}

impl SwiftModule {
    /// Create one activation, giving Swift its effective grants before use.
    ///
    /// # Safety
    /// `symbols` must implement `ExactNative.swift`'s ownership and locking
    /// contract, including one-shot completion and synchronized unlisten.
    pub unsafe fn new(symbols: SwiftSymbols, grants: &str) -> Self {
        let handle = NonNull::new((symbols.create)(grants.as_ptr(), grants.len()))
            .expect("the Swift factory returns a retained instance");
        Self {
            instance: Arc::new(Instance { symbols, handle }),
            changed: None,
        }
    }
}

impl Drop for SwiftModule {
    fn drop(&mut self) {
        // SAFETY: unlisten synchronizes with every callback before Rust
        // releases the context, even if a long-call entry is still running.
        unsafe { (self.instance.symbols.unlisten)(self.instance.handle.as_ptr()) };
    }
}

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
    // SAFETY: `context` lives until unlisten returns; `topic` is valid here.
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

extern "C" fn discard(context: *mut c_void) {
    // SAFETY: Swift calls either done or discard once, never both.
    drop(unsafe { Box::from_raw(context as *mut NativeReply) });
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
        unsafe {
            (self.instance.symbols.configure)(
                self.instance.handle.as_ptr(),
                data.as_ptr(),
                cache.as_ptr(),
                temporary.as_ptr(),
            )
        };
        Ok(())
    }

    fn call(&mut self, request: &Value) -> Result<Value, String> {
        let request = CString::new(request.to_string()).map_err(|e| e.to_string())?;
        let mut failed = 0;
        // SAFETY: a valid request string and flag; the reply is taken once.
        let instance = &self.instance;
        let text = unsafe {
            take(
                &instance.symbols,
                (instance.symbols.call)(instance.handle.as_ptr(), request.as_ptr(), &mut failed),
            )
        };
        parsed(&text, failed != 0)
    }

    fn changes(&mut self, changed: Changed) {
        let instance = &self.instance;
        // SAFETY: retire the previous listener before replacing its context.
        unsafe { (instance.symbols.unlisten)(instance.handle.as_ptr()) };
        let changed = self.changed.insert(Box::new(changed));
        let context = &mut **changed as *mut Changed as *mut c_void;
        // SAFETY: `context` is boxed until synchronized unlisten above/on drop.
        unsafe { (instance.symbols.listen)(instance.handle.as_ptr(), context, announce) };
    }

    fn later(&mut self) -> Option<LaterHandler> {
        let instance = Arc::downgrade(&self.instance);
        Some(Arc::new(move |request: Value, reply: NativeReply| {
            let Some(instance) = instance.upgrade() else {
                return; // Dropping the unsent reply aborts a retired activation.
            };
            let request = match CString::new(request.to_string()) {
                Ok(request) => request,
                Err(e) => return reply.send(Err(e.to_string())),
            };
            let context = Box::into_raw(Box::new(reply)) as *mut c_void;
            // SAFETY: Swift owns the boxed reply until done or discard, once.
            unsafe {
                (instance.symbols.later)(
                    instance.handle.as_ptr(),
                    request.as_ptr(),
                    context,
                    done,
                    discard,
                )
            };
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
            #[link_name = concat!(env!("EXACT_SWIFT_SYMBOL_PREFIX"), "_create")]
            fn exact_native_create(grants: *const u8, length: usize) -> *mut ::std::ffi::c_void;
            #[link_name = concat!(env!("EXACT_SWIFT_SYMBOL_PREFIX"), "_destroy")]
            fn exact_native_destroy(instance: *mut ::std::ffi::c_void);
            #[link_name = concat!(env!("EXACT_SWIFT_SYMBOL_PREFIX"), "_configure")]
            fn exact_native_configure(
                instance: *mut ::std::ffi::c_void,
                data: *const ::std::ffi::c_char,
                cache: *const ::std::ffi::c_char,
                temporary: *const ::std::ffi::c_char,
            );
            #[link_name = concat!(env!("EXACT_SWIFT_SYMBOL_PREFIX"), "_call")]
            fn exact_native_call(
                instance: *mut ::std::ffi::c_void,
                request: *const ::std::ffi::c_char,
                failed: *mut i32,
            ) -> *mut ::std::ffi::c_char;
            #[link_name = concat!(env!("EXACT_SWIFT_SYMBOL_PREFIX"), "_later")]
            fn exact_native_later(
                instance: *mut ::std::ffi::c_void,
                request: *const ::std::ffi::c_char,
                context: *mut ::std::ffi::c_void,
                done: $crate::swift::Done,
                discard: $crate::swift::Discard,
            );
            #[link_name = concat!(env!("EXACT_SWIFT_SYMBOL_PREFIX"), "_free")]
            fn exact_native_free(pointer: *mut ::std::ffi::c_char);
            #[link_name = concat!(env!("EXACT_SWIFT_SYMBOL_PREFIX"), "_listen")]
            fn exact_native_listen(
                instance: *mut ::std::ffi::c_void,
                context: *mut ::std::ffi::c_void,
                announce: $crate::swift::Announce,
            );
            #[link_name = concat!(env!("EXACT_SWIFT_SYMBOL_PREFIX"), "_unlisten")]
            fn exact_native_unlisten(instance: *mut ::std::ffi::c_void);
        }

        /// The app's Swift native module, for `Module::with_native`.
        pub fn native_module(grants: &str) -> Box<dyn $crate::NativeModule> {
            // SAFETY: these app-specific symbols are built from ExactNative.swift.
            Box::new(unsafe {
                $crate::swift::SwiftModule::new(
                    $crate::swift::SwiftSymbols {
                        create: exact_native_create,
                        destroy: exact_native_destroy,
                        configure: exact_native_configure,
                        call: exact_native_call,
                        later: exact_native_later,
                        free: exact_native_free,
                        listen: exact_native_listen,
                        unlisten: exact_native_unlisten,
                    },
                    grants,
                )
            })
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    // A stand-in for the Swift side, with its conventions: strdup'd replies,
    // `failed` set for a refusal, `done` called once from another thread.
    unsafe extern "C" fn create(grants: *const u8, length: usize) -> *mut c_void {
        let grants = std::str::from_utf8(std::slice::from_raw_parts(grants, length)).unwrap();
        Box::into_raw(Box::new(grants.to_owned())).cast()
    }
    unsafe extern "C" fn destroy(instance: *mut c_void) {
        drop(Box::from_raw(instance.cast::<String>()));
    }
    unsafe extern "C" fn configure(
        _: *mut c_void,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
    ) {
    }
    unsafe extern "C" fn call(
        instance: *mut c_void,
        request: *const c_char,
        failed: *mut i32,
    ) -> *mut c_char {
        let text = CStr::from_ptr(request).to_string_lossy().into_owned();
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["op"] == "refuse" {
            *failed = 1;
            return CString::new("refused by Swift").unwrap().into_raw();
        }
        *failed = 0;
        if value["op"] == "grants" {
            return CString::new(serde_json::to_string(&*instance.cast::<String>()).unwrap())
                .unwrap()
                .into_raw();
        }
        CString::new(format!("{{\"echo\":{text}}}"))
            .unwrap()
            .into_raw()
    }
    unsafe extern "C" fn later(
        _: *mut c_void,
        request: *const c_char,
        context: *mut c_void,
        done: Done,
        discard: Discard,
    ) {
        let text = CStr::from_ptr(request).to_string_lossy().into_owned();
        if text.contains("drop") {
            discard(context);
            return;
        }
        let context = context as usize;
        std::thread::spawn(move || {
            let reply = CString::new(format!("{{\"later\":{text}}}")).unwrap();
            done(context as *mut c_void, reply.as_ptr(), 0);
        });
    }
    unsafe extern "C" fn free(text: *mut c_char) {
        drop(CString::from_raw(text));
    }
    unsafe extern "C" fn listen(_: *mut c_void, context: *mut c_void, announce: Announce) {
        let topic = CString::new("meter").unwrap();
        announce(context, topic.as_ptr());
    }

    unsafe extern "C" fn unlisten(_: *mut c_void) {}

    fn module() -> SwiftModule {
        unsafe {
            SwiftModule::new(
                SwiftSymbols {
                    create,
                    destroy,
                    configure,
                    call,
                    later,
                    free,
                    listen,
                    unlisten,
                },
                "fs.read app:/data",
            )
        }
    }

    #[test]
    fn a_swift_module_answers_now_refuses_and_answers_later_from_its_own_thread() {
        let mut m = module();
        assert_eq!(
            m.call(&serde_json::json!({"op": "grants"})).unwrap(),
            "fs.read app:/data"
        );
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

    #[test]
    fn dropped_completions_and_retired_handlers_abort_and_release_the_listener() {
        let mut m = module();
        let changed: Changed = Arc::new(|_| {});
        m.changes(changed.clone());
        assert_eq!(Arc::strong_count(&changed), 2);
        let handler = m.later().unwrap();
        let (tx, rx) = mpsc::channel();
        let reply = |tx: mpsc::Sender<_>| {
            NativeReply::new(exact_runner::Reply::new(move |o| tx.send(o).unwrap()))
        };
        handler(serde_json::json!({"op": "drop"}), reply(tx.clone()));
        drop(m);
        assert_eq!(Arc::strong_count(&changed), 1);
        handler(serde_json::json!({}), reply(tx));
        for _ in 0..2 {
            assert!(matches!(
                rx.recv().unwrap(),
                exact_runner::Outcome::Failed {
                    kind: exact_runner::FailureKind::Aborted,
                    ..
                }
            ));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn swift_bridge_serializes_instances_and_owns_callbacks() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = std::env::temp_dir().join(format!("exact-swift-bridge-{}", std::process::id()));
        let built = std::process::Command::new("xcrun")
            .args(["swiftc"])
            .arg(root.join("native/ExactNative.swift"))
            .arg(root.join("native/tests/main.swift"))
            .arg("-o")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let result = std::process::Command::new(&path).output().unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
