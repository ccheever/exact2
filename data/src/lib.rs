//! Language-independent application data capabilities and composition.
//! @ref LLP 1027.001 — the same operation behind either language's data seam.

pub mod envelope;
mod mixed;
pub mod placed;
pub mod storage;
pub use mixed::Mixed;
pub use placed::Placed;

#[cfg(test)]
mod mixed_tests;
