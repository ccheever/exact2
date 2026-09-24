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

/// The web entry's `EXACT_LINKED` (LLP 1047 D3): the capabilities `plan`
/// uses, each registered from `exact-web-capabilities`, so the linker drops
/// the rest. A development build links every capability (D7): the dev loop
/// restarts from new plans without rebuilding the wasm, so `host/web/dev.mjs`
/// sets `EXACT_WEB_LINK=all`.
pub fn web_linked(plan: &exact_plan::Plan) -> String {
    use exact_runner::{Capability, Uses};
    println!("cargo:rerun-if-env-changed=EXACT_WEB_LINK");
    let uses = if std::env::var_os("EXACT_WEB_LINK").is_some_and(|v| v == "all") {
        Capability::ALL.into_iter().fold(Uses::NONE, Uses::with)
    } else {
        exact_runner::uses(plan)
    };
    let names: Vec<&str> = uses.iter().map(|c| c.name()).collect();
    let mut entry = format!("/// What this artifact links beyond the core (LLP 1047 D3).\nconst EXACT_LINKED: ::exact_web::Linked = ::exact_web_capabilities::linked!({});\n", names.join(", "));
    // A capability's export group, where it has one.
    for capability in uses.iter() {
        match capability {
            Capability::Motion => entry.push_str("::exact_web::motion_exports!();\n"),
            Capability::Collections => entry.push_str("::exact_web::list_exports!();\n"),
            // Drag's input rides motion's export.
            Capability::Markdown | Capability::Drag => {}
        }
    }
    entry
}
