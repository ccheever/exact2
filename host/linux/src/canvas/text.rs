//! The Canvas host's platform text (`EXACT_TEXT=platform`;
//! [`crate::text::platform`]): measurement by the reader's `PlatformText`
//! (Minikin through `Paint.getTextRunAdvances` and the fonts the zygote
//! loaded), one JNI call per paragraph. The host's thread may be one Rust
//! made (the booting thread): it is attached to the VM for its calls and
//! detached when it is done ([`detach`]).
#![allow(unsafe_code)]

use crate::text::platform::{Segment, Shaper, Style};
use jni_sys::{
    jboolean, jclass, jfloatArray, jint, jmethodID, jobject, jsize, JNIEnv, JavaVM, JNI_FALSE,
    JNI_OK, JNI_TRUE, JNI_VERSION_1_6,
};
use std::cell::Cell;
use std::ffi::CStr;

/// The reader's measuring class and its methods, resolved once on a thread
/// whose class loader sees it.
struct Jni {
    vm: *mut JavaVM,
    class: jclass,
    declare: jmethodID,
    style: jmethodID,
    measure: jmethodID,
    metrics: jmethodID,
}

// SAFETY: a JavaVM pointer, a global class reference and method ids are valid
// on every thread.
unsafe impl Send for Jni {}
unsafe impl Sync for Jni {}

thread_local! {
    /// Whether this thread was attached here (and so is detached here).
    static ATTACHED: Cell<bool> = const { Cell::new(false) };
}

/// Install the reader's `PlatformText` as the process's shaper, when
/// `EXACT_TEXT=platform` asks for it. `env` is a Java thread's (its class
/// loader finds the app's classes).
///
/// # Safety
/// `env` is the current JNI call's.
pub unsafe fn install(env: *mut JNIEnv) {
    if !crate::text::platform::requested() {
        return;
    }
    let e = &**env;
    let mut vm: *mut JavaVM = std::ptr::null_mut();
    if e.GetJavaVM.expect("jni")(env, &mut vm) != JNI_OK {
        return;
    }
    let local =
        e.FindClass.expect("jni")(env, c"dev/exact/bench/exactcanvas/PlatformText".as_ptr());
    if local.is_null() {
        e.ExceptionClear.expect("jni")(env);
        crate::android::log("exact: EXACT_TEXT=platform but no PlatformText class; Rust text");
        return;
    }
    let class = e.NewGlobalRef.expect("jni")(env, local) as jclass;
    e.DeleteLocalRef.expect("jni")(env, local);
    let method = |name: &CStr, sig: &CStr| {
        e.GetStaticMethodID.expect("jni")(env, class, name.as_ptr(), sig.as_ptr())
    };
    let jni = Jni {
        vm,
        class,
        declare: method(c"declare", c"(I[Ljava/lang/String;[I[Z)Z"),
        style: method(c"style", c"(IIZFZ)I"),
        measure: method(c"measure", c"([C[I[F[F)V"),
        metrics: method(c"metrics", c"(IF[F)V"),
    };
    if [jni.declare, jni.style, jni.measure, jni.metrics]
        .iter()
        .any(|m| m.is_null())
    {
        e.ExceptionClear.expect("jni")(env);
        crate::android::log("exact: PlatformText lacks a method; Rust text");
        return;
    }
    crate::text::platform::install(Box::new(jni));
    crate::android::log("exact: platform text (Minikin)");
}

/// Detach this thread if it was attached for platform text: a thread Rust
/// made calls this before it ends (ART aborts on an attached thread's exit).
pub fn detach() {
    if !ATTACHED.with(|a| a.replace(false)) {
        return;
    }
    if let Some(vm) = VM.get() {
        // SAFETY: this thread attached itself through the same VM.
        unsafe { (**vm.0).DetachCurrentThread.expect("jni")(vm.0) };
    }
}

struct Vm(*mut JavaVM);
// SAFETY: the process's one VM.
unsafe impl Send for Vm {}
unsafe impl Sync for Vm {}
static VM: std::sync::OnceLock<Vm> = std::sync::OnceLock::new();

impl Jni {
    /// This thread's JNIEnv, attaching it if it is not a Java thread.
    unsafe fn env(&self) -> Option<*mut JNIEnv> {
        let _ = VM.set(Vm(self.vm));
        let invoke = &**self.vm;
        let mut env: *mut std::ffi::c_void = std::ptr::null_mut();
        if invoke.GetEnv.expect("jni")(self.vm, &mut env, JNI_VERSION_1_6) == JNI_OK {
            return Some(env.cast());
        }
        // Named as it is (ART would call it "Thread-N").
        let name = std::ffi::CString::new(std::thread::current().name().unwrap_or("exact"))
            .unwrap_or_default();
        let mut args = jni_sys::JavaVMAttachArgs {
            version: JNI_VERSION_1_6,
            name: name.as_ptr() as *mut std::ffi::c_char,
            group: std::ptr::null_mut(),
        };
        if invoke.AttachCurrentThread.expect("jni")(
            self.vm,
            &mut env,
            (&mut args as *mut jni_sys::JavaVMAttachArgs).cast(),
        ) != JNI_OK
        {
            return None;
        }
        ATTACHED.with(|a| a.set(true));
        Some(env.cast())
    }

    /// Clear a pending exception (described in the log); whether there was one.
    unsafe fn failed(env: *mut JNIEnv) -> bool {
        let e = &**env;
        if e.ExceptionCheck.expect("jni")(env) == JNI_TRUE {
            e.ExceptionDescribe.expect("jni")(env);
            e.ExceptionClear.expect("jni")(env);
            return true;
        }
        false
    }

    unsafe fn floats(env: *mut JNIEnv, n: usize) -> jfloatArray {
        (**env).NewFloatArray.expect("jni")(env, n as jsize)
    }

    unsafe fn drop_local(env: *mut JNIEnv, o: jobject) {
        if !o.is_null() {
            (**env).DeleteLocalRef.expect("jni")(env, o);
        }
    }
}

fn flag(b: bool) -> jboolean {
    if b {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

impl Shaper for Jni {
    fn declare(&self, stack: u16, faces: &[(String, u16, bool)]) -> bool {
        // SAFETY: JNI calls on this thread's env, with local references
        // deleted before returning.
        unsafe {
            let Some(env) = self.env() else { return false };
            let e = &**env;
            let string_class = e.FindClass.expect("jni")(env, c"java/lang/String".as_ptr());
            let paths = e.NewObjectArray.expect("jni")(
                env,
                faces.len() as jsize,
                string_class,
                std::ptr::null_mut(),
            );
            for (i, (path, _, _)) in faces.iter().enumerate() {
                let units: Vec<u16> = path.encode_utf16().collect();
                let s = e.NewString.expect("jni")(env, units.as_ptr(), units.len() as jsize);
                e.SetObjectArrayElement.expect("jni")(env, paths, i as jsize, s);
                Self::drop_local(env, s);
            }
            let weights: Vec<jint> = faces.iter().map(|f| jint::from(f.1)).collect();
            let italics: Vec<jboolean> = faces.iter().map(|f| flag(f.2)).collect();
            let w = e.NewIntArray.expect("jni")(env, weights.len() as jsize);
            e.SetIntArrayRegion.expect("jni")(env, w, 0, weights.len() as jsize, weights.as_ptr());
            let it = e.NewBooleanArray.expect("jni")(env, italics.len() as jsize);
            e.SetBooleanArrayRegion.expect("jni")(
                env,
                it,
                0,
                italics.len() as jsize,
                italics.as_ptr(),
            );
            let ok = e.CallStaticBooleanMethod.expect("jni")(
                env,
                self.class,
                self.declare,
                jint::from(stack),
                paths,
                w,
                it,
            );
            let failed = Self::failed(env);
            for o in [string_class, paths, w, it] {
                Self::drop_local(env, o);
            }
            !failed && ok == JNI_TRUE
        }
    }

    fn style(&self, style: &Style) -> u32 {
        // SAFETY: as in `declare`.
        unsafe {
            let Some(env) = self.env() else { return 0 };
            let k = (**env).CallStaticIntMethod.expect("jni")(
                env,
                self.class,
                self.style,
                style.family.code(),
                jint::from(style.weight),
                jint::from(flag(style.italic)),
                f64::from(style.letter_spacing),
                jint::from(flag(style.tabular)),
            );
            if Self::failed(env) {
                0
            } else {
                k as u32
            }
        }
    }

    fn measure(
        &self,
        chars: &[u16],
        segments: &[Segment],
        advances: &mut [f32],
        metrics: &mut [f32],
    ) {
        // SAFETY: as in `declare`; the arrays are sized from the slices.
        unsafe {
            let Some(env) = self.env() else { return };
            let e = &**env;
            let text = e.NewCharArray.expect("jni")(env, chars.len() as jsize);
            e.SetCharArrayRegion.expect("jni")(env, text, 0, chars.len() as jsize, chars.as_ptr());
            let ints: Vec<jint> = segments
                .iter()
                .flat_map(|s| {
                    [
                        s.start as jint,
                        s.len as jint,
                        jint::from(s.rtl) | jint::from(s.fallback) << 1,
                        s.style as jint,
                    ]
                })
                .collect();
            let segs = e.NewIntArray.expect("jni")(env, ints.len() as jsize);
            e.SetIntArrayRegion.expect("jni")(env, segs, 0, ints.len() as jsize, ints.as_ptr());
            let sizes: Vec<f32> = segments.iter().map(|s| s.size).collect();
            let size = Self::floats(env, sizes.len());
            e.SetFloatArrayRegion.expect("jni")(env, size, 0, sizes.len() as jsize, sizes.as_ptr());
            let n = chars.len() + 3 * segments.len();
            let out = Self::floats(env, n);
            e.CallStaticVoidMethod.expect("jni")(
                env,
                self.class,
                self.measure,
                text,
                segs,
                size,
                out,
            );
            if !Self::failed(env) {
                e.GetFloatArrayRegion.expect("jni")(
                    env,
                    out,
                    0,
                    advances.len() as jsize,
                    advances.as_mut_ptr(),
                );
                e.GetFloatArrayRegion.expect("jni")(
                    env,
                    out,
                    advances.len() as jsize,
                    metrics.len() as jsize,
                    metrics.as_mut_ptr(),
                );
            }
            for o in [text, segs, size, out] {
                Self::drop_local(env, o);
            }
        }
    }

    fn font_metrics(&self, style: u32, size: f32) -> [f32; 3] {
        let mut m = [size * 0.9, size * 0.3, 0.0];
        // SAFETY: as in `declare`.
        unsafe {
            let Some(env) = self.env() else { return m };
            let out = Self::floats(env, 3);
            (**env).CallStaticVoidMethod.expect("jni")(
                env,
                self.class,
                self.metrics,
                style as jint,
                f64::from(size),
                out,
            );
            if !Self::failed(env) {
                (**env).GetFloatArrayRegion.expect("jni")(env, out, 0, 3, m.as_mut_ptr());
            }
            Self::drop_local(env, out);
        }
        m
    }
}
