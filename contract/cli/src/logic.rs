//! Concrete app composition selected at bake, never a Cargo feature on a host.
//! @ref LLP 1029.000 — disabling replacement removes its reachable loader code.

/// Generate `AppData` and `app_data()` for a host's source type and constructor.
/// `mode` is the validated `compat.inputs.rustMode` from the actual target bake.
pub fn rust_entry(data: &str, constructor: &str, mode: &str) -> Result<String, String> {
    let supplemental = if let Some(directory) = std::env::var_os("EXACT_RUST_BUNDLE") {
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::path::Path::new(&directory).join("app.module.json"))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let file = receipt["module"]["file"]
            .as_str()
            .ok_or("supplemental Rust receipt names no module")?;
        if !matches!(
            file,
            "app.module.wasm"
                | "app.module.dylib"
                | "app.module.so"
                | "app.module.dll"
                | "app.module.bin"
        ) {
            return Err("invalid supplemental Rust module filename".into());
        }
        format!("{{ use exact_logic::exact_runner::DataSource; exact_logic::Swappable::{mode}({constructor}).replacement(include_bytes!(concat!(env!(\"OUT_DIR\"), \"/app.plan\")), include_str!(concat!(env!(\"OUT_DIR\"), \"/rust/app.module.json\")), include_bytes!(concat!(env!(\"OUT_DIR\"), \"/rust/{file}\")).to_vec()).expect(\"validated baked Rust pair\") }}")
    } else {
        format!("exact_logic::Swappable::{mode}({constructor})")
    };
    match mode {
        "off" => Ok(format!("type AppData = {data};\n#[allow(dead_code)]\nfn app_data() -> AppData {{ {constructor} }}\n")),
        "native" | "tiered" | "wasm" | "browser" => Ok(format!(
            "exact_logic::configured!(AppData, {data}, || {supplemental});\n#[allow(dead_code)]\nfn app_data() -> AppData {{ Default::default() }}\n"
        )),
        _ => Err(format!("unknown Rust executor {mode:?}")),
    }
}
