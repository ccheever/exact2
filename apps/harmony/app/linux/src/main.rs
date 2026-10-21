//! Harmony on Linux (and Android): the Contract UI and the TypeScript data
//! module, over the native Snapback4 device.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

/// The Snapback4 device as the data module's native module (snapback4/README.md, "From TypeScript").
struct Snapback(exact_snapback4::Module);
impl exact_js::NativeModule for Snapback {
    fn configure_storage(&mut self, data: std::path::PathBuf, cache: std::path::PathBuf, temporary: std::path::PathBuf) -> Result<(), String> {
        self.0.configure_storage(data, cache, temporary)
    }
    fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        self.0.call(request)
    }
}
fn snapback(_source: &str) -> Box<dyn exact_js::NativeModule> {
    Box::new(Snapback(exact_snapback4::Module::new(APP, GRANTS).expect("Snapback4 takes the app's baked identity")))
}

type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS)
        .with_canvas_surfaces(CANVAS_SURFACES)
        .with_native(snapback)
        .placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
fn main() {
    std::process::exit(exact_linux::run::<AppData>(PLAN, COMPAT));
}
