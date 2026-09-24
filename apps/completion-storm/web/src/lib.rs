//! Completion Storm on the existing web host, with binary-bound Rust data logic.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_web::host!(
    completion_storm_data::Storm,
    PLAN,
    COMPAT,
    completion_storm_data::Storm::default
);
