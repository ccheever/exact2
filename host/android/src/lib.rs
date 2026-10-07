//! Imperative Android adapter over Exact's shared native runtime.
//!
//! Android owns a kept platform view tree; Rust owns app state, CSS layout,
//! motion and request execution. One borrowed transaction crosses JNI for each
//! event or active display frame, with fixed binary layout/motion records.

#![deny(unsafe_code)]
#![deny(missing_docs)]

mod bridge;
mod carrier;
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

/// Build-selected general owner and the sealed uninhabited typed fallback.
pub use bridge::{CoreOnly, General, GeneralRuntime};
/// Conservative whole-plan predicate shared by bake selection and runtime.
pub use core::eligible as core_eligible;
#[cfg(test)]
mod carrier_tests;

/// Explicit source contract and conservative bake-time selection.
pub use carrier::{baked_core_eligible, DataContract};
