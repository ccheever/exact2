//! The native T3 client: Contract views, TypeScript projections, and an app-local
//! Swift transport. T3's separately running server owns all agent execution.

mod markdown;

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

type ExactEmbeddedData = markdown::Data<exact_js::Placed<exact_js::Module>>;
fn embedded_data() -> ExactEmbeddedData {
    markdown::mixed(
        exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS).placed(TYPESCRIPT_PLACEMENT),
        RUST_PLACEMENT,
    )
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
exact_apple::host!(AppData, PLAN, COMPAT, None, std::ptr::null(), app_data);

#[cfg(test)]
mod title_snooze_tests;
