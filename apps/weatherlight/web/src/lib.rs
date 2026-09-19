//! Weatherlight: the shared Contract UI and deferred TypeScript data module on the web.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

type ExactEmbeddedData = exact_js_web::Module;
fn embedded_data() -> ExactEmbeddedData {
    exact_js_web::Module::new(APP, GRANTS, REVISION).placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
exact_web::host!(
    AppData,
    PLAN,
    COMPAT,
    app_data,
    [
        include_bytes!(concat!(env!("OUT_DIR"), "/app.module.json")) as &[u8],
        include_bytes!(concat!(env!("OUT_DIR"), "/app.js")) as &[u8],
        include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc")) as &[u8]
    ]
);
