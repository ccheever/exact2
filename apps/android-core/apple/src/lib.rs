//! The shared core Contract over the standard Apple host and its retained data.

#![deny(missing_docs)]

/// The baked plan shared with the Android and web examples.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
/// The embedded Apple carrier's native compatibility receipt.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

exact_apple::host!(android_core_data::Core, PLAN, COMPAT);
