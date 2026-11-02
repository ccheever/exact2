//! Heavy list on Apple platforms: the host's C exports over the data source
//! and the baked plan.

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs, written by `build.rs`.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

exact_apple::host!(exact_heavylist_data::Heavy, PLAN, COMPAT);
