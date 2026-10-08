//! Native Android control and virtualized-list integration fixture.
#![deny(missing_docs)]
/// The shared fixture plan, baked with the existing immutable row provider.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
/// This fixture's binary-only native compatibility receipt.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
exact_android::host!(android_core_data::Core, PLAN, COMPAT);

#[cfg(test)]
mod tests;
