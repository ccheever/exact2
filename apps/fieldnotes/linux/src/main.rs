//! Fieldnotes on Linux: the host over the notes' TypeScript (Hermes
//! bytecode), the Rust backup source and the baked plan, with app storage.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
type ExactEmbeddedData =
    exact_data_host::Storage<fieldnotes_data::Data<exact_js::Placed<exact_js::Module>>>;
fn embedded_data() -> ExactEmbeddedData {
    exact_data_host::Storage::new(fieldnotes_data::mixed(
        exact_js::Module::new(APP_BYTECODE, APP, GRANTS).placed(TYPESCRIPT_PLACEMENT),
        RUST_PLACEMENT,
    ))
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
fn main() {
    std::process::exit(exact_linux::run::<AppData>(PLAN, COMPAT));
}
