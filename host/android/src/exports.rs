/// Export one app's Android C ABI. JNI owns the platform callback context and
/// decodes each returned direct buffer before making another operation.
#[macro_export]
macro_rules! host {
    ($data:ty, $plan:expr, $compat:expr) => {
        $crate::host!($data, $plan, $compat, $crate::General<$data>);
    };
    ($data:ty, $plan:expr, $compat:expr, $general:ty) => {
        #[cfg(target_os = "android")]
        #[global_allocator]
        static EXACT_ANDROID_ALLOCATOR: $crate::MiMalloc = $crate::MiMalloc;

        thread_local! {
            static EXACT_ANDROID: ::std::cell::RefCell<$crate::session::Registry<$data, $general>> =
                ::std::cell::RefCell::new($crate::session::Registry::default());
        }

        /// Create an owner-thread runtime; zero means handle space exhausted.
        #[no_mangle]
        pub extern "C" fn exact_android_create() -> u32 {
            EXACT_ANDROID.with(|r| r.borrow_mut().create())
        }
        /// Destroy a runtime and retire its wake callback before returning.
        #[no_mangle]
        pub extern "C" fn exact_android_destroy(rt: u32) {
            EXACT_ANDROID.with(|r| r.borrow_mut().destroy(rt));
        }
        /// Register platform paragraph measurement before boot.
        #[no_mangle]
        pub extern "C" fn exact_android_set_measure(
            rt: u32,
            measure: Option<$crate::measure::MeasureFn>,
            ctx: *mut ::std::ffi::c_void,
        ) {
            $crate::session::with_session(
                &EXACT_ANDROID,
                rt,
                |s| {
                    s.hooks.measure = measure;
                    s.hooks.ctx = ctx;
                },
                || (),
            );
        }
        /// Enable retained row rebinding only for a presenter supporting `renew`.
        /// Off by default; unsupported native state keeps fresh row mounts.
        #[no_mangle]
        pub extern "C" fn exact_android_set_row_reuse(rt: u32, on: u32) {
            $crate::session::with_session(
                &EXACT_ANDROID,
                rt,
                |s| s.bridge.set_row_reuse(on != 0),
                || (),
            );
        }
        /// Register asynchronous, coalesced executor wake notification.
        #[no_mangle]
        pub extern "C" fn exact_android_set_wake(
            rt: u32,
            wake: Option<$crate::executor::WakeFn>,
            ctx: *mut ::std::ffi::c_void,
        ) {
            $crate::session::with_session(
                &EXACT_ANDROID,
                rt,
                |s| {
                    s.hooks.wake = wake;
                    s.hooks.wake_ctx = ctx;
                },
                || (),
            );
        }
        /// Install declared plan fonts before the first text measurement.
        #[no_mangle]
        pub extern "C" fn exact_android_set_fonts(
            rt: u32,
            fonts: Option<$crate::measure::FontsFn>,
            ctx: *mut ::std::ffi::c_void,
        ) {
            $crate::session::with_session(
                &EXACT_ANDROID,
                rt,
                |s| s.bridge.set_fonts(fonts, ctx),
                || (),
            );
        }
        /// Resize the owned input buffer; valid until the next input resize.
        #[no_mangle]
        pub extern "C" fn exact_android_in(rt: u32, len: usize) -> *mut u8 {
            $crate::session::with_session(
                &EXACT_ANDROID,
                rt,
                |s| s.bridge.input(len),
                ::std::ptr::null_mut,
            )
        }
        /// Borrow the last output; consumed before another host operation.
        #[no_mangle]
        pub extern "C" fn exact_android_out(rt: u32) -> *const u8 {
            let entry = EXACT_ANDROID.with(|r| r.borrow().get(rt));
            entry
                .and_then(|e| e.try_borrow().ok().map(|s| s.output().as_ptr()))
                .unwrap_or_else($crate::session::refusal_ptr)
        }
        /// Boot the static baked plan in logical CSS pixels.
        #[no_mangle]
        pub extern "C" fn exact_android_boot(rt: u32, width: f32, height: f32) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, hooks| {
                b.set_compat($compat);
                b.boot_selected(
                    $plan,
                    || <$data as ::std::default::Default>::default(),
                    hooks,
                    width,
                    height,
                )
            })
        }
        /// Apply one authored initial press before the first layout and publication.
        #[no_mangle]
        pub extern "C" fn exact_android_boot_initial(
            rt: u32,
            width: f32,
            height: f32,
            len: usize,
        ) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, hooks| {
                b.set_compat($compat);
                b.boot_selected_initial(
                    $plan,
                    || <$data as ::std::default::Default>::default(),
                    hooks,
                    width,
                    height,
                    len,
                )
            })
        }
        /// Deliver one native event with an optional UTF-8 input payload.
        #[no_mangle]
        pub extern "C" fn exact_android_dispatch(
            rt: u32,
            view: u32,
            kind: u32,
            len: usize,
            now: f64,
        ) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| {
                b.dispatch(view, kind, len, now)
            })
        }
        /// Change the viewport; Rust computes layout once for the transaction.
        #[no_mangle]
        pub extern "C" fn exact_android_resize(rt: u32, width: f32, height: f32) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| b.resize(width, height))
        }
        /// Advance timers, motion and authored frame tasks at a display frame.
        #[no_mangle]
        pub extern "C" fn exact_android_frame(rt: u32, now: f64) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| b.frame(now))
        }
        /// Advance the controllable app clock through all due timers.
        #[no_mangle]
        pub extern "C" fn exact_android_advance(rt: u32, now: f64) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| b.advance(now, false))
        }
        /// Sample motion without running application timer work.
        #[no_mangle]
        pub extern "C" fn exact_android_tick(rt: u32, now: f64) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| b.tick(now))
        }
        /// Deliver the bounded native executor's queued completion.
        #[no_mangle]
        pub extern "C" fn exact_android_pump(rt: u32, now: f64) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| b.pump(now))
        }
        /// Activate deferred data only after the presenter has drawn first pixel.
        #[no_mangle]
        pub extern "C" fn exact_android_painted(rt: u32) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| b.data_ready())
        }
        /// Report platform display preferences by the shared native bit mask.
        #[no_mangle]
        pub extern "C" fn exact_android_preferences(rt: u32, bits: u32) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| b.set_preferences(bits))
        }
        /// Report logical safe-area and keyboard insets.
        #[no_mangle]
        pub extern "C" fn exact_android_insets(
            rt: u32,
            top: f32,
            right: f32,
            bottom: f32,
            left: f32,
        ) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| {
                b.insets(top, right, bottom, left)
            })
        }
        /// One replaced element's natural size; negative dimensions clear it.
        #[no_mangle]
        pub extern "C" fn exact_android_intrinsic(
            rt: u32,
            view: u32,
            width: f32,
            height: f32,
        ) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| {
                b.intrinsic(view, width, height)
            })
        }
        /// Several natural sizes in one input payload and one layout pass.
        #[no_mangle]
        pub extern "C" fn exact_android_intrinsics(rt: u32, len: usize) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| b.intrinsics(len))
        }
        /// Query native control contents; JSON consumed before the next call.
        #[no_mangle]
        pub extern "C" fn exact_android_control_query(rt: u32, view: u32, kind: u32) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, true, |b, _| {
                b.control_query(view, kind)
            })
        }
        /// Record native scroll metadata; no output buffer or publication changes.
        #[no_mangle]
        pub extern "C" fn exact_android_scrolled(rt: u32, view: u32, left: f64, top: f64) -> u32 {
            $crate::session::with_session(
                &EXACT_ANDROID,
                rt,
                |s| u32::from(s.bridge.scrolled(view, left, top)),
                || 0,
            )
        }
        /// Report a mounted viewport using the shared little-endian v3 protocol.
        #[no_mangle]
        pub extern "C" fn exact_android_collection_feedback(rt: u32, len: usize, now: f64) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, false, |b, _| {
                b.collection_feedback(len, now)
            })
        }
        /// Query the shared agent API; output is UTF-8 JSON instead of EXA1.
        #[no_mangle]
        pub extern "C" fn exact_android_agent(rt: u32, len: usize) -> u32 {
            $crate::session::transaction(&EXACT_ANDROID, rt, true, |b, _| b.agent(len))
        }
    };
}
