//! The Linux host.
//!
//! @ref LLP 1015 (Linux host v1)
//! @ref LLP 1001 §5–6 (layout is a host call; text measurement is injected)
//! @ref LLP 1002 D2, §4 (every host but the web runs `exact-motion`)
//!
//! The third host and the first that paints. The runner and kernel run
//! natively; after every commit the host lays the tree out with the
//! kernel's own layout (Taffy, measuring text with cosmic-text), seeks the
//! motion engine to the app's clock, and the painter draws the kernel tree
//! itself with tiny-skia — **the kernel is the display list**: no batch, no
//! mirror, no view tree of the platform's, because the platform has none
//! to offer. The pixels go to DRM/KMS dumb buffers with evdev input, or
//! into a buffer with no display at all, which is how the host is tested:
//! the agent API (LLP 1012) over stdio, a screenshot, a smoke run — on a
//! fleet Linux box with no GPU, or on macOS in the seconds-loop. Pure Rust
//! end to end; no system library is linked.
//!
//! - [`text`] — cosmic-text: one paragraph cache answers measure and paint.
//! - [`paint`] — the painter: the tree → pixels, every box recorded.
//! - [`host`] — the runner wrapped: commits → layout → motion.
//! - [`image`] — sources under the asset root, PNG decoded off-thread.
//! - [`presenter`] — scroll, focus, hit-testing, the host-side operations.
//! - [`agent`] — the agent API on stdio (`Agent.swift`'s twin).
//! - [`app`] — the entry: the environment, headless or display.
//! - [`display`], [`input`] (Linux) — KMS dumb buffers and evdev.

#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod agent;
pub mod app;
#[cfg(target_os = "linux")]
pub mod display;
pub mod host;
pub mod image;
#[cfg(target_os = "linux")]
pub mod input;
pub mod paint;
pub mod presenter;
pub mod text;

pub use app::run;
pub use host::{Host, HostError};
pub use presenter::Presenter;
