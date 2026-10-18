//! Fieldnotes: shared Contract UI with TypeScript notes and Rust backup on Apple.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
include!(concat!(env!("OUT_DIR"), "/baked.rs"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

type ExactEmbeddedData =
    exact_data_host::Storage<fieldnotes_data::Data<exact_js::Placed<exact_js::Module>>>;
fn embedded_data() -> ExactEmbeddedData {
    exact_data_host::Storage::new(fieldnotes_data::mixed(
        exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS).placed(TYPESCRIPT_PLACEMENT),
        RUST_PLACEMENT,
    ))
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_apple::host!(AppData, exact_plan(), exact_compat(), None, std::ptr::null(), app_data; linked = EXACT_LINKED);
