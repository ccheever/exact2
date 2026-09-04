//! The optional delivery adapter boundary (LLP 1030 D4).
//! A binary-only app supplies neither hooks nor a C API; no store is linked.

use exact_runner::Delivery;
use std::ffi::c_void;

/// Process-owned delivery operations, supplied by the app's linked adapter.
pub struct Hooks {
    /// The selected candidate, before boot.
    pub selected_plan: fn() -> Option<(String, Vec<u8>)>,
    /// Record an attempted boot.
    pub boot_started: fn(),
    /// Refuse a selected candidate and fall back to the embedded plan.
    pub entry_refused: fn(&str, &str),
    /// Copy the adapter's latest facts into the runner's delivery value.
    pub status_into: fn(&mut Delivery),
    /// Take the boot's diagnostic once.
    pub take_note: fn() -> Option<String>,
    /// The most recent asynchronous outcome.
    pub last_line: fn() -> Option<String>,
}

/// A check's completion, called on its worker with bytes alive for the call.
pub type DoneFn = extern "C" fn(*mut c_void, *const u8, usize);

/// The optional adapter's C calls. A null pointer means no adapter was linked.
/// Kept in step with `ExactDeliveryApi` in `include/exact.h`.
#[repr(C)]
pub struct Api {
    /// Allocate the adapter's input buffer.
    pub input: extern "C" fn(usize) -> *mut u8,
    /// The adapter's most recent output.
    pub output: extern "C" fn() -> *const u8,
    /// Open from a path payload; zero means success.
    pub open: extern "C" fn(usize) -> u32,
    /// The selection as JSON in the output buffer.
    pub select: extern "C" fn() -> u32,
    /// Record first pixel.
    pub boot_succeeded: extern "C" fn(),
    /// Start the asynchronous check.
    pub check: extern "C" fn(Option<DoneFn>, *mut c_void) -> u32,
    /// A staged plan in the output buffer, or zero.
    pub activate: extern "C" fn() -> u32,
}
