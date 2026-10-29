//! The Linux host.
//!
//! @ref LLP 1015 (Linux host v1)
//! @ref LLP 1001 §5–6 (layout is a host call; text measurement is injected)
//! @ref LLP 1002 D2, §4 (every host but the web runs `exact-motion`)
//!
//! The third host and the first that paints. The runner and kernel run
//! natively; after every commit the host lays the tree out with the
//! kernel's own layout (Taffy, measuring text with Parley), seeks the
//! motion engine to the app's clock, and the painter draws the kernel tree
//! itself — vello on the GPU, tiny-skia on the CPU where there is none —
//! **the kernel is the display list**: no batch, no mirror, no view tree of
//! the platform's, because the platform has none to offer. The pixels go to DRM/KMS dumb buffers with evdev input, or
//! into a buffer with no display at all, which is how the host is tested:
//! the agent API (LLP 1012) over stdio, a screenshot, a smoke run — on a
//! fleet Linux box with no GPU, or on macOS in the seconds-loop. Pure Rust
//! end to end; no system library is linked.
//!
//! - [`text`] — Parley: one paragraph cache answers measure and paint.
//! - [`paint`] — the painter: one walk of the tree, every box recorded,
//!   emitted to a backend.
//! - [`gpu`] — the vello backend, the main one: wgpu, a texture, a readback.
//! - [`raster`] — the tiny-skia backend: the fallback with no adapter, the
//!   pixel oracle.
//! - [`host`] — the runner wrapped: commits → layout → motion.
//! - [`image`] — sources under the asset root, PNG decoded off-thread.
//! - [`presenter`] — scroll, focus, hit-testing, the host-side operations.
//! - [`hatches`] — access hatches: an app's own code at its nodes' boxes,
//!   their overlays and the input that lands there (LLP 1075.003.000.001).
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
#[cfg(target_os = "android")]
pub mod android_hint;
pub mod app;
mod build_channel;
#[cfg(any(target_os = "android", test))]
pub mod canvas;
mod canvas2d;
#[cfg(windows)]
pub mod com;
pub mod content_region;
pub mod delivery;
pub mod desktop;
// The packages' own Rust names this crate so; `desktop_tests.rs` compiles it here.
#[cfg(test)]
extern crate self as exact_linux;
#[cfg(target_os = "linux")]
pub mod display;
mod doc_watch;
pub mod executor;
pub mod fetch;
pub mod fields;
mod file;
pub mod frames;
pub mod gamepad;
pub mod gpu;
pub mod hatches;
pub mod host;
pub mod image;
#[cfg(target_os = "linux")]
pub mod input;
mod kept;
mod media_session;
pub mod navigation;
pub mod package;
pub mod pads;
pub mod paint;
pub mod picker;
mod placement;
pub mod presence_secrets;
pub mod presenter;
pub mod raster;
mod scroll_anchor;
mod surfaces;
mod tamber;
pub mod teardown;
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

pub use app::{run, run_composed, run_with_hatches, Composed};
pub use hatches::Hatches;
pub use host::{Host, HostError};
pub use presenter::Presenter;

/// Whether a driver drives Android's reader (`canvas/agent.rs`).
pub(crate) fn reader_driven() -> bool {
    #[cfg(target_os = "android")]
    return canvas::agent::driven();
    #[cfg(not(target_os = "android"))]
    false
}

/// Run `f` inside an atrace section on Android (Perfetto shows it on this
/// thread); elsewhere just `f`.
#[inline]
pub(crate) fn traced<T>(_name: &core::ffi::CStr, f: impl FnOnce() -> T) -> T {
    #[cfg(target_os = "android")]
    return android::trace(_name, f);
    #[cfg(not(target_os = "android"))]
    f()
}

/// A Contract source compiled and encoded once per test binary: the tests boot fresh presenters
/// from a few constant sources many times, and a compile depends on nothing but its source.
#[cfg(test)]
pub(crate) fn test_plan(source: &str) -> Vec<u8> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static PLANS: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    let plans = PLANS.get_or_init(Default::default);
    if let Some(plan) = plans.lock().unwrap_or_else(|e| e.into_inner()).get(source) {
        return plan.clone();
    }
    // Compiled outside the lock, so one test's compile never waits on another's. Shelf's at its own
    // path, where its app.json is read.
    let plan = if source == include_str!("../../../apps/shelf/app.contract") {
        let shelf = concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/shelf/app.contract");
        contract::compile_path_source(std::path::Path::new(shelf), source)
            .unwrap()
            .encode()
    } else {
        test_compile(source)
    };
    plans
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(source.to_owned(), plan.clone());
    plan
}

/// A source with no path can't resolve a `use`, so one that uses this checkout's
/// packages (Shelf's `use … from "@exact/…"`) is compiled as an app of its own: a
/// temporary directory holding just the source and a `package.json` depending on each
/// package it names, each linked under `node_modules/@exact` as an install places it.
#[cfg(test)]
fn test_compile(source: &str) -> Vec<u8> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let packages: Vec<&str> = source
        .lines()
        .filter(|l| l.starts_with("use "))
        .filter_map(|l| l.split("from \"@exact/").nth(1)?.split('"').next())
        .collect();
    if packages.is_empty() {
        return contract::compile(source).unwrap().encode();
    }
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("exact-linux-plan-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let scope = dir.join("node_modules/@exact");
    std::fs::create_dir_all(&scope).unwrap();
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
    let dependencies: Vec<String> = packages
        .iter()
        .map(|n| format!("\"@exact/{n}\": \"*\""))
        .collect();
    let manifest = format!("{{\"dependencies\": {{{}}}}}", dependencies.join(", "));
    std::fs::write(dir.join("package.json"), manifest).unwrap();
    for name in packages {
        let to = checkout.join(name).canonicalize().unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(to, scope.join(name)).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(to, scope.join(name)).unwrap();
    }
    let path = dir.join("app.contract");
    std::fs::write(&path, source).unwrap();
    let plan = contract::compile_path(&path);
    let _ = std::fs::remove_dir_all(&dir);
    plan.unwrap().encode()
}
