//! Bench on Windows: the host's window over the app's TypeScript data module
//! (`app.ts` as Hermes bytecode) and its baked plan.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    exact_js::Module::new(APP_BYTECODE, APP, GRANTS).placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
// The packages' Windows implementations the build composes (`exact_packages()`).
include!(concat!(env!("OUT_DIR"), "/packages.rs"));
fn main() {
    let composed = exact_linux::Composed {
        packages: exact_packages(),
        ..Default::default()
    };
    std::process::exit(exact_windows::run_composed::<AppData>(
        "Bench",
        window(),
        PLAN,
        COMPAT,
        composed,
    ));
}
