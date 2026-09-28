//! Type Tour on Linux: the host's entry over the app's (empty) data source
//! and its baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs (LLP 1030 D3a), written by
/// `build.rs` beside the plan.
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

include!(concat!(env!("OUT_DIR"), "/entry.rs"));
