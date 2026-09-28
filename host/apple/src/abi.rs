//! The C ABI, with no `unsafe` on this side.
//!
//! @ref LLP 1008 §4; `host/apple/include/exact.h` (the header)
//!
//! The host owns the buffers: `exact_in(len)` resizes input and returns its
//! address; each call returns the output length, read via `exact_out()`.
//! Text measurement and the plan font catalog call registered host functions
//! the other way ([`crate::measure`]).
//!
//! Every export takes a runtime handle (LLP 1031 D2): `exact_create` returns
//! a never-reused `u32` from the thread-local [`Registry`], never a pointer.
//! Invalid/destroyed handles are refused; `exact_destroy` frees the session.
//! Calls stay on the presenter's main thread. Re-entrant calls return a `busy`
//! batch instead of trapping. [`host!`] exports one app's source and baked plan;
//! each process links one app archive because the C names are fixed.

use crate::host::{Host, PlanBytes};
use crate::measure::{install_fonts, CallbackMeasurer, FontsFn, MeasureFn};
use crate::store::{endow, snapshot_of, Platform};
use exact_kernel::{MonospaceMeasurer, TextMeasurer};
use exact_runner::{
    DataSource, Event, FailureKind, Outcome, SurfaceOutcome, SurfaceRequest, MAX_HOST_WORK_BYTES,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

/// What the presenter hands the library at boot: the text measurer (LLP
/// 1008 §3) and the wake for a request's reply (LLP 1016 D2), each with an
/// opaque context the library passes back untouched.
#[derive(Clone, Copy)]
pub struct Hooks {
    /// Measures a paragraph; `None` for the monospace reference measurer.
    pub measure: Option<MeasureFn>,
    /// Passed back to `measure`.
    pub ctx: *mut c_void,
    /// Called on the executor's thread when a reply is queued; `None` and
    /// replies wait for the next `exact_pump`.
    pub wake: Option<crate::executor::WakeFn>,
    /// Passed back to `wake`.
    pub wake_ctx: *mut c_void,
    /// Measures a Canvas 2D run with Core Text (LLP 1056 D8), with `ctx`.
    pub canvas_text: Option<crate::canvas_text::CanvasTextFn>,
}

impl Hooks {
    /// No callbacks: the reference measurer, and replies on `pump` only.
    pub const fn none() -> Hooks {
        Hooks {
            measure: None,
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
        }
    }
}

/// The buffers and the host behind the exports.
pub struct Bridge<D: DataSource> {
    host: Option<Host<D>>,
    region: Option<crate::content_region::ContentRegionRegistration>,
    prepared: Option<PreparedHost<D>>,
    painted: bool,
    executor: Option<crate::executor::Executor>,
    refusal_turn: bool,
    fonts: Option<FontsFn>,
    fonts_ctx: *mut c_void,
    /// The archive's `compat.json` (LLP 1030 D3a), from the `host!`
    /// invocation: what the runner's `delivery` resource says about this
    /// binary's cohort, its update store, and its executors.
    compat: Option<&'static str>,
    delivery: Option<&'static crate::delivery::Hooks>,
    /// Requests whose continuation a source held at dispatch (LLP 1027.002
    /// D3): released after a later commit, by token.
    parked: std::collections::BTreeMap<u64, exact_runner::RequestOut>,
    launch: Option<String>,
    /// The session's app module (LLP 1067.000 Q6): installed into each
    /// activated source's native slot, so it outlives activations.
    app_module: Option<exact_runner::NativeHandler>,
    app_call: Option<exact_runner::NativeCall>,
    input: Vec<u8>,
    output: Vec<u8>,
}

struct PreparedHost<D: DataSource> {
    host: Host<D>,
    batch: String,
    bindings: Option<ibex2::host::Bindings>,
    hooks: Hooks,
    module: bool,
}

fn not_booted() -> String {
    "{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"not booted\"}".to_string()
}

impl<D: DataSource> Bridge<D> {
    /// Empty; `boot` fills it.
    pub const fn new() -> Bridge<D> {
        Bridge {
            host: None,
            region: None,
            prepared: None,
            painted: false,
            executor: None,
            refusal_turn: false,
            fonts: None,
            fonts_ctx: std::ptr::null_mut(),
            compat: None,
            delivery: None,
            parked: std::collections::BTreeMap::new(),
            launch: None,
            app_module: None,
            app_call: None,
            input: Vec::new(),
            output: Vec::new(),
        }
    }

    /// Canonical location from the input URL, in the output buffer. @ref LLP 1038 D8
    pub fn location_of(&mut self, len: usize) -> u32 {
        let href = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        self.emit(exact_route::location_of(&href))
    }

    /// A pre-boot location; a live session receives dispatch kind 14 instead.
    pub fn set_launch_location(&mut self, len: usize) {
        if self.host.is_none() {
            self.launch = Some(
                String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned(),
            );
        }
    }

    /// Explicit authored region, used by every subsequent fresh/candidate boot.
    pub fn set_content_region(
        &mut self,
        region: Option<crate::content_region::ContentRegionRegistration>,
    ) {
        self.region = region;
    }

    /// Resize the input buffer and return its address.
    pub fn input(&mut self, len: usize) -> *mut u8 {
        self.input.clear();
        self.input.resize(len, 0);
        self.input.as_mut_ptr()
    }

    /// Write `bytes` into the input buffer (what the app does through the
    /// address `input` returned); the length written.
    pub fn input_write(&mut self, bytes: &[u8]) -> usize {
        self.input.clear();
        self.input.extend_from_slice(bytes);
        self.input.len()
    }

    /// The output buffer's address.
    pub fn output(&self) -> *const u8 {
        self.output.as_ptr()
    }

    /// The output buffer's first `len` bytes.
    pub fn output_bytes(&self, len: usize) -> &[u8] {
        &self.output[..len.min(self.output.len())]
    }

    /// Copy the immutable binary bake receipt to the output buffer.
    pub fn baked_compat(&mut self, compat: &str) -> u32 {
        self.output.clear();
        self.output.extend_from_slice(compat.as_bytes());
        self.output.len() as u32
    }

    /// Register the synchronous plan-font hook used by subsequent boots,
    /// with the context it is handed back.
    pub fn set_fonts(&mut self, fonts: Option<FontsFn>, ctx: *mut c_void) {
        self.fonts = fonts;
        self.fonts_ctx = ctx;
    }

    /// This binary's `compat.json` (LLP 1030 D3a), for the delivery facts
    /// every subsequent boot hands the runner before its first frame. The
    /// `host!` macro passes the app's `COMPAT` const; nothing crosses the C
    /// ABI for it.
    pub fn set_compat(&mut self, json: &'static str) {
        self.compat = Some(json);
    }

    /// Select the linked delivery adapter, or none for a binary-only app.
    pub fn set_delivery(&mut self, hooks: Option<&'static crate::delivery::Hooks>) {
        self.delivery = hooks;
    }

    /// Refuse an analysis bake before constructing app data or invoking hooks.
    /// The exported entrypoints call this before evaluating their app arguments;
    /// direct Bridge calls repeat it before bindings or selection are consulted.
    pub fn refuse_analysis(&mut self) -> Option<u32> {
        let why = exact_runner::delivery::refuse_analysis(self.compat?).err()?;
        Some(self.refuse_preparation(why))
    }

    fn emit(&mut self, mut s: String) -> u32 {
        // Whatever the last call asked the host to run goes to the executor
        // with the batch (LLP 1016 D2); the presenter never sees a request.
        // A continuation is dispatched here, on this thread, after the
        // commit that handed it out (LLP 1027.002 D3); one a source holds
        // is parked and released after a later commit.
        let Bridge {
            host,
            executor,
            parked,
            ..
        } = self;
        if let (Some(h), Some(x)) = (host.as_mut(), executor.as_ref()) {
            x.forget(|ticket| h.runner().holds(ticket));
            if !h.has_ordered_request_refusals() {
                x.resume_ordered();
            }
            let admitted = h.grants();
            let mut presenter = crate::batch::Batch::new();
            Self::auth_forgotten(h, &mut presenter);
            for r in h.take_requests() {
                if r.request.is_auth() {
                    Self::auth_arm(h, x, &mut presenter, &r);
                    continue;
                }
                if let Some(surface) = r.request.surface.as_deref() {
                    let oversized = matches!(surface, SurfaceRequest::Restore { bytes, .. } if bytes.len() > MAX_HOST_WORK_BYTES);
                    let refusal = oversized
                        .then(|| "surface restore exceeds 16 MiB".to_string())
                        .or_else(|| r.request.check_surface_grant(&admitted).err());
                    presenter.surface_work(&r, refusal.as_deref());
                    continue;
                }
                let dispatch = match r.request.continuation {
                    Some(token) => h.dispatch_work(token),
                    None if r.request.is_native() => h.native_work(&r.request),
                    None => {
                        Self::run_dispatch(h, x, parked, r, exact_runner::Dispatch::Missing);
                        continue;
                    }
                };
                Self::run_dispatch(h, x, parked, r, dispatch);
            }
            for (token, dispatch) in h.release_work() {
                if let Some(r) = parked.remove(&token) {
                    Self::run_dispatch(h, x, parked, r, dispatch);
                }
            }
            presenter.prepend_to(&mut s);
        }
        self.output = s.into_bytes();
        self.output.len() as u32
    }

    fn run_dispatch(
        h: &mut Host<D>,
        x: &crate::executor::Executor,
        parked: &mut std::collections::BTreeMap<u64, exact_runner::RequestOut>,
        r: exact_runner::RequestOut,
        dispatch: exact_runner::Dispatch,
    ) {
        let ticket = r.ticket;
        let ordered = r.request.is_ordered();
        let result = match dispatch {
            exact_runner::Dispatch::Run(work) => x.run(r, Some(work)),
            exact_runner::Dispatch::Held => {
                if let Some(token) = r.request.continuation {
                    parked.insert(token, r);
                }
                Ok(())
            }
            exact_runner::Dispatch::Host(_) | exact_runner::Dispatch::Missing => x.run(r, None),
        };
        if let Err(reason) = result {
            h.refuse_request(ticket, reason, ordered);
            x.notify();
        }
    }

    /// The executor's queued outcomes into the runner (LLP 1016 D2): the
    /// presenter calls this on its thread after the wake; the output is the
    /// batch of every reply's commit.
    pub fn pump(&mut self, now_ms: f64) -> u32 {
        self.refusal_turn = !self.refusal_turn;
        let outcomes = match (self.host.as_mut(), self.executor.as_ref()) {
            (Some(host), Some(executor)) => {
                executor.begin_pump();
                let mut outcomes = if self.refusal_turn {
                    host.take_request_refusal(executor.ordered_idle())
                        .into_iter()
                        .collect()
                } else {
                    executor.drain()
                };
                if outcomes.is_empty() {
                    outcomes = if self.refusal_turn {
                        executor.drain()
                    } else {
                        host.take_request_refusal(executor.ordered_idle())
                            .into_iter()
                            .collect()
                    };
                }
                if host.has_request_refusals(executor.ordered_idle()) {
                    executor.notify();
                }
                outcomes
            }
            _ => vec![],
        };
        let out = match self.host.as_mut() {
            Some(h) => h.fulfill_all(outcomes, now_ms),
            None => not_booted(),
        };
        self.emit(out)
    }

    /// Whether the current runner still owns a presenter-work ticket.
    pub fn request_active(&self, ticket: u64) -> bool {
        self.host.as_ref().is_some_and(|host| {
            host.runner()
                .pending()
                .iter()
                .any(|(_, held)| *held == ticket)
        })
    }

    /// A surface operation completed on the presenter thread.
    pub fn fulfill_surface(&mut self, ticket: u64, kind: u32, len: usize, now_ms: f64) -> u32 {
        let body = self.input[..len.min(self.input.len())].to_vec();
        let outcome = match kind {
            6 if body.len() <= MAX_HOST_WORK_BYTES => {
                Outcome::Surface(SurfaceOutcome::Captured(body))
            }
            7 if body.is_empty() => Outcome::Surface(SurfaceOutcome::Restored),
            2..=4 => Outcome::Failed {
                kind: match kind {
                    2 => FailureKind::Refused,
                    4 => FailureKind::Aborted,
                    _ => FailureKind::Unsupported,
                },
                message: String::from_utf8_lossy(&body).into_owned(),
            },
            _ => Outcome::Failed {
                kind: FailureKind::Refused,
                message: "invalid or oversized surface outcome".into(),
            },
        };
        let out = self.host.as_mut().map_or_else(not_booted, |host| {
            host.fulfill_all(vec![(ticket, outcome)], now_ms)
        });
        self.emit(out)
    }

    /// Boot from `plan` with `data`, measuring text through `measure` (or
    /// the monospace reference measurer when none is given) under a
    /// viewport; the output is the first batch.
    pub fn boot(&mut self, plan: &[u8], data: D, hooks: Hooks, width: f32, height: f32) -> u32 {
        self.boot_bytes(PlanBytes::Copied(plan), data, hooks, width, height)
    }

    fn boot_bytes(
        &mut self,
        plan: PlanBytes<'_>,
        data: D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> u32 {
        if let Some(refusal) = self.refuse_analysis() {
            return refusal;
        }
        match self.boot_fresh(plan, data, hooks, width, height) {
            Ok(batch) => self.emit(batch),
            Err(e) => self.emit(format!(
                "{{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"boot: {}\"}}",
                escape(&e)
            )),
        }
    }

    /// `exact_boot`: boot what the update store selected (LLP 1026 D9) —
    /// the selected entry's plan when there is one, else `embedded`, the
    /// bytes baked into the library — counting the boot first (D11). An
    /// entry whose plan is refused at boot boots entry zero in the same
    /// launch, the refusal journaled and the failure left standing in the
    /// record, so first pixel does not bless it. `data` makes the source
    /// for each attempt.
    pub fn boot_selected(
        &mut self,
        embedded: &'static [u8],
        mut data: impl FnMut() -> D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> u32 {
        if let Some(refusal) = self.refuse_analysis() {
            return refusal;
        }
        let Some(delivery) = self.delivery else {
            return self.boot_bytes(PlanBytes::Static(embedded), data(), hooks, width, height);
        };
        let selected = (delivery.selected_plan)();
        if let Some((entry, bytes)) = selected {
            // Selection already verified the stored bytes. Count this attempt
            // even when decoding or booting that verified plan refuses it.
            (delivery.boot_started)();
            let admitted = data();
            let source = (delivery.selected_module)().and_then(|module| match module {
                Some((receipt, module)) => admitted
                    .replacement(&bytes, &receipt, module)
                    .map_err(|e| format!("module generation: {e:?}")),
                None => Ok(admitted),
            });
            match source.and_then(|source| {
                self.boot_fresh(PlanBytes::Copied(&bytes), source, hooks, width, height)
            }) {
                Ok(batch) => return self.emit(batch),
                Err(e) => (delivery.entry_refused)(&entry, &e),
            }
        }
        self.boot_bytes(PlanBytes::Static(embedded), data(), hooks, width, height)
    }

    fn boot_fresh(
        &mut self,
        plan: PlanBytes<'_>,
        data: D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> Result<String, String> {
        if let Some(compat) = self.compat {
            exact_runner::delivery::refuse_analysis(compat).map_err(str::to_string)?;
        }
        let measurer: Box<dyn TextMeasurer> = match hooks.measure {
            Some(f) => Box::new(CallbackMeasurer::new(f, hooks.ctx)),
            None => Box::new(MonospaceMeasurer::default()),
        };
        // The app's bindings, once (LLP 1016 D6; LLP 1018 D6): the secrets it
        // kept are read into a snapshot before the runner boots, so the first
        // frame is a returning user's; the executor thread takes the same
        // bindings for its requests. Build beside any running host: the dev
        // menu may use this fresh-state path to reload the baked plan.
        let (bindings, unbound) = match endow(data.grants()) {
            Ok(b) => (Some(b), None),
            Err(e) => (None, Some(e)),
        };
        let snapshot = snapshot_of(bindings.as_ref());
        let secrets = bindings.as_ref().map(Platform::of);
        let fonts = self.fonts;
        let fonts_ctx = self.fonts_ctx;
        match Host::boot_stored_after_decode(
            plan,
            data,
            measurer,
            width,
            height,
            None,
            snapshot,
            secrets,
            self.compat,
            self.delivery,
            None,
            self.launch.as_deref().unwrap_or("/"),
            self.region,
            move |decoded| {
                if let Some(callback) = fonts {
                    install_fonts(decoded, callback, fonts_ctx);
                }
            },
        ) {
            Ok((mut host, batch)) => {
                if let Some(why) = unbound {
                    host.log(&format!("{why}; every request is refused"));
                }
                host.commit_boot();
                let executor = crate::executor::Executor::start(
                    bindings,
                    &host.grants(),
                    hooks.wake.map(|w| (w, hooks.wake_ctx)),
                );
                host.listen(executor.waker());
                if let Some(f) = hooks.canvas_text {
                    host.set_canvas_text(f, hooks.ctx);
                }
                self.executor = Some(executor);
                self.host = Some(host);
                self.parked.clear();
                Ok(batch)
            }
            Err(e) => Err(format!("{e:?}")),
        }
    }

    /// What the update store has to say, into this runtime's runner (LLP
    /// 1030 D7) — after a check, after an activation; the output is the
    /// batch of the `delivery` resource's re-answer.
    pub fn sync_delivery(&mut self) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.sync_delivery());
        self.emit(out)
    }

    /// The session's app module (LLP 1067.000): `later(ctx, body, len,
    /// reply)` takes each long native call on the executor's thread and
    /// answers it once ([`crate::app_module::reply`]); `call` answers a
    /// `native.call` before it returns ([`crate::app_module::answer`]).
    /// `None` removes each.
    pub fn set_app_module(
        &mut self,
        later: Option<crate::app_module::LaterFn>,
        call: Option<crate::app_module::CallFn>,
        ctx: *mut c_void,
    ) {
        self.app_module = later.map(|later| crate::app_module::handler(later, ctx));
        self.app_call = call.map(|call| crate::app_module::caller(call, ctx));
        self.adopt_app_module();
    }

    /// The app module announced `topic` (LLP 1016.002): the live source's
    /// watching answers are asked again.
    pub fn app_changed(&mut self, topic: &str) {
        if let Some(host) = &self.host {
            host.native_slot().changed(topic);
        }
    }

    fn adopt_app_module(&self) {
        if let Some(host) = &self.host {
            let slot = host.native_slot();
            slot.host(self.app_module.clone());
            slot.host_call(self.app_call.clone());
        }
    }

    /// The presenter has painted this session; deferred logic can now load.
    pub fn data_ready(&mut self) -> u32 {
        if self.host.is_none() {
            return self.emit(not_booted());
        }
        self.painted = true;
        self.adopt_app_module();
        let out = self.host.as_mut().expect("checked").activate_data();
        self.emit(out)
    }

    /// Prepare plan + UTF-8 pairing receipt + compiled module from one input buffer.
    /// This is a development-origin API, not an authenticated update channel.
    pub fn prepare_module(
        &mut self,
        lengths: [usize; 3],
        admitted: D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> u32 {
        self.prepare_module_with_delivery(lengths, admitted, hooks, width, height, None)
    }

    /// Prepare a verified signed generation with its candidate delivery facts.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_module_with_delivery(
        &mut self,
        lengths: [usize; 3],
        admitted: D,
        hooks: Hooks,
        width: f32,
        height: f32,
        delivery: Option<exact_runner::Delivery>,
    ) -> u32 {
        if let Some(refusal) = self.refuse_analysis() {
            return refusal;
        }
        self.discard_plan();
        if self
            .host
            .as_ref()
            .is_some_and(|host| host.runner().has_pending())
        {
            return self.refuse_preparation(
                "Rust/module replacement waits for pending requests; retry after they settle",
            );
        }
        let [plan_len, receipt_len, module_len] = lengths;
        let total = plan_len
            .checked_add(receipt_len)
            .and_then(|n| n.checked_add(module_len));
        if total != Some(self.input.len())
            || plan_len > 32 * 1024 * 1024
            || receipt_len > 1024 * 1024
            || module_len > 32 * 1024 * 1024
        {
            return self.refuse_preparation("invalid module generation lengths");
        }
        let receipt = match std::str::from_utf8(&self.input[plan_len..plan_len + receipt_len]) {
            Ok(text) => text,
            Err(_) => return self.refuse_preparation("module receipt is not UTF-8"),
        };
        let replacement = || {
            admitted.replacement(
                &self.input[..plan_len],
                receipt,
                self.input[plan_len + receipt_len..].to_vec(),
            )
        };
        let data = match replacement() {
            Ok(data) => data,
            Err(error) => return self.refuse_preparation(&format!("module generation: {error:?}")),
        };
        let mut validated = None;
        if self.painted {
            match data.preload() {
                Ok(false) => return self.emit("{\"ops\":[],\"pending\":true}".into()),
                Err(error) => {
                    return self.refuse_preparation(&format!("candidate preload: {error:?}"))
                }
                Ok(true) => {}
            }
            let mut validation = match replacement() {
                Ok(data) => data,
                Err(error) => {
                    return self.refuse_preparation(&format!("module generation: {error:?}"))
                }
            };
            if let Err(error) = validation.activate_for_validation() {
                return self.refuse_preparation(&format!("candidate module: {error:?}"));
            }
            // Validate carried-state answers and layout, without endowing real
            // storage or releasing candidate effects. A refusal keeps the live
            // host. The accepted validation runner is disposable, too.
            let length = self.prepare_plan_with_delivery(
                plan_len,
                validation,
                hooks,
                width,
                height,
                delivery.clone(),
            );
            if self.prepared.is_none() {
                return length;
            }
            let mut carried = self.prepared.as_ref().unwrap().host.carry();
            // Only validated answers cross this boundary. Store effects belong
            // to the committed session, never to the disposable validation pass.
            if let Some(live) = &self.host {
                carried.store = live.carry().store;
            }
            validated = Some(carried);
            self.discard_plan();
        }
        // The real candidate remains deferred through all-session acceptance.
        // After commit, its paint receipt configures storage before activation
        // and data_ready refreshes its baked/kept external-reading resources.
        let length =
            self.prepare_plan_carried(plan_len, data, hooks, width, height, delivery, validated);
        if let Some(prepared) = &mut self.prepared {
            prepared.module = true;
        }
        length
    }

    /// Boot from the input buffer's first `len` bytes (a plan the app
    /// fetched — the dev loop's restart).
    pub fn prepare_plan(
        &mut self,
        len: usize,
        data: D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> u32 {
        self.prepare_plan_with_delivery(len, data, hooks, width, height, None)
    }

    /// Prepare with candidate delivery facts, without publishing them globally.
    pub fn prepare_plan_with_delivery(
        &mut self,
        len: usize,
        data: D,
        hooks: Hooks,
        width: f32,
        height: f32,
        delivery: Option<exact_runner::Delivery>,
    ) -> u32 {
        self.prepare_plan_carried(len, data, hooks, width, height, delivery, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_plan_carried(
        &mut self,
        len: usize,
        data: D,
        hooks: Hooks,
        width: f32,
        height: f32,
        delivery: Option<exact_runner::Delivery>,
        carried: Option<exact_runner::Carried>,
    ) -> u32 {
        if let Some(refusal) = self.refuse_analysis() {
            return refusal;
        }
        self.prepared = None;
        let plan = self.input[..len.min(self.input.len())].to_vec();
        // Build the candidate beside the live host. A decode, app-identity,
        // or runner refusal must not turn a reload into an empty window.
        let carried = carried.or_else(|| self.host.as_ref().map(Host::carry));
        let measurer: Box<dyn TextMeasurer> = match hooks.measure {
            Some(f) => Box::new(CallbackMeasurer::new(f, hooks.ctx)),
            None => Box::new(MonospaceMeasurer::default()),
        };
        // A reload carries the running store (`Carried::store`). A fresh
        // session takes the granted platform snapshot before its first query,
        // just like boot_fresh; neither path releases effects until commit.
        let (bindings, unbound) = match endow(data.grants()) {
            Ok(b) => (Some(b), None),
            Err(e) => (None, Some(e)),
        };
        let snapshot = if carried.is_none() {
            snapshot_of(bindings.as_ref())
        } else {
            Vec::new()
        };
        let secrets = bindings.as_ref().map(Platform::of);
        let fonts = self.fonts;
        let fonts_ctx = self.fonts_ctx;
        match Host::boot_stored_after_decode(
            PlanBytes::Copied(&plan),
            data,
            measurer,
            width,
            height,
            carried.as_ref(),
            snapshot,
            secrets,
            self.compat,
            self.delivery,
            delivery,
            self.launch.as_deref().unwrap_or("/"),
            self.region,
            move |decoded| {
                if let Some(callback) = fonts {
                    install_fonts(decoded, callback, fonts_ctx);
                }
            },
        ) {
            Ok((mut host, batch)) => {
                if let Some(why) = unbound {
                    host.log(&format!("{why}; every request is refused"));
                }
                self.output = batch.as_bytes().to_vec();
                self.prepared = Some(PreparedHost {
                    host,
                    batch,
                    bindings,
                    hooks,
                    module: false,
                });
                self.output.len() as u32
            }
            Err(e) => self.prepare_error(format!(
                "{{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"boot: {}\"}}",
                escape(&format!("{e:?}"))
            )),
        }
    }

    /// Prepare and commit a single session's plan replacement.
    pub fn boot_plan(&mut self, len: usize, data: D, hooks: Hooks, width: f32, height: f32) -> u32 {
        let len = self.prepare_plan(len, data, hooks, width, height);
        if self.prepared.is_some() {
            self.commit_plan()
        } else {
            len
        }
    }

    /// Refuse an invalid composition context without touching the live host.
    pub fn refuse_preparation(&mut self, reason: &str) -> u32 {
        self.prepared = None;
        self.prepare_error(format!("{{\"ops\":[],\"error\":\"{}\"}}", escape(reason)))
    }

    fn prepare_error(&mut self, error: String) -> u32 {
        self.output = error.into_bytes();
        self.output.len() as u32
    }

    /// Commit the already-accepted candidate, without decoding or laying it
    /// out again. The app calls this only after every session prepared.
    pub fn commit_plan(&mut self) -> u32 {
        if self
            .prepared
            .as_ref()
            .is_some_and(|candidate| candidate.module)
            && self
                .host
                .as_ref()
                .is_some_and(|host| host.runner().has_pending())
        {
            self.discard_plan();
            return self.refuse_preparation(
                "Rust/module replacement waits for pending requests; retry after they settle",
            );
        }
        let Some(mut candidate) = self.prepared.take() else {
            return self.prepare_error("{\"ops\":[],\"error\":\"no prepared plan\"}".into());
        };
        candidate.host.commit_boot();
        let executor = crate::executor::Executor::start(
            candidate.bindings,
            &candidate.host.grants(),
            candidate.hooks.wake.map(|w| (w, candidate.hooks.wake_ctx)),
        );
        candidate.host.listen(executor.waker());
        if let Some(f) = candidate.hooks.canvas_text {
            candidate.host.set_canvas_text(f, candidate.hooks.ctx);
        }
        self.executor = Some(executor);
        self.host = Some(candidate.host);
        self.adopt_app_module();
        self.parked.clear();
        self.emit(candidate.batch)
    }

    /// Drop an uncommitted candidate and retain the live host and executor.
    pub fn discard_plan(&mut self) {
        self.prepared = None;
    }

    /// Content has settled ([`Host::trim`]).
    pub fn trim(&mut self) {
        if let Some(host) = self.host.as_mut() {
            host.trim();
        }
    }

    /// Dispatch an event at `now_ms`; `kind` is 0 = press, 1 = change,
    /// 2 = hover in, 3 = hover out, 4 = focus, 5 = blur, 6 = key, 7 = submit,
    /// 8 = load, 9 = message (the payload — a change's text, a key's name,
    /// or a guest message — is the input buffer's first `len` bytes, UTF-8).
    /// Kind 14 is navigate: one UTF-8 location at the navigation root (LLP 1038 D8).
    /// Kind 23 is a text field's `input`; 24 and 25 a checkbox's `change`
    /// and `input`, the payload `true` or `false` (LLP 1069.001 D4).
    pub fn dispatch(&mut self, view: u32, kind: u32, len: usize, now_ms: f64) -> u32 {
        let payload =
            String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        let event = match kind {
            0 => Event::Press,
            1 => Event::Change(payload.into()),
            // @ref LLP 1069.001 D4 — 23 is a text field's `input`; 24 and
            // 25 a checkbox's `change` and `input`, the payload `true`/`false`.
            // @ref LLP 1069.002 D3, D2 — 26 is a file input's `change`, one
            // picked file per line; 27 its `cancel`.
            26 => {
                let Some(files) = exact_runner::Picked::payload(&payload) else {
                    return self.emit(r#"{"ops":[],"error":"invalid picked files"}"#.into());
                };
                Event::Change(exact_runner::ControlValue::Files(files))
            }
            27 => Event::Cancel,
            23 => Event::Input(payload.into()),
            24 | 25 => {
                let Some(on) = Event::checked_payload(&payload) else {
                    return self.emit(r#"{"ops":[],"error":"invalid checked state"}"#.into());
                };
                if kind == 24 {
                    Event::Change(on)
                } else {
                    Event::Input(on)
                }
            }
            2 => Event::Hover(true),
            3 => Event::Hover(false),
            4 => Event::Focus,
            5 => Event::Blur,
            6 => Event::Key(payload),
            7 => Event::Submit,
            8 => Event::Load,
            9 => Event::Message(payload),
            10 => Event::Contextmenu,
            11 => Event::Dblclick,
            12 => Event::Swiperight,
            // The platform's pull-to-refresh control fired.
            22 => Event::Refresh,
            // Scroll, media, pan, selection and pan release (LLP 1057 §10.6).
            13 | 19 | 20 | 21 | 28 => match Event::of_host_kind(kind, &payload) {
                Ok(event) => event,
                Err(error) => return self.emit(format!(r#"{{"ops":[],"error":"{error}"}}"#)),
            },
            // @ref LLP 1038 D8 — the next ABI kind after scroll.
            14 => Event::Navigate(payload),
            15 => {
                let Some(event) = Event::height_release_payload(&payload) else {
                    let out = self.host.as_ref().map_or_else(not_booted, |h| {
                        h.hold_refusal("invalid height release coordinates")
                    });
                    return self.emit(out);
                };
                event
            }
            16 | 17 => {
                let event = if kind == 16 {
                    Event::transform_geometry_payload(&payload)
                } else {
                    Event::transform_release_payload(&payload)
                };
                let Some(event) = event else {
                    let out = self
                        .host
                        .as_ref()
                        .map_or_else(not_booted, |h| h.hold_refusal("invalid transform event"));
                    return self.emit(out);
                };
                event
            }
            // The header's collection move: never the text of a change.
            18 => {
                let Some(event) = self.input.get(..len).and_then(Event::reorder_drop_payload)
                else {
                    return self.emit(r#"{"ops":[],"error":"invalid reorder event"}"#.into());
                };
                event
            }
            _ => {
                return self.emit(format!(
                    r#"{{"ops":[],"error":"unknown event kind {kind}"}}"#
                ))
            }
        };
        let out = match self.host.as_mut() {
            Some(h) => h.dispatch_at(view, event, now_ms),
            None => not_booted(),
        };
        self.emit(out)
    }

    /// Consume the exact120-byte paired transform packet from the owned input buffer.
    pub fn transform_motion(&mut self, len: usize) -> u32 {
        let out = match self.host.as_mut() {
            Some(host) if len == 120 && self.input.len() >= len => {
                host.transform_motion(&self.input[..len])
            }
            Some(host) => format!(
                "{{\"accepted\":false,\"batch\":{}}}",
                host.hold_refusal("malformed transform length")
            ),
            None => format!("{{\"accepted\":false,\"batch\":{}}}", not_booted()),
        };
        self.emit(out)
    }

    /// Start a generic property hold (0 translate, 1 scale, 2 rotate, 3 opacity).
    pub fn hold_begin(&mut self, view: u32, property: u32, now_ms: f64) -> u32 {
        let out = match exact_motion::Property::ALL.get(property as usize) {
            Some(property) => self
                .host
                .as_mut()
                .map_or_else(not_booted, |h| h.hold_begin(view, *property, now_ms)),
            None => self
                .host
                .as_ref()
                .map_or_else(not_booted, |h| h.hold_refusal("unknown motion property")),
        };
        self.emit(out)
    }

    /// Begin an authored header's resolved generational binding.
    pub fn height_drag_begin(&mut self, handle: u64, target: u64, now_ms: f64) -> u32 {
        let key = |packed: u64| exact_kernel::NodeKey {
            index: packed as u32,
            generation: (packed >> 32) as u32,
        };
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.height_drag_begin(key(handle), key(target), now_ms)
        });
        self.emit(out)
    }
    /// Move only a live header/target/token triple.
    pub fn height_drag_update(&mut self, token: u64, height: f64, now_ms: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.height_drag_update(token, height, now_ms));
        self.emit(out)
    }
    /// Apply the final sample and typed release action while held.
    /// At the engine's measured velocity (LLP 1057.001 §3).
    pub fn height_drag_release(&mut self, token: u64, height: f64, now_ms: f64) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.dispatch_height_measured(token, height, now_ms)
        });
        self.emit(out)
    }

    /// Arrange (LLP 1041 §8.5): catch a `reorderFor` handle's row.
    pub fn reorder_begin(&mut self, handle: u32, scroll_top: f64, now_ms: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.reorder_begin(handle, scroll_top, now_ms));
        self.emit(out)
    }
    /// A pointer sample for the live contact `token`.
    pub fn reorder_move(&mut self, token: u64, dy: f64, top: f64, inside: u32, now: f64) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.reorder_move(token, dy, top, inside != 0, now)
        });
        self.emit(out)
    }
    /// The contact ended: drop (nonzero) or cancel.
    #[allow(clippy::too_many_arguments)]
    pub fn reorder_end(
        &mut self,
        token: u64,
        drop: u32,
        dy: f64,
        top: f64,
        inside: u32,
        velocity: f64,
        now: f64,
    ) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.reorder_end(token, drop != 0, dy, top, inside != 0, velocity, now)
        });
        self.emit(out)
    }

    /// Liveness before an authored completion; never advances a clock.
    pub fn has_hold(&self, token: u64) -> bool {
        self.host.as_ref().is_some_and(|h| h.has_hold(token))
    }

    /// Update presentation using a runtime-owned opaque handle.
    pub fn hold_update(&mut self, token: u64, x: f64, y: f64, now_ms: f64) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.hold_update(token, exact_motion::Value::new(x, y), now_ms)
        });
        self.emit(out)
    }

    /// Release (or cancel) a live hold after its authored action.
    /// `cancel`: 0 releases at `vx, vy`, 1 cancels, 2 releases at the
    /// engine's measured velocity (LLP 1057.001 §3).
    pub fn hold_end(&mut self, token: u64, cancel: u32, vx: f64, vy: f64, now_ms: f64) -> u32 {
        let end = match cancel {
            0 => exact_motion::HoldEnd::Release {
                velocity: exact_motion::Value::new(vx, vy),
            },
            2 => {
                let out = self
                    .host
                    .as_mut()
                    .map_or_else(not_booted, |h| h.hold_end_measured(token, now_ms));
                return self.emit(out);
            }
            _ => exact_motion::HoldEnd::Cancel,
        };
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.hold_end(token, end, now_ms));
        self.emit(out)
    }

    /// Move the clock (timers).
    pub fn advance(&mut self, now_ms: f64, until_request: bool) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| match until_request {
                true => h.advance_until_request(now_ms),
                false => h.advance(now_ms),
            });
        self.emit(out)
    }

    /// A name alone clears a surface; name NUL JSON publishes it, even if empty.
    pub fn surface_record(&mut self, len: usize) -> u32 {
        let Ok(text) = std::str::from_utf8(&self.input[..len.min(self.input.len())]) else {
            return self.emit(exact_runner::agent::error("surface record: invalid UTF-8"));
        };
        let (name, json) = text
            .split_once('\0')
            .map_or((text, None), |(name, json)| (name, Some(json)));
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("surface record: not booted"),
            |host| host.surface_record(name, json),
        );
        self.emit(out)
    }

    /// The viewport changed.
    pub fn resize(&mut self, width: f32, height: f32) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.resize(width, height));
        self.emit(out)
    }

    /// The user's display preferences changed or became known (LLP 1061
    /// D4; LLP 1069.000 D1): bit 0 reduced motion, bit 1 reduced
    /// transparency, bit 2 contrast more, bit 3 contrast less, bit 4 a dark
    /// system.
    pub fn set_preferences(&mut self, bits: u32) -> u32 {
        let preferences = exact_runner::Preferences::from_bits(bits);
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_preferences(preferences));
        self.emit(out)
    }

    /// The root font size in points (LLP 1069.000 D3): a relayout.
    pub fn set_root_font_size(&mut self, px: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_root_font_size(px));
        self.emit(out)
    }

    /// The page's facts changed or became known (LLP 1069.000 D2): bit 0
    /// hidden, bit 1 offline, bit 2 a share sheet.
    pub fn set_page(&mut self, bits: u32) -> u32 {
        let page = exact_runner::Page::from_bits(bits);
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_page(page));
        self.emit(out)
    }

    /// The date changed or became known (LLP 1027.000.000).
    pub fn set_time(&mut self, epoch_at_zero: f64, utc_offset: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_time(epoch_at_zero, utc_offset));
        self.emit(out)
    }

    /// The locale and time zone, as `locale NUL timeZone` in the input buffer.
    pub fn set_place(&mut self, len: usize) -> u32 {
        // `locale NUL timeZone`, then `NUL seed` at launch.
        let text = std::str::from_utf8(&self.input[..len.min(self.input.len())]).unwrap_or("");
        let mut fields = text.split('\0');
        let (Some(locale), Some(zone)) = (fields.next(), fields.next()) else {
            return self.emit(exact_runner::agent::error(
                "place: expected locale NUL timeZone",
            ));
        };
        let seed = fields.next().and_then(|s| s.parse::<f64>().ok());
        let (locale, zone) = (locale.to_owned(), zone.to_owned());
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_place(&locale, &zone, seed));
        self.emit(out)
    }

    /// The safe-area insets changed.
    pub fn insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_insets(top, right, bottom, left));
        self.emit(out)
    }

    /// The presenter's appearance, which `light-dark()` paint motion resolves by.
    pub fn scheme(&mut self, dark: bool) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_scheme(dark));
        self.emit(out)
    }

    /// One view's own appearance, where the presenter finds it differs from
    /// the session's (LLP 1062 D4).
    pub fn view_scheme(&mut self, view: u32, dark: bool) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_view_scheme(view, dark));
        self.emit(out)
    }

    /// An image's intrinsic size (pixel counts, one-for-one as points); a
    /// finite width or height ≤ 0 clears it; a non-finite value is refused
    /// by the kernel and comes back as an error.
    pub fn intrinsic(&mut self, view: u32, width: f32, height: f32) -> u32 {
        let clears = width.is_finite() && height.is_finite() && (width <= 0.0 || height <= 0.0);
        let size = (!clears).then_some((width, height));
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_intrinsic(view, size));
        self.emit(out)
    }

    /// `intrinsic` for several views from the input buffer's first `len`
    /// bytes: LE records of (u32 view, f32 width, f32 height), one layout.
    pub fn intrinsics(&mut self, len: usize) -> u32 {
        let bytes = self.input.get(..len).unwrap_or(&[]);
        let out = if bytes.len() % 12 != 0 {
            "{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"intrinsics: truncated record\"}".to_string()
        } else {
            let sizes: Vec<_> = bytes
                .chunks_exact(12)
                .map(|r| {
                    let word = |i: usize| [r[i], r[i + 1], r[i + 2], r[i + 3]];
                    let (w, h) = (f32::from_le_bytes(word(4)), f32::from_le_bytes(word(8)));
                    let clears = w.is_finite() && h.is_finite() && (w <= 0.0 || h <= 0.0);
                    (u32::from_le_bytes(word(0)), (!clears).then_some((w, h)))
                })
                .collect();
            self.host
                .as_mut()
                .map_or_else(not_booted, |h| h.set_intrinsics(&sizes))
        };
        self.emit(out)
    }

    /// The display for Canvas 2D (LLP 1056 D4): its scale and memory, for
    /// every session; a booted host redraws its canvases at the new scale.
    pub fn canvas_display(&mut self, scale: f64, memory: f64) -> u32 {
        crate::host::canvas2d::set_display(scale, memory);
        let out = match self.host.as_mut() {
            Some(h) => h.canvas_display(),
            None => "{\"ops\":[],\"timers\":false,\"motion\":false}".to_string(),
        };
        self.emit(out)
    }

    /// A Canvas 2D image handle (the input buffer's first `len` bytes,
    /// UTF-8) decoded at `width` × `height` pixels, or not (`ok` false):
    /// the canvases that asked draw again (LLP 1056 D9).
    pub fn canvas_image(&mut self, len: usize, width: u32, height: u32, ok: bool) -> u32 {
        let src = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.canvas_image(&src, ok.then_some((width, height)))
        });
        self.emit(out)
    }

    /// A motion frame.
    pub fn tick(&mut self, now_ms: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.tick(now_ms));
        self.emit(out)
    }

    /// An agent request (the input buffer's first `len` bytes, JSON); the
    /// output is the reply, not a batch.
    pub fn agent(&mut self, len: usize) -> u32 {
        let request =
            String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        // `tap @t` / `type @t` answer a held device request (LLP 1069.007
        // D4) before any view is looked up.
        let out = match self.host.as_mut() {
            Some(h) => h.answer_hold(&request).unwrap_or_else(|| h.agent(&request)),
            None => exact_runner::agent::error("not booted"),
        };
        // An answered auth hold is delivered by the next pump (LLP 1069.006 D7).
        if out.contains("\"capability\":\"auth\"") {
            self.executor.as_ref().inspect(|x| x.notify());
        }
        self.emit(out)
    }
}

impl<D: DataSource> Default for Bridge<D> {
    fn default() -> Self {
        Bridge::new()
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A thread-local bridge cell, for the exports.
pub type Cell<D> = RefCell<Bridge<D>>;

/// One runtime the registry holds: its bridge and the hooks it was created
/// with (the measurer and the wake, passed at every boot).
pub struct Entry<D: DataSource> {
    /// The bridge.
    pub bridge: Bridge<D>,
    /// The callbacks given at `exact_create`.
    pub hooks: Hooks,
}

/// Every live runtime on this thread, by handle (LLP 1031 D2). Handles come
/// from one process-wide counter — never 0, never reused, unique across
/// threads even though each thread keeps its own registry — so a late call
/// on a destroyed runtime is refused, never confused with a successor, and
/// a handle from another thread never resolves here by coincidence.
pub struct Registry<D: DataSource> {
    entries: std::collections::HashMap<u32, Rc<RefCell<Entry<D>>>>,
}

/// The process-wide handle counter (see [`Registry`]).
static NEXT_HANDLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

impl<D: DataSource> Default for Registry<D> {
    fn default() -> Self {
        Registry {
            entries: std::collections::HashMap::new(),
        }
    }
}

impl<D: DataSource> Registry<D> {
    /// A new runtime with no callbacks yet; its handle. Exhaustion of the
    /// counter (four billion runtimes) is a `0` the caller must refuse.
    pub fn create(&mut self) -> u32 {
        let rt = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if rt == 0 || rt == u32::MAX {
            return 0;
        }
        self.entries.insert(
            rt,
            Rc::new(RefCell::new(Entry {
                bridge: Bridge::new(),
                hooks: Hooks::none(),
            })),
        );
        rt
    }

    /// Drop a runtime: its runner, executor sender, buffers, and journal go
    /// with it (LLP 1031 D2). `false` when there was no such runtime.
    pub fn destroy(&mut self, rt: u32) -> bool {
        self.entries.remove(&rt).is_some()
    }

    /// The runtime, if it lives.
    pub fn get(&self, rt: u32) -> Option<Rc<RefCell<Entry<D>>>> {
        self.entries.get(&rt).cloned()
    }

    /// How many runtimes live.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether none lives.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

thread_local! {
    /// The refusal a call on a dead or busy runtime answers with: a batch
    /// whose `error` names it, in a buffer no runtime owns.
    static REFUSAL: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Record a refusal for a call that reached no runtime — a handle nobody
/// holds, or one busy with another call — and return its length; the bytes
/// are at [`refusal_ptr`].
pub fn refuse(rt: u32, why: &str) -> u32 {
    REFUSAL.with(|r| {
        let mut r = r.borrow_mut();
        *r = format!(
            "{{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"runtime {rt}: {}\"}}",
            escape(why)
        )
        .into_bytes();
        r.len() as u32
    })
}

/// The same refusal in the agent API's shape (`{"error":…}`, LLP 1012),
/// for `exact_agent` on a dead or busy runtime.
pub fn refuse_agent(rt: u32, why: &str) -> u32 {
    REFUSAL.with(|r| {
        let mut r = r.borrow_mut();
        *r = exact_runner::agent::error(&format!("runtime {rt}: {why}")).into_bytes();
        r.len() as u32
    })
}

/// The last refusal's bytes.
pub fn refusal_ptr() -> *const u8 {
    REFUSAL.with(|r| r.borrow().as_ptr())
}

/// Run `f` on runtime `rt`'s bridge, or refuse: no such runtime, or one
/// already inside a call on this thread (`busy`). `refused` gets the
/// refusal's length; `agent` chooses the agent API's `{"error":…}` shape
/// over a batch's.
pub fn with_runtime<D: DataSource, T>(
    registry: &'static std::thread::LocalKey<RefCell<Registry<D>>>,
    rt: u32,
    agent: bool,
    f: impl FnOnce(&mut Bridge<D>, Hooks) -> T,
    refused: impl FnOnce(u32) -> T,
) -> T {
    let refusal = |why: &str| {
        if agent {
            refuse_agent(rt, why)
        } else {
            refuse(rt, why)
        }
    };
    let entry = registry.with(|r| r.borrow().get(rt));
    let Some(entry) = entry else {
        return refused(refusal("no such runtime (destroyed, or never created)"));
    };
    let mut guard = match entry.try_borrow_mut() {
        Ok(guard) => guard,
        Err(_) => {
            return refused(refusal(
                "busy: a call is already in progress on this runtime",
            ))
        }
    };
    let hooks = guard.hooks;
    let out = f(&mut guard.bridge, hooks);
    drop(guard);
    out
}

/// Run `f` on runtime `rt`'s entry (a setter); silently nothing for a dead
/// or busy runtime — a setter returns nothing, and the next call says why.
pub fn with_entry<D: DataSource>(
    registry: &'static std::thread::LocalKey<RefCell<Registry<D>>>,
    rt: u32,
    f: impl FnOnce(&mut Entry<D>),
) {
    let entry = registry.with(|r| r.borrow().get(rt));
    if let Some(entry) = entry {
        if let Ok(mut e) = entry.try_borrow_mut() {
            f(&mut e);
        }
    }
}

mod exports;

#[path = "abi/commands.rs"]
mod commands;

#[cfg(test)]
#[path = "abi_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "executor_order_tests.rs"]
mod executor_order_tests;

#[cfg(test)]
#[path = "collection_tests.rs"]
mod collection_tests;

#[path = "abi_collections.rs"]
mod collections;

/// `exact_gesture_constant`: a threshold by index, NaN past the end.
pub fn gesture_constant(which: u32) -> f64 {
    exact_motion::gesture::CONSTANTS
        .get(which as usize)
        .copied()
        .unwrap_or(f64::NAN)
}
