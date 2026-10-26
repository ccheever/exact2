//! The reader's extensions' calls into the host (LLP 1133.001 D5): what
//! `canvas_jni!` defines beside its own, over the same `HOST`, and the
//! session's clock and agent flag they read. An
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

/// The session's clock as of the host's last turn, and the wall instant it
/// was read at: what an extension's `ExtensionContext.now()` reads from the
/// main thread without waiting for exact2's (LLP 1133.009 D10 item 2).
static CLOCK: std::sync::Mutex<Option<(f64, std::time::Instant)>> = std::sync::Mutex::new(None);

/// The host's turn ended at session time `now_ms`.
pub(crate) fn stamp(now_ms: f64) {
    *CLOCK.lock().unwrap_or_else(|e| e.into_inner()) = Some((now_ms, std::time::Instant::now()));
}

/// Whether this process runs under the agent (`EXACT_AGENT=1`).
pub fn agent() -> bool {
    std::env::var("EXACT_AGENT").is_ok_and(|v| v == "1")
}

/// The session's clock, ms: under the agent, as its last turn left it (the
/// agent moves it); else that, plus the wall time since.
pub fn now() -> f64 {
    match *CLOCK.lock().unwrap_or_else(|e| e.into_inner()) {
        Some((ms, _)) if agent() => ms,
        Some((ms, at)) => ms + at.elapsed().as_secs_f64() * 1000.0,
        None => 0.0,
    }
}

/// Whether extension `name` runs told it is under the agent: under the
/// agent, unless the drive binds it as in production (`--bind`).
pub fn under_agent(name: &str) -> bool {
    let bound = std::env::var("EXACT_AGENT_BIND").unwrap_or_default();
    agent() && !bound.split(',').any(|b| b == name)
}

/// A package's sample for a canvas input (`Native.sensorInput`, LLP 1133.013
/// D6): whether none waited before it.
pub fn sensor_input(
    surface: &str,
    sensor: &str,
    values: &[f64],
    dropped: i32,
    at: f64,
    ended: bool,
) -> bool {
    crate::surfaces::sensors::write(surface, sensor, values, dropped.max(0) as u32, at, ended)
}

/// Whether the page places a canvas of surface `name` (`Native.surfaceHas`).
pub fn surface_has(name: &str) -> bool {
    crate::surfaces::sensors::has(name)
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

        /// The session's clock, ms (`ExtensionContext.now`).
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_now(
            _env: *mut JNIEnv,
            _class: jclass,
        ) -> jdouble {
            $crate::canvas::jni::jni_extensions::now()
        }

        /// The file an `app:/` path names (`ExtensionContext.file`, LLP
        /// 1133.009 D10 item 4), or null.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_appFile(
            env: *mut JNIEnv,
            _class: jclass,
            path: jstring,
        ) -> jstring {
            let path = $crate::canvas::field::text(env, path);
            match $crate::picker::resolve(&path) {
                Some(file) => $crate::canvas::field::java(env, &file.to_string_lossy()),
                None => std::ptr::null_mut(),
            }
        }

        /// Whether extension `name` is told it runs under the agent.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_underAgent(
            env: *mut JNIEnv,
            _class: jclass,
            name: jstring,
        ) -> jboolean {
            flag($crate::canvas::jni::jni_extensions::under_agent(&string(
                env, name,
            )))
        }

        /// How a presence check the reader made ended (`Presence.kt`, LLP
        /// 1133.010 D13): on the store's completion path, never journaled.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_presence(
            env: *mut JNIEnv,
            _class: jclass,
            answer: jstring,
        ) {
            let answer = $crate::canvas::field::text(env, answer);
            with(|h| h.presence_reply(&answer));
        }

        /// What the activity asked for the orientation lock (`Orientation.kt`,
        /// LLP 1133.029 D6): JSON, a conflict, and a line to journal or null.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_orientationApplied(
            env: *mut JNIEnv,
            _class: jclass,
            applied: jstring,
            conflict: jboolean,
            line: jstring,
        ) {
            let applied = $crate::canvas::field::text(env, applied);
            let line = (!line.is_null()).then(|| $crate::canvas::field::text(env, line));
            with(|h| h.orientation_applied(&applied, conflict == JNI_TRUE, line.as_deref()));
        }

        /// The activity started (`false`) or stopped (`true`): the page's
        /// `visibilityState` (LLP 1133.010 §9).
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_hidden(
            _env: *mut JNIEnv,
            _class: jclass,
            hidden: jboolean,
        ) {
            with(|h| h.page_hidden(hidden == JNI_TRUE));
        }

        /// Whether the page places a canvas of surface `name`. Any thread.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_surfaceHas(
            env: *mut JNIEnv,
            _class: jclass,
            name: jstring,
        ) -> jboolean {
            flag($crate::canvas::jni::jni_extensions::surface_has(&string(
                env, name,
            )))
        }

        /// A package's sample for surface `surface`'s canvas input, or its
        /// end: x, y, z and w of `values`. Any thread; whether none waited.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_sensorInput(
            env: *mut JNIEnv,
            _class: jclass,
            surface: jstring,
            sensor: jstring,
            values: $crate::canvas::jni_sys::jdoubleArray,
            dropped: jint,
            at: jdouble,
            ended: jboolean,
        ) -> jboolean {
            let mut v = [0f64; 4];
            let n = ((**env).GetArrayLength.expect("jni"))(env, values).clamp(0, 4);
            ((**env).GetDoubleArrayRegion.expect("jni"))(env, values, 0, n, v.as_mut_ptr());
            flag($crate::canvas::jni::jni_extensions::sensor_input(
                &string(env, surface),
                &string(env, sensor),
                &v[..n as usize],
                dropped,
                at,
                ended == JNI_TRUE,
            ))
        }

        /// A package's picture for request `id` (`Exact.picture`, LLP
        /// 1133.019 D11 item 1): `bitmap`, or `error` when it is null.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_picture(
            env: *mut JNIEnv,
            _class: jclass,
            id: jint,
            bitmap: jobject,
            error: jstring,
        ) {
            let answer = if bitmap.is_null() {
                Err($crate::canvas::field::text(env, error))
            } else {
                $crate::canvas::jni::jni_picture::pixels(env, bitmap)
                    .ok_or_else(|| "EncodingError".to_owned())
            };
            with(|h| h.picture_answer(id as u32, answer));
        }

        /// Picture `source` changed (asked again) or was revoked
        /// (`ExtensionContext.changedPicture`, `revokePicture`).
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_pictureChanged(
            env: *mut JNIEnv,
            _class: jclass,
            source: jstring,
            revoked: jboolean,
        ) {
            let source = string(env, source);
            with(|h| h.picture_changed(&source, revoked == JNI_TRUE));
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
