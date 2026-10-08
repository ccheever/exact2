//! Native Android control and virtualized-list integration fixture.
#![deny(missing_docs)]
/// The shared fixture plan, baked with the existing immutable row provider.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
/// This fixture's binary-only native compatibility receipt.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_web::host!(android_native_data::Core, PLAN, COMPAT);
