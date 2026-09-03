//! Caltrain on Linux: the host's entry over the app's data source and its
//! baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs (LLP 1030 D3a), written by
/// `build.rs` beside the plan; read by the `delivery` resource once the
/// host answers it (1030.000 stage 3), embedded now so the binary knows
/// its cohort.
#[allow(dead_code)]
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

fn main() {
    std::process::exit(exact_linux::run::<caltrain_data::Caltrain>(PLAN));
}
