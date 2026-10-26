//! The Canvas host's JNI, once for every app (LLP 1076): an app's library is
//! its plan, its data source and `exact_linux::canvas_jni!(Data)`, which
//! defines `dev.exact.canvas.Native`'s methods (the reader's class,
//! `host/android/canvas`) over a [`super::CanvasHost`] that lives on the
//! thread that called `start`.
//!
//! The reader may call `start` from a thread of its own as soon as the
//! process starts (at the size it expects the view to be, `resize` if it
//! is not), then make every other call on that thread: the host boots while
//! Android sets the activity up.
// FFI to the JNI, jnigraphics and AHardwareBuffer APIs.
#![allow(unsafe_code)]

#[path = "jni_bytes.rs"]
mod byte_arrays;
pub use byte_arrays::{bytes, utf8};

#[path = "jni_extensions.rs"]
pub mod jni_extensions;
#[path = "jni_keys.rs"]
mod jni_keys;
#[path = "jni_picture.rs"]
pub mod jni_picture;
#[path = "jni_pad.rs"]
mod pad;

/// Define the JNI for an app over `CanvasHost<$data>` with the plan and
/// compat the app's build script writes to `OUT_DIR` (`app.plan`,
/// `compat.json`). `hatches = …` is the app's [`crate::hatches::Install`]
/// when it has hatches (`hatches::install_with_reader`, so the reader's
/// Kotlin ones hear them too).
#[macro_export]
macro_rules! canvas_jni {
    ($data:ty) => {
        $crate::canvas_jni!($data, hatches = None);
    };
    ($data:ty, hatches = $hatches:expr) => {
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

            fn pipeline_cache() {
                use std::hash::{Hash, Hasher};
                let mut key = std::collections::hash_map::DefaultHasher::new();
                // The baked plan includes the GPU artifact’s content digest.
                PLAN.hash(&mut key);
                std::env::set_var("EXACT_GPU_CACHE_KEY", format!("{:016x}", key.finish()));
            }

            type Host = $crate::canvas::CanvasHost<$data>;

            fn hatches() -> Option<$crate::hatches::Install> {
                $hatches
            }

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

            /// A Java string's UTF-16, as Rust's: JNI's UTF-8 functions give
            /// modified UTF-8, which writes a character beyond the first plane
            /// as two surrogates and a NUL as two bytes.
            unsafe fn string(env: *mut JNIEnv, s: jstring) -> String {
                let get = (**env).GetStringChars.expect("jni");
                let release = (**env).ReleaseStringChars.expect("jni");
                let length = (**env).GetStringLength.expect("jni")(env, s);
                let p = get(env, s, std::ptr::null_mut());
                let units = std::slice::from_raw_parts(p, length.max(0) as usize);
                let out = String::from_utf16_lossy(units);
                release(env, s, p);
                out
            }

            $crate::canvas_jni_extensions!();
            $crate::canvas_jni_keys!();

            fn flag(b: bool) -> jboolean {
                if b {
                    JNI_TRUE
                } else {
                    JNI_FALSE
                }
            }

            /// Whether a host runs in this process: a reader that starts
            /// again adopts it (LLP 1115.001 D15.1) and boots no second one.
            static RUNNING: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            /// The size and scale a reader asked `startAsync` for while a
            /// host ran: what `adopt` adopts it at.
            static ADOPT: std::sync::Mutex<Option<(u32, u32, f32)>> = std::sync::Mutex::new(None);

            /// The host on this thread, adopted by a new reader at `size`
            /// and `scale` (`CanvasHost::adopt`): what this thread kept of
            /// the last reader's streams goes with it. False when no host
            /// runs here.
            fn adopt_running(size: (u32, u32), scale: f32) -> bool {
                if with(|h| h.adopt(size, scale)).is_none() {
                    return false;
                }
                LAST.with(|l| l.borrow_mut().clear());
                MOVE.with(|m| m.borrow_mut().clear());
                DRAWN.with(|d| *d.borrow_mut() = None);
                PRIMED.with(|p| *p.borrow_mut() = None);
                true
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_start(
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
                // The activity was made again in this process: its reader
                // adopts the host that runs. Nothing boots, and the
                // environment, which other threads read by now, is left alone.
                if adopt_running((width as u32, height as u32), scale) {
                    return JNI_TRUE;
                }
                // The reader hosts the app's text fields (LLP 1115.001 D8).
                $crate::canvas::field::attach(env, scale);
                $crate::canvas::legacy::attach(env);
                $crate::canvas::presence::attach(env);
                let dir = string(env, dir);
                pipeline_cache();
                std::env::set_var("HOME", &dir);
                std::env::set_var("XDG_DATA_HOME", format!("{dir}/data"));
                std::env::set_var("EXACT_ASSETS", format!("{dir}/app"));
                $crate::app::show_material_look();
                std::env::set_var("EXACT_FONTS", "/system/fonts");
                std::env::set_var("EXACT_FONT", "Roboto");
                std::env::set_var("BENCH_LIVE", if live == JNI_TRUE { "1" } else { "0" });
                match Host::boot(PLAN, COMPAT, (width as u32, height as u32), scale, hatches()) {
                    Ok(h) => {
                        HOST.with(|slot| *slot.borrow_mut() = Some(h));
                        RUNNING.store(true, std::sync::atomic::Ordering::Release);
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_startAsync(
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
                // A host runs already (the activity was made again): nothing
                // boots, and `adopt` adopts that one at this size.
                if RUNNING.load(std::sync::atomic::Ordering::Acquire) {
                    *ADOPT.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some((width as u32, height as u32, scale));
                    return;
                }
                $crate::canvas::field::attach(env, scale);
                $crate::canvas::legacy::attach(env);
                $crate::canvas::presence::attach(env);
                // The environment is set here, before any other thread reads it.
                let dir = string(env, dir);
                pipeline_cache();
                std::env::set_var("HOME", &dir);
                std::env::set_var("XDG_DATA_HOME", format!("{dir}/data"));
                std::env::set_var("EXACT_ASSETS", format!("{dir}/app"));
                $crate::app::show_material_look();
                std::env::set_var("EXACT_FONTS", "/system/fonts");
                std::env::set_var("EXACT_FONT", "Roboto");
                std::env::set_var("BENCH_LIVE", if live == JNI_TRUE { "1" } else { "0" });
                std::thread::Builder::new()
                    .name("exact-boot".into())
                    .spawn(move || {
                        let booted = $crate::android::trace(c"exact boot", || {
                            Host::boot(PLAN, COMPAT, (width as u32, height as u32), scale, hatches()).map(
                                |mut h| {
                                    h.set_borrowed(true);
                                    let first = $crate::canvas::agent::after_turn(h.frame());
                                    h.roles_here(false);
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_adopt(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                let again = ADOPT.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some((width, height, scale)) = again {
                    return flag(adopt_running((width, height), scale));
                }
                let mut slot = BOOTED.lock().unwrap_or_else(|e| e.into_inner());
                while slot.is_none() {
                    slot = READY.wait(slot).unwrap_or_else(|e| e.into_inner());
                }
                match slot.take().expect("booted") {
                    Ok(Booted(mut h, first)) => {
                        h.set_borrowed(false);
                        h.roles_here(true);
                        HOST.with(|s| *s.borrow_mut() = Some(h));
                        RUNNING.store(true, std::sync::atomic::Ordering::Release);
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

            /// The device's settings, whole (LLP 1115.001 D3.1): before
            /// `start`, from any thread, and on the host's thread again
            /// whenever one changes. A host that runs on the calling thread
            /// takes them now; one that boots later starts with them.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_configure(
                env: *mut JNIEnv,
                _class: jclass,
                night: jboolean,
                font_scale: jfloat,
                locale: jstring,
                rtl: jboolean,
                hour24: jboolean,
                no_animations: jboolean,
                bold_text: jboolean,
                high_contrast: jboolean,
                touch: jboolean,
                tv: jboolean,
                touch_slop: jfloat,
                dates: jstring,
            ) {
                let device = $crate::presenter::Device {
                    night: night == JNI_TRUE,
                    font_scale,
                    locale: string(env, locale),
                    rtl: rtl == JNI_TRUE,
                    hour24: hour24 == JNI_TRUE,
                    reduced_motion: no_animations == JNI_TRUE,
                    bold_text: bold_text == JNI_TRUE,
                    high_contrast: high_contrast == JNI_TRUE,
                    touch: touch == JNI_TRUE,
                    tv: tv == JNI_TRUE,
                    touch_slop,
                    dates: string(env, dates),
                };
                $crate::canvas::configure(&device);
                // What a hosted field measured was under the old settings.
                $crate::canvas::field::forget();
                with(|h| h.configure(&device));
            }

            /// The window (LLP 1115.001 D3.3): its size in pixels, the
            /// display's pixels a point, and sixteen raw insets in pixels,
            /// four (top, right, bottom, left) each for the system bars,
            /// the display cutout, the software keyboard and the system's
            /// gestures. Before `start`, from any thread, so the host boots
            /// laid out in it, and on the host's thread again whenever any
            /// of it changes. The reader pads nothing itself: `chrome` says
            /// where the page is.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_window(
                env: *mut JNIEnv,
                _class: jclass,
                width: jint,
                height: jint,
                scale: jfloat,
                insets: jintArray,
            ) {
                let mut told = $crate::canvas::Told {
                    size: (width.max(0) as u32, height.max(0) as u32),
                    scale,
                    insets: [0; 16],
                };
                let n = ((**env).GetArrayLength.expect("jni"))(env, insets).clamp(0, 16);
                ((**env).GetIntArrayRegion.expect("jni"))(
                    env,
                    insets,
                    0,
                    n,
                    told.insets.as_mut_ptr(),
                );
                $crate::canvas::window(told);
                with(|h| h.set_window(told));
            }

            /// The device's colours (LLP 1115.001 D4): Material 3's colour
            /// roles as the reader resolved them, twelve light and then the
            /// same twelve dark, ARGB, in `presenter::Material`'s order.
            /// Before `start`, from any thread, so the host boots resolving
            /// every role by them, and on the host's thread again whenever
            /// one changes. Any other count of colours is ignored.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_colors(
                env: *mut JNIEnv,
                _class: jclass,
                roles: jintArray,
            ) {
                let n = ((**env).GetArrayLength.expect("jni"))(env, roles).max(0);
                let mut words = vec![0i32; n as usize];
                ((**env).GetIntArrayRegion.expect("jni"))(env, roles, 0, n, words.as_mut_ptr());
                let Some(palette) = $crate::presenter::Palette::from_words(&words) else {
                    return;
                };
                $crate::canvas::colors(palette);
                with(|h| h.set_colors(&palette));
            }

            /// What the presenter asks of the platform since the reader
            /// last took it (LLP 1115.001 D5), as words, or null when it
            /// asks nothing: for each request its kind, its id, the count
            /// of words that follow and those words
            /// (`presenter/requests.rs`). The presenter wakes the reader
            /// (the first of `fds`) when it queues one.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_requests(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jintArray {
                match with(|h| h.requests()).flatten() {
                    Some(words) => ints(env, &words),
                    None => std::ptr::null_mut(),
                }
            }

            /// The reader's answer to request `id`, or null for its
            /// cancellation.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_answer(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                answer: jstring,
            ) {
                let answer = (!answer.is_null()).then(|| string(env, answer));
                with(|h| h.answer(id as u32, answer.as_deref()));
            }

            /// What the reader draws around the page, when that is not what
            /// this last returned (else null): where the page is in the
            /// window (x, y, width, height, pixels), the colour behind it
            /// (ARGB), and 1 when its scheme is dark.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_chrome(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jintArray {
                let Some(chrome) = with(|h| h.chrome()).flatten() else {
                    return std::ptr::null_mut();
                };
                let arr = ((**env).NewIntArray.expect("jni"))(env, 6);
                ((**env).SetIntArrayRegion.expect("jni"))(env, arr, 0, 6, chrome.as_ptr());
                arr
            }

            /// A new op stream as a direct ByteBuffer over Rust memory (valid until
            /// the next call), or null when nothing changed.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_frame(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jobject {
                // The first frame the booting thread painted; then what a driver's
                // request painted and the reader has not taken (`canvas/agent.rs`);
                // then the host's own turn.
                let primed = PRIMED.with(|p| p.borrow_mut().take());
                let waiting = primed.or_else($crate::canvas::agent::next_stream);
                // The host's own turn is the frame a scrub frame's trace waits
                // for (EXACT_SCRUB_TRACE), its stream or none (a view of the
                // app's own took it), never a stream queued before.
                let own = waiting.is_none();
                let ops = waiting.or_else(|| {
                    $crate::canvas::agent::after_turn(with(|h| h.frame()).flatten())
                });
                if own {
                    $crate::presenter::preferences::scrub_frame_drawn();
                }
                let Some(ops) = ops else {
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_refine(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_owed(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                flag(with(|h| h.owed()).unwrap_or(false))
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_animating(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                // A stream a driver's request left is owed a turn to be taken in.
                let waiting = $crate::canvas::agent::waiting();
                flag(waiting || with(|h| h.animating()).unwrap_or(false))
            }

            /// One request of the agent API (LLP 1012; `canvas/agent.rs`), a
            /// JSON line as UTF-8, and its answer the same way: the Linux
            /// host's own handler, on the host's thread. A driver's carrier
            /// calls it (`scripts/agent.mjs android --reader`).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_agent(
                env: *mut JNIEnv,
                _class: jclass,
                line: $crate::canvas::jni_sys::jbyteArray,
            ) -> $crate::canvas::jni_sys::jbyteArray {
                let line = $crate::canvas::jni::utf8(env, line);
                let reply = with(|h| h.agent(&line));
                let reply = reply.unwrap_or_else(|| r#"{"error":"no host on this thread"}"#.into());
                $crate::canvas::jni::bytes(env, reply.as_bytes())
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_nextDue(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jdouble {
                with(|h| h.next_due()).flatten().unwrap_or(-1.0)
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fds(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jintArray {
                let mut fds = with(|h| h.fds()).unwrap_or([-1, -1]).to_vec();
                fds.push(with(|h| h.acquire_fd()).unwrap_or(-1));
                let arr = ((**env).NewIntArray.expect("jni"))(env, fds.len() as i32);
                ((**env).SetIntArrayRegion.expect("jni"))(env, arr, 0, fds.len() as i32, fds.as_ptr());
                arr
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_scroll(
                _env: *mut JNIEnv,
                _class: jclass,
                dy: jfloat,
            ) {
                with(|h| h.scroll(dy));
            }

            fn ints(env: *mut JNIEnv, words: &[i32]) -> jintArray {
                // SAFETY: `env` is the JNI call's; the region is `words`.
                unsafe {
                    let arr = ((**env).NewIntArray.expect("jni"))(env, words.len() as i32);
                    ((**env).SetIntArrayRegion.expect("jni"))(
                        env,
                        arr,
                        0,
                        words.len() as i32,
                        words.as_ptr(),
                    );
                    arr
                }
            }

            /// What is under a point of the view (pixels), for the reader
            /// deciding what a drag from there is (LLP 1115.001 D7.3) and
            /// whose a touch there is (D8): the presenter's own claims, a
            /// count, the hosted text field the touch is for, and eleven
            /// words a scroller, innermost first
            /// (`Presenter::reader_under`).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_under(
                env: *mut JNIEnv,
                _class: jclass,
                x: jfloat,
                y: jfloat,
            ) -> jintArray {
                let words = with(|h| h.under(x, y)).unwrap_or_else(|| vec![0, 0, 0]);
                ints(env, &words)
            }

            /// What the reader is told of the text fields it hosts (LLP
            /// 1115.001 D8), when that is not what this last returned
            /// (else null): `Presenter::reader_fields` has the words.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fields(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jintArray {
                match with(|h| h.fields()).flatten() {
                    Some(words) => ints(env, &words),
                    None => std::ptr::null_mut(),
                }
            }

            /// The reader's field `id`, known at `generation`, took the
            /// focus or lost it.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fieldFocused(
                _env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                generation: jint,
                focused: jboolean,
            ) {
                with(|h| h.field_focused(id as u32, generation as u32, focused == JNI_TRUE));
            }

            /// The reader's field `id`, known at `generation`, had real input
            /// (a finger down, a key down).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fieldInput(
                _env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                generation: jint,
            ) {
                with(|h| h.field_input(id as u32, generation as u32));
            }

            /// The reader's field `id` holds `value` after its edit number
            /// `seq`, with its selection at `start` to `end` (UTF-16
            /// units): the person's edit, composing text included.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fieldEdited(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                generation: jint,
                seq: jint,
                value: jstring,
                start: jint,
                end: jint,
            ) {
                let value = $crate::canvas::field::text(env, value);
                let (edit, selection) = (
                    (generation as u32, seq as u32),
                    (start.max(0) as u32, end.max(0) as u32),
                );
                with(|h| h.field_edited(id as u32, edit, &value, selection));
            }

            /// The reader's field `id` began to compose, holding `value`,
            /// or its composition ended: told before the edit that began
            /// or ended it (`Presenter::reader_field_composing`).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fieldComposing(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                generation: jint,
                composing: jboolean,
                value: jstring,
            ) {
                let value = $crate::canvas::field::text(env, value);
                with(|h| {
                    h.field_composing(id as u32, generation as u32, composing == JNI_TRUE, &value)
                });
            }

            /// The reader's field `id` moved its selection with no edit.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fieldSelected(
                _env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                generation: jint,
                start: jint,
                end: jint,
            ) {
                let selection = (start.max(0) as u32, end.max(0) as u32);
                with(|h| h.field_selected(id as u32, generation as u32, selection));
            }

            /// The reader's field `id` had its action key pressed, or
            /// Enter or Tab on a hardware keyboard: `action` is what the
            /// key said it does (`presenter::field_action`).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fieldAction(
                _env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                generation: jint,
                action: jint,
            ) {
                with(|h| h.field_action(id as u32, generation as u32, action));
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_moduleEvent(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                slot: jint,
                generation: jint,
                kind: jstring,
                payload: jstring,
            ) {
                let kind = string(env, kind);
                let payload = string(env, payload);
                let key = $crate::canvas::ModuleKey {
                    index: slot as u32,
                    generation: generation as u32,
                };
                with(|h| h.module_event(id as u32, key, &kind, payload));
            }

            /// The reader moved scroller `id` by (`dx`, `dy`) pixels,
            /// knowing revision `rev` of it.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_scrolled(
                _env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                dx: jfloat,
                dy: jfloat,
                rev: jint,
            ) {
                with(|h| h.scrolled(id as u32, dx, dy, rev as u32));
            }

            /// What the reader is told of the scrollers it knows, nine
            /// words each (`Presenter::reader_scrollers`), when that is not
            /// what this last returned (else null).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_scrollers(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jintArray {
                match with(|h| h.scrollers()).flatten() {
                    Some(words) => ints(env, &words),
                    None => std::ptr::null_mut(),
                }
            }

            /// A touch (0 down, 1 up, 2 move, 3 cancel) at the view's
            /// pixels, and what touched: Android's tool type (1 a finger,
            /// 2 a stylus, 3 a mouse, 4 an eraser; LLP 1115.001 D7.4).
            /// Action 4: the touch that is down was held for the system's
            /// long-press time (a context menu opens, D10).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_touch(
                _env: *mut JNIEnv,
                _class: jclass,
                action: jint,
                x: jfloat,
                y: jfloat,
                tool: jint,
            ) {
                with(|h| h.touch(action, x, y, tool));
            }

            /// What happened in the window the reader made for surface
            /// `serial` of the top layer (LLP 1115.001 D10;
            /// `presenter::top::event`): a touch (0 down, 1 up, 2 move, 3
            /// cancel) at (`x`, `y`), pixels from the window's top left;
            /// the system dismissed the window (8: a tap outside a menu,
            /// Back, Escape); the reader could not make it (9); a key (16
            /// Enter, 17 Tab, 18 Shift+Tab, 19 to 22 the arrows, 23 Space).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_surface(
                _env: *mut JNIEnv,
                _class: jclass,
                serial: jint,
                what: jint,
                x: jfloat,
                y: jfloat,
            ) {
                with(|h| h.surface(serial as u32, what, x, y));
            }

            /// What the reader is told of the app's bars (LLP 1115.001
            /// D11), when that is not what this last returned (else
            /// null): `Presenter::reader_bars` has the words.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_bars(
                env: *mut JNIEnv,
                _class: jclass,
            ) -> jintArray {
                match with(|h| h.bars()).flatten() {
                    Some(words) => ints(env, &words),
                    None => std::ptr::null_mut(),
                }
            }

            /// What happened in bar `which` (1 the top app bar, 2 the
            /// navigation bar): a touch (0 down, 1 up, 2 move, 3 cancel)
            /// at (`x`, `y`), pixels from the bar's own top left.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_bar(
                _env: *mut JNIEnv,
                _class: jclass,
                which: jint,
                what: jint,
                x: jfloat,
                y: jfloat,
            ) {
                with(|h| h.bar(which, what, x, y));
            }

            /// What the reader is told of the accessibility tree (LLP
            /// 1115.001 D13), when that is not what this last returned
            /// (else null): `Presenter::reader_access` has the words. The
            /// reader calls it only while an accessibility service is on;
            /// `again` when it holds nothing of the tree.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_access(
                env: *mut JNIEnv,
                _class: jclass,
                again: jboolean,
            ) -> jintArray {
                match with(|h| h.access(again != JNI_FALSE)).flatten() {
                    Some(words) => ints(env, &words),
                    None => std::ptr::null_mut(),
                }
            }

            /// An accessibility service acts on node `id` of the tree last
            /// told (`presenter::access::act`): 1 press, 2 hold, 3 scroll
            /// on or step up, 4 scroll back or step down, 5 set a range to
            /// `value`. Whether anything took it.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_accessAct(
                _env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                action: jint,
                value: jfloat,
            ) -> jboolean {
                flag(with(|h| h.access_act(id, action, value)).unwrap_or(false))
            }

            /// What the reader's hatches ask for node `node`, 0 for their
            /// module (LLP 1115.001.000 D4; `hatches::reader::act` names
            /// each kind): its answer, a ticket for a registration, -1
            /// when refused.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_hatchAct(
                env: *mut JNIEnv,
                _class: jclass,
                node: jint,
                kind: jint,
                text: jstring,
            ) -> jlong {
                let text = (!text.is_null()).then(|| string(env, text));
                with(|h| h.hatch_act(node as u32, kind, text.as_deref())).unwrap_or(-1)
            }

            /// No accessibility service is on any more: the presenter
            /// forgets the tree it told and builds none.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_accessOff(
                _env: *mut JNIEnv,
                _class: jclass,
            ) {
                with(|h| h.access_off());
            }

            /// One slice of laying out what the touch that is down would
            /// open (`CanvasHost::ahead`); whether more is left.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_ahead(
                _env: *mut JNIEnv,
                _class: jclass,
                budget_us: jint,
            ) -> jboolean {
                flag(with(|h| h.ahead(budget_us.max(0) as u32)).unwrap_or(false))
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_acquired(
                _env: *mut JNIEnv, _class: jclass, running: jboolean,
            ) {
                with(|h| h.acquired(running != JNI_FALSE));
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_displayFrame(
                _env: *mut JNIEnv, _class: jclass,
            ) {
                with(|h| h.display_frame());
                // A scrub frame's trace waits for the frame this reader draws.
                $crate::presenter::preferences::scrub_frames_said();
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_poll(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jboolean {
                flag(with(|h| h.poll()).unwrap_or(false))
            }

            /// Picture `id` (IMAGE_DEF) into an ARGB_8888 bitmap of its size.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_copyImage(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
                bitmap: jobject,
            ) -> jboolean {
                flag($crate::canvas::jni::copy_bitmap(env, id as u32, bitmap))
            }

            /// A glyph by its id below API 31 (`LegacyGlyphs`, `canvas/legacy.rs`).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_LegacyGlyphs_glyph(
                env: *mut JNIEnv,
                _class: jclass,
                font: jobject,
                index: jint,
                axes: $crate::canvas::jni_sys::jintArray,
                id: jint,
            ) -> $crate::canvas::jni_sys::jintArray {
                $crate::canvas::legacy::glyph(env, font, index, axes, id)
            }

            /// Picture `id` into a GPU buffer the reader wraps as a hardware bitmap.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_copyImageToBuffer(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_hardwarePictures(
                _env: *mut JNIEnv,
                _class: jclass,
                on: jboolean,
            ) {
                $crate::image::hardware_pictures(on == JNI_TRUE);
            }

            /// Picture `id`'s own GPU buffer as a `HardwareBuffer` to wrap, or
            /// null when its pixels are on the heap (copy them instead).
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_imageBuffer(
                env: *mut JNIEnv,
                _class: jclass,
                id: jint,
            ) -> jobject {
                $crate::canvas::jni::image_buffer(env, id as u32)
            }

            /// A GPU canvas's window (op 28, kind 3); the window to pass back to
            /// `detachSurface`, or 0.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_attachSurface(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_attachBuffers(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_canvasFrame(
                _env: *mut JNIEnv,
                _class: jclass,
                view: jint,
            ) -> jlong {
                $crate::canvas::jni::canvas_frame(view as u32)
            }

            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_canvasReady(
                _env: *mut JNIEnv, _class: jclass, view: jint,
            ) -> jint {
                $crate::canvas::jni::window_ready(view as u32)
            }

            /// That frame's buffer as a `HardwareBuffer` to wrap, or null.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_canvasBuffer(
                env: *mut JNIEnv,
                _class: jclass,
                view: jint,
            ) -> jobject {
                $crate::canvas::jni::canvas_buffer(env, view as u32)
            }

            /// The canvas's window is going: no more presenting into it.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_detachSurface(
                _env: *mut JNIEnv,
                _class: jclass,
                view: jint,
                window: jlong,
            ) {
                with(|h| h.detach_window(view as u32));
                $crate::canvas::jni::release_window(window as *mut c_void);
            }

            $crate::canvas_jni_pad!();

            /// Pictures announced and not yet fetched, as id, width, height triples:
            /// a reader on its own thread makes them before the stream that draws
            /// them reaches its main thread.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_pendingPictures(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fastCores(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_hintSession(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_inputHint(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_colocate(
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
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_fastThread(
                _env: *mut JNIEnv,
                _class: jclass,
                tid: jint,
                fast: jboolean,
            ) -> jboolean {
                flag($crate::android_hint::fast_thread(tid, fast != 0))
            }

            /// The feed: the scroller a finger last moved up or down, or a
            /// driver's `scroll` found (its group's id in the stream), or 0.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_feed(
                _env: *mut JNIEnv,
                _class: jclass,
            ) -> jint {
                with(|h| h.feed().map_or(0, |v| v as jint)).unwrap_or(0)
            }

            /// What `ExactTamber(view)` asks (LLP 1121 D12), a JSON request,
            /// and its answer (`Host::tamber_json`, `host/linux/src/tamber.rs`),
            /// on the host's thread; both by UTF-16, so a name crosses whole.
            #[no_mangle]
            pub unsafe extern "system" fn Java_dev_exact_canvas_Native_tamber(
                env: *mut JNIEnv,
                _class: jclass,
                request: jstring,
            ) -> jstring {
                let request = $crate::canvas::field::text(env, request);
                let reply = with(|h| h.tamber_json(&request));
                let reply = reply.unwrap_or_else(|| r#"{"error":"no host on this thread"}"#.into());
                $crate::canvas::field::java(env, &reply)
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
/// The system allocator (scudo: the reader's nodes and display lists, not
/// this library's own blocks) gives its freed pages back to the system:
/// bionic's `mallopt(M_PURGE, 0)`.
pub(crate) fn purge() {
    const M_PURGE: i32 = -101;
    extern "C" {
        fn mallopt(param: i32, value: i32) -> i32;
    }
    // SAFETY: a libc call with no pointers; it frees nothing in use.
    unsafe {
        mallopt(M_PURGE, 0);
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

/// Window readiness published by the presenter, read without running a frame.
pub fn window_ready(view: u32) -> i32 {
    crate::surfaces::window_ready(view)
}
