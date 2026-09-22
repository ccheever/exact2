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

use crate::host::Host;
use crate::measure::{install_fonts, CallbackMeasurer, FontsFn, MeasureFn};
use crate::store::{endow, snapshot_of};
use exact_kernel::{MonospaceMeasurer, TextMeasurer};
use exact_runner::{DataSource, Event};
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
}

impl Hooks {
    /// No callbacks: the reference measurer, and replies on `pump` only.
    pub const fn none() -> Hooks {
        Hooks {
            measure: None,
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
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

    fn emit(&mut self, s: String) -> u32 {
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
            if !h.has_ordered_request_refusals() {
                x.resume_ordered();
            }
            for r in h.take_requests() {
                let dispatch = match r.request.continuation {
                    Some(token) => h.dispatch_work(token),
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

    /// Boot from `plan` with `data`, measuring text through `measure` (or
    /// the monospace reference measurer when none is given) under a
    /// viewport; the output is the first batch.
    pub fn boot(&mut self, plan: &[u8], data: D, hooks: Hooks, width: f32, height: f32) -> u32 {
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
        embedded: &[u8],
        mut data: impl FnMut() -> D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> u32 {
        if let Some(refusal) = self.refuse_analysis() {
            return refusal;
        }
        let Some(delivery) = self.delivery else {
            return self.boot(embedded, data(), hooks, width, height);
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
            match source.and_then(|source| self.boot_fresh(&bytes, source, hooks, width, height)) {
                Ok(batch) => return self.emit(batch),
                Err(e) => (delivery.entry_refused)(&entry, &e),
            }
        }
        self.boot(embedded, data(), hooks, width, height)
    }

    fn boot_fresh(
        &mut self,
        plan: &[u8],
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
        let bindings = endow(data.grants());
        let snapshot = snapshot_of(bindings.as_ref());
        let secrets = bindings.as_ref().map(|b| b.secrets.clone());
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
                host.commit_boot();
                self.executor = Some(crate::executor::Executor::start(
                    bindings,
                    &host.grants(),
                    hooks.wake.map(|w| (w, hooks.wake_ctx)),
                ));
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

    /// The presenter has painted this session; deferred logic can now load.
    pub fn data_ready(&mut self) -> u32 {
        if self.host.is_none() {
            return self.emit(not_booted());
        }
        self.painted = true;
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
        let bindings = endow(data.grants());
        let snapshot = if carried.is_none() {
            snapshot_of(bindings.as_ref())
        } else {
            Vec::new()
        };
        let secrets = bindings.as_ref().map(|b| b.secrets.clone());
        let fonts = self.fonts;
        let fonts_ctx = self.fonts_ctx;
        match Host::boot_stored_after_decode(
            &plan,
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
            Ok((host, batch)) => {
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
        self.executor = Some(crate::executor::Executor::start(
            candidate.bindings,
            &candidate.host.grants(),
            candidate.hooks.wake.map(|w| (w, candidate.hooks.wake_ctx)),
        ));
        self.host = Some(candidate.host);
        self.parked.clear();
        self.emit(candidate.batch)
    }

    /// Drop an uncommitted candidate and retain the live host and executor.
    pub fn discard_plan(&mut self) {
        self.prepared = None;
    }

    /// Dispatch an event at `now_ms`; `kind` is 0 = press, 1 = change,
    /// 2 = hover in, 3 = hover out, 4 = focus, 5 = blur, 6 = key, 7 = submit,
    /// 8 = load, 9 = message (the payload — a change's text, a key's name,
    /// or a guest message — is the input buffer's first `len` bytes, UTF-8).
    /// Kind 14 is navigate: one UTF-8 location at the navigation root (LLP 1038 D8).
    pub fn dispatch(&mut self, view: u32, kind: u32, len: usize, now_ms: f64) -> u32 {
        let payload =
            String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        let event = match kind {
            0 => Event::Press,
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
            13 => {
                let Some(event) = Event::scroll_payload(&payload) else {
                    return self
                        .emit(r#"{"ops":[],"error":"invalid scroll coordinates"}"#.to_string());
                };
                event
            }
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
            19 => {
                let Some(event) = Event::media_payload(&payload) else {
                    return self.emit(r#"{"ops":[],"error":"invalid media event"}"#.into());
                };
                event
            }
            20 => {
                let Some(event) = Event::pan_payload(&payload) else {
                    return self.emit(r#"{"ops":[],"error":"invalid pan deltas"}"#.into());
                };
                event
            }
            _ => Event::Change(payload),
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
    pub fn height_drag_release(
        &mut self,
        token: u64,
        height: f64,
        velocity: f64,
        now_ms: f64,
    ) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.dispatch_height_held(token, height, velocity, now_ms)
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
    pub fn hold_end(&mut self, token: u64, cancel: bool, vx: f64, vy: f64, now_ms: f64) -> u32 {
        let end = if cancel {
            exact_motion::HoldEnd::Cancel
        } else {
            exact_motion::HoldEnd::Release {
                velocity: exact_motion::Value::new(vx, vy),
            }
        };
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.hold_end(token, end, now_ms));
        self.emit(out)
    }

    /// Move the clock (timers).
    pub fn advance(&mut self, now_ms: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.advance(now_ms));
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

    /// The safe-area insets changed.
    pub fn insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_insets(top, right, bottom, left));
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
        let out = match self.host.as_ref() {
            Some(h) => h.agent(&request),
            None => exact_runner::agent::error("not booted"),
        };
        self.emit(out)
    }

    /// A host line into the runner's journal (`exact_log`).
    pub fn log(&mut self, len: usize) -> u32 {
        let line = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        if let Some(h) = self.host.as_mut() {
            h.log(&line);
        }
        0
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

        /// The viewport changed; returns the batch's length.
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
