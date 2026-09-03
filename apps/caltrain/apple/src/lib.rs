//! Caltrain on Apple platforms: the host's C exports over the app's data
//! source and its baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs (LLP 1030 D3a), written by
/// `build.rs` beside the plan: `{"id":…,"inputs":{…}}`. The runner reads
/// it at boot and answers the `delivery` resource from it (LLP 1030 D7).
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

exact_apple::host!(caltrain_data::Caltrain, PLAN, COMPAT);
