//! Shelf, the native recipe app (LLP 1115): its Contract UI and TypeScript data.

include!(concat!(env!("OUT_DIR"), "/module.rs"));

type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    exact_js::Module::new(APP_BYTECODE, APP, GRANTS).placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
include!(concat!(env!("OUT_DIR"), "/host.rs"));
