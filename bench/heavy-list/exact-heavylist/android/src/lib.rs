//! The original Heavy List app over the retained Android Views host.
#![deny(missing_docs)]
/// The shared ahead-of-time Contract, including the original baked feed.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
/// The shared Android compatibility record.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
// The full provider/executor and General fallback remain part of this benchmark.
exact_android::host!(exact_heavylist_data::Heavy, PLAN, COMPAT);
