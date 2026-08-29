//! Caltrain on Linux: the host's entry over the app's data source and its
//! baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

fn main() {
    std::process::exit(exact_linux::run::<caltrain_data::Caltrain>(PLAN));
}
