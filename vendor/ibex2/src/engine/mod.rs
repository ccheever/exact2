//! The engine seam, and its one implementation.

#[cfg(feature = "hermes")]
pub mod hermes;

mod seam;
pub use seam::Engine;
