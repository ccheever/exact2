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
    // The bake asks every resource for its first frame. This source's only
    // source reaches a filesystem, and a bake has none: `open("")` is the
    // welcome document, compiled in (LLP 1027 D4).
    let baked =
        contract::bake(plan, llp_data::Llp::new()).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        // It reads an llp/ directory from the file system, which an Apple TV has none of.
        Ok("tvos") => panic!(
            "LLP has no tvOS build: it reads documents from a directory an Apple TV does not have"
        ),
        Ok("ios") => "ios",
        _ => "macos",
    };
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let source = llp_data::Llp::new();
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
            "{}\n{}{host}::host!(AppData, PLAN, COMPAT; linked = EXACT_LINKED);\n",
            contract::rust_entry(
                "llp_data::Llp",
                "llp_data::Llp::new()",
                compat.inputs["rustMode"].as_str().unwrap()
            )
            .unwrap(),
            linked
        ),
    )
    .unwrap();
}
