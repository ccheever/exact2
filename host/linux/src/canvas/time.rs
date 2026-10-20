//! Android's DateUtils and DateFormat, called on the runner's owner thread.
#![allow(unsafe_code)]
use exact_kernel::time::TimeRequest;
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

/// Install from a JNI entry, whose loader can see the reader's class.
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
        let local = ((**env).FindClass?)(env, c"dev/exact/canvas/TimeFormatting".as_ptr());
        if local.is_null() {
            ((**env).ExceptionClear?)(env);
            return None;
        }
        let class = ((**env).NewGlobalRef?)(env, local);
        ((**env).DeleteLocalRef?)(env, local);
        if class.is_null() {
            ((**env).ExceptionClear?)(env);
            return None;
        }
        let method = ((**env).GetStaticMethodID?)(
            env,
            class,
            c"format".as_ptr(),
            c"(DDLjava/lang/String;Ljava/lang/String;Ljava/lang/String;DI)Ljava/lang/String;"
                .as_ptr(),
        );
        if method.is_null() {
            ((**env).ExceptionClear?)(env);
            ((**env).DeleteGlobalRef?)(env, class);
            return None;
        }
        Some(Reader { vm, class, method })
    };
    if let Some(reader) = read() {
        if let Err(unused) = READER.set(reader) {
            if let Some(delete) = (**env).DeleteGlobalRef {
                delete(env, unused.class);
            }
        }
    }
}

/// Ask the reader synchronously; the headless host uses the English fallback.
pub fn format(r: &TimeRequest<'_>) -> Option<String> {
    let reader = READER.get()?;
    // SAFETY: attach retained the class globally; local references are dropped
    // before a thread we attached is detached.
    unsafe {
        let vm = reader.vm;
        let mut env: *mut JNIEnv = std::ptr::null_mut();
        let out = (&mut env as *mut *mut JNIEnv).cast::<*mut c_void>();
        let attached = ((**vm).GetEnv?)(vm, out, JNI_VERSION_1_6) != JNI_OK;
        if attached && ((**vm).AttachCurrentThread?)(vm, out, std::ptr::null_mut()) != JNI_OK {
            return None;
        }
        let names = [r.format, r.locale, r.time_zone].map(|s| super::field::java(env, s));
        let (n, unit) = exact_kernel::time::relative(r.datetime - r.now);
        let unit = ["minute", "hour", "day", "month", "year"]
            .iter()
            .position(|s| *s == unit)
            .unwrap() as i32;
        let args = [
            jvalue { d: r.datetime },
            jvalue { d: r.now },
            jvalue { l: names[0] },
            jvalue { l: names[1] },
            jvalue { l: names[2] },
            jvalue { d: n as f64 },
            jvalue { i: unit },
        ];
        let value =
            ((**env).CallStaticObjectMethodA?)(env, reader.class, reader.method, args.as_ptr())
                as jstring;
        let threw = ((**env).ExceptionCheck?)(env) != 0;
        let text = if threw || value.is_null() {
            None
        } else {
            Some(super::field::text(env, value))
        };
        if threw {
            ((**env).ExceptionClear?)(env);
        }
        for name in names {
            ((**env).DeleteLocalRef?)(env, name);
        }
        if !value.is_null() {
            ((**env).DeleteLocalRef?)(env, value);
        }
        if attached {
            ((**vm).DetachCurrentThread?)(vm);
        }
        text
    }
}
