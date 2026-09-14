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
type HostData = update_lab_data::Lab<exact_js::Module, AppData>;
fn lab_data() -> HostData {
    update_lab_data::compose(
        exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS),
        app_data(),
        RUST_UPDATES,
        |_| Ok(app_data()),
    )
}
exact_apple::host!(HostData, PLAN, COMPAT, None, std::ptr::null(), lab_data);
