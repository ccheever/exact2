//! Duo Lab on Linux (LLP 1077 D8): flat unless an agent `prefer`s a posture or
//! segments — how the five checks exercise the fold without a Duo.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS)
        .with_canvas_surfaces(CANVAS_SURFACES)
        .placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
fn main() {
    std::process::exit(exact_linux::run::<AppData>(PLAN, COMPAT));
}
