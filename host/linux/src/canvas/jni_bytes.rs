//! Byte-array conversion at the Android JNI boundary.
#![allow(unsafe_code)]

use jni_sys::JNIEnv;

/// A Java `byte[]` of UTF-8 as a string (what is not UTF-8 is replaced).
///
/// # Safety
/// `env` and `array` are the JNI call's.
pub unsafe fn utf8(env: *mut JNIEnv, array: jni_sys::jbyteArray) -> String {
    let len = ((**env).GetArrayLength.expect("jni"))(env, array).max(0);
    let mut buf = vec![0u8; len as usize];
    ((**env).GetByteArrayRegion.expect("jni"))(env, array, 0, len, buf.as_mut_ptr().cast());
    String::from_utf8_lossy(&buf).into_owned()
}

/// `bytes` as a new Java `byte[]`.
///
/// # Safety
/// `env` is the JNI call's.
pub unsafe fn bytes(env: *mut JNIEnv, bytes: &[u8]) -> jni_sys::jbyteArray {
    let len = bytes.len() as i32;
    let array = ((**env).NewByteArray.expect("jni"))(env, len);
    if !array.is_null() {
        ((**env).SetByteArrayRegion.expect("jni"))(env, array, 0, len, bytes.as_ptr().cast());
    }
    array
}
