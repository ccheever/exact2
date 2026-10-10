//! Mobile on Apple: the Contract UI and the TypeScript data module.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    let mut module =
        exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS).with_canvas_surfaces(CANVAS_SURFACES);
    // App-owned history experiment; the failed measured footprint needs headroom.
    // This runtime ceiling also bounds captured result strings.
    module.set_max_heap(128 << 20);
    module.placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
exact_apple::host!(AppData, PLAN, COMPAT, None, std::ptr::null(), app_data);
