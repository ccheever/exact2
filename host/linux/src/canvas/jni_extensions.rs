//! The reader's extensions' calls into the host (LLP 1133.001 D5): what
//! `canvas_jni!` defines beside its own, over the same `HOST`. An
//! extension's call itself comes the other way, as a request (kind 11,
//! `presenter/requests.rs`), and its reply as that request's answer.

/// The names the activity bound before a host ran (it binds as it is made):
/// the host's next turn binds them (`CanvasHost::requests`).
static PENDING: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Keep `names` for the host's next turn.
pub fn bind_later(names: String) {
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(names);
}

/// The names kept, once.
pub(crate) fn take_bound() -> Option<String> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// `Native.changed` and `Native.bindExtensions`, expanded inside
/// `canvas_jni!`'s module, whose `with` and `string` they use.
#[macro_export]
#[doc(hidden)]
macro_rules! canvas_jni_extensions {
    () => {
        /// A topic an extension announced: its readers are asked again.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_changed(
            env: *mut JNIEnv,
            _class: jclass,
            topic: jstring,
        ) {
            let topic = string(env, topic);
            with(|h| h.extension_changed(&topic));
        }

        /// The extensions the APK carries, one a line, bound at the host's next turn.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_bindExtensions(
            env: *mut JNIEnv,
            _class: jclass,
            names: jstring,
        ) {
            $crate::canvas::jni::jni_extensions::bind_later(string(env, names));
        }
    };
}
