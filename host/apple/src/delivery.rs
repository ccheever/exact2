//! The optional delivery adapter boundary (LLP 1030 D4).
//! A binary-only app supplies neither hooks nor a C API; no store is linked.

use exact_runner::Delivery;
use std::ffi::c_void;

/// Process-owned delivery operations, supplied by the app's linked adapter.
pub struct Hooks {
    /// The selected candidate, before boot.
    pub selected_plan: fn() -> Option<(String, Vec<u8>)>,
    /// Candidate facts supplied before the containing app commits it.
    pub candidate_delivery: fn(u64, &str) -> Option<Delivery>,
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
    pub boot_succeeded: extern "C" fn(u64),
    /// Start the asynchronous check.
    pub check: extern "C" fn(Option<DoneFn>, *mut c_void) -> u32,
    /// Prepare an immutable candidate descriptor, or zero.
    pub prepare: extern "C" fn() -> u32,
    /// Pinned plan bytes for a generation token.
    pub plan: extern "C" fn(u64) -> u32,
    /// Verified asset bytes, named in the input buffer.
    pub asset: extern "C" fn(u64, usize) -> u32,
    /// Commit the candidate after every session accepts it.
    pub commit: extern "C" fn(u64) -> u32,
    /// Release an uncommitted candidate.
    pub discard: extern "C" fn(u64),
    /// Refuse a corrupt initial generation before first pixel.
    pub refuse: extern "C" fn(u64, usize) -> u32,
    /// A session accepted the generation, before first pixel.
    pub started: extern "C" fn(u64),
}
