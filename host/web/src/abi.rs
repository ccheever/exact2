//! The wasm ABI, with no `unsafe`.
//!
//! @ref LLP 1007 §3
//!
//! The exports include `exact_plan` for the build's exact baked bytes,
//! `exact_boot_plan` for the dev loop, `exact_fonts` for plan-owned font
//! catalog data, and `exact_agent` for LLP 1012's agent API.
//! The glue never hands the host a pointer it did not get from
//! the host: `exact_in(len)` resizes a host-owned input buffer and returns its
//! address; the glue writes the payload there; every call returns the length
//! of the output buffer, whose address `exact_out()` reports. Both buffers
//! are plain `Vec<u8>`s in a thread-local; wasm is single-threaded.
//!
//! The macro [`host!`] instantiates these exports for one app: its data source
//! and its baked plan bytes. An app's wasm crate is one line.

use crate::host::Host;
use exact_runner::{DataSource, Event};
use std::cell::RefCell;

/// The buffers and the host behind the exports.
pub struct Bridge<D: DataSource> {
    host: Option<Host<D>>,
    /// The page's snapshot of the app's kept secrets (LLP 1018 D6), handed
    /// in through `exact_store` before boot and taken by the next boot.
    snapshot: Vec<(String, String)>,
    /// The archive's `compat.json` (LLP 1030 D3a), from the `host!`
    /// invocation: what the runner's `delivery` resource says about this
    /// binary's cohort, its update store, and its executors.
    compat: Option<&'static str>,
    input: Vec<u8>,
    output: Vec<u8>,
}

impl<D: DataSource> Bridge<D> {
    /// Empty; `boot` fills it.
    pub const fn new() -> Bridge<D> {
        Bridge {
            host: None,
            snapshot: Vec::new(),
            compat: None,
            input: Vec::new(),
            output: Vec::new(),
        }
    }

    /// This wasm's `compat.json` (LLP 1030 D3a), for the delivery facts
    /// every subsequent boot hands the runner before its first frame. The
    /// `host!` macro passes the app's `COMPAT` const; nothing crosses the
    /// wasm ABI for it.
    pub fn set_compat(&mut self, json: &'static str) {
        self.compat = Some(json);
    }

    /// The page's snapshot of the app's kept secrets (LLP 1018 D6): the
    /// input buffer's first `len` bytes as `name NUL value NUL …`, taken by
    /// the next `boot` (a `boot_plan` carries the running store instead).
    pub fn store(&mut self, len: usize) {
        let bytes = &self.input[..len.min(self.input.len())];
        let text = String::from_utf8_lossy(bytes);
        let mut parts = text.split('\0');
        let mut snapshot = Vec::new();
        while let (Some(name), Some(value)) = (parts.next(), parts.next()) {
            if !name.is_empty() {
                snapshot.push((name.to_string(), value.to_string()));
            }
        }
        self.snapshot = snapshot;
    }

    /// Resize the input buffer and return its address.
    pub fn input(&mut self, len: usize) -> *mut u8 {
        self.input.clear();
        self.input.resize(len, 0);
        self.input.as_mut_ptr()
    }

    /// The output buffer's address.
    pub fn output(&self) -> *const u8 {
        self.output.as_ptr()
    }

    /// Write `bytes` into the input buffer (what the glue does through the
    /// address `input` returned); the length written.
    pub fn input_write(&mut self, bytes: &[u8]) -> usize {
        self.input.clear();
        self.input.extend_from_slice(bytes);
        self.input.len()
    }

    /// The output buffer's first `len` bytes.
    pub fn output_bytes(&self, len: usize) -> &[u8] {
        &self.output[..len.min(self.output.len())]
    }

    fn emit(&mut self, s: String) -> u32 {
        self.output = s.into_bytes();
        self.output.len() as u32
    }

    /// Copy the plan baked into an app wasm into the output buffer. The web
    /// build extracts this after linking, so its app.plan cannot come from a
    /// different source snapshot than the plan `exact_boot` will use.
    pub fn baked_plan(&mut self, plan: &[u8]) -> u32 {
        self.output.clear();
        self.output.extend_from_slice(plan);
        self.output.len() as u32
    }

    /// Binary-admitted module metadata, without activating any logic.
    pub fn logic_info(&mut self, data: D) -> u32 {
        if let Some(revision) = data.revision() {
            let mut json = String::from("{");
            for (i, (key, value)) in [
                ("appId", data.app_id()),
                ("grants", data.grants()),
                ("revision", revision),
            ]
            .iter()
            .enumerate()
            {
                if i > 0 {
                    json.push(',');
                }
                exact_runner::agent::quote(key, &mut json);
                json.push(':');
                exact_runner::agent::quote(value, &mut json);
            }
            json.push('}');
            self.emit(json)
        } else {
            self.emit("null".into())
        }
    }

    /// Activate after first pixel; no-op for binary-bound sources.
    pub fn data_ready(&mut self) -> u32 {
        let batch = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            Host::data_ready,
        );
        self.emit(batch)
    }

    /// The input concatenates plan, pairing receipt, and browser environment id.
    /// The JS loader prepares that private environment before this synchronous swap.
    pub fn boot_module(&mut self, lengths: [usize; 3], admitted: D) -> u32 {
        if self
            .host
            .as_ref()
            .is_some_and(|host| host.runner().has_pending())
        {
            return self.emit(exact_runner::agent::error(
                "module replacement waits for in-flight requests to settle; retry the update",
            ));
        }
        let [plan, receipt, module] = lengths;
        if plan
            .checked_add(receipt)
            .and_then(|n| n.checked_add(module))
            != Some(self.input.len())
            || plan > 32 << 20
            || receipt > 1 << 20
            || module > 32 << 20
        {
            return self.emit(exact_runner::agent::error(
                "invalid module generation lengths",
            ));
        }
        let result = std::str::from_utf8(&self.input[plan..plan + receipt])
            .map_err(|e| e.to_string())
            .and_then(|receipt_text| {
                admitted
                    .replacement(
                        &self.input[..plan],
                        receipt_text,
                        self.input[plan + receipt..].to_vec(),
                    )
                    .map_err(|e| format!("{e:?}"))
            });
        let mut data = match result {
            Ok(data) => data,
            Err(error) => return self.emit(exact_runner::agent::error(&error)),
        };
        if let Err(error) = data.activate() {
            return self.emit(exact_runner::agent::error(&format!("{error:?}")));
        }
        self.boot_plan(plan, data)
    }

    /// Boot from `plan` with `data` and the snapshot `store` handed in; the
    /// output is the first batch.
    pub fn boot(&mut self, plan: &[u8], data: D) -> u32 {
        let snapshot = std::mem::take(&mut self.snapshot);
        match Host::boot_delivered(plan, data, None, snapshot, self.compat) {
            Ok((host, batch)) => {
                self.host = Some(host);
                self.emit(batch)
            }
            Err(e) => self.emit(format!(
                "{{\"ops\":[],\"timers\":false,\"error\":\"boot: {}\"}}",
                escape(&format!("{e:?}"))
            )),
        }
    }

    /// Boot from the input buffer's first `len` bytes — the dev loop's
    /// restart from a freshly compiled plan. Build the candidate beside the
    /// live host: only a successful boot replaces it, while a refusal leaves
    /// the old runner available to its page and in-flight work.
    pub fn boot_plan(&mut self, len: usize, data: D) -> u32 {
        let plan = self.input[..len.min(self.input.len())].to_vec();
        let carried = self.host.as_ref().map(Host::carry);
        match Host::boot_delivered(&plan, data, carried.as_ref(), Vec::new(), self.compat) {
            Ok((host, batch)) => {
                self.host = Some(host);
                self.emit(batch)
            }
            Err(e) => self.emit(format!(
                "{{\"ops\":[],\"timers\":false,\"error\":\"boot: {}\"}}",
                escape(&format!("{e:?}"))
            )),
        }
    }

    /// Inspect candidate fonts while the live host continues to run.
    pub fn plan_fonts(&mut self, len: usize) -> u32 {
        let bytes = &self.input[..len.min(self.input.len())];
        match crate::host::plan_font_catalog(bytes) {
            Ok(catalog) => self.emit(catalog),
            Err(error) => self.emit(format!("{{\"error\":\"{}\"}}", escape(&error.to_string()))),
        }
    }

    /// Query the current plan's declared face catalog separately from the
    /// operation batch returned by boot and dispatch calls.
    pub fn fonts(&mut self) -> u32 {
        let out = self
            .host
            .as_ref()
            .map_or_else(|| "[]".to_string(), |host| host.font_catalog().to_string());
        self.emit(out)
    }

    /// Dispatch an event at `now_ms` (the page's clock); `kind` is 0 = press,
    /// 1 = change, 2 = hover in, 3 = hover out, 4 = focus, 5 = blur, 6 = key,
    /// 7 = submit, 8 = load, 9 = message (the payload — a change's text, a
    /// key's name, or a guest message — is the input buffer's first `len`
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
            _ => Event::Change(payload),
        };
        let out = match self.host.as_mut() {
            Some(h) => h.dispatch_at(view, event, now_ms),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
        self.emit(out)
    }

    /// A request's outcome from the page (LLP 1016 D2): the input buffer
    /// holds `hlen` bytes of header lines, then `blen` bytes of body.
    pub fn fulfill(
        &mut self,
        ticket: f64,
        kind: u32,
        status: u32,
        hlen: usize,
        blen: usize,
        now_ms: f64,
    ) -> u32 {
        let n = self.input.len();
        let hlen = hlen.min(n);
        let blen = blen.min(n - hlen);
        let headers = String::from_utf8_lossy(&self.input[..hlen]).into_owned();
        let body = self.input[hlen..hlen + blen].to_vec();
        let out = match self.host.as_mut() {
            Some(h) => h.fulfill_at(ticket as u64, kind, status, &headers, body, now_ms),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
        self.emit(out)
    }

    /// Move the clock.
    pub fn advance(&mut self, now_ms: f64) -> u32 {
        let out = match self.host.as_mut() {
            Some(h) => h.advance(now_ms),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
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

    /// A page line into the runner's journal (`exact_log`).
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

/// Instantiate the web exports for one app.
///
/// `$data` is the app's `DataSource` type (constructed with `Default`);
/// `$plan` a `&'static [u8]` of baked plan bytes (typically `include_bytes!`
/// of what the app's `build.rs` wrote).
/// A fourth argument supplies a data factory; a fifth supplies the paired
/// module's `[receipt, browser script, native bytecode]` byte slices for bake extraction.
#[macro_export]
macro_rules! host {
    ($data:ty, $plan:expr, $compat:expr, $new:expr, $module:expr) => {
        $crate::host!($data, $plan, $compat, $new);
        /// Copy one exact embedded module artifact for the web producer.
        #[no_mangle]
        pub extern "C" fn exact_module_artifact(index: u32) -> u32 {
            let artifacts: &[&[u8]] = &$module;
            EXACT_BRIDGE.with(|b| b.borrow_mut().baked_plan(artifacts.get(index as usize).copied().unwrap_or(&[])))
        }
    };
    ($data:ty, $plan:expr, $compat:expr) => {
        $crate::host!($data, $plan, $compat, || <$data as ::std::default::Default>::default());
    };
    ($data:ty, $plan:expr, $compat:expr, $new:expr) => {
        thread_local! {
            static EXACT_BRIDGE: $crate::abi::Cell<$data> = ::std::cell::RefCell::new($crate::abi::Bridge::new());
        }

        /// Resize the input buffer; returns its address.
        #[no_mangle]
        pub extern "C" fn exact_in(len: u32) -> *mut u8 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().input(len as usize))
        }

        /// The output buffer's address.
        #[no_mangle]
        pub extern "C" fn exact_out() -> *const u8 {
            EXACT_BRIDGE.with(|b| b.borrow().output())
        }

        /// Copy the plan baked into this wasm to the output buffer. The web
        /// build uses these exact bytes for app.plan and exact.json.
        #[no_mangle]
        pub extern "C" fn exact_plan() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().baked_plan($plan))
        }

        /// Copy the exact compatibility receipt embedded in this wasm.
        #[no_mangle]
        pub extern "C" fn exact_compat() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().baked_plan($compat.as_bytes()))
        }

        /// The page's snapshot of the app's kept secrets (LLP 1018 D6), from
        /// the input buffer's first `len` bytes (`name NUL value NUL …`),
        /// for the next `exact_boot`.
        #[no_mangle]
        pub extern "C" fn exact_store(len: u32) {
            EXACT_BRIDGE.with(|b| b.borrow_mut().store(len as usize))
        }

        /// Boot; returns the first batch's length.
        #[no_mangle]
        pub extern "C" fn exact_boot() -> u32 {
            EXACT_BRIDGE.with(|b| {
                let mut b = b.borrow_mut();
                b.set_compat($compat);
                b.boot($plan, ($new)())
            })
        }

        /// Boot from plan bytes in the input buffer (the dev loop's restart).
        #[no_mangle]
        pub extern "C" fn exact_boot_plan(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| {
                let mut b = b.borrow_mut();
                b.set_compat($compat);
                b.boot_plan(len as usize, ($new)())
            })
        }

        /// Metadata for the optional browser module loader.
        #[no_mangle]
        pub extern "C" fn exact_logic() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().logic_info(($new)()))
        }
        /// The first pixel has been painted; activate deferred logic.
        #[no_mangle]
        pub extern "C" fn exact_data_ready() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().data_ready())
        }
        /// Replace the paired plan and privately prepared browser module.
        #[no_mangle]
        pub extern "C" fn exact_boot_module(plan: u32, receipt: u32, module: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().boot_module([plan as usize, receipt as usize, module as usize], ($new)()))
        }

        /// Inspect a plan's fonts without changing the live host.
        #[no_mangle]
        pub extern "C" fn exact_plan_fonts(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().plan_fonts(len as usize))
        }

        /// Query the current plan's declared font catalog. The returned JSON
        /// is separate from operation batches (LLP 1019 D5).
        #[no_mangle]
        pub extern "C" fn exact_fonts() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().fonts())
        }

        /// Dispatch an event; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_dispatch(view: u32, kind: u32, len: u32, now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().dispatch(view, kind, len as usize, now_ms))
        }

        /// A request's outcome (LLP 1016 D2): `kind` 0 response / 1 network /
        /// 2 refused / 3 unsupported / 4 aborted; the input buffer holds
        /// `hlen` bytes of `name: value` header lines then `blen` bytes of
        /// body (or the message). Returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_fulfill(ticket: f64, kind: u32, status: u32, hlen: u32, blen: u32, now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().fulfill(ticket, kind, status, hlen as usize, blen as usize, now_ms))
        }

        /// Move the clock; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_advance(now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().advance(now_ms))
        }

        /// An agent request from the input buffer; returns the reply's length.
        #[no_mangle]
        pub extern "C" fn exact_agent(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().agent(len as usize))
        }

        /// A page line for the runner's journal (LLP 1012 §3; LLP 1035.001
        /// D6): the input buffer's first `len` bytes. Returns 0.
        #[no_mangle]
        pub extern "C" fn exact_log(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().log(len as usize))
        }
    };
}
