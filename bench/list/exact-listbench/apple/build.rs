//! Compile and bake `app.contract` into `OUT_DIR/app.plan` (the 10,000 rows
//! are the resource's compiled boot value), the compatibility id, and the
//! entry that links what the plan uses (apps/hello/apple/build.rs's shape).

use contract::DataSource;

fn main() {
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../Cargo.lock");
    let plan = contract::compile_path(std::path::Path::new("../app.contract"))
        .unwrap_or_else(|e| panic!("app.contract:{e}"));
    let baked = contract::bake(plan, exact_listbench_data::ListBench::default())
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios" | "tvos") => "ios",
        _ => "macos",
    };
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let source = exact_listbench_data::ListBench::default();
    let compat = exact_bake::compatibility_id(&app_dir, platform, &target, &manifest, Some(source.grants()))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    assert_eq!(compat.inputs["store"]["L"], "0", "app.json deploy.store must stay 0 (no update store)");
    let linked = exact_bake::apple_link(&compat, "exact_apple");
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{}\n{}exact_apple::host!(AppData, PLAN, COMPAT; linked = EXACT_LINKED);\n",
            contract::rust_entry(
                "exact_listbench_data::ListBench",
                "exact_listbench_data::ListBench::default()",
                compat.inputs["rustMode"].as_str().unwrap()
            )
            .unwrap(),
            linked
        ),
    )
    .unwrap();
}
