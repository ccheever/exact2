//! The wasm ABI, with no `unsafe`.
//!
//! @ref LLP 1007 §3
//!
//! Five exports. The glue never hands the host a pointer it did not get from
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
    input: Vec<u8>,
    output: Vec<u8>,
}

impl<D: DataSource> Bridge<D> {
    /// Empty; `boot` fills it.
    pub const fn new() -> Bridge<D> {
        Bridge {
            host: None,
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

    /// The output buffer's address.
    pub fn output(&self) -> *const u8 {
        self.output.as_ptr()
    }

    fn emit(&mut self, s: String) -> u32 {
        self.output = s.into_bytes();
        self.output.len() as u32
    }

    /// Boot from `plan` with `data`; the output is the first batch.
    pub fn boot(&mut self, plan: &[u8], data: D) -> u32 {
        match Host::boot(plan, data) {
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

    /// Dispatch an event; `kind` is 0 = press, 1 = change (payload = the input buffer's first `len` bytes, UTF-8).
    pub fn dispatch(&mut self, view: u32, kind: u32, len: usize) -> u32 {
        let payload =
            String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        let event = match kind {
            0 => Event::Press,
            _ => Event::Change(payload),
        };
        let out = match self.host.as_mut() {
            Some(h) => h.dispatch(view, event),
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

/// Instantiate the five exports for one app.
///
/// `$data` is the app's `DataSource` type (constructed with `Default`);
/// `$plan` a `&'static [u8]` of baked plan bytes (typically `include_bytes!`
/// of what the app's `build.rs` wrote).
#[macro_export]
macro_rules! host {
    ($data:ty, $plan:expr) => {
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

        /// Boot; returns the first batch's length.
        #[no_mangle]
        pub extern "C" fn exact_boot() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().boot($plan, <$data as ::std::default::Default>::default()))
        }

        /// Dispatch an event; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_dispatch(view: u32, kind: u32, len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().dispatch(view, kind, len as usize))
        }

        /// Move the clock; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_advance(now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().advance(now_ms))
        }
    };
}
