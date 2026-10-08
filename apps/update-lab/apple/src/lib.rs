//! Update Lab: the shared Contract UI and deferred TypeScript data module on Apple.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

type ExactEmbeddedData = update_lab_data::Probe;
fn embedded_data() -> ExactEmbeddedData {
    update_lab_data::Probe
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
type HostData =
    update_lab_data::Lab<exact_js::Placed<exact_js::Module>, update_lab_data::Placed<AppData>>;
/// The Rust half, placed (LLP 1027.002 D1): built on its owner from a fresh
/// `app_data()` when placed on a worker, the instance itself on `main`.
fn placed_rust() -> update_lab_data::Placed<AppData> {
    update_lab_data::Placed::built(app_data(), RUST_PLACEMENT, |_| Box::new(|| Ok(app_data())))
}
fn lab_data() -> HostData {
    update_lab_data::compose(
        exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS).placed(TYPESCRIPT_PLACEMENT),
        placed_rust(),
        RUST_UPDATES,
        |_| Ok(placed_rust()),
    )
}
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_apple::host!(HostData, PLAN, COMPAT, None, std::ptr::null(), lab_data; linked = EXACT_LINKED);
