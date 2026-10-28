//! The reader's keys and system Back (`canvas/keys.rs`): what `canvas_jni!`
//! defines beside its own, over the same `HOST`.

/// `Native.key`, `Native.namedKey`, `Native.back`, `Native.open` and `Native.share`, expanded inside
/// `canvas_jni!`'s module, whose `with`, `string` and `flag` they use.
#[macro_export]
#[doc(hidden)]
macro_rules! canvas_jni_keys {
    () => {
        /// A key typed at the focused field: `code` a Unicode scalar, or 0
        /// with `backspace` set; `'\n'` is Enter.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_key(
            _env: *mut JNIEnv,
            _class: jclass,
            code: jint,
            backspace: jboolean,
        ) {
            let ch = char::from_u32(code as u32).filter(|_| code > 0);
            with(|h| h.key(ch, backspace == JNI_TRUE));
        }

        /// A remote's key, down or up, by the web's `code` and `key`
        /// (LLP 1115.001.003 D3): the D-pad's arrows and Select.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_namedKey(
            env: *mut JNIEnv,
            _class: jclass,
            code: jstring,
            key: jstring,
            down: jboolean,
            repeat: jboolean,
        ) {
            let (code, key) = (string(env, code), string(env, key));
            with(|h| h.named_key(&code, &key, down == JNI_TRUE, repeat == JNI_TRUE));
        }

        /// System Back: whether the app took it.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_back(
            _env: *mut JNIEnv,
            _class: jclass,
        ) -> jboolean {
            flag(with(|h| h.back()).unwrap_or(false))
        }

        /// A URL given to the running app (`CanvasHost::open`).
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_open(
            env: *mut JNIEnv,
            _class: jclass,
            href: jstring,
        ) {
            let href = string(env, href);
            with(|h| h.open(&href));
        }

        /// A share given to the running app (`CanvasHost::share`).
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_share(
            env: *mut JNIEnv,
            _class: jclass,
            request: jstring,
        ) {
            let request = string(env, request);
            with(|h| h.share(&request));
        }
    };
}
