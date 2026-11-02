//! Update Lab: the shared Contract UI and deferred TypeScript data module on the web.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

type ExactEmbeddedData = update_lab_data::Probe;
fn embedded_data() -> ExactEmbeddedData {
    update_lab_data::Probe
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
type HostData = update_lab_data::Lab<exact_js_web::Module, update_lab_data::Placed<AppData>>;
fn placed_rust() -> update_lab_data::Placed<AppData> {
    update_lab_data::Placed::built(app_data(), RUST_PLACEMENT, |_| Box::new(|| Ok(app_data())))
}
fn lab_data() -> HostData {
    update_lab_data::compose(
        exact_js_web::Module::new(APP, GRANTS, REVISION).placed(TYPESCRIPT_PLACEMENT),
        placed_rust(),
        RUST_UPDATES,
        |_| Ok(placed_rust()),
    )
}
exact_web::host!(
    HostData,
    PLAN,
    COMPAT,
    lab_data,
    [
        include_bytes!(concat!(env!("OUT_DIR"), "/app.module.json")) as &[u8],
        include_bytes!(concat!(env!("OUT_DIR"), "/app.js")) as &[u8]
    ]
);
