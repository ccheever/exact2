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
        $crate::app_module_exports!();
        $crate::material_exports!();
        $crate::corner_exports!();
        thread_local! {
            static EXACT_RUNTIMES: ::std::cell::RefCell<$crate::abi::Registry<$data>> = ::std::cell::RefCell::new($crate::abi::Registry::default());
        }
        $crate::pan_velocity_exports!();

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

        /// A paragraph's line boxes (LLP 1093 D6), called with the context
        /// `exact_set_measure` was given; `None` keeps paragraphs whole in a
        /// multi-column flow.
        #[no_mangle]
        pub extern "C" fn exact_set_lines(
            rt: u32,
            lines: ::std::option::Option<$crate::measure::LinesFn>,
        ) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| { e.hooks.lines = lines; });
        }

        /// The Canvas 2D text measurer (LLP 1056 D8): called on the runtime's
        /// thread with the context `exact_set_measure` was given.
        #[no_mangle]
        pub extern "C" fn exact_set_canvas_text(
            rt: u32,
            measure: ::std::option::Option<$crate::canvas_text::CanvasTextFn>,
        ) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| { e.hooks.canvas_text = measure; });
        }

        /// A Canvas 2D image handle decoded (LLP 1056 D9): the handle is
        /// the input buffer's first `len` bytes; the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_canvas_image(rt: u32, len: usize, width: u32, height: u32, ok: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.canvas_image(len, width, height, ok != 0), |n| n)
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

        /// The session's app module (LLP 1067.000): `later` takes each long
        /// native call and answers it once with `exact_app_reply`; `call`
        /// answers a `native.call` with `exact_app_answer` before returning.
        /// `None` removes each.
        #[no_mangle]
        pub extern "C" fn exact_set_app_module(
            rt: u32,
            later: ::std::option::Option<$crate::app_module::LaterFn>,
            call: ::std::option::Option<$crate::app_module::CallFn>,
            ctx: *mut ::std::ffi::c_void,
        ) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| e.bridge.set_app_module(later, call, ctx));
        }

        /// The app module announced a topic (LLP 1016.002), on this thread.
        #[no_mangle]
        pub extern "C" fn exact_app_changed(rt: u32, topic: *const u8, len: usize) {
            let topic = $crate::app_module::text(topic, len);
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| e.bridge.app_changed(&topic));
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

        /// Content has settled: give back storage beyond the live nodes.
        #[no_mangle]
        pub extern "C" fn exact_trim(rt: u32) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.trim(), |_| ())
        }

        /// Dispatch an event; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_dispatch(rt: u32, view: u32, kind: u32, len: usize, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.dispatch(view, kind, len, now_ms), |n| n)
        }

        /// A scroller (or, nonzero `page`, the page) now stands at `left`,
        /// `top` CSS px: what `frame` subtracts (LLP 1051.000 D1). No batch.
        #[no_mangle]
        pub extern "C" fn exact_scrolled(rt: u32, page: u32, view: u32, left: f64, top: f64) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.scrolled(page != 0, view, left, top), |_| ())
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
        pub extern "C" fn exact_height_drag_release(rt: u32, token: u64, height: f64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.height_drag_release(token, height, now_ms), |n| n)
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
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.hold_end(token, cancel, vx, vy, now_ms), |n| n)
        }
        /// A threshold exact2 defines itself, by index (`exact_motion::gesture::CONSTANTS`); NaN past the end.
        #[no_mangle]
        pub extern "C" fn exact_gesture_constant(which: u32) -> f64 {
            $crate::abi::gesture_constant(which)
        }

        /// Move the clock; `mode` as `Bridge::advance` (0 the wall clock, 1
        /// the agent's jump, 2 an input's `then`s). Returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_advance(rt: u32, now_ms: f64, mode: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.advance(now_ms, mode), |n| n)
        }

        /// A presented display frame at `now_ms` (LLP 1073 D5): timers due
        /// by then, then every frame task once. Returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_frame(rt: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.frame(now_ms), |n| n)
        }

        /// Nonzero: the display drives frame tasks (`exact_frame` turns it
        /// on); zero when the agent's clock takes over (LLP 1073 D4).
        #[no_mangle]
        pub extern "C" fn exact_present_frames(rt: u32, on: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| { b.present_frames(on != 0); 0 }, |n| n)
        }

        /// Publish or clear a named surface record; returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_surface_record(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.surface_record(len), |n| n)
        }

        /// The date: Unix ms at clock zero and minutes east of UTC.
        #[no_mangle]
        pub extern "C" fn exact_set_time(rt: u32, epoch_at_zero: f64, utc_offset: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_time(epoch_at_zero, utc_offset), |n| n)
        }

        /// The display preferences: bit 0 reduced motion, bit 1 reduced
        /// transparency, bit 2 contrast more, bit 3 contrast less, bit 4 dark.
        #[no_mangle]
        pub extern "C" fn exact_set_preferences(rt: u32, bits: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_preferences(bits), |n| n)
        }

        /// The root font size in points (Dynamic Type on iOS, 16 on the Mac).
        #[no_mangle]
        pub extern "C" fn exact_set_root_font_size(rt: u32, px: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_root_font_size(px), |n| n)
        }

        /// The page's facts: bit 0 hidden, bit 1 offline, bit 2 a share sheet.
        #[no_mangle]
        pub extern "C" fn exact_set_page(rt: u32, bits: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_page(bits), |n| n)
        }

        /// The locale and time zone: `locale NUL timeZone` in the input buffer.
        #[no_mangle]
        pub extern "C" fn exact_set_place(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_place(len), |n| n)
        }

        /// The display's scale and physical memory for Canvas 2D (LLP 1056
        /// D4); callable before boot. Returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_canvas_display(rt: u32, scale: f64, memory: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.canvas_display(scale, memory), |n| n)
        }

        /// A 2D canvas's replay is behind (`held` 1) or caught up (0): a
        /// frame request for it waits while held (LLP 1056 D5).
        #[no_mangle]
        pub extern "C" fn exact_canvas_held(rt: u32, view: u32, held: u32) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.canvas_held(view, held != 0), |_| ())
        }

        /// The viewport changed; returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_resize(rt: u32, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.resize(width, height), |n| n)
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

        /// The posture and the viewport segments (LLP 1078 D4); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_segments(rt: u32, posture: u32, cols: u32, rows: u32, count: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.segments(posture, cols, rows, count), |n| n)
        }

        /// The presenter's appearance: nonzero is dark (LLP 1062); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_scheme(rt: u32, dark: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.scheme(dark != 0), |n| n)
        }

        /// One view's appearance (nonzero: dark), where it differs from the
        /// session's (LLP 1062 D4); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_view_scheme(rt: u32, view: u32, dark: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.view_scheme(view, dark != 0), |n| n)
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

        /// What native containers cover of boxes, from the input buffer, one
        /// layout (LLP 1075.003 §3.5); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_host_covers(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.covers(len), |n| n)
        }

        /// Common LE collection feedback from the input buffer; returns batch length.
        #[no_mangle]
        pub extern "C" fn exact_collection_feedback(rt: u32, len: usize, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.collection_feedback(len, now_ms), |n| n)
        }

        /// `key\nblock\ninline` in the input buffer: the agent's `tap <list>
        /// into <key>` (LLP 1070.000 §5); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_into_view(rt: u32, view: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.into_view(view, len), |n| n)
        }

        /// Canvas draws in a turn of their own (`deferred` nonzero), or in every
        /// turn (LLP 1072 §8.5): the batch says `canvasOwed` and
        /// `exact_canvas_draw` runs them, off the turns main waits on.
        #[no_mangle]
        pub extern "C" fn exact_canvas_defer(rt: u32, deferred: u32) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.canvas_defer(deferred != 0), |_| ())
        }

        /// The canvas draws owed (LLP 1072 §8.5); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_canvas_draw(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.canvas_draw(), |n| n)
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

        /// An auth session's word (LLP 1069.006): `hold` under the agent, or
        /// `done` with the callback URL or a status; JSON in the input buffer.
        #[no_mangle]
        pub extern "C" fn exact_auth(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, true, |b, _| b.auth(len), |_| 0)
        }

        /// A native button's title and symbol (LLP 1069.011 D5), JSON;
        /// returns its length.
        #[no_mangle]
        pub extern "C" fn exact_press_face(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.press_face(view), |n| n)
        }

        /// A grouped list's sections and rows (LLP 1084 D4), JSON; returns
        /// its length.
        #[no_mangle]
        pub extern "C" fn exact_grouped_list(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.grouped_list(view), |n| n)
        }

        /// A select's options and the one it shows (LLP 1069.001 D5), JSON;
        /// returns its length.
        #[no_mangle]
        pub extern "C" fn exact_select_options(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.select_options(view), |n| n)
        }

        /// A command's data (`share`, LLP 1069.003; `saveFile`, LLP
        /// 1069.010), JSON in the input buffer; returns the ruling's length
        /// (`refused`, `ticket`, `present`).
        #[no_mangle]
        pub extern "C" fn exact_command(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, true, |b, _| b.command(len), |n| n)
        }

        /// What SVG pixel work the live plan can need, as bits: 1 an island
        /// (a `mask` or a `filter`; `exact_runner::svg_islands`), when the
        /// host should open its island module off the main thread now (LLP
        /// 1055.000 §8 ruling 4); 2 a filter (`exact_runner::svg_filters`),
        /// when it should make its GPU filter pipelines now.
        #[no_mangle]
        pub extern "C" fn exact_svg_islands(rt: u32) -> u8 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.svg_islands(), |_| 0)
        }
    };
}
