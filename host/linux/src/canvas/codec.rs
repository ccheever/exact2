//! Android's image codec (LLP 1133.019 D16): `compressImage` and
//! `transformImage` decoded by `ImageDecoder`, turned, cut and sized as
//! `Bitmap`s, encoded by `Bitmap.compress` and given their metadata by
//! `ExifInterface`, the reader's `ImageCodec` (Kotlin), called from the
//! storage worker that runs the request. The order, the geometry and the
//! search are `exact_data::image::platform`'s, so they are Apple's.
#![allow(unsafe_code)]
use exact_data::image::platform::{Decoded, Plan, Platform};
use exact_data::image::Metadata;
use jni_sys::{jclass, jmethodID, jobject, jvalue, JNIEnv, JavaVM, JNI_OK, JNI_VERSION_1_6};
use std::ffi::{c_void, CStr};
use std::sync::OnceLock;

/// `ImageCodec`'s methods, by name and JNI signature.
const METHODS: [(&CStr, &CStr); 6] = [
    (c"decode", c"([BZJ)Ljava/lang/String;"),
    (c"edit", c"(JLjava/lang/String;)Ljava/lang/String;"),
    (c"encode", c"(JIILjava/lang/String;DZ)[B"),
    (
        c"metadata",
        c"([B[BLjava/lang/String;Ljava/lang/String;II)[B",
    ),
    (c"close", c"(J)V"),
    (c"encodes", c"(Ljava/lang/String;)Z"),
];

struct Codec {
    vm: *mut JavaVM,
    class: jclass,
    methods: [jmethodID; 6],
}
// SAFETY: a JavaVM pointer, a global class reference and method ids are the
// process's, valid on every thread.
unsafe impl Send for Codec {}
unsafe impl Sync for Codec {}

static FOUND: OnceLock<()> = OnceLock::new();

/// Find `ImageCodec` from a JNI entry, whose class loader sees the reader's
/// classes (a storage worker's would not), and install the codec. A reader
/// without the class leaves the image calls `unsupported`.
///
/// # Safety
/// `env` is the JNI call's.
pub unsafe fn attach(env: *mut JNIEnv) {
    FOUND.get_or_init(|| {
        if let Some(codec) = find(env) {
            exact_data::image::platform::install(Box::new(codec));
        }
    });
}

unsafe fn find(env: *mut JNIEnv) -> Option<Codec> {
    let clear = || {
        if ((**env).ExceptionCheck?)(env) != 0 {
            ((**env).ExceptionClear?)(env);
        }
        Some(())
    };
    let mut vm = std::ptr::null_mut();
    if ((**env).GetJavaVM?)(env, &mut vm) != JNI_OK {
        return None;
    }
    let local = ((**env).FindClass?)(env, c"dev/exact/canvas/ImageCodec".as_ptr());
    if local.is_null() {
        clear();
        return None;
    }
    let class = ((**env).NewGlobalRef?)(env, local);
    ((**env).DeleteLocalRef?)(env, local);
    let mut methods = [std::ptr::null_mut(); 6];
    for (slot, (name, signature)) in methods.iter_mut().zip(METHODS) {
        *slot = ((**env).GetStaticMethodID?)(env, class, name.as_ptr(), signature.as_ptr());
        if slot.is_null() {
            clear();
            ((**env).DeleteGlobalRef?)(env, class);
            return None;
        }
    }
    Some(Codec { vm, class, methods })
}

/// What a failed call says.
type Failure = (String, String);

fn unknown(detail: &str) -> Failure {
    ("UnknownError".into(), detail.into())
}

impl Codec {
    /// Run `f` with this thread's environment, attaching a storage worker
    /// for the call and detaching it after; a Java exception is cleared and
    /// fails the call.
    fn with<T>(&self, f: impl FnOnce(*mut JNIEnv) -> Result<T, Failure>) -> Result<T, Failure> {
        // SAFETY: `vm` is the process's; an environment is used only on the
        // thread it belongs to, and local references die with the call.
        unsafe {
            let vm = self.vm;
            let mut env: *mut JNIEnv = std::ptr::null_mut();
            let out = (&mut env as *mut *mut JNIEnv).cast::<*mut c_void>();
            let attached =
                ((**vm).GetEnv.ok_or_else(|| unknown("jni"))?)(vm, out, JNI_VERSION_1_6) != JNI_OK;
            if attached
                && ((**vm).AttachCurrentThread.ok_or_else(|| unknown("jni"))?)(
                    vm,
                    out,
                    std::ptr::null_mut(),
                ) != JNI_OK
            {
                return Err(unknown("the storage worker could not attach to the VM"));
            }
            let frame = ((**env).PushLocalFrame.expect("jni"))(env, 16) == JNI_OK;
            let mut result = f(env);
            if ((**env).ExceptionCheck.expect("jni"))(env) != 0 {
                ((**env).ExceptionClear.expect("jni"))(env);
                result = Err(unknown("the platform's codec threw"));
            }
            if frame {
                ((**env).PopLocalFrame.expect("jni"))(env, std::ptr::null_mut());
            }
            if attached {
                ((**vm).DetachCurrentThread.expect("jni"))(vm);
            }
            result
        }
    }

    unsafe fn object(&self, env: *mut JNIEnv, method: usize, args: &[jvalue]) -> jobject {
        ((**env).CallStaticObjectMethodA.expect("jni"))(
            env,
            self.class,
            self.methods[method],
            args.as_ptr(),
        )
    }

    /// A decode's or an edit's answer: the picture, or `{"error", "message"}`.
    unsafe fn decoded(&self, env: *mut JNIEnv, answer: jobject) -> Result<Decoded, Failure> {
        if answer.is_null() {
            return Err(unknown("the platform's codec answered nothing"));
        }
        let text = super::field::text(env, answer);
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| unknown(&e.to_string()))?;
        if let Some(code) = v["error"].as_str() {
            let message = v["message"].as_str().unwrap_or_default();
            return Err((code.to_owned(), message.to_owned()));
        }
        let n = |k: &str| v[k].as_u64().ok_or_else(|| unknown(&format!("no {k}")));
        Ok(Decoded {
            handle: n("handle")?,
            width: n("width")? as u32,
            height: n("height")? as u32,
            media_type: v["type"].as_str().unwrap_or("image/jpeg").to_owned(),
            color_space: v["colorSpace"].as_str().unwrap_or("other").to_owned(),
            hdr: v["hdr"].as_bool().unwrap_or(false),
        })
    }

    unsafe fn read(env: *mut JNIEnv, array: jobject) -> Option<Vec<u8>> {
        if array.is_null() {
            return None;
        }
        let len = ((**env).GetArrayLength.expect("jni"))(env, array).max(0);
        let mut out = vec![0u8; len as usize];
        ((**env).GetByteArrayRegion.expect("jni"))(env, array, 0, len, out.as_mut_ptr().cast());
        Some(out)
    }
}

impl Platform for Codec {
    fn decode(&self, source: &[u8], srgb: bool, max_pixels: u64) -> Result<Decoded, Failure> {
        self.with(|env| unsafe {
            let bytes = super::jni::bytes(env, source);
            let answer = self.object(
                env,
                0,
                &[
                    jvalue { l: bytes },
                    jvalue { z: u8::from(srgb) },
                    jvalue {
                        j: max_pixels.min(i64::MAX as u64) as i64,
                    },
                ],
            );
            self.decoded(env, answer)
        })
    }

    fn edit(&self, handle: u64, plan: &Plan) -> Result<Decoded, Failure> {
        self.with(|env| unsafe {
            let plan = super::field::java(env, &plan.json().to_string());
            let answer = self.object(env, 1, &[jvalue { j: handle as i64 }, jvalue { l: plan }]);
            self.decoded(env, answer)
        })
    }

    fn encode(
        &self,
        handle: u64,
        (width, height): (u32, u32),
        media_type: &str,
        quality: f64,
        hdr: bool,
    ) -> Result<Vec<u8>, Failure> {
        self.with(|env| unsafe {
            let kind = super::field::java(env, media_type);
            let answer = self.object(
                env,
                2,
                &[
                    jvalue { j: handle as i64 },
                    jvalue { i: width as i32 },
                    jvalue { i: height as i32 },
                    jvalue { l: kind },
                    jvalue { d: quality },
                    jvalue { z: u8::from(hdr) },
                ],
            );
            Self::read(env, answer).ok_or_else(|| {
                (
                    "unsupported".to_owned(),
                    format!("the platform did not encode {media_type}"),
                )
            })
        })
    }

    fn metadata(
        &self,
        source: &[u8],
        encoded: &[u8],
        media_type: &str,
        metadata: Metadata,
        (width, height): (u32, u32),
    ) -> Option<Vec<u8>> {
        self.with(|env| unsafe {
            let args = [
                jvalue {
                    l: super::jni::bytes(env, source),
                },
                jvalue {
                    l: super::jni::bytes(env, encoded),
                },
                jvalue {
                    l: super::field::java(env, media_type),
                },
                jvalue {
                    l: super::field::java(env, metadata.word()),
                },
                jvalue { i: width as i32 },
                jvalue { i: height as i32 },
            ];
            Ok(Self::read(env, self.object(env, 3, &args)))
        })
        .ok()
        .flatten()
    }

    fn close(&self, handle: u64) {
        let _ = self.with(|env| unsafe {
            ((**env).CallStaticVoidMethodA.expect("jni"))(
                env,
                self.class,
                self.methods[4],
                [jvalue { j: handle as i64 }].as_ptr(),
            );
            Ok(())
        });
    }

    fn encodes(&self, media_type: &str) -> bool {
        self.with(|env| unsafe {
            let kind = super::field::java(env, media_type);
            Ok(((**env).CallStaticBooleanMethodA.expect("jni"))(
                env,
                self.class,
                self.methods[5],
                [jvalue { l: kind }].as_ptr(),
            ) != 0)
        })
        .unwrap_or(false)
    }
}
