/// Instantiate the C exports for one app (see `include/exact.h`).
///
/// `$data` is the app's `DataSource` type (constructed with `Default`, or
/// the sixth argument's factory for a deferred bytecode module);
/// `$plan` a `&'static [u8]` of baked plan bytes. Every export takes the
/// runtime handle `exact_create` returned (LLP 1031 D2).
#[macro_export]
macro_rules! host {
    ($data:ty, $plan:expr, $compat:expr) => {
        $crate::host!($data, $plan, $compat, None, ::std::ptr::null());
    };
    ($data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr) => {
        $crate::host!($data, $plan, $compat, $delivery, $api, || <$data as ::std::default::Default>::default());
    };
    ($data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr, $new:expr) => {
        $crate::host!($data, $plan, $compat, $delivery, $api, $new, None);
    };
    ($data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr, $new:expr, $region:expr) => {
        $crate::raster_exports!();
        $crate::textflow_exports!();
        $crate::markup_exports!();
        $crate::collapse_exports!();
        thread_local! {
            static EXACT_RUNTIMES: ::std::cell::RefCell<$crate::abi::Registry<$data>> = ::std::cell::RefCell::new($crate::abi::Registry::default());
        }

        /// Create a runtime; returns its handle (never 0). Its callbacks are
        /// set with `exact_set_measure`, `exact_set_wake`, and
        /// `exact_set_fonts` before its first boot.
        #[no_mangle]
        pub extern "C" fn exact_create() -> u32 {
            let id = EXACT_RUNTIMES.with(|r| r.borrow_mut().create());
            $crate::abi::with_entry(&EXACT_RUNTIMES, id, |e| e.bridge.set_content_region($region));
            id
        }

        /// The text measurer for a runtime (LLP 1008 §3); `None` is the
        /// monospace reference measurer.
        #[no_mangle]
        pub extern "C" fn exact_set_measure(
            rt: u32,
            measure: ::std::option::Option<$crate::measure::MeasureFn>,
            ctx: *mut ::std::ffi::c_void,
        ) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| { e.hooks.measure = measure; e.hooks.ctx = ctx; });
        }

        /// The wake for a request's reply (LLP 1016 D2), called on the
        /// executor's thread with `ctx`; `None` and replies wait for the next
        /// `exact_pump`.
        #[no_mangle]
        pub extern "C" fn exact_set_wake(
            rt: u32,
            wake: ::std::option::Option<$crate::executor::WakeFn>,
            ctx: *mut ::std::ffi::c_void,
        ) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| { e.hooks.wake = wake; e.hooks.wake_ctx = ctx; });
        }

        /// The plan-font hook, called synchronously by each boot on this
        /// runtime before its first text measurement, with `ctx`.
        #[no_mangle]
        pub extern "C" fn exact_set_fonts(
            rt: u32,
            fonts: ::std::option::Option<$crate::measure::FontsFn>,
            ctx: *mut ::std::ffi::c_void,
        ) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| e.bridge.set_fonts(fonts, ctx));
        }

        /// Destroy a runtime: everything attributable to it goes; a late
        /// call on its handle is refused. Idempotent.
        #[no_mangle]
        pub extern "C" fn exact_destroy(rt: u32) {
            EXACT_RUNTIMES.with(|r| r.borrow_mut().destroy(rt));
        }

        /// Resize the input buffer; returns its address (null: no such runtime).
        #[no_mangle]
        pub extern "C" fn exact_in(rt: u32, len: usize) -> *mut u8 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.input(len), |_| ::std::ptr::null_mut())
        }

        /// The output buffer's address — the last batch or reply on this
        /// runtime, or the refusal when the last call reached no runtime.
        #[no_mangle]
        pub extern "C" fn exact_out(rt: u32) -> *const u8 {
            let entry = EXACT_RUNTIMES.with(|r| r.borrow().get(rt));
            match entry.and_then(|e| e.try_borrow().ok().map(|e| e.bridge.output())) {
                Some(p) => p,
                None => $crate::abi::refusal_ptr(),
            }
        }

        /// The immutable compatibility and bundle receipt baked into this archive.

        #[no_mangle]
        pub extern "C" fn exact_baked_compat(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.baked_compat($compat), |n| n)
        }

        /// Derive the location of the input URL; UTF-8 output, no boot required.
        #[no_mangle]
        pub extern "C" fn exact_location_of(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, true, |b, _| b.location_of(len), |n| n)
        }

        /// Supply the launch location before the first boot. @ref LLP 1038 D5/D8
        #[no_mangle]
        pub extern "C" fn exact_set_launch_location(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| { b.set_launch_location(len); 0 }, |n| n)
        }

        /// Boot the selected plan — the update store's entry when one is
        /// selected (LLP 1026 D9), else the baked one; returns the first
        /// batch's length.
        #[no_mangle]
        pub extern "C" fn exact_boot(rt: u32, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| {
                b.set_compat($compat);
                if let Some(refusal) = b.refuse_analysis() { return refusal; }
                b.set_delivery($delivery);
                b.boot_selected($plan, $new, hooks, width, height)
            }, |n| n)
        }

        /// The linked delivery adapter, null in a binary-only app.
        #[no_mangle]
        pub extern "C" fn exact_delivery_api() -> *const $crate::delivery::Api { $api }


        /// Refresh this session's delivery facts; the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_delivery_sync(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.sync_delivery(), |n| n)
        }

        /// The executor's queued replies into the runner (LLP 1016 D2), on
        /// this thread, after a wake; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_pump(rt: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.pump(now_ms), |n| n)
        }

        /// Whether a presenter-owned operation may still affect its surface.
        #[no_mangle]
        pub extern "C" fn exact_request_active(rt: u32, ticket: u64) -> u8 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| u8::from(b.request_active(ticket)), |_| 0)
        }

        /// Complete presenter-owned surface work; the input is bytes or a failure message.
        #[no_mangle]
        pub extern "C" fn exact_fulfill_surface(rt: u32, ticket: u64, kind: u32, len: usize, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.fulfill_surface(ticket, kind, len, now_ms), |n| n)
        }

        /// Boot from plan bytes in the input buffer; returns the first batch's length.
        #[no_mangle]
        pub extern "C" fn exact_boot_plan(rt: u32, len: usize, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| {
                b.set_compat($compat);
                if let Some(refusal) = b.refuse_analysis() { return refusal; }
                b.set_delivery($delivery);
                b.boot_plan(len, ($new)(), hooks, width, height)
            }, |n| n)
        }

        /// Prepare one session, optionally using a composition-owned generation.
        #[no_mangle]
        pub extern "C" fn exact_prepare_plan(rt: u32, token: u64, len: usize, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| {
                b.set_compat($compat);
                if let Some(refusal) = b.refuse_analysis() { return refusal; }
                b.set_delivery($delivery);
                let delivery: ::std::option::Option<&'static $crate::delivery::Hooks> = $delivery;
                let facts = delivery.and_then(|h| (h.candidate_delivery)(token, $compat));
                if token != 0 && facts.is_none() { return b.refuse_preparation("unknown composition generation"); }
                b.prepare_plan_with_delivery(len, ($new)(), hooks, width, height, facts)
            }, |n| n)
        }

        /// First pixel has been presented; activate deferred app logic.
        #[no_mangle]
        pub extern "C" fn exact_data_ready(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.data_ready(), |n| n)
        }

        /// Prepare an admitted module generation, optionally carrying a delivery token.
        #[no_mangle]
        pub extern "C" fn exact_prepare_module(rt: u32, token: u64, plan: usize, receipt: usize, module: usize, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| {
                b.set_compat($compat);
                if let Some(refusal) = b.refuse_analysis() { return refusal; }
                b.set_delivery($delivery);
                let delivery: ::std::option::Option<&'static $crate::delivery::Hooks> = $delivery;
                let facts = delivery.and_then(|h| (h.candidate_delivery)(token, $compat));
                if token != 0 && facts.is_none() { return b.refuse_preparation("unknown composition generation"); }
                b.prepare_module_with_delivery([plan, receipt, module], ($new)(), hooks, width, height, facts)
            }, |n| n)
        }

        /// Commit an accepted prepared plan.
        #[no_mangle]
        pub extern "C" fn exact_commit_plan(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.commit_plan(), |n| n)
        }

        /// Abort a prepared plan.
        #[no_mangle]
        pub extern "C" fn exact_discard_plan(rt: u32) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.discard_plan(), |_| ())
        }

        /// Dispatch an event; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_dispatch(rt: u32, view: u32, kind: u32, len: usize, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.dispatch(view, kind, len, now_ms), |n| n)
        }

        /// Copy current region source/paint metadata. No returned bytes outlive exact_out.
        #[no_mangle]
        pub extern "C" fn exact_region_request(rt: u32, id: u64, known_source: u64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.region_request(id, known_source), |n| n)
        }
        /// Invalidate metrics for a completed native paragraph revision.
        #[no_mangle]
        pub extern "C" fn exact_text_ready(rt: u32, index: u32, generation: u32, revision: u64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false,
                |b, _| b.text_ready(index, generation, revision), |n| n)
        }

        /// Takes one native retain on every path, including destroyed/busy runtimes.
        #[no_mangle]
        pub extern "C" fn exact_region_complete(rt: u32, id: u64, metrics: $crate::measure::CMetrics,
            owner: *mut ::std::ffi::c_void, release: $crate::content_region::RegionRelease) -> u32 {
            let retained = ::std::rc::Rc::new($crate::content_region::NativeRegionOwner::new(owner, release));
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.region_complete(id, metrics, retained), |n| n)
        }

        /// Process one frozen paired transform packet from exact_in.
        #[no_mangle]
        pub extern "C" fn exact_transform_motion(rt: u32, len: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.transform_motion(len as usize), |n| n)
        }

        /// Capture one property's native presentation.
        #[no_mangle]
        pub extern "C" fn exact_hold_begin(rt: u32, view: u32, property: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.hold_begin(view, property, now_ms), |n| n)
        }
        /// Begin a header binding using exact packed generational keys.
        #[no_mangle]
        pub extern "C" fn exact_height_drag_begin(rt: u32, handle: u64, target: u64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.height_drag_begin(handle, target, now_ms), |n| n)
        }
        /// Update an eligible header's live token.
        #[no_mangle]
        pub extern "C" fn exact_height_drag_update(rt: u32, token: u64, height: f64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.height_drag_update(token, height, now_ms), |n| n)
        }
        /// Final sample then typed action; the caller ends the token afterward.
        #[no_mangle]
        pub extern "C" fn exact_height_drag_release(rt: u32, token: u64, height: f64, velocity: f64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.height_drag_release(token, height, velocity, now_ms), |n| n)
        }
        /// Arrange: catch a handle's row at the List's actual scrollTop.
        #[no_mangle]
        pub extern "C" fn exact_reorder_begin(rt: u32, handle: u32, scroll_top: f64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reorder_begin(handle, scroll_top, now_ms), |n| n)
        }
        /// Arrange: one pointer sample for the live contact.
        #[no_mangle]
        pub extern "C" fn exact_reorder_move(rt: u32, token: u64, dy: f64, scroll_top: f64, inside: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reorder_move(token, dy, scroll_top, inside, now_ms), |n| n)
        }
        /// Arrange: the contact ended; drop (nonzero) or cancel.
        #[no_mangle]
        pub extern "C" fn exact_reorder_end(rt: u32, token: u64, drop: u32, dy: f64, scroll_top: f64, inside: u32, velocity: f64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reorder_end(token, drop, dy, scroll_top, inside, velocity, now_ms), |n| n)
        }
        /// Check before dispatching an authored completion.
        #[no_mangle]
        pub extern "C" fn exact_has_hold(rt: u32, token: u64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| u32::from(b.has_hold(token)), |_| 0)
        }
        /// Change a held property's presentation.
        #[no_mangle]
        pub extern "C" fn exact_hold_update(rt: u32, token: u64, x: f64, y: f64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.hold_update(token, x, y, now_ms), |n| n)
        }
        /// End ownership once, with velocity in displayed units/second.
        #[no_mangle]
        pub extern "C" fn exact_hold_end(rt: u32, token: u64, cancel: u32, vx: f64, vy: f64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.hold_end(token, cancel != 0, vx, vy, now_ms), |n| n)
        }

        /// Move the clock; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_advance(rt: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.advance(now_ms), |n| n)
        }

        /// Publish or clear a named surface record; returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_surface_record(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.surface_record(len), |n| n)
        }

        /// The viewport changed; returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_resize(rt: u32, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.resize(width, height), |n| n)
        }

        /// Report an actual list scrollport and bounded interaction pins.
        #[no_mangle]
        pub extern "C" fn exact_list(rt: u32, view: u32, top: f64, height: f64, width: f64, origin: f64, focus: u32, interaction: u32, limit: u32, velocity: f64) -> u32 {
            // Zero asks for the whole window; `limit - 1` rows beyond the scrollport otherwise.
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.list_viewport(view, $crate::ListViewport {
                top, height, width, origin, velocity, pins: [focus, interaction], rows: &[],
            }, limit.checked_sub(1).map(|rows| rows as usize)), |n| n)
        }

        /// Whether that list's last report left rows to create or retire: 1 or 0.
        #[no_mangle]
        pub extern "C" fn exact_list_pending(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.list_pending(view), |_| 0)
        }

        /// Resolve a logical row key in the input buffer, or UINT32_MAX.
        #[no_mangle]
        pub extern "C" fn exact_list_index(rt: u32, view: u32, len: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.list_index(view, len as usize), |_| u32::MAX)
        }

        /// Copy logical text without materializing native views. Input is
        /// two concatenated UTF-8 row keys; first_len == 0 means all text.
        #[no_mangle]
        pub extern "C" fn exact_list_text(rt: u32, view: u32, first_len: u32, len: u32, first_paragraph: u32, first_offset: u32, last_paragraph: u32, last_offset: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.list_text(view, first_len as usize, len as usize, first_paragraph as usize, first_offset as usize, last_paragraph as usize, last_offset as usize), |_| 0)
        }

        /// The safe-area insets changed; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_insets(rt: u32, top: f32, right: f32, bottom: f32, left: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.insets(top, right, bottom, left), |n| n)
        }

        /// An image loaded (or failed: a size ≤ 0); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_intrinsic(rt: u32, view: u32, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.intrinsic(view, width, height), |n| n)
        }

        /// Several images' intrinsic sizes from the input buffer, one layout;
        /// returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_intrinsics(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.intrinsics(len), |n| n)
        }

        /// Common LE collection feedback from the input buffer; returns batch length.
        #[no_mangle]
        pub extern "C" fn exact_collection_feedback(rt: u32, len: usize, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.collection_feedback(len, now_ms), |n| n)
        }

        /// A motion frame; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_tick(rt: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.tick(now_ms), |n| n)
        }

        /// An agent request from the input buffer; returns the reply's length.
        #[no_mangle]
        pub extern "C" fn exact_agent(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, true, |b, _| b.agent(len), |n| n)
        }

        /// A host line for the runner's journal (LLP 1012 §3; LLP 1035.001
        /// D6 — a refused intent is a line, never silence): the input
        /// buffer's first `len` bytes. Returns 0.
        #[no_mangle]
        pub extern "C" fn exact_log(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, true, |b, _| b.log(len), |_| 0)
        }
    };
}
