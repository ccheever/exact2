//! Completion Storm on the existing linux host, with binary-bound Rust data.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
fn main() {
    std::process::exit(exact_linux::run::<completion_storm_data::Storm>(
        PLAN, COMPAT,
    ));
}
