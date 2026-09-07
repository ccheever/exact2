//! Fieldnotes: the shared Contract UI and deferred TypeScript data module on Apple.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

exact_apple::host!(
    exact_js::Module,
    PLAN,
    COMPAT,
    None,
    std::ptr::null(),
    || exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS)
);
