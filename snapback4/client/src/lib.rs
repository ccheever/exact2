#![forbid(unsafe_code)]
//! Snapback4's client protocol for Exact, without I/O ([`client`]), and what
//! every host that holds a device shares: the JSON dispatch an app calls
//! ([`dispatch`]) and the partition's binding to its app, origin and viewer
//! ([`partition`]). The native adapter (`exact-snapback4`) brings SQLite under
//! the app's grants; the web's wasm (`exact-snapback4-web`) brings memory
//! persisted by the page.

pub mod backend;
pub mod client;
pub mod dispatch;
mod journal;
pub mod partition;
mod round;
mod uploads;

pub use client::{Client, Config, Core, Fetch, Reply, Step, NEEDS_BACKEND};
pub use dispatch::dispatch;

/// Identity supplied by the compiled engine, shared by native and web requests.
pub fn engine_info() -> serde_json::Value {
    serde_json::to_value(snapback4_core::device_protocol::DeviceEngine::current())
        .expect("device engine identity")
}
