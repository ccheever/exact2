//! Bake Caltrain through its Rust data source. Apple app builds link the
//! plan separately; direct Cargo and embedded builds carry it in the archive.

use contract::DataSource;

fn main() {
    if exact_bake::take_linked(&["entry.rs"]).unwrap_or_else(|e| panic!("linked bake: {e}")) {
        return;
    }
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    println!("cargo:rerun-if-changed=build.rs");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let entries = contract::snapshot::entries(std::path::Path::new("..")).unwrap();
    contract::snapshot::check_refreshes(&entries, &mut caltrain_data::Caltrain)
        .unwrap_or_else(|e| panic!("widget refresh: {e}"));
    let baked =
        contract::bake(plan, caltrain_data::Caltrain).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    // The compatibility id (LLP 1030 D3a) for this platform, beside the
    // plan: what a bundle may depend on and this binary cannot replace.
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../gpu/shaders");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        // tvOS and watchOS reuse the manifest's iOS policy, including interpreted Rust updates.
        Ok("ios" | "tvos" | "watchos") => "ios",
        _ => "macos",
    };
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let grants = caltrain_data::Caltrain.grants();
    let compat = exact_bake::compatibility_id_sources_with_entries(
        &app_dir,
        platform,
        &target,
        &manifest,
        Some(grants),
        None,
        Some(&entries),
    )
    .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    let host = if compat.inputs["store"]["L"] == "0" {
        "exact_apple"
    } else {
        "exact_apple_update"
    };
    // What the archive links, as its compatibility inputs name it (LLP 1047.001 D2).
    let linked = exact_bake::apple_link(&compat, host);
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    let pair = if std::env::var_os("EXACT_BAKED").is_some() {
        std::fs::write(out_dir.join("app.hbc"), []).unwrap();
        "fn exact_plan() -> &'static [u8] { exact_apple::baked::plan() }\n\
         fn exact_compat() -> &'static str { exact_apple::baked::compat() }"
    } else {
        "fn exact_plan() -> &'static [u8] { include_bytes!(concat!(env!(\"OUT_DIR\"), \"/app.plan\")) }\n\
         fn exact_compat() -> &'static str { include_str!(concat!(env!(\"OUT_DIR\"), \"/compat.json\")) }"
    };
    let entry = format!(
        "{pair}\n{}\n{}{host}::host!(AppData, exact_plan(), exact_compat(); linked = EXACT_LINKED);\n",
        contract::rust_entry(
            "caltrain_data::Caltrain",
            "caltrain_data::Caltrain",
            compat.inputs["rustMode"].as_str().unwrap()
        ).unwrap(),
        linked
    );
    let path = out_dir.join("entry.rs");
    if std::fs::read(&path).ok().as_deref() != Some(entry.as_bytes()) {
        std::fs::write(path, entry).unwrap();
    }
}
