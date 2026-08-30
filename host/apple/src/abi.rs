//! The C ABI, with no `unsafe` on this side.
//!
//! @ref LLP 1008 §4; `host/apple/include/exact.h` (the header)
//!
//! The web host's buffer discipline over `extern "C"`: the app never hands
//! the host a pointer the host did not give out. `exact_in(len)` resizes a
//! host-owned input buffer and returns its address; the app writes a payload
//! there; every call returns the length of the output buffer, whose address
//! `exact_out()` reports; the app reads a UTF-8 JSON batch from it. Text
//! measurement is the one call the other way: a function the app registers
//! at boot ([`crate::measure`]). All calls are on one thread (the main
//! thread); the bridge is thread-local. [`host!`] instantiates the exports
//! for one app: its data source and its baked plan bytes.

use crate::host::Host;
use crate::measure::{CallbackMeasurer, MeasureFn};
use exact_kernel::{MonospaceMeasurer, TextMeasurer};
use exact_runner::{DataSource, Event};
use std::cell::RefCell;
use std::ffi::c_void;

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
    executor: Option<crate::executor::Executor>,
    input: Vec<u8>,
    output: Vec<u8>,
}

fn not_booted() -> String {
    "{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"not booted\"}".to_string()
}

impl<D: DataSource> Bridge<D> {
    /// Empty; `boot` fills it.
    pub const fn new() -> Bridge<D> {
        Bridge {
            host: None,
            executor: None,
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
        let measurer: Box<dyn TextMeasurer> = match hooks.measure {
            Some(f) => Box::new(CallbackMeasurer::new(f, hooks.ctx)),
            None => Box::new(MonospaceMeasurer::default()),
        };
        self.host = None;
        self.executor = None;
        match Host::boot(plan, data, measurer, width, height) {
            Ok((mut host, batch)) => {
                let grants = host.grants();
                self.executor = Some(crate::executor::Executor::start(
                    &grants,
                    hooks.wake.map(|w| (w, hooks.wake_ctx)),
                ));
                self.host = Some(host);
                self.emit(batch)
            }
            Err(e) => self.emit(format!(
                "{{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"boot: {}\"}}",
                escape(&format!("{e:?}"))
            )),
        }
    }

    /// Boot from the input buffer's first `len` bytes (a plan the app
    /// fetched — the dev loop's restart).
    pub fn boot_plan(&mut self, len: usize, data: D, hooks: Hooks, width: f32, height: f32) -> u32 {
        let plan = self.input[..len.min(self.input.len())].to_vec();
        let carried = self.host.take().map(|h| h.carry());
        let measurer: Box<dyn TextMeasurer> = match hooks.measure {
            Some(f) => Box::new(CallbackMeasurer::new(f, hooks.ctx)),
            None => Box::new(MonospaceMeasurer::default()),
        };
        match Host::boot_with(&plan, data, measurer, width, height, carried.as_ref()) {
            Ok((mut host, batch)) => {
                let grants = host.grants();
                self.executor = Some(crate::executor::Executor::start(
                    &grants,
                    hooks.wake.map(|w| (w, hooks.wake_ctx)),
                ));
                self.host = Some(host);
                self.emit(batch)
            }
            Err(e) => self.emit(format!(
                "{{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"boot: {}\"}}",
                escape(&format!("{e:?}"))
            )),
        }
    }

    /// Dispatch an event at `now_ms`; `kind` is 0 = press, 1 = change,
    /// 2 = hover in, 3 = hover out, 4 = focus, 5 = blur, 6 = key (the payload
    /// — a change's text, a key's name — is the input buffer's first `len`
    /// bytes, UTF-8).
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

/// Instantiate the C exports for one app (see `include/exact.h`).
///
/// `$data` is the app's `DataSource` type (constructed with `Default`);
/// `$plan` a `&'static [u8]` of baked plan bytes.
#[macro_export]
macro_rules! host {
    ($data:ty, $plan:expr) => {
        thread_local! {
            static EXACT_BRIDGE: $crate::abi::Cell<$data> = ::std::cell::RefCell::new($crate::abi::Bridge::new());
        }

        /// Resize the input buffer; returns its address.
        #[no_mangle]
        pub extern "C" fn exact_in(len: usize) -> *mut u8 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().input(len))
        }

        /// The output buffer's address.
        #[no_mangle]
        pub extern "C" fn exact_out() -> *const u8 {
            EXACT_BRIDGE.with(|b| b.borrow().output())
        }

        /// Boot the baked plan; returns the first batch's length.
        #[no_mangle]
        pub extern "C" fn exact_boot(
            measure: ::std::option::Option<$crate::measure::MeasureFn>,
            ctx: *mut ::std::ffi::c_void,
            wake: ::std::option::Option<$crate::executor::WakeFn>,
            wake_ctx: *mut ::std::ffi::c_void,
            width: f32,
            height: f32,
        ) -> u32 {
            EXACT_BRIDGE.with(|b| {
                b.borrow_mut().boot(
                    $plan,
                    <$data as ::std::default::Default>::default(),
                    $crate::abi::Hooks { measure, ctx, wake, wake_ctx },
                    width,
                    height,
                )
            })
        }

        /// The executor's queued replies into the runner (LLP 1016 D2), on
        /// this thread, after a wake; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_pump(now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().pump(now_ms))
        }

        /// Boot from plan bytes in the input buffer; returns the first batch's length.
        #[no_mangle]
        pub extern "C" fn exact_boot_plan(
            len: usize,
            measure: ::std::option::Option<$crate::measure::MeasureFn>,
            ctx: *mut ::std::ffi::c_void,
            wake: ::std::option::Option<$crate::executor::WakeFn>,
            wake_ctx: *mut ::std::ffi::c_void,
            width: f32,
            height: f32,
        ) -> u32 {
            EXACT_BRIDGE.with(|b| {
                b.borrow_mut().boot_plan(
                    len,
                    <$data as ::std::default::Default>::default(),
                    $crate::abi::Hooks { measure, ctx, wake, wake_ctx },
                    width,
                    height,
                )
            })
        }

        /// Dispatch an event; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_dispatch(view: u32, kind: u32, len: usize, now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().dispatch(view, kind, len, now_ms))
        }

        /// Move the clock; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_advance(now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().advance(now_ms))
        }

        /// The viewport changed; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_resize(width: f32, height: f32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().resize(width, height))
        }

        /// An image loaded (or failed: a size ≤ 0); returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_intrinsic(view: u32, width: f32, height: f32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().intrinsic(view, width, height))
        }

        /// A motion frame; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_tick(now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().tick(now_ms))
        }

        /// An agent request from the input buffer; returns the reply's length.
        #[no_mangle]
        pub extern "C" fn exact_agent(len: usize) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().agent(len))
        }
    };
}
