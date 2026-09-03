//! Caltrain on the web: the host's five exports over the app's data source
//! and its baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs (LLP 1030 D3a), written by
/// `build.rs` beside the plan: `{"id":…,"inputs":{…}}`.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

exact_web::host!(caltrain_data::Caltrain, PLAN);
