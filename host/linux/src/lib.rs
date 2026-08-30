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
//! itself — vello on the GPU, tiny-skia on the CPU where there is none —
//! **the kernel is the display list**: no batch, no mirror, no view tree of
//! the platform's, because the platform has none to offer. The pixels go to DRM/KMS dumb buffers with evdev input, or
//! into a buffer with no display at all, which is how the host is tested:
//! the agent API (LLP 1012) over stdio, a screenshot, a smoke run — on a
//! fleet Linux box with no GPU, or on macOS in the seconds-loop. Pure Rust
//! end to end; no system library is linked.
//!
//! - [`text`] — cosmic-text: one paragraph cache answers measure and paint.
//! - [`paint`] — the painter: one walk of the tree, every box recorded,
//!   emitted to a backend.
//! - [`gpu`] — the vello backend, the main one: wgpu, a texture, a readback.
//! - [`raster`] — the tiny-skia backend: the fallback with no adapter, the
//!   pixel oracle.
//! - [`host`] — the runner wrapped: commits → layout → motion.
//! - [`image`] — sources under the asset root, PNG decoded off-thread.
//! - [`presenter`] — scroll, focus, hit-testing, the host-side operations.
//! - [`agent`] — the agent API on stdio (`Agent.swift`'s twin).
//! - [`executor`] — `ibex2::host` on a worker thread for a request that
//!   leaves the process (LLP 1016 D2), its wake a socketpair the loop polls.
//! - [`app`] — the entry: the environment, headless or display.
//! - [`display`], [`input`], [`vnc`] (Linux) — KMS dumb buffers, evdev, and
//!   the screen over RFB with a client's pointer and keys as input.

#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod agent;
pub mod app;
#[cfg(target_os = "linux")]
pub mod display;
pub mod executor;
pub mod gpu;
pub mod host;
pub mod image;
#[cfg(target_os = "linux")]
pub mod input;
pub mod paint;
pub mod presenter;
pub mod raster;
pub mod text;
#[cfg(target_os = "linux")]
pub mod vnc;

pub use app::run;
pub use host::{Host, HostError};
pub use presenter::Presenter;
