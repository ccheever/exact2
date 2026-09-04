//! The C ABI, with no `unsafe` on this side.
//!
//! @ref LLP 1008 §4; `host/apple/include/exact.h` (the header)
//!
//! The web host's buffer discipline over `extern "C"`: the app never hands
//! the host a pointer the host did not give out. `exact_in(len)` resizes a
//! host-owned input buffer and returns its address; the app writes a payload
//! there; every call returns the length of the output buffer, whose address
//! `exact_out()` reports; the app reads a UTF-8 JSON batch from it. Text
//! measurement and the plan font catalog are the calls the other way:
//! functions the app registers when it creates a runtime
//! ([`crate::measure`]).
//!
//! **Every export takes a runtime handle** (LLP 1031 D2): `exact_create`
//! hands out a `u32` into a thread-local [`Registry`] — never a pointer, so
//! nothing here is `unsafe` and a destroyed or invented handle is refused by
//! name instead of being undefined — and `exact_destroy` frees everything
//! attributable to it. Handles are never reused. All calls for one runtime
//! are on one thread (the presenter's main thread); a re-entrant call — one
//! made while another is in progress on the same runtime, from a callback —
//! is refused with a `busy` batch rather than trapping. [`host!`]
//! instantiates the exports for one app: its data source and its baked plan
//! bytes; one app archive per process, since the C names are fixed.

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
    prepared: Option<PreparedHost<D>>,
    executor: Option<crate::executor::Executor>,
    fonts: Option<FontsFn>,
    fonts_ctx: *mut c_void,
    /// The archive's `compat.json` (LLP 1030 D3a), from the `host!`
    /// invocation: what the runner's `delivery` resource says about this
    /// binary's cohort, its update store, and its executors.
    compat: Option<&'static str>,
    delivery: Option<&'static crate::delivery::Hooks>,
    input: Vec<u8>,
    output: Vec<u8>,
}

struct PreparedHost<D: DataSource> {
    host: Host<D>,
    batch: String,
    bindings: Option<ibex2::host::Bindings>,
    hooks: Hooks,
}

fn not_booted() -> String {
    "{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"not booted\"}".to_string()
}

impl<D: DataSource> Bridge<D> {
    /// Empty; `boot` fills it.
    pub const fn new() -> Bridge<D> {
        Bridge {
            host: None,
            prepared: None,
            executor: None,
            fonts: None,
            fonts_ctx: std::ptr::null_mut(),
            compat: None,
            delivery: None,
            input: Vec::new(),
            output: Vec::new(),
        }
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

    fn emit(&mut self, s: String) -> u32 {
        // Whatever the last call asked the host to run goes to the executor
        // with the batch (LLP 1016 D2); the presenter never sees a request.
        if let (Some(h), Some(x)) = (self.host.as_mut(), self.executor.as_ref()) {
            for r in h.take_requests() {
                x.run(r);
            }
        }
        self.output = s.into_bytes();
        self.output.len() as u32
    }

    /// The executor's queued outcomes into the runner (LLP 1016 D2): the
    /// presenter calls this on its thread after the wake; the output is the
    /// batch of every reply's commit.
    pub fn pump(&mut self, now_ms: f64) -> u32 {
        let outcomes = self
            .executor
            .as_ref()
            .map(|x| x.drain())
            .unwrap_or_default();
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
        let Some(delivery) = self.delivery else {
            return self.boot(embedded, data(), hooks, width, height);
        };
        let selected = (delivery.selected_plan)();
        if let Some((entry, bytes)) = selected {
            match self.boot_fresh(&bytes, data(), hooks, width, height) {
                Ok(batch) => {
                    (delivery.boot_started)();
                    return self.emit(batch);
                }
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
                    hooks.wake.map(|w| (w, hooks.wake_ctx)),
                ));
                self.host = Some(host);
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
        self.prepared = None;
        let plan = self.input[..len.min(self.input.len())].to_vec();
        // Build the candidate beside the live host. A decode, app-identity,
        // or runner refusal must not turn a reload into an empty window.
        let carried = self.host.as_ref().map(Host::carry);
        let measurer: Box<dyn TextMeasurer> = match hooks.measure {
            Some(f) => Box::new(CallbackMeasurer::new(f, hooks.ctx)),
            None => Box::new(MonospaceMeasurer::default()),
        };
        // A reload carries the running store (`Carried::store`); the bindings
        // are endowed afresh for the new executor.
        let bindings = endow(data.grants());
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
            Vec::new(),
            secrets,
            self.compat,
            self.delivery,
            delivery,
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
        let Some(mut candidate) = self.prepared.take() else {
            return self.prepare_error("{\"ops\":[],\"error\":\"no prepared plan\"}".into());
        };
        candidate.host.commit_boot();
        self.executor = Some(crate::executor::Executor::start(
            candidate.bindings,
            candidate.hooks.wake.map(|w| (w, candidate.hooks.wake_ctx)),
        ));
        self.host = Some(candidate.host);
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
            _ => Event::Change(payload),
        };
        let out = match self.host.as_mut() {
            Some(h) => h.dispatch_at(view, event, now_ms),
            None => not_booted(),
        };
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
/// `$data` is the app's `DataSource` type (constructed with `Default`);
/// `$plan` a `&'static [u8]` of baked plan bytes. Every export takes the
/// runtime handle `exact_create` returned (LLP 1031 D2).
#[macro_export]
macro_rules! host {
    ($data:ty, $plan:expr, $compat:expr) => {
        $crate::host!($data, $plan, $compat, None, ::std::ptr::null());
    };
    ($data:ty, $plan:expr, $compat:expr, $delivery:expr, $api:expr) => {
        thread_local! {
            static EXACT_RUNTIMES: ::std::cell::RefCell<$crate::abi::Registry<$data>> = ::std::cell::RefCell::new($crate::abi::Registry::default());
        }

        /// Create a runtime; returns its handle (never 0). Its callbacks are
        /// set with `exact_set_measure`, `exact_set_wake`, and
        /// `exact_set_fonts` before its first boot.
        #[no_mangle]
        pub extern "C" fn exact_create() -> u32 {
            EXACT_RUNTIMES.with(|r| r.borrow_mut().create())
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

        /// Boot the selected plan — the update store's entry when one is
        /// selected (LLP 1026 D9), else the baked one; returns the first
        /// batch's length.
        #[no_mangle]
        pub extern "C" fn exact_boot(rt: u32, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| {
                b.set_compat($compat);
                b.set_delivery($delivery);
                b.boot_selected($plan, || <$data as ::std::default::Default>::default(), hooks, width, height)
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
                b.set_delivery($delivery);
                b.boot_plan(len, <$data as ::std::default::Default>::default(), hooks, width, height)
            }, |n| n)
        }

        /// Prepare one session, optionally using a composition-owned generation.
        #[no_mangle]
        pub extern "C" fn exact_prepare_plan(rt: u32, token: u64, len: usize, width: f32, height: f32) -> u32 {
            $crate::abi::with_runtime(&EXACT_RUNTIMES, rt, false, |b, hooks| {
                b.set_compat($compat);
                b.set_delivery($delivery);
                let delivery: ::std::option::Option<&'static $crate::delivery::Hooks> = $delivery;
                let facts = delivery.and_then(|h| (h.candidate_delivery)(token, $compat));
                if token != 0 && facts.is_none() { return b.refuse_preparation("unknown composition generation"); }
                b.prepare_plan_with_delivery(len, <$data as ::std::default::Default>::default(), hooks, width, height, facts)
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
    };
}
