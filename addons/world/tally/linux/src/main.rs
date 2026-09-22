//! Tally on the Linux host: the host's binary over the app's data
//! source and its baked plan (`exact_linux_update::run`).

/// The baked plan, written by `build.rs`.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
/// The compatibility id (exact2 LLP 1030 D3a), beside the plan.
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

fn main() {
    tally_data::metrics::entry();
    std::process::exit(exact_linux_update::run::<tally_data::TallySource>(
        PLAN, COMPAT,
    ));
}
