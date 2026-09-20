//! Tally on the web: the host's exports over the app's data source
//! and its baked plan.

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs (exact2 LLP 1030 D3a), written by
/// `build.rs` beside the plan; the runner answers the `delivery` resource
/// from it.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

exact_web::host!(tally_data::TallySource, PLAN, COMPAT);

#[no_mangle]
pub extern "C" fn x1_first_tick() -> f64 {
    tally_data::metrics::FIRST_TICK_US.load(std::sync::atomic::Ordering::Relaxed) as f64
}
#[no_mangle]
pub extern "C" fn x1_allocations() -> f64 {
    tally_data::metrics::ALLOCS.load(std::sync::atomic::Ordering::Relaxed) as f64
}
