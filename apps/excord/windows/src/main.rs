//! Excord on Windows: the host's window over the app's TypeScript data module
//! (`app.ts` as Hermes bytecode) and its baked plan.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    let mut module = exact_js::Module::new(APP_BYTECODE, APP, GRANTS).with_native(host);
    // A large account's READY is megabytes of JSON, and parsed it is several times that: a 16 MB READY
    // ran out of the default 64 MiB heap.
    module.set_max_heap(256 << 20);
    module.placed(TYPESCRIPT_PLACEMENT)
}

/// The app's native module: which host this is, which the data module cannot tell
/// (`live.ts` `hostOf`, as `modules/apple/Host.swift` answers on Apple).
struct Host;
impl exact_js::NativeModule for Host {
    fn configure_storage(&mut self, _storage: exact_js::NativeStorage) -> Result<(), String> {
        Ok(())
    }
    fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        match request["op"].as_str() {
            Some("host") => Ok(serde_json::json!({ "host": "windows" })),
            _ => Err("host answers op host".into()),
        }
    }
}
fn host(_source: &str) -> Box<dyn exact_js::NativeModule> {
    Box::new(Host)
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
        "Excord",
        window(),
        PLAN,
        COMPAT,
        composed,
    ));
}
