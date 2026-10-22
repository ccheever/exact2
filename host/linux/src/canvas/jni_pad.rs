//! The gamepad's JNI method (LLP 1127.003 D2), expanded by `canvas_jni!`
//! inside its module, where `with` reaches the thread's host.

#[doc(hidden)]
#[macro_export]
macro_rules! canvas_jni_pad {
    () => {
        /// A gamepad's state (LLP 1127.003 D2): `values` its 17 standard
        /// buttons, then its 4 axes.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_pad(
            env: *mut $crate::canvas::jni_sys::JNIEnv,
            _class: $crate::canvas::jni_sys::jclass,
            index: $crate::canvas::jni_sys::jint,
            connected: $crate::canvas::jni_sys::jboolean,
            values: $crate::canvas::jni_sys::jfloatArray,
        ) {
            let mut v = [0f32; 21];
            if ((**env).GetArrayLength.expect("jni"))(env, values) != 21 {
                return;
            }
            ((**env).GetFloatArrayRegion.expect("jni"))(env, values, 0, 21, v.as_mut_ptr());
            let (mut buttons, mut axes) = ([0f32; 17], [0f32; 4]);
            buttons.copy_from_slice(&v[..17]);
            axes.copy_from_slice(&v[17..]);
            with(|h| {
                h.pad(
                    index as u8,
                    connected == $crate::canvas::jni_sys::JNI_TRUE,
                    buttons,
                    axes,
                )
            });
        }
    };
}
