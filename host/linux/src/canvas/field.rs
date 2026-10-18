//! Text fields the reader hosts (LLP 1115.001 D8): each `input` and
//! `textarea` is Android's own `EditText`, in the reader
//! (`host/android/canvas`, `Fields.kt`). What the presenter and the reader
//! say to each other about them is `presenter/hosted.rs`; this is the
//! Canvas host's half of the calls, and the one call that goes the other
//! way while layout runs.
//!
//! **Measured during layout.** The kernel lays a field out; the text in it
//! is the `EditText`'s to set, so its size is the `EditText`'s to say
//! ([`crate::fields`]). When the kernel measures a field's text it asks its
//! measurer, which calls up into the reader from the thread layout runs on
//! (`Fields.measure`, a static method) and has the answer before it
//! returns: an `EditText` of the reader's own, never attached, set as the
//! field is and measured. Nothing of this takes the channel of requests
//! (D5), whose queue is taken after the frame. Each distinct question is
//! asked once ([`MEMO`]).
// FFI to the JNI: the reader's static method, called from Rust.
#![allow(unsafe_code)]

use super::CanvasHost;
use crate::fields::{FieldFace, FieldMetrics};
use exact_runner::DataSource;
use jni_sys::{jclass, jmethodID, jstring, jvalue, JNIEnv, JavaVM, JNI_OK, JNI_VERSION_1_6};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

impl<D: DataSource + Default> CanvasHost<D> {
    /// What the reader is told of the text fields it hosts, when that is
    /// not what it was last told
    /// ([`crate::presenter::Presenter::reader_fields`] has the words).
    pub fn fields(&mut self) -> Option<Vec<i32>> {
        let scale = self.scale;
        self.p.reader_fields(scale)
    }

    /// The reader's field `id` took the focus, or lost it.
    pub fn field_focused(&mut self, id: u32, generation: u32, focused: bool) {
        let now = self.now();
        self.p.reader_field_focused(id, generation, focused, now);
        self.force = true;
    }

    /// The reader's field `id` holds `value` after its edit `seq`, its
    /// selection at `start` to `end`.
    pub fn field_edited(&mut self, id: u32, edit: (u32, u32), value: &str, selection: (u32, u32)) {
        let now = self.now();
        self.p.reader_field_edited(id, edit, value, selection, now);
        self.force = true;
    }

    /// The reader's field `id` began to compose, holding `value`, or its
    /// composition ended.
    pub fn field_composing(&mut self, id: u32, generation: u32, composing: bool, value: &str) {
        self.p
            .reader_field_composing(id, generation, composing, value);
        self.force = true;
    }

    /// The reader's field `id` moved its selection.
    pub fn field_selected(&mut self, id: u32, generation: u32, selection: (u32, u32)) {
        let now = self.now();
        self.p.reader_field_selected(id, generation, selection, now);
    }

    /// The reader's field `id` had its action key, Enter or Tab pressed
    /// ([`crate::presenter::field_action`]).
    pub fn field_action(&mut self, id: u32, generation: u32, action: i32) {
        let now = self.now();
        self.p.reader_field_action(id, generation, action, now);
        self.force = true;
    }
}

/// The reader's `Fields.measure`, found once.
struct Reader {
    vm: *mut JavaVM,
    class: jclass,
    measure: jmethodID,
}
// SAFETY: a JavaVM pointer, a global reference and a method id are the
// process's, valid on every thread.
unsafe impl Send for Reader {}
unsafe impl Sync for Reader {}

static READER: OnceLock<Option<Reader>> = OnceLock::new();

/// A question: the face's numbers and the width, its slant and whether it
/// wraps, its file, and the text.
type Asked = ([u32; 5], bool, bool, String, String);
/// Every question asked so far, with its answer. Emptied when it grows
/// large: a form asks a handful, a field typed into one a key.
static MEMO: Mutex<Option<HashMap<Asked, [f32; 3]>>> = Mutex::new(None);
const MEMO_MOST: usize = 512;

/// `Fields.measure(size, weight, flags, spacing, line, family, text, width)`.
const MEASURE: (&std::ffi::CStr, &std::ffi::CStr) =
    (c"measure", c"(FIIFFLjava/lang/String;Ljava/lang/String;F)J");

/// The reader boots the host (`Native.start`, `Native.startAsync`): its
/// field measurer is found by the class loader of the call, and from now on
/// this process's text fields are the reader's to host and to measure, at
/// `scale` pixels a point. A reader with no such method (one built before
/// D8) hosts none, and its fields are painted as desktop Linux's are.
///
/// # Safety
/// `env` is the JNI call's.
pub unsafe fn attach(env: *mut JNIEnv, scale: f32) {
    super::time::attach(env);
    let found = READER.get_or_init(|| {
        let mut vm: *mut JavaVM = std::ptr::null_mut();
        if ((**env).GetJavaVM.expect("jni"))(env, &mut vm) != JNI_OK {
            return None;
        }
        let clear = || {
            if ((**env).ExceptionCheck.expect("jni"))(env) != 0 {
                ((**env).ExceptionClear.expect("jni"))(env);
            }
        };
        let local = ((**env).FindClass.expect("jni"))(env, c"dev/exact/canvas/Fields".as_ptr());
        if local.is_null() {
            clear();
            return None;
        }
        let class = ((**env).NewGlobalRef.expect("jni"))(env, local);
        let measure = ((**env).GetStaticMethodID.expect("jni"))(
            env,
            class,
            MEASURE.0.as_ptr(),
            MEASURE.1.as_ptr(),
        );
        if measure.is_null() {
            clear();
            return None;
        }
        Some(Reader { vm, class, measure })
    });
    if found.is_some() {
        crate::fields::host(&Bridge, scale);
    }
}

/// A Java string of `s` (UTF-16, so every scalar crosses whole).
///
/// # Safety
/// `env` is the JNI call's.
pub unsafe fn java(env: *mut JNIEnv, s: &str) -> jstring {
    let units: Vec<u16> = s.encode_utf16().collect();
    ((**env).NewString.expect("jni"))(env, units.as_ptr(), units.len() as i32)
}

/// A Java string as Rust's, by its UTF-16 units: a scalar outside the
/// basic plane (an emoji) is one scalar here, where the JNI's modified
/// UTF-8 has it as two surrogates.
///
/// # Safety
/// `env` and `s` are the JNI call's.
pub unsafe fn text(env: *mut JNIEnv, s: jstring) -> String {
    if s.is_null() {
        return String::new();
    }
    let n = ((**env).GetStringLength.expect("jni"))(env, s).max(0);
    let mut units = vec![0u16; n as usize];
    ((**env).GetStringRegion.expect("jni"))(env, s, 0, n, units.as_mut_ptr());
    String::from_utf16_lossy(&units)
}

/// The reader's own `EditText`, asked from the thread layout runs on.
struct Bridge;

impl Bridge {
    /// Ask the reader. `None` when it cannot be asked from this thread or
    /// it threw.
    unsafe fn ask(face: &FieldFace, text: &str, width: Option<f32>) -> Option<[f32; 3]> {
        let reader = READER.get()?.as_ref()?;
        let vm = reader.vm;
        let mut env: *mut JNIEnv = std::ptr::null_mut();
        let out = (&mut env as *mut *mut JNIEnv).cast::<*mut c_void>();
        // A thread of Java's own has an environment. One this library made
        // (a host booting on its own thread) is attached for the call.
        let mut attached = false;
        if ((**vm).GetEnv.expect("jni"))(vm, out, JNI_VERSION_1_6) != JNI_OK {
            if ((**vm).AttachCurrentThread.expect("jni"))(vm, out, std::ptr::null_mut()) != JNI_OK {
                return None;
            }
            attached = true;
        }
        let family = java(env, &face.family);
        let string = java(env, text);
        let flags = i32::from(face.italic) | i32::from(face.multiline) << 1;
        let args = [
            jvalue { f: face.size },
            jvalue {
                i: i32::from(face.weight),
            },
            jvalue { i: flags },
            jvalue { f: face.spacing },
            jvalue { f: face.line },
            jvalue { l: family },
            jvalue { l: string },
            jvalue {
                f: width.unwrap_or(-1.0),
            },
        ];
        let call = (**env).CallStaticLongMethodA.expect("jni");
        let packed = call(env, reader.class, reader.measure, args.as_ptr());
        let threw = ((**env).ExceptionCheck.expect("jni"))(env) != 0;
        if threw {
            ((**env).ExceptionDescribe.expect("jni"))(env);
            ((**env).ExceptionClear.expect("jni"))(env);
        }
        ((**env).DeleteLocalRef.expect("jni"))(env, family);
        ((**env).DeleteLocalRef.expect("jni"))(env, string);
        if attached {
            ((**vm).DetachCurrentThread.expect("jni"))(vm);
        }
        // Three counts of pixels, 21 bits each: width, height, baseline.
        let part = |shift: u32| ((packed >> shift) & 0x1f_ffff) as f32;
        (!threw && packed >= 0).then(|| [part(0), part(21), part(42)])
    }
}

impl FieldMetrics for Bridge {
    fn measure(&self, face: &FieldFace, text: &str, width: Option<f32>) -> [f32; 3] {
        let key: Asked = (
            [
                face.size.to_bits(),
                u32::from(face.weight),
                face.spacing.to_bits(),
                face.line.to_bits(),
                width.map_or(u32::MAX, f32::to_bits),
            ],
            face.italic,
            face.multiline,
            face.family.clone(),
            text.to_owned(),
        );
        let known = |memo: &Option<HashMap<Asked, [f32; 3]>>| memo.as_ref()?.get(&key).copied();
        if let Some(answer) = known(&MEMO.lock().unwrap_or_else(|e| e.into_inner())) {
            return answer;
        }
        // SAFETY: the reader's class and method were found by `attach`.
        let answer = unsafe { Bridge::ask(face, text, width) }.unwrap_or_else(|| {
            // No reader to ask: about what a line of this size takes.
            let lines = text.split('\n').count().max(1) as f32;
            let across = text
                .split('\n')
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(0) as f32;
            let line = if face.line > 0.0 {
                face.line
            } else {
                face.size * 1.2
            };
            [across * face.size * 0.5, lines * line, face.size]
        });
        let mut memo = MEMO.lock().unwrap_or_else(|e| e.into_inner());
        let memo = memo.get_or_insert_with(HashMap::new);
        if memo.len() >= MEMO_MOST {
            memo.clear();
        }
        memo.insert(key, answer);
        answer
    }
}

/// The reader's settings changed (a new font scale, a new locale, another
/// density): what its field measured under the old ones is asked again.
pub fn forget() {
    *MEMO.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
