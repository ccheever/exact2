//! The reader's Keystore for locked secrets (LLP 1133.010 D6): the host's
//! presence vault adds an item through `dev.exact.canvas.Presence.keystore`,
//! called synchronously on the thread the backend runs on. The reader
//! answers `ok:<text>` or `err:<why>`.
#![allow(unsafe_code)]
use jni_sys::{jclass, jmethodID, jstring, jvalue, JNIEnv, JavaVM, JNI_OK, JNI_VERSION_1_6};
use std::{ffi::c_void, sync::OnceLock};

struct Reader {
    vm: *mut JavaVM,
    class: jclass,
    method: jmethodID,
}
// The VM, global class reference and method ID are valid across threads.
unsafe impl Send for Reader {}
unsafe impl Sync for Reader {}
static READER: OnceLock<Reader> = OnceLock::new();

/// Install from a JNI entry, whose loader can see the reader's class, and
/// hand the vault its hook.
/// # Safety
/// `env` belongs to this JNI entry and thread.
pub unsafe fn attach(env: *mut JNIEnv) {
    if READER.get().is_some() {
        return;
    }
    let read = || {
        let mut vm = std::ptr::null_mut();
        if ((**env).GetJavaVM?)(env, &mut vm) != JNI_OK {
            return None;
        }
        let local = ((**env).FindClass?)(env, c"dev/exact/canvas/Presence".as_ptr());
        if local.is_null() {
            ((**env).ExceptionClear?)(env);
            return None;
        }
        let class = ((**env).NewGlobalRef?)(env, local);
        ((**env).DeleteLocalRef?)(env, local);
        let signature = c"(ILjava/lang/String;Ljava/lang/String;)Ljava/lang/String;";
        let method =
            ((**env).GetStaticMethodID?)(env, class, c"keystore".as_ptr(), signature.as_ptr());
        if method.is_null() {
            ((**env).ExceptionClear?)(env);
            ((**env).DeleteGlobalRef?)(env, class);
            return None;
        }
        Some(Reader { vm, class, method })
    };
    if let Some(reader) = read() {
        if READER.set(reader).is_ok() {
            crate::presence_secrets::on_keystore(keystore);
        }
    }
}

/// The reader's Keystore, asked synchronously.
fn keystore(op: u32, request: &str, value: &str) -> Result<String, String> {
    let reader = READER
        .get()
        .ok_or("the reader's Keystore is not attached")?;
    // SAFETY: attach retained the class globally; local references are
    // dropped before a thread attached here is detached.
    let answer = unsafe {
        let vm = reader.vm;
        let mut env: *mut JNIEnv = std::ptr::null_mut();
        let out = (&mut env as *mut *mut JNIEnv).cast::<*mut c_void>();
        let attached = ((**vm).GetEnv.ok_or("jni")?)(vm, out, JNI_VERSION_1_6) != JNI_OK;
        if attached
            && ((**vm).AttachCurrentThread.ok_or("jni")?)(vm, out, std::ptr::null_mut()) != JNI_OK
        {
            return Err("the Keystore's thread could not attach".into());
        }
        let strings = [request, value].map(|s| super::field::java(env, s));
        let args = [
            jvalue { i: op as i32 },
            jvalue { l: strings[0] },
            jvalue { l: strings[1] },
        ];
        let call = (**env).CallStaticObjectMethodA.ok_or("jni")?;
        let got = call(env, reader.class, reader.method, args.as_ptr()) as jstring;
        let threw = ((**env).ExceptionCheck.ok_or("jni")?)(env) != 0;
        if threw {
            ((**env).ExceptionClear.ok_or("jni")?)(env);
        }
        let text = (!threw && !got.is_null()).then(|| super::field::text(env, got));
        for s in strings {
            ((**env).DeleteLocalRef.ok_or("jni")?)(env, s);
        }
        if !got.is_null() {
            ((**env).DeleteLocalRef.ok_or("jni")?)(env, got);
        }
        if attached {
            ((**vm).DetachCurrentThread.ok_or("jni")?)(vm);
        }
        text
    };
    match answer.as_deref().map(|a| a.split_once(':')) {
        Some(Some(("ok", text))) => Ok(text.to_owned()),
        Some(Some((_, why))) => Err(why.to_owned()),
        _ => Err("the reader's Keystore threw".into()),
    }
}
