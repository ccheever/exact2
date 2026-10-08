//! Compile and bake `app.contract` into `OUT_DIR/app.plan` (the feed is not baked: the launch asks
//! `feed()` for it), the compatibility id, and the entry that links what the plan uses
//! (apps/map-demo/apple/build.rs's shape).

use contract::DataSource;

fn main() {
    for p in ["../app.contract", "../svgs.contract", "../lottie.contract", "../rings.contract", "../app.json", "../data", "../assets", "../Cargo.lock", "build.rs"] {
        println!("cargo:rerun-if-changed={p}");
    }
    let plan = contract::compile_path(std::path::Path::new("../app.contract")).unwrap_or_else(|e| panic!("app.contract:{e}"));
    let baked = contract::bake(plan, exact_xheavy_data::XHeavy::for_bake()).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios" | "tvos") => "ios",
        _ => "macos",
    };
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let source = exact_xheavy_data::XHeavy::for_bake();
    let compat = exact_bake::compatibility_id(&app_dir, platform, &target, &manifest, Some(source.grants()))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    assert_eq!(compat.inputs["store"]["L"], "0", "app.json deploy.store must stay 0 (no update store)");
    // What the archive links, as its compatibility inputs name it (LLP 1047.001 D2).
    let linked = exact_bake::apple_link(&compat, "exact_apple");
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{}\n{}exact_apple::host!(AppData, PLAN, COMPAT; linked = EXACT_LINKED);\n",
            contract::rust_entry(
                "exact_xheavy_data::XHeavy",
                "exact_xheavy_data::XHeavy::default()",
                compat.inputs["rustMode"].as_str().unwrap()
            )
            .unwrap(),
            linked
        ),
    )
    .unwrap();
}
