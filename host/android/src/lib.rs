//! Imperative Android adapter over Exact's shared native runtime.
//!
//! Android owns a kept platform view tree; Rust owns app state, CSS layout,
//! motion and request execution. One borrowed transaction crosses JNI for each
//! event or active display frame, with fixed binary layout/motion records.

#![deny(unsafe_code)]
#![deny(missing_docs)]

mod bridge;
mod core;
mod exports;
pub mod session;
pub mod wire;

/// Registered measurement and wake callbacks for one runtime.
pub use exact_apple::abi::Hooks;
/// The native executor's coalesced wake callback type.
pub use exact_apple::executor;
/// The existing native host's platform text callback structures.
pub use exact_apple::measure;

/// Conservative whole-plan predicate used by the automatic runtime selection.
pub use core::eligible as core_eligible;
#[cfg(test)]
mod carrier_tests;

#[cfg(test)]
mod controls_tests;

/// The allocator used by Android apps' Rust runtime.
#[cfg(target_os = "android")]
pub use mimalloc::MiMalloc;
