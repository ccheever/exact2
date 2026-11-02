//! The geofencing fixture on Linux: the host's entry over the app's data source and its
//! baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs, written by
/// `build.rs` beside the plan; the runner answers the `delivery` resource
/// and `state.delivery` from it.
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

include!(concat!(env!("OUT_DIR"), "/entry.rs"));
