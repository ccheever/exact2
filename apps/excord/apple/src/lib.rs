//! Excord: a Discord client (`apps/excord/llp/0001`).

include!(concat!(env!("OUT_DIR"), "/module.rs"));
include!(concat!(env!("OUT_DIR"), "/baked.rs"));

type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    exact_js::Module::new(APP_BYTECODE, APP, GRANTS).placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_apple::host!(AppData, exact_plan(), exact_compat(), None, std::ptr::null(), app_data; linked = EXACT_LINKED);
