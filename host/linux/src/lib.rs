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
//! - [`delivery`] — the optional app-supplied delivery adapter boundary
//!   (LLP 1030 D4); no updater is linked by the host.
//! - [`app`] — the entry: the environment, headless or display.
//! - [`display`], [`input`], [`vnc`] (Linux) — KMS dumb buffers, evdev, and
//!   the screen over RFB with a client's pointer and keys as input.

#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod agent;
#[cfg(target_os = "android")]
pub mod android;
pub mod app;
#[cfg(target_os = "android")]
pub mod canvas;
mod canvas2d;
pub mod content_region;
pub mod delivery;
#[cfg(target_os = "linux")]
pub mod display;
pub mod executor;
pub mod fetch;
mod file;
pub mod frames;
pub mod gpu;
pub mod host;
pub mod image;
#[cfg(target_os = "linux")]
pub mod input;
pub mod navigation;
pub mod paint;
pub mod picker;
mod placement;
pub mod presenter;
pub mod raster;
mod surfaces;
pub mod text;
#[cfg(any(target_os = "android", test))]
#[path = "canvas/travel.rs"]
mod travel;
#[cfg(target_os = "linux")]
pub mod vnc;
mod wake;
mod zone;

/// Install the Windows event-loop wake for native work, images and text results.
/// Removing it at shutdown releases the event loop without stopping shared workers.
#[cfg(windows)]
pub fn set_event_waker(waker: Option<std::sync::Arc<dyn Fn() + Send + Sync>>) {
    wake::install(waker);
}

pub use app::run;
pub use host::{Host, HostError};
pub use presenter::Presenter;

/// Run `f` inside an atrace section on Android (Perfetto shows it on this
/// thread); elsewhere just `f`.
#[inline]
pub(crate) fn traced<T>(_name: &core::ffi::CStr, f: impl FnOnce() -> T) -> T {
    #[cfg(target_os = "android")]
    return android::trace(_name, f);
    #[cfg(not(target_os = "android"))]
    f()
}
