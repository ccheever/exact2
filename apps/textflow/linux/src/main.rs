//! Interaction gallery on Linux: the host's entry over the app's data source and its
//! baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs (LLP 1030 D3a), written by
/// `build.rs` beside the plan; the runner answers the `delivery` resource
/// and `state.delivery` from it (LLP 1030 D7).
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

include!(concat!(env!("OUT_DIR"), "/entry.rs"));

#[cfg(test)]
mod tests;

#[cfg(test)]
mod showpiece_tests;
