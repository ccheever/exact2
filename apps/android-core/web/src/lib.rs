//! The shared Android core Contract on the web parity oracle.

#![deny(missing_docs)]

/// The same ahead-of-time plan and baked rows as the Android archive.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
/// The binary-only web compatibility receipt.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_web::host!(android_core_data::Core, PLAN, COMPAT);
