//! Completion Storm on the existing apple host, with binary-bound Rust data.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_apple::host!(completion_storm_data::Storm, PLAN, COMPAT; linked = EXACT_LINKED);
