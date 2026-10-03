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

/// The Rust executor a web entry links (LLP 1047 D3): the compatibility
/// inputs' `rustMode`, or `off` when the manifest names no `rust.module`. A
/// browser then has no Rust module to swap in (the dev loop builds one only
/// for a declared module), so it links no executor.
pub fn web_rust_mode(inputs: &serde_json::Value) -> &str {
    let module = inputs["rustModule"]
        .as_str()
        .is_some_and(|module| !module.is_empty());
    match inputs["rustMode"].as_str() {
        Some(mode) if module => mode,
        _ => "off",
    }
}

/// Whether the app's Rust data crate (`../data` beside the web crate that
/// builds this) names a wide colour function in its source: Canvas 2D's
/// `lab()`, `lch()`, `oklab()`, `oklch()` or `color()` (LLP 1056 §8.2).
/// TypeScript draws parse colours in their own recorder, not the wasm.
fn names_wide_colors() -> bool {
    let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") else {
        return false;
    };
    let data = std::path::Path::new(&dir).join("../data");
    // A missing path would rerun the build script on every build (Cargo
    // counts it as changed); an app without a data crate names none.
    if !data.exists() {
        return false;
    }
    println!("cargo:rerun-if-changed={}", data.display());
    fn scan(dir: &std::path::Path) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|e| {
            let p = e.path();
            if p.is_dir() {
                return scan(&p);
            }
            p.extension().is_some_and(|x| x == "rs")
                && std::fs::read_to_string(&p).is_ok_and(|s| {
                    // Inside string literals only: the odd pieces between
                    // quotes on a line.
                    s.lines().any(|line| {
                        line.split('"').skip(1).step_by(2).any(|lit| {
                            ["lab(", "lch(", "oklab(", "oklch(", "color("]
                                .iter()
                                .any(|f| lit.contains(f))
                        })
                    })
                })
        })
    }
    scan(&data)
}

/// Whether the bake's grant ceiling (every source's grants, LLP 1027.001)
/// names `capability`: a device capability its sources may use (LLP 1069),
/// linked by that grant, since no plan row says so.
fn grants(inputs: &serde_json::Value, capability: &str) -> bool {
    inputs["grantCeiling"].as_str().is_some_and(|ceiling| {
        ceiling
            .lines()
            .any(|line| line.split_whitespace().next() == Some(capability))
    })
}

/// The web entry's `EXACT_LINKED` (LLP 1047 D3): the capabilities `plan`
/// uses, each registered from `exact-web-capabilities`, so the linker drops
/// the rest. A development build links every capability (D7): the dev loop
/// restarts from new plans without rebuilding the wasm, so `host/web/dev.mjs`
/// sets `EXACT_WEB_LINK=all`.
///
/// Beside it, `EXACT_REPLACEMENT`: whether a running page can take a new data
/// module. The dev loop's can, and so can a production client whose
/// compatibility `inputs` name a Rust module replaced in the browser (LLP
/// 1029.000); every other build refuses `exact_boot_module` by name.
pub fn web_linked(plan: &exact_plan::Plan, inputs: &serde_json::Value) -> String {
    use exact_runner::{Capability, Uses};
    println!("cargo:rerun-if-env-changed=EXACT_WEB_LINK");
    let all = std::env::var_os("EXACT_WEB_LINK").is_some_and(|v| v == "all");
    let uses = if all {
        Capability::ALL.into_iter().fold(Uses::NONE, Uses::with)
    } else {
        exact_runner::uses(plan)
    };
    let replacement = all
        || (inputs["rustMode"] == "browser"
            && inputs["rustModule"]
                .as_str()
                .is_some_and(|module| !module.is_empty()));
    // Inspection is linked by policy, not by use: in production too, so the
    // smoked artifact is the shipped one (LLP 1047 §10, Q3).
    let names: Vec<&str> = uses
        .iter()
        .map(|c| c.name())
        .chain(["inspection"])
        .chain((all || names_wide_colors()).then_some("canvas_colors"))
        .chain((all || grants(inputs, "auth.session")).then_some("auth"))
        .collect();
    let mut entry = format!("/// What this artifact links beyond the core (LLP 1047 D3).\nconst EXACT_LINKED: ::exact_web::Linked = ::exact_web_capabilities::linked!({});\n", names.join(", "));
    entry.push_str(&format!("/// Whether a running page can take a new data module (LLP 1029.000).\nconst EXACT_REPLACEMENT: bool = {replacement};\n"));
    // A capability's export group, where it has one.
    for capability in uses.iter() {
        match capability {
            Capability::Motion => entry.push_str("::exact_web::motion_exports!();\n"),
            Capability::Collections => entry.push_str("::exact_web::list_exports!();\n"),
            Capability::Surfaces => entry.push_str("::exact_web::surface_exports!();\n"),
            // Drag's input rides motion's export; the others have none.
            Capability::Markdown
            | Capability::Drag
            | Capability::Router
            | Capability::Format
            | Capability::Materials
            | Capability::Backdrop
            | Capability::Share
            | Capability::Documents
            | Capability::Picker
            | Capability::Timelines
            | Capability::TextTransform
            | Capability::Effects
            | Capability::Animations
            | Capability::Gradients
            | Capability::Grid
            | Capability::Geometry
            | Capability::Segments => {}
        }
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::web_rust_mode;

    #[test]
    fn a_web_entry_links_a_rust_executor_only_for_a_declared_module() {
        let module = serde_json::json!({"rustMode": "browser", "rustModule": "app-logic"});
        assert_eq!(web_rust_mode(&module), "browser");
        let none = serde_json::json!({"rustMode": "browser", "rustModule": null});
        assert_eq!(web_rust_mode(&none), "off");
        let off = serde_json::json!({"rustMode": "off", "rustModule": "app-logic"});
        assert_eq!(web_rust_mode(&off), "off");
    }
}
