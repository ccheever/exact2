//! The Canvas host's JNI, once for every app (LLP 1076): an app's library is
//! its plan, its data source and `exact_linux::canvas_jni!(Data)`, which
//! defines `dev.exact.bench.exactcanvas.Native`'s methods over a
//! [`super::CanvasHost`] that lives on the thread that called `start`.
//!
//! The reader may call `start` from a thread of its own as soon as the
//! process starts (at the size it expects the view to be, `resize` if it
//! is not), then make every other call on that thread: the host boots while
//! Android sets the activity up.
// FFI to the JNI, jnigraphics and AHardwareBuffer APIs.
#![allow(unsafe_code)]

/// Define the JNI for an app over `CanvasHost<$data>` with the plan and
/// compat the app's build script writes to `OUT_DIR` (`app.plan`,
/// `compat.json`).
#[macro_export]
macro_rules! canvas_jni {
    ($data:ty) => {
        #[global_allocator]
        static EXACT_ALLOCATOR: $crate::canvas::jni::Allocator = $crate::canvas::jni::Allocator;

        mod exact_canvas_jni {
            #![allow(clippy::missing_safety_doc, unsafe_code)]
            use std::cell::RefCell;
            use std::ffi::{c_void, CStr};
            use $crate::canvas::jni_sys::{
                jboolean, jclass, jdouble, jfloat, jint, jintArray, jlong, jobject, jstring,
                JNIEnv, JNI_FALSE, JNI_TRUE,
            };

            const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
            const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

            type Host = $crate::canvas::CanvasHost<$data>;

            /// How long after boot the library's read-only pages are let go of
            /// (boot's own work is done; see `release_library_pages_after`).
            const RELEASE_AFTER: std::time::Duration = std::time::Duration::from_secs(2);

            thread_local! {
                static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
                /// The op stream the last `frame` returned; the reader reads it in place.
                static LAST: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
                /// A move of the last stream (op 22), read at once; LAST stays drawn.
                static MOVE: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
                /// What the reader draws: the last stream's drawing, without its
                /// definitions (`None` after a move, or unknown).
                static DRAWN: RefCell<Option<Vec<u32>>> = const { RefCell::new(None) };
            }

            fn with<T>(f: impl FnOnce(&mut Host) -> T) -> Option<T> {
                HOST.with(|h| h.borrow_mut().as_mut().map(f))
            }

            unsafe fn string(env: *mut JNIEnv, s: jstring) -> String {
                let get = (**env).GetStringUTFChars.expect("jni");
                let release = (**env).ReleaseStringUTFChars.expect("jni");
                let p = get(env, s, std::ptr::null_mut());
                let out = CStr::from_ptr(p).to_string_lossy().into_owned();
                release(env, s, p);
                out
            }

            fn flag(b: bool) -> jboolean {
                if b {
                    JNI_TRUE
                } else {
                    JNI_FALSE
                }
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_start(
                env: *mut JNIEnv,
                _class: jclass,
                width: jint,
                height: jint,
                scale: jfloat,
                dir: jstring,
                live: jboolean,
            ) -> jboolean {
                static LOG: std::sync::Once = std::sync::Once::new();
                LOG.call_once($crate::android::redirect_stdio);
                let dir = string(env, dir);
                std::env::set_var("HOME", &dir);
                std::env::set_var("XDG_DATA_HOME", format!("{dir}/data"));
                std::env::set_var("EXACT_ASSETS", format!("{dir}/app"));
                std::env::set_var("EXACT_FONTS", "/system/fonts");
                std::env::set_var("EXACT_FONT", "Roboto");
                std::env::set_var("BENCH_LIVE", if live == JNI_TRUE { "1" } else { "0" });
                match Host::boot(PLAN, COMPAT, (width as u32, height as u32), scale) {
                    Ok(h) => {
                        HOST.with(|slot| *slot.borrow_mut() = Some(h));
                        $crate::android::release_library_pages_after(RELEASE_AFTER);
                        JNI_TRUE
                    }
                    Err(e) => {
                        $crate::android::log(&format!("exact: boot: {e}"));
                        JNI_FALSE
                    }
                }
            }

            /// What a booting thread hands the thread that runs the host: the
            /// host and its first frame. The
            /// host's `Rc`s move with it whole; nothing on the booting thread
            /// keeps one (its image and executor threads hold only `Send` state).
            struct Booted(Host, Option<Vec<u32>>);
            // SAFETY: see above: the booting thread lets go of all of it.
            unsafe impl Send for Booted {}
            static BOOTED: std::sync::Mutex<Option<Result<Booted, String>>> =
                std::sync::Mutex::new(None);
            static READY: std::sync::Condvar = std::sync::Condvar::new();
            thread_local! {
                /// The first frame a booting thread painted, returned by the next `frame`.
                static PRIMED: RefCell<Option<Vec<u32>>> = const { RefCell::new(None) };
            }

            /// `start`, on a thread of its own: the host boots and paints its first
            /// frame while the reader sets up; `adopt` takes it.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_startAsync(
                env: *mut JNIEnv,
                _class: jclass,
                width: jint,
                height: jint,
                scale: jfloat,
                dir: jstring,
                live: jboolean,
            ) {
                static LOG: std::sync::Once = std::sync::Once::new();
                LOG.call_once($crate::android::redirect_stdio);
                // The environment is set here, before any other thread reads it.
                let dir = string(env, dir);
                std::env::set_var("HOME", &dir);
                std::env::set_var("XDG_DATA_HOME", format!("{dir}/data"));
                std::env::set_var("EXACT_ASSETS", format!("{dir}/app"));
                std::env::set_var("EXACT_FONTS", "/system/fonts");
                std::env::set_var("EXACT_FONT", "Roboto");
                std::env::set_var("BENCH_LIVE", if live == JNI_TRUE { "1" } else { "0" });
                std::thread::Builder::new()
                    .name("exact-boot".into())
                    .spawn(move || {
                        let booted = $crate::android::trace(c"exact boot", || {
                            Host::boot(PLAN, COMPAT, (width as u32, height as u32), scale).map(
                                |mut h| {
                                    h.set_borrowed(true);
                                    let first = h.frame();
                                    Booted(h, first)
                                },
                            )
                        });
                        *BOOTED.lock().unwrap_or_else(|e| e.into_inner()) = Some(booted);
                        READY.notify_all();
                    })
                    .expect("boot thread");
            }

            /// Take the host `startAsync` booted, waiting for it; whether it booted.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_adopt(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                let mut slot = BOOTED.lock().unwrap_or_else(|e| e.into_inner());
                while slot.is_none() {
                    slot = READY.wait(slot).unwrap_or_else(|e| e.into_inner());
                }
                match slot.take().expect("booted") {
                    Ok(Booted(mut h, first)) => {
                        h.set_borrowed(false);
                        HOST.with(|s| *s.borrow_mut() = Some(h));
                        PRIMED.with(|p| *p.borrow_mut() = first);
                        $crate::android::release_library_pages_after(RELEASE_AFTER);
                        JNI_TRUE
                    }
                    Err(e) => {
                        $crate::android::log(&format!("exact: boot: {e}"));
                        JNI_FALSE
                    }
                }
            }

            /// The view's size in pixels, when it is not the size `start` was given.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_resize(
                _env: *mut JNIEnv,
                _class: jclass,
                width: jint,
                height: jint,
            ) {
                with(|h| h.resize((width as u32, height as u32)));
            }

            /// A new op stream as a direct ByteBuffer over Rust memory (valid until
            /// the next call), or null when nothing changed.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_frame(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jobject {
                // The first frame the booting thread painted, then the host's own.
                let primed = PRIMED.with(|p| p.borrow_mut().take());
                let Some(ops) = primed.or_else(|| with(|h| h.frame()).flatten()) else {
                    return std::ptr::null_mut();
                };
                let new = (**env).NewDirectByteBuffer.expect("jni");
                if ops.first() == Some(&22) {
                    DRAWN.with(|d| *d.borrow_mut() = None);
                    return MOVE.with(|m| {
                        let mut m = m.borrow_mut();
                        *m = ops;
                        new(env, m.as_mut_ptr().cast(), (m.len() * 4) as i64)
                    });
                }
                LAST.with(|last| {
                    let mut last = last.borrow_mut();
                    // The same stream draws the same pixels: no new frame for Android.
                    if *last == ops {
                        return std::ptr::null_mut();
                    }
                    // What the reader draws already, with nothing to define (a
                    // touch down after a stream that defined rows): no frame.
                    let now = $crate::canvas::drawing(&ops);
                    let same = DRAWN.with(|d| {
                        let mut d = d.borrow_mut();
                        let same = matches!((&now, &*d), (Some((n, false)), Some(o)) if n == o);
                        *d = now.map(|(n, _)| n);
                        same
                    });
                    if same {
                        return std::ptr::null_mut();
                    }
                    *last = ops;
                    new(env, last.as_mut_ptr().cast(), (last.len() * 4) as i64)
                })
            }

            /// The collection pass a scroll left: rows mount after the frame. With
            /// `limit` >= 0, a slice: at most that many rows past what shows per
            /// list, the scrolled list leading toward `velocity` (px/s). Bit 0: a
            /// paint is wanted; bit 1: rows are left for another slice.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_refine(
                _env: *mut JNIEnv,
                _class: jclass,
                limit: jint,
                velocity: jfloat,
            ) -> jint {
                with(|h| {
                    let scale = h.scale();
                    let wanted = h.refine_slice(
                        (limit >= 0).then_some(limit as u32),
                        f64::from(velocity / scale),
                    );
                    i32::from(wanted) | i32::from(h.refine_pending()) << 1
                })
                .unwrap_or(0)
            }

            /// Whether a moved paint owes a paint once scrolling pauses.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_owed(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                flag(with(|h| h.owed()).unwrap_or(false))
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_animating(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                flag(with(|h| h.animating()).unwrap_or(false))
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_nextDue(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jdouble {
                with(|h| h.next_due()).flatten().unwrap_or(-1.0)
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_fds(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jintArray {
                let fds = with(|h| h.fds()).unwrap_or([-1, -1]);
                let arr = ((**env).NewIntArray.expect("jni"))(env, 2);
                ((**env).SetIntArrayRegion.expect("jni"))(env, arr, 0, 2, fds.as_ptr());
                arr
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_scroll(
                _env: *mut JNIEnv,
                _class: jclass,
                dy: jfloat,
            ) {
                with(|h| h.scroll(dy));
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_touch(
                _env: *mut JNIEnv,
                _class: jclass,
                action: jint,
                x: jfloat,
                y: jfloat,
            ) {
                with(|h| h.touch(action, x, y));
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_poll(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                flag(with(|h| h.poll()).unwrap_or(false))
            }

            /// Picture `id` (IMAGE_DEF) into an ARGB_8888 bitmap of its size.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_copyImage(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                bitmap: jobject,
            ) -> jboolean {
                flag($crate::canvas::jni::copy_bitmap(env, id as u32, bitmap))
            }

            /// Picture `id` into a GPU buffer the reader wraps as a hardware bitmap.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_copyImageToBuffer(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                buffer: jobject,
            ) -> jboolean {
                flag($crate::canvas::jni::copy_buffer(env, id as u32, buffer))
            }

            /// The reader wraps a picture's own GPU buffer (`imageBuffer`): from
            /// now on platform-decoded pictures are decoded into one.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_hardwarePictures(
                _env: *mut JNIEnv,
                _class: jclass,
                on: jboolean,
            ) {
                $crate::image::hardware_pictures(on == JNI_TRUE);
            }

            /// Picture `id`'s own GPU buffer as a `HardwareBuffer` to wrap, or
            /// null when its pixels are on the heap (copy them instead).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_imageBuffer(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
            ) -> jobject {
                $crate::canvas::jni::image_buffer(env, id as u32)
            }

            /// A GPU canvas's window (op 28, kind 3); the window to pass back to
            /// `detachSurface`, or 0.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_attachSurface(
                env: *mut JNIEnv,
                _class: jclass,
                view: jint,
                surface: jobject,
                width: jint,
                height: jint,
            ) -> jlong {
                let window = $crate::canvas::jni::window_from_surface(env, surface);
                if window.is_null() {
                    return 0;
                }
                with(|h| {
                    h.attach_window(view as u32, window as usize, (width as u32, height as u32))
                });
                window as jlong
            }

            /// A GPU canvas without a window: it renders into buffers the
            /// reader draws itself (`canvasFrame`, `canvasBuffer`).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_attachBuffers(
                _env: *mut JNIEnv,
                _class: jclass,
                view: jint,
                width: jint,
                height: jint,
            ) {
                with(|h| h.attach_window(view as u32, 0, (width as u32, height as u32)));
            }

            /// A buffered canvas's newest frame, from any thread: its ring's
            /// generation (24 bits), the buffer's slot (8) and the frame's
            /// serial (32); -1 before the first, -2 when this device has no
            /// buffers for it (give it a window).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_canvasFrame(
                _env: *mut JNIEnv,
                _class: jclass,
                view: jint,
            ) -> jlong {
                $crate::canvas::jni::canvas_frame(view as u32)
            }

            /// That frame's buffer as a `HardwareBuffer` to wrap, or null.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_canvasBuffer(
                env: *mut JNIEnv,
                _class: jclass,
                view: jint,
            ) -> jobject {
                $crate::canvas::jni::canvas_buffer(env, view as u32)
            }

            /// The canvas's window is going: no more presenting into it.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_detachSurface(
                _env: *mut JNIEnv,
                _class: jclass,
                view: jint,
                window: jlong,
            ) {
                with(|h| h.detach_window(view as u32));
                $crate::canvas::jni::release_window(window as *mut c_void);
            }

            /// A key typed at the focused field: `code` a Unicode scalar, or 0
            /// with `backspace` set; `'\n'` is Enter.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_key(
                _env: *mut JNIEnv,
                _class: jclass,
                code: jint,
                backspace: jboolean,
            ) {
                let ch = char::from_u32(code as u32).filter(|_| code > 0);
                with(|h| h.key(ch, backspace == JNI_TRUE));
            }

            /// Whether an editable field holds the focus (1), a textarea (3), or none (0).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_editing(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jint {
                with(|h| {
                    if !h.editing() {
                        0
                    } else if h.multiline() {
                        3
                    } else {
                        1
                    }
                })
                .unwrap_or(0)
            }

            /// Pictures announced and not yet fetched, as id, width, height triples:
            /// a reader on its own thread makes them before the stream that draws
            /// them reaches its main thread.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_pendingPictures(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jintArray {
                let flat: Vec<i32> = $crate::canvas::pending_pictures()
                    .into_iter()
                    .flatten()
                    .map(|n| n as i32)
                    .collect();
                let arr = ((**env).NewIntArray.expect("jni"))(env, flat.len() as i32);
                ((**env).SetIntArrayRegion.expect("jni"))(
                    env,
                    arr,
                    0,
                    flat.len() as i32,
                    flat.as_ptr(),
                );
                arr
            }

            /// Run the calling thread on the cores outside the slowest cluster
            /// (a reader's own exact2 thread); whether it was set.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_fastCores(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                if $crate::android::fast_cores() {
                    JNI_TRUE
                } else {
                    JNI_FALSE
                }
            }

            /// An ADPF hint session over `tids` (this process's threads) with
            /// `target_ns` as their work's target; whether there is one.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_hintSession(
                env: *mut JNIEnv,
                _class: jclass,
                tids: jintArray,
                target_ns: jlong,
            ) -> jboolean {
                let n = ((**env).GetArrayLength.expect("jni"))(env, tids).max(0) as usize;
                let mut v = vec![0i32; n];
                ((**env).GetIntArrayRegion.expect("jni"))(env, tids, 0, n as i32, v.as_mut_ptr());
                if $crate::android_hint::session(&v, target_ns) {
                    JNI_TRUE
                } else {
                    JNI_FALSE
                }
            }

            /// Input arrived: the hint session's threads are about to work
            /// (`hintSession`); whether a hint was sent.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_inputHint(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                if $crate::android_hint::input() {
                    JNI_TRUE
                } else {
                    JNI_FALSE
                }
            }

            /// Run thread `tid` on the calling thread's CPU until it widens its
            /// own mask (`fastCores`); whether it was set.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_colocate(
                _env: *mut JNIEnv,
                _class: jclass,
                tid: jint,
            ) -> jboolean {
                if $crate::android_hint::colocate(tid) {
                    JNI_TRUE
                } else {
                    JNI_FALSE
                }
            }

            /// Thread `tid` on the fast cores only (`fast`), or anywhere again.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_fastThread(
                _env: *mut JNIEnv,
                _class: jclass,
                tid: jint,
                fast: jboolean,
            ) -> jboolean {
                flag($crate::android_hint::fast_thread(tid, fast != 0))
            }

            /// The scroller `scroll` moves (its group's id in the stream), or 0.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_feed(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jint {
                with(|h| h.feed().map_or(0, |v| v as jint)).unwrap_or(0)
            }

            /// System Back: whether the app took it.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_bench_exactcanvas_Native_back(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                flag(with(|h| h.back()).unwrap_or(false))
            }
        }
    };
}

use jni_sys::{jobject, JNIEnv};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU8, Ordering};

/// An app's allocator: mimalloc (2.x), which spends less per call than
/// Android's scudo (half of a cold boot's CPU, LLP 1076: heavy's cold start
/// 98 -> 69 ms on a Pixel 10 Pro XL); the reader sets
/// `MIMALLOC_PURGE_DELAY=0` before the library loads (mimalloc reads its
/// options from a constructor as it loads), so freed memory goes back at once
/// (decoded pictures otherwise stay held, ~110 MB more on heavy); the system's with
/// `EXACT_MALLOC=system`, to compare. Chosen once, at the first allocation, so
/// every block is freed by the allocator that made it. (mimalloc 3.x crashed
/// in a new thread's first allocation; 2.x ran clean.)
pub struct Allocator;

static CHOSEN: AtomicU8 = AtomicU8::new(0);

fn mimalloc_chosen() -> bool {
    match CHOSEN.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            // getenv does not allocate.
            let chosen_name = unsafe { libc_getenv(c"EXACT_MALLOC".as_ptr()) };
            let use_system = !chosen_name.is_null()
                && unsafe { std::ffi::CStr::from_ptr(chosen_name) }.to_bytes() == b"system";
            let chosen = if use_system { 2 } else { 1 };
            // Two threads' first allocations agree on one choice.
            match CHOSEN.compare_exchange(0, chosen, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => chosen == 1,
                Err(other) => other == 1,
            }
        }
    }
}

extern "C" {
    #[link_name = "getenv"]
    fn libc_getenv(name: *const std::ffi::c_char) -> *const std::ffi::c_char;
}

// SAFETY: each call goes to the one allocator chosen for the process.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if mimalloc_chosen() {
            mimalloc::MiMalloc.alloc(layout)
        } else {
            System.alloc(layout)
        }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if mimalloc_chosen() {
            mimalloc::MiMalloc.alloc_zeroed(layout)
        } else {
            System.alloc_zeroed(layout)
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if mimalloc_chosen() {
            mimalloc::MiMalloc.dealloc(ptr, layout)
        } else {
            System.dealloc(ptr, layout)
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if mimalloc_chosen() {
            mimalloc::MiMalloc.realloc(ptr, layout, new_size)
        } else {
            System.realloc(ptr, layout, new_size)
        }
    }
}
use std::ffi::c_void;

#[repr(C)]
struct BitmapInfo {
    width: u32,
    height: u32,
    stride: u32,
    format: i32,
    flags: u32,
}

#[repr(C)]
struct BufferDesc {
    width: u32,
    height: u32,
    layers: u32,
    format: u32,
    usage: u64,
    stride: u32,
    rfu0: u32,
    rfu1: u64,
}

#[link(name = "jnigraphics")]
extern "C" {
    fn AndroidBitmap_getInfo(env: *mut JNIEnv, bitmap: jobject, info: *mut BitmapInfo) -> i32;
    fn AndroidBitmap_lockPixels(env: *mut JNIEnv, bitmap: jobject, pixels: *mut *mut c_void)
        -> i32;
    fn AndroidBitmap_unlockPixels(env: *mut JNIEnv, bitmap: jobject) -> i32;
}

#[link(name = "android")]
extern "C" {
    fn ANativeWindow_fromSurface(env: *mut JNIEnv, surface: jobject) -> *mut c_void;
    fn ANativeWindow_release(window: *mut c_void);
    fn AHardwareBuffer_fromHardwareBuffer(env: *mut JNIEnv, buffer: jobject) -> *mut c_void;
    fn AHardwareBuffer_toHardwareBuffer(env: *mut JNIEnv, buffer: *mut c_void) -> jobject;
    fn AHardwareBuffer_release(buffer: *mut c_void);
    fn AHardwareBuffer_describe(buffer: *const c_void, desc: *mut BufferDesc);
    fn AHardwareBuffer_lock(
        buffer: *mut c_void,
        usage: u64,
        fence: i32,
        rect: *const c_void,
        out: *mut *mut c_void,
    ) -> i32;
    fn AHardwareBuffer_unlock(buffer: *mut c_void, fence: *mut i32) -> i32;
}

/// Copy `rows` rows of `row` bytes from `src` into `dst`, `stride` bytes apart.
///
/// # Safety
/// `dst` holds `rows` rows `stride` bytes apart, each at least `row` long.
unsafe fn copy_rows(src: &[u8], dst: *mut u8, row: usize, stride: usize, rows: usize) {
    if stride == row {
        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, row * rows);
    } else {
        for y in 0..rows {
            std::ptr::copy_nonoverlapping(src.as_ptr().add(y * row), dst.add(y * stride), row);
        }
    }
}

/// Picture `id` into a locked ARGB_8888 `bitmap` of its size.
///
/// # Safety
/// `env` and `bitmap` are the JNI call's.
pub unsafe fn copy_bitmap(env: *mut JNIEnv, id: u32, bitmap: jobject) -> bool {
    let Some(image) = super::image(id) else {
        return false;
    };
    let mut info = BitmapInfo {
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
        flags: 0,
    };
    if AndroidBitmap_getInfo(env, bitmap, &mut info) != 0
        || info.width != image.width()
        || info.height != image.height()
    {
        return false;
    }
    let mut dst = std::ptr::null_mut();
    if AndroidBitmap_lockPixels(env, bitmap, &mut dst) != 0 {
        return false;
    }
    let src: &[u8] = image.as_ref();
    let row = info.width as usize * 4;
    copy_rows(
        src,
        dst.cast(),
        row,
        info.stride as usize,
        info.height as usize,
    );
    AndroidBitmap_unlockPixels(env, bitmap);
    true
}

/// [`crate::surfaces::buffer_frame`] as one word for a reader.
pub fn canvas_frame(view: u32) -> i64 {
    match crate::surfaces::buffer_frame(view, false) {
        Ok(([generation, slot, serial], _)) => {
            (i64::from(generation & 0xff_ffff) << 40)
                | (i64::from(slot & 0xff) << 32)
                | i64::from(serial)
        }
        Err(false) => -1,
        Err(true) => -2,
    }
}

/// A buffered canvas's newest frame's buffer as a Java `HardwareBuffer`
/// (which holds its own reference), or null.
///
/// # Safety
/// `env` is the JNI call's.
pub unsafe fn canvas_buffer(env: *mut JNIEnv, view: u32) -> jobject {
    let Ok((_, buffer)) = crate::surfaces::buffer_frame(view, true) else {
        return std::ptr::null_mut();
    };
    let object = AHardwareBuffer_toHardwareBuffer(env, buffer as *mut c_void);
    AHardwareBuffer_release(buffer as *mut c_void);
    object
}

/// Picture `id`'s own GPU buffer as a Java `HardwareBuffer` (which holds its
/// own reference), taking the picture as fetched; null, and the picture left
/// to be copied, when its pixels are on the heap.
///
/// # Safety
/// `env` is the JNI call's.
pub unsafe fn image_buffer(env: *mut JNIEnv, id: u32) -> jobject {
    let mut pending = super::pending();
    let Some(super::Picture::Bitmap(bitmap)) = pending.get(&id) else {
        return std::ptr::null_mut();
    };
    let Some(buffer) = bitmap.hardware() else {
        return std::ptr::null_mut();
    };
    let object = AHardwareBuffer_toHardwareBuffer(env, buffer);
    if !object.is_null() {
        pending.remove(&id);
    }
    object
}

/// Picture `id` into the `HardwareBuffer` `buffer` (RGBA_8888, CPU-writable):
/// one copy, and no texture upload when the reader first draws it.
///
/// # Safety
/// `env` and `buffer` are the JNI call's.
pub unsafe fn copy_buffer(env: *mut JNIEnv, id: u32, buffer: jobject) -> bool {
    let Some(image) = super::image(id) else {
        return false;
    };
    let hb = AHardwareBuffer_fromHardwareBuffer(env, buffer);
    if hb.is_null() {
        return false;
    }
    let mut desc = BufferDesc {
        width: 0,
        height: 0,
        layers: 0,
        format: 0,
        usage: 0,
        stride: 0,
        rfu0: 0,
        rfu1: 0,
    };
    AHardwareBuffer_describe(hb, &mut desc);
    if desc.width != image.width() || desc.height != image.height() {
        return false;
    }
    let mut dst = std::ptr::null_mut();
    // AHARDWAREBUFFER_USAGE_CPU_WRITE_OFTEN
    if AHardwareBuffer_lock(hb, 3 << 4, -1, std::ptr::null(), &mut dst) != 0 {
        return false;
    }
    let src: &[u8] = image.as_ref();
    let row = desc.width as usize * 4;
    copy_rows(
        src,
        dst.cast(),
        row,
        desc.stride as usize * 4,
        desc.height as usize,
    );
    AHardwareBuffer_unlock(hb, std::ptr::null_mut());
    true
}

/// A `Surface`'s `ANativeWindow`, or null.
///
/// # Safety
/// `env` and `surface` are the JNI call's.
pub unsafe fn window_from_surface(env: *mut JNIEnv, surface: jobject) -> *mut c_void {
    ANativeWindow_fromSurface(env, surface)
}

/// Release a window [`window_from_surface`] gave (null: nothing).
///
/// # Safety
/// `window` came from [`window_from_surface`] and is released once.
pub unsafe fn release_window(window: *mut c_void) {
    if !window.is_null() {
        ANativeWindow_release(window);
    }
}
