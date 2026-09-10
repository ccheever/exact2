//! Messages: the shared Contract UI and deferred TypeScript data module on the web.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

exact_web::host!(
    exact_js_web::Module,
    PLAN,
    COMPAT,
    || exact_js_web::Module::new(APP, GRANTS, REVISION),
    [
        include_bytes!(concat!(env!("OUT_DIR"), "/app.module.json")) as &[u8],
        include_bytes!(concat!(env!("OUT_DIR"), "/app.js")) as &[u8],
        include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc")) as &[u8]
    ]
);
