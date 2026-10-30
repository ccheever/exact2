/// Instantiate the C exports for one app (see `include/exact.h`).
///
/// `$data` is the app's `DataSource` type (constructed with `Default`, or
/// the sixth argument's factory for a deferred bytecode module);
/// `$plan` a `&'static [u8]` of baked plan bytes. Every export takes the
/// runtime handle `exact_create` returned (LLP 1031 D2).
#[macro_export]
macro_rules! host {
    // A generated entry names what it links (LLP 1047.001 D2, D3) after the
    // arguments, `; linked = EXACT_LINKED`, and invokes the export groups of
    // those capabilities itself; without it, every capability is linked.
    ($data:ty, $plan:expr, $compat:expr $(; linked = $linked:expr)?) => {
        $crate::host!($data, $plan, $compat, None, ::std::ptr::null() $(; linked = $linked)?);
    };
    ($data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr $(; linked = $linked:expr)?) => {
        $crate::host!($data, $plan, $compat, $delivery, $api, || <$data as ::std::default::Default>::default() $(; linked = $linked)?);
    };
    ($data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr, $new:expr $(; linked = $linked:expr)?) => {
        $crate::host!($data, $plan, $compat, $delivery, $api, $new, None $(; linked = $linked)?);
    };
    ($data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr, $new:expr, $region:expr; linked = $linked:expr) => {
        $crate::host!(@core $data, $plan, $compat, $delivery, $api, $new, $region, $linked);
    };
    ($data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr, $new:expr, $region:expr) => {
        $crate::grouped_list_exports!();
        $crate::markup_exports!();

        $crate::host!(@core $data, $plan, $compat, $delivery, $api, $new, $region, $crate::link::ALL);
    };
    (@core $data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr, $new:expr, $region:expr, $linked:expr) => {
        /// A textarea's decorations resolved against its text (LLP 1114.007).
        #[no_mangle]
        pub extern "C" fn exact_field_decorations(input: *const u8, len: usize, count: *mut usize) -> *const u8 {
            $crate::decorations::read(input, len, count)
        }
        $crate::raster_exports!();
        $crate::svg_image_exports!();
        $crate::textflow_exports!();
        $crate::collapse_exports!();
        $crate::app_module_exports!();
        $crate::material_exports!();
        $crate::corner_exports!();
        $crate::gradient_exports!();
        $crate::calc_exports!();
        $crate::local_network_exports!();
        thread_local! {
            static EXACT_RUNTIMES: ::std::cell::RefCell<$crate::abi::Registry<$data>> = ::std::cell::RefCell::new($crate::abi::Registry::default());
        }
        /// What every boot links, made from the entry's set in a `const` so
        /// the linker sees nothing else (LLP 1047.001 D3).
        const EXACT_LINKS: $crate::link::Links<$data> = $crate::link::Links::of($linked);
        $crate::pan_velocity_exports!();
        $crate::frame_health_exports!();

        /// Create a runtime; returns its handle (never 0). Its callbacks are
        /// set with `exact_set_measure`, `exact_set_wake`, and
        /// `exact_set_fonts` before its first boot.
        #[no_mangle]
        pub extern "C" fn exact_create() -> u32 {
            $crate::link::set($linked);
            let id = EXACT_RUNTIMES.with(|r| r.borrow_mut().create_linked(EXACT_LINKS));
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

        /// UIKit control hooks, using the registered text engine context.
        #[no_mangle]
        pub extern "C" fn exact_set_control_text(rt: u32, text: Option<$crate::control_text::ControlTextFn>, chrome: Option<$crate::control_text::FieldChromeFn>) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| e.bridge.set_control_text(text, chrome));
        }
        /// Native button height-for-width, with the registered text context.
        #[no_mangle]
        pub extern "C" fn exact_set_button_measure(rt: u32, measure: Option<$crate::control_text::ButtonMeasureFn>) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| e.bridge.set_button_measure(measure));
        }
        /// Remeasure the controls after a trait change, through set_env.
        #[no_mangle]
        pub extern "C" fn exact_control_text_changed(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| b.control_text_changed(hooks), |n| n)
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

        /// Each inline box's origin in its paragraph (LLP 1011.002 D2), with
        /// the context `exact_set_measure` was given; `None` leaves an image
        /// at its paragraph's content origin.
        #[no_mangle]
        pub extern "C" fn exact_set_inline_boxes(
            rt: u32,
            boxes: ::std::option::Option<$crate::measure::LinesFn>,
        ) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| { e.hooks.inline_boxes = boxes; });
        }

        /// A system symbol's natural size, measured before layout
        /// (`TextMeasurer::symbol_size`), with the context `exact_set_measure`
        /// was given, beside the text measurer it registered: with the
        /// reference measurer, as with `None`, a symbol is the size its view
        /// reports.
        #[no_mangle]
        pub extern "C" fn exact_set_symbol_measure(
            rt: u32,
            measure: ::std::option::Option<$crate::measure::SymbolFn>,
        ) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| { e.hooks.symbol = measure; });
        }

        /// Format live time with the platform, on the runner clock.
        #[no_mangle]
        pub extern "C" fn exact_set_time_format(rt: u32, f: Option<$crate::time::TimeFormatFn>) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| { e.hooks.time_format = f; });
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

        /// Where a document the app forgot goes (LLP 1069.010.000 D5): off
        /// the platform's recents, by the Swift host.
        #[no_mangle]
        pub extern "C" fn exact_on_document_forgotten(forgotten: Option<extern "C" fn(u64, u64)>) {
            if let Some(forgotten) = forgotten {
                $crate::documents::on_forgotten(forgotten);
            }
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

        /// The first root's literal viewport-fit, read before boot. Zero len selects the bake.
        #[no_mangle]
        pub extern "C" fn exact_plan_covers_viewport(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| u32::from(b.plan_covers_viewport(len, $plan)), |_| 0)
        }

        /// Derive a declared protocol handler location from href, scheme and template.
        #[no_mangle]
        pub extern "C" fn exact_protocol_location_of(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, true, |b, _| b.protocol_location_of(len), |n| n)
        }

        /// Whether the input location names a declared route. @ref LLP 1038 §7
        #[no_mangle]
        pub extern "C" fn exact_route_matches(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.route_matches(len), |_| 0)
        }

        /// A package's call of the app's handler, answered once through
        /// `reply`. @ref LLP 1133.016 D15 item 1
        #[no_mangle]
        pub extern "C" fn exact_module_invoke(rt: u32, len: usize, reply: extern "C" fn(*mut std::ffi::c_void, *const u8, u32), ctx: *mut std::ffi::c_void) {
            let refused = |_| {
                let text = r#"{"error":"InvalidStateError","message":"the runtime is busy or gone"}"#;
                reply(ctx, text.as_ptr(), text.len() as u32);
            };
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.module_invoke(len, reply, ctx), refused)
        }

        /// The location beneath a visit, UTF-8; empty for none. @ref LLP 1115 D5
        #[no_mangle]
        pub extern "C" fn exact_location_beneath(rt: u32, id: u64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.location_beneath(id), |_| 0)
        }

        /// A retained visit's canonical location, UTF-8; empty when absent.
        #[no_mangle]
        pub extern "C" fn exact_visit_location(rt: u32, id: u64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.visit_location(id), |_| 0)
        }

        /// Complete a platform Back-history jump, a batch.
        #[no_mangle]
        pub extern "C" fn exact_host_back_to(rt: u32, source: u64, destination: u64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.host_back_to(source, destination, now_ms), |n| n)
        }

        /// The platform's own Back from a visit, a batch. @ref LLP 1115 D5
        #[no_mangle]
        pub extern "C" fn exact_host_back(rt: u32, id: u64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.host_back(id, now_ms), |n| n)
        }

        /// Follow a UTF-8 location with the runner's own go.
        #[no_mangle]
        pub extern "C" fn exact_host_go(rt: u32, len: usize, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.host_go(len, now_ms), |n| n)
        }

        /// A received share, a batch. @ref LLP 1133.024 D2–D4
        #[no_mangle]
        pub extern "C" fn exact_share(rt: u32, len: usize, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.share(len, now_ms), |n| n)
        }

        /// The files of a share claimed before the boot, landed: a batch.
        #[no_mangle]
        pub extern "C" fn exact_share_land(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.share_land(), |n| n)
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

        /// Snapshot bytes, kind and optional props JSON, concatenated in the input buffer.
        #[no_mangle]
        pub extern "C" fn exact_boot_entry(rt: u32, plan: usize, kind: usize, props: usize, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| {
                b.set_compat($compat);
                if let Some(refusal) = b.refuse_analysis() { return refusal; }
                b.set_delivery($delivery);
                b.boot_entry(plan, kind, props, ($new)(), hooks, width, height)
            }, |n| n)
        }
        /// Refresh the entry's declared source; returns a JSON timeline or an error.
        #[no_mangle]
        // The synchronous caller owns both slices until this call returns;
        // the C ABI's exports are not `unsafe fn`, as the others here are not.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn exact_entry_timeline(rt: u32, now: f64, remaining_ms: f64, receipt: *const u8, receipt_len: usize, module: *const u8, module_len: usize) -> u32 {
            let receipt = if receipt.is_null() { &[] } else { unsafe { std::slice::from_raw_parts(receipt, receipt_len) } };
            let module = if module.is_null() { &[] } else { unsafe { std::slice::from_raw_parts(module, module_len) } };
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.entry_timeline_module(now, remaining_ms, receipt, module), |n| n)
        }

        /// Refresh a headless entry, retaining preview props when unavailable.
        #[no_mangle]
        pub extern "C" fn exact_refresh_entry(rt: u32, now: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| {
                b.refresh_entry(($new)(), now)
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

        /// First pixel has been presented; evaluate deferred app logic, uncommitted.
        #[no_mangle]
        pub extern "C" fn exact_prepare_data(rt: u32) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.prepare_data(), |_| ())
        }

        /// First pixel has been presented; activate deferred app logic.
        #[no_mangle]
        pub extern "C" fn exact_data_ready(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.data_ready(), |n| n)
        }

        /// Re-read the shared store and ask its resources again (LLP 1126.001 WP12).
        #[no_mangle]
        pub extern "C" fn exact_store_changed(rt: u32, retry_only: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.store_changed(retry_only != 0), |n| n)
        }

        /// Prepare an admitted module generation, optionally carrying a delivery token;
        /// `let_go`: the dev loop's, with requests in flight too (LLP 1007 §6).
        #[no_mangle]
        pub extern "C" fn exact_prepare_module(rt: u32, token: u64, plan: usize, receipt: usize, module: usize, width: f32, height: f32, let_go: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| {
                b.set_compat($compat);
                if let Some(refusal) = b.refuse_analysis() { return refusal; }
                b.set_delivery($delivery);
                let delivery: ::std::option::Option<&'static $crate::delivery::Hooks> = $delivery;
                let facts = delivery.and_then(|h| (h.candidate_delivery)(token, $compat));
                if token != 0 && facts.is_none() { return b.refuse_preparation("unknown composition generation"); }
                b.prepare_module_with_delivery([plan, receipt, module], ($new)(), hooks, width, height, facts, let_go != 0)
            }, |n| n)
        }

        /// Refuse a candidate whose source changed after preparation.
        #[no_mangle]
        pub extern "C" fn exact_check_prepared(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.check_prepared(), |n| n)
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

        /// The presenter's synchronous viewport union for an inline run.
        #[no_mangle]
        pub extern "C" fn exact_set_inline_geometry(rt: u32, read: Option<$crate::geometry::InlineFrameFn>, range: Option<$crate::geometry::RangeFrameFn>, ctx: *mut ::std::ffi::c_void) {
            $crate::abi::with_entry(&EXACT_RUNTIMES, rt, |e| e.bridge.set_inline_geometry(read, range, ctx));
        }

        /// The open top-layer nodes before a native action read. No batch.
        ///
        /// # Safety
        /// `views` points to `count` valid ids, `rects` to `count * 4` doubles.
        /// `clips` is null or points to `count * 4` doubles; NaN at an
        /// entry's x means no clip.
        #[no_mangle]
        pub unsafe extern "C" fn exact_top_layer(rt: u32, views: *const u32, rects: *const f64, clips: *const f64, count: u32, modal: u32) {
            let ids = if count == 0 { &[] } else {
                // SAFETY: the caller keeps `count` ids live for this synchronous call.
                unsafe { std::slice::from_raw_parts(views, count as usize) }
            };
            let rects = if count == 0 { &[] } else {
                // SAFETY: the caller keeps four doubles per id live for this call.
                unsafe { std::slice::from_raw_parts(rects, count as usize * 4) }
            };
            let clips = if clips.is_null() || count == 0 { &[] } else {
                // SAFETY: the caller keeps four doubles per id live for this call.
                unsafe { std::slice::from_raw_parts(clips, count as usize * 4) }
            };
            let frames: Vec<_> = ids.iter().zip(rects.chunks_exact(4)).enumerate()
                .map(|(i, (&id, r))| {
                    let clip = clips.get(i * 4..i * 4 + 4).filter(|c| !c[0].is_nan())
                        .map(|c| [c[0], c[1], c[2], c[3]]);
                    (id, [r[0], r[1], r[2], r[3]], clip)
                }).collect();
            let modal = (modal != u32::MAX).then_some(modal as usize);
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.top_layer(&frames, modal), |_| ())
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
        /// Dropping across lists (LLP 1094): lift a grouped grip, with a ghost or for keys.
        #[no_mangle]
        pub extern "C" fn exact_reorder_group_begin(rt: u32, handle: u32, scroll_top: f64, ghost: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reorder_group_begin(handle, scroll_top, ghost, now_ms), |n| n)
        }
        /// The ghost's centre at `content_y` in `target`'s content (LLP 1094 D5).
        #[no_mangle]
        pub extern "C" fn exact_reorder_move_into(rt: u32, token: u64, target: u32, content_y: f64, target_scroll_top: f64, inside: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reorder_move_into(token, target, content_y, target_scroll_top, inside, now_ms), |n| n)
        }
        /// A key's or custom action's step: 1 earlier, 2 later, 3 previous list, 4 next (LLP 1094 D9).
        #[no_mangle]
        pub extern "C" fn exact_reorder_step(rt: u32, token: u64, step: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reorder_group_step(token, step, now_ms), |n| n)
        }
        /// The grouped contact ended: drop (nonzero) into the target, or cancel.
        #[no_mangle]
        pub extern "C" fn exact_reorder_group_end(rt: u32, token: u64, drop: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reorder_group_end(token, drop, now_ms), |n| n)
        }
        /// The ghost landed or faded: the session ends and the row shows.
        #[no_mangle]
        pub extern "C" fn exact_reorder_group_finish(rt: u32, token: u64, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reorder_group_finish(token, now_ms), |n| n)
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

        /// A route's header as its bar shows it (LLP 1115.001 D11): the
        /// kernel's rule (`exact_kernel::bars`) over the host's own mirror
        /// of the route, `count` words at `words`. Writes the answer to
        /// `out` and returns its length; 0 for a route that is not
        /// header-shaped, or an answer longer than `cap`. No runtime: the
        /// rule reads only what it is handed.
        ///
        /// # Safety
        /// `words` holds `count` words and `out` room for `cap`.
        #[no_mangle]
        pub unsafe extern "C" fn exact_header_shape(
            words: *const u32, count: usize, flags: u32, out: *mut u32, cap: usize,
        ) -> u32 {
            if words.is_null() || out.is_null() {
                return 0;
            }
            // SAFETY: the caller's buffers, as the contract above says.
            let (words, out) = unsafe {
                (::std::slice::from_raw_parts(words, count), ::std::slice::from_raw_parts_mut(out, cap))
            };
            $crate::abi::header_shape(words, flags, out)
        }

        /// What a route shows of the platform's bar (`exact_kernel::Bar`):
        /// 0 its header, 1 the platform's Back alone, 2 nothing. `facts`:
        /// bit 0 the route says `navigationBar="none"`, bit 1 its header is
        /// the bar's, bit 2 it is pushed, bit 3 the platform's bar has a
        /// Back button.
        #[no_mangle]
        pub extern "C" fn exact_route_bar(facts: u32) -> u32 {
            $crate::abi::route_bar(facts)
        }

        /// Move the clock; `mode` as `Bridge::advance` (0 the wall clock, 1
        /// the agent's jump, 2 an input's `then`s, 3 those `then`s only when
        /// one is the earliest work due). Returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_timer_waits(rt: u32, kinds: u32, cutoff: f64, mode: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.timer_waits(kinds as u8, cutoff, mode), |n| n)
        }

        /// Delegate ordinary live timers to independent native callbacks.
        #[no_mangle]
        pub extern "C" fn exact_native_timers(rt: u32, enabled: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| { b.native_timers(enabled != 0); 0 }, |n| n)
        }

        /// Deliver one native timer instance without draining other native timers.
        #[no_mangle]
        pub extern "C" fn exact_fire_native_timer(rt: u32, index: u32, generation: u64, now: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.fire_native_timer(index, generation, now), |n| n)
        }

        /// Move the clock through ordinary deadlines.
        #[no_mangle]
        pub extern "C" fn exact_advance(rt: u32, now_ms: f64, mode: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.advance(now_ms, mode), |n| n)
        }

        /// Frame tasks after timer admission, with the display target and wall clock.
        #[no_mangle]
        pub extern "C" fn exact_admitted_frame(rt: u32, target_ms: f64, wall_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.admitted_frame(target_ms, wall_ms), |n| n)
        }

        /// A presented display frame, including ordinary timer work.
        #[no_mangle]
        pub extern "C" fn exact_frame(rt: u32, now_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.frame(now_ms), |n| n)
        }

        /// `exact_frame` at the target `now_ms`, the wall at `wall_ms`
        /// stopping the motion engine's input clock (LLP 1003.001 D5).
        #[no_mangle]
        pub extern "C" fn exact_frame_at(rt: u32, now_ms: f64, wall_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.frame_at(now_ms, wall_ms), |n| n)
        }

        /// Nonzero: motion an author's commit begins waits for the first
        /// presented frame (LLP 1003.001 D7), in every host booted after;
        /// zero at the agent's takeover, where what waits starts at `at_ms`.
        /// Returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_start_on_frame(rt: u32, on: u32, at_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.start_on_frame(on != 0, at_ms), |n| n)
        }

        /// Nonzero: the display drives frame tasks (`exact_frame` turns it
        /// on); zero when the agent's clock takes over (LLP 1073 D4).
        #[no_mangle]
        pub extern "C" fn exact_present_frames(rt: u32, on: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| { b.present_frames(on != 0); 0 }, |n| n)
        }

        /// The presenter's clock owns time (`wall`, in the runner's
        /// milliseconds), or the agent's seekable one does (null): a live
        /// clock fires a repeat that missed beats once (the timers RFC, D1).
        #[no_mangle]
        pub extern "C" fn exact_set_live_clock(rt: u32, wall: Option<$crate::abi::LiveClockFn>) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| { b.set_live_clock(wall); 0 }, |n| n)
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

        /// The date the next boot or candidate settles under, before it.
        #[no_mangle]
        pub extern "C" fn exact_set_launch_time(rt: u32, epoch_at_zero: f64, utc_offset: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| { b.set_launch_time(epoch_at_zero, utc_offset); 0 }, |n| n)
        }

        /// Where the viewer is (`locale NUL timeZone NUL seed` input) for the next boot or candidate, before it.
        #[no_mangle]
        pub extern "C" fn exact_set_launch_place(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_launch_place(len), |n| n)
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

        /// The host's zoom over the root size (the Mac's View ▸ Zoom In).
        #[no_mangle]
        pub extern "C" fn exact_set_root_font_zoom(rt: u32, factor: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_root_font_zoom(factor), |n| n)
        }

        /// Whether a confirmation's rows are an alert's (LLP 1115 D6): 1 for
        /// explanatory text, one cancel and at most three buttons. The rule
        /// is the kernel's; needs no runtime.
        #[no_mangle]
        pub extern "C" fn exact_confirmation_is_alert(texts: u32, actions: u32, cancels: u32) -> u32 {
            u32::from($crate::abi::confirmation_is_alert(texts, actions, cancels))
        }

        /// The display's scale, device pixels a point (UIKit's
        /// `displayScale`, AppKit's `backingScaleFactor`): what a hairline is
        /// one device pixel of (LLP 1115.000 D10.3).
        #[no_mangle]
        pub extern "C" fn exact_set_display_scale(rt: u32, scale: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_display_scale(scale), |n| n)
        }

        /// The page's facts: bit 0 hidden, bit 1 offline, bit 2 a share sheet.
        #[no_mangle]
        pub extern "C" fn exact_set_page(rt: u32, bits: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_page(bits), |n| n)
        }

        /// 1 when the running plan reads `onLine`, and before boot: only
        /// then does the host watch the network.
        #[no_mangle]
        pub extern "C" fn exact_reads_on_line(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.reads_on_line(), |_| 1)
        }

        /// The presenter declares a packaged arm before any boot.
        #[no_mangle]
        pub extern "C" fn exact_link_activities(rt: u32) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.link_activities(), |_| ());
        }

        /// Activity facts or one send's completion in the input buffer.
        #[no_mangle]
        pub extern "C" fn exact_activities(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.activities(len), |n| n)
        }

        /// MIDI ports or messages from the host, or the agent's `prefer midi` and `tap midi`.
        #[no_mangle]
        pub extern "C" fn exact_midi(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.midi(len), |n| n)
        }

        /// The extensions the app's module composes, a name a line in the
        /// input buffer (LLP 1133.001 D4).
        #[no_mangle]
        pub extern "C" fn exact_bind_extensions(rt: u32, len: usize) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.bind_extensions(len), |_| ())
        }

        /// An `img` source of a package's scheme, admitted (LLP 1133.019 D11
        /// item 1): the source in the input buffer; the output is
        /// `{"extension", "agent", "owned"?}`, `{"error", "message"}`, or
        /// empty for a source of no package's scheme.
        #[no_mangle]
        pub extern "C" fn exact_picture(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.picture(len), |n| n)
        }

        /// The agent's `prefer extension` in the input buffer (LLP 1124 D7).
        #[no_mangle]
        pub extern "C" fn exact_extension(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.extension(len), |n| n)
        }

        /// A module event the platform reported (LLP 1133.004 D6): `{"extension",
        /// "event", "args", "activation"}` in the input buffer.
        #[no_mangle]
        pub extern "C" fn exact_module_event(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.module_event(len), |n| n)
        }

        /// App modes, `axis=value` pins in the input buffer (LLP 1121 D5).
        #[no_mangle]
        pub extern "C" fn exact_set_mode(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.set_mode(len), |n| n)
        }

        /// A live design batch or snapshot in the input buffer (LLP 1121
        /// stage 4); development builds only.
        #[no_mangle]
        pub extern "C" fn exact_design_rows(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.design_rows(len), |n| n)
        }

        /// The binding core (LLP 1121 D12): `{"view", "names"}` in the input
        /// buffer, `{"inputs", "colors"}` UTF-8 in the output buffer.
        #[no_mangle]
        pub extern "C" fn exact_tamber(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.tamber(len), |n| n)
        }

        /// The design digest the running app runs, UTF-8; empty before boot.
        #[no_mangle]
        pub extern "C" fn exact_design_digest(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.design_digest(), |_| 0)
        }

        /// The running plan's digest, UTF-8; empty before boot.
        #[no_mangle]
        pub extern "C" fn exact_plan_digest(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.plan_digest(), |_| 0)
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

        /// A placement root's resolved margins and its width's authorship (LLP 1126.001
        /// WP10): the output holds JSON `[left, top, right, bottom, authored]`, no batch
        /// around it; returns its length.
        #[no_mangle]
        pub extern "C" fn exact_placement_box(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.placement_box(view), |n| n)
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

        /// The layout regions' insets (LLP 1123 D10): `count` is 20 (five
        /// regions of top, right, bottom, left) or 0 (every region is the safe
        /// area); returns the batch's length.
        #[no_mangle]
        pub unsafe extern "C" fn exact_regions(rt: u32, insets: *const f32, count: u32) -> u32 {
            let insets = if count == 0 || insets.is_null() { &[] } else {
                // SAFETY: the caller keeps `count` floats live for this synchronous call.
                unsafe { std::slice::from_raw_parts(insets, count as usize) }
            };
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.regions(insets), |n| n)
        }

        /// The title-bar area beside the window's controls (Window Controls
        /// Overlay): `count` is 4 (x, y, width, height) or 0 (none, so every
        /// `env(titlebar-area-*)` is undefined); returns the batch's length.
        #[no_mangle]
        pub unsafe extern "C" fn exact_titlebar_area(rt: u32, rect: *const f32, count: u32) -> u32 {
            let rect = if count == 0 || rect.is_null() { &[] } else {
                // SAFETY: the caller keeps `count` floats live for this synchronous call.
                unsafe { std::slice::from_raw_parts(rect, count as usize) }
            };
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.titlebar_area(rect), |n| n)
        }

        /// The window's size, which every viewport unit resolves
        /// against everywhere, or none (a nonpositive size), LLP 1075.003
        /// §9.11; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_screen(rt: u32, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.screen(width, height), |n| n)
        }

        /// What changed of the view at once, from the input buffer
        /// (`Bridge::view`): the window's size, the safe-area insets, the
        /// layout regions and the viewport, one layout; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_view(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.view(len), |n| n)
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

        /// Every colour reference the presenter should resolve (LLP 1095
        /// D1), as JSON in the output buffer; returns its length.
        #[no_mangle]
        pub extern "C" fn exact_color_references(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.color_references(), |n| n)
        }

        /// The presenter's colour resolutions from the input buffer (LE
        /// records: u8 kind, u8 dark, u16 id, rgba8); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_colors(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.colors(len), |n| n)
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

        /// A trial layout of a block's root (`Bridge::measure_block`): its border
        /// box and margins, six LE f32s, in the output buffer; 0 when refused.
        /// No batch.
        #[no_mangle]
        pub extern "C" fn exact_measure_block(rt: u32, view: u32, width: f32, height: f32, width_policy: u32, height_policy: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.measure_block(view, [width, height], [width_policy, height_policy]), |_| 0)
        }

        /// What moves whenever a block measure could answer differently
        /// (`Bridge::layout_stamp`): a host keeps measures while it stands.
        #[no_mangle]
        pub extern "C" fn exact_layout_stamp(rt: u32) -> u64 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.layout_stamp(), |_| 0)
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

        /// What UIKit covers of list `view`'s port at its start and end, in
        /// points: a row the runner aligns lands clear of it.
        #[no_mangle]
        pub extern "C" fn exact_list_covered(rt: u32, view: u32, start: f64, end: f64) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.list_covered(view, start, end), |_| ())
        }

        /// Canvas draws in a turn of their own (`deferred` nonzero), or in every
        /// turn (LLP 1072 §8.5): the batch says `canvasOwed` and
        /// `exact_canvas_draw` runs them, off the turns main waits on.
        #[no_mangle]
        pub extern "C" fn exact_canvas_defer(rt: u32, deferred: u32) {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.canvas_defer(deferred != 0), |_| ())
        }

        /// Runtimes booted from now count each plan site's work for `perf`
        /// (LLP 1079 D1) from the first ask, `exact_measure_work`, not from
        /// boot: a release build's choice.
        #[no_mangle]
        pub extern "C" fn exact_measure_work_later() {
            $crate::abi::measure_work_later()
        }

        /// The first ask for `perf`: this runtime counts its work from now.
        #[no_mangle]
        pub extern "C" fn exact_measure_work(rt: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.measure_work(), |n| n)
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

        /// A motion frame for the display frame presented at `frame_ms`
        /// (LLP 1003.001 D5); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_tick_at(rt: u32, now_ms: f64, frame_ms: f64) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.tick_at(now_ms, frame_ms), |n| n)
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

        /// What the presenter applied of the orientation lock (LLP 1133.029
        /// §8): JSON in the input buffer's first `len` bytes, and whether
        /// its resolution found a conflict. Returns 0.
        #[no_mangle]
        pub extern "C" fn exact_orientation_applied(rt: u32, len: usize, conflict: u8) -> u32 {
            $crate::abi::with_runtime(
                &EXACT_RUNTIMES,
                rt,
                true,
                |b, _| b.orientation_applied(len, conflict != 0),
                |_| 0,
            )
        }

        /// An auth session's word (LLP 1069.006): `hold` under the agent, or
        /// `done` with the callback URL or a status; JSON in the input buffer.
        #[no_mangle]
        pub extern "C" fn exact_auth(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, true, |b, _| b.auth(len), |_| 0)
        }

        /// How a presence check the host made ended (LLP 1133.010 D13), JSON
        /// in the input buffer, on the store's completion path.
        #[no_mangle]
        pub extern "C" fn exact_presence(rt: u32, len: usize) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, true, |b, _| b.presence(len), |_| 0)
        }

        /// The keychain the presence backend keeps locked items in (LLP
        /// 1133.010 D6), ExactKit's; once a process, before the first boot.
        #[no_mangle]
        pub extern "C" fn exact_on_presence_keychain(
            keychain: Option<$crate::presence_secrets::KeychainFn>,
        ) {
            $crate::presence_secrets::on_keychain(keychain);
        }

        /// A native button's title and symbol (LLP 1069.011 D5), JSON;
        /// returns its length.
        #[no_mangle]
        pub extern "C" fn exact_press_face(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.press_face(view), |n| n)
        }

        /// A select's options and the one it shows (LLP 1069.001 D5), JSON;
        /// returns its length.
        #[no_mangle]
        pub extern "C" fn exact_select_options(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.select_options(view), |n| n)
        }

        /// A radio's group and the radios its arrows move to (x2apps
        /// survey #2), JSON; returns its length.
        #[no_mangle]
        pub extern "C" fn exact_radio_group(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.radio_group(view), |n| n)
        }

        /// A navigation root's tabs by the kernel's rule (LLP 1115.001
        /// D11), JSON; returns its length.
        #[no_mangle]
        pub extern "C" fn exact_root_tabs(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.root_tabs(view), |n| n)
        }

        /// Whether a stack splits, by the runner's rule (LLP 1115.002 D4),
        /// JSON; returns its length.
        #[no_mangle]
        pub extern "C" fn exact_stack_split(rt: u32, view: u32, retained: u32, pressed: u32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, _| b.stack_split(view, retained, pressed), |n| n)
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

/// `exact_gesture_constant`: a threshold by index, NaN past the end.
pub fn gesture_constant(which: u32) -> f64 {
    exact_motion::gesture::CONSTANTS
        .get(which as usize)
        .copied()
        .unwrap_or(f64::NAN)
}

/// `exact_header_shape`: the kernel's header rule over a host's mirror of a
/// route (`exact_kernel::bars::header_shape_words`), written to `out`; the
/// answer's length, 0 for none or one that does not fit.
pub fn header_shape(words: &[u32], flags: u32, out: &mut [u32]) -> u32 {
    match exact_kernel::bars::header_shape_words(words, flags) {
        Some(shape) if shape.len() <= out.len() => {
            out[..shape.len()].copy_from_slice(&shape);
            shape.len() as u32
        }
        _ => 0,
    }
}

/// `exact_route_bar`: what a route shows of the platform's bar.
pub fn route_bar(facts: u32) -> u32 {
    let bit = |n: u32| facts & (1 << n) != 0;
    exact_kernel::Bar::of(bit(0), bit(1), bit(2), bit(3)) as u32
}

/// The grouped list's export (LLP 1084 D4; LLP 1047.001 D3): beside `host!`,
/// whose runtimes it reads, in an archive that links grouped lists.
#[macro_export]
macro_rules! grouped_list_exports {
    () => {
        /// A grouped list's sections and rows (LLP 1084 D4), JSON; returns
        /// its length.
        #[no_mangle]
        pub extern "C" fn exact_grouped_list(rt: u32, view: u32) -> u32 {
            $crate::abi::with_runtime(
                &EXACT_RUNTIMES,
                rt,
                false,
                |b, _| b.grouped_list(view),
                |n| n,
            )
        }
    };
}
