//! Compile and bake `app.contract` into `OUT_DIR/app.plan` at build time, so
//! the library carries its plan and the app links exactly one archive.

use contract::DataSource;

fn main() {
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../../markdown/parse");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    // Bake only the default 16 KiB fixture and its first 40 blocks.
    let baked = contract::bake(plan, markdown_stress_data::MarkdownStress::default())
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => "ios",
        _ => "macos",
    };
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let source = markdown_stress_data::MarkdownStress::default();
    let grants = source.grants();
    let compat = exact_bake::compatibility_id(&app_dir, platform, &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    let host = if compat.inputs["store"]["L"] == "0" {
        "exact_apple"
    } else {
        "exact_apple_update"
    };
    // What the archive links, as its compatibility inputs name it (LLP 1047.001 D2).
    let linked = exact_bake::apple_link(&compat, host);
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{}\nfn region_launch() -> Option<exact_apple::content_region::ContentRegionRegistration> {{
    let value = match std::env::var(\"EXACT_CONTENT_REGION\") {{ Ok(v) => v, Err(std::env::VarError::NotPresent) => return None, Err(_) => panic!(\"invalid content-region launch value\") }};
    if cfg!(target_os = \"ios\") {{ panic!(\"content-region AppKit trial is unavailable on iOS\"); }}
    let activate = match value.as_str() {{ \"1048576\" => \"launchParagraph1MiB\", \"4194304\" => \"launchParagraph4MiB\", _ => panic!(\"EXACT_CONTENT_REGION requires 1048576 or 4194304\") }};
    Some(exact_apple::content_region::ContentRegionRegistration {{ activate: Some(activate), owner: \"markdown-region-owner\", content: \"markdown-region-content\", pending: \"markdown-region-pending\" }})
}}\n{}{host}::host!(AppData, PLAN, COMPAT, None, ::std::ptr::null(), AppData::default, region_launch(); linked = EXACT_LINKED);\n",
            contract::rust_entry(
                "markdown_stress_data::NativeMarkdownStress",
                "markdown_stress_data::NativeMarkdownStress::default()",
                compat.inputs["rustMode"].as_str().unwrap()
            )
            .unwrap(),
            linked
        ),
    )
    .unwrap();
}
