//! Language-independent application data capabilities and composition.
//! @ref LLP 1027.001 — the same operation behind either language's data seam.

mod mixed;
pub mod storage;
pub use mixed::Mixed;

#[cfg(test)]
mod mixed_tests;
