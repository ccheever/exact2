//! Update Lab: the shared Contract UI and deferred TypeScript data module on the web.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

type ExactEmbeddedData = update_lab_data::Probe;
fn embedded_data() -> ExactEmbeddedData {
    update_lab_data::Probe
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
type HostData = update_lab_data::Lab<exact_js_web::Module, AppData>;
fn lab_data() -> HostData {
    update_lab_data::compose(
        exact_js_web::Module::new(APP, GRANTS, REVISION),
        app_data(),
        RUST_UPDATES,
        |_| Ok(app_data()),
    )
}
exact_web::host!(
    HostData,
    PLAN,
    COMPAT,
    lab_data,
    [
        include_bytes!(concat!(env!("OUT_DIR"), "/app.module.json")) as &[u8],
        include_bytes!(concat!(env!("OUT_DIR"), "/app.js")) as &[u8],
        include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc")) as &[u8]
    ]
);
