//! Bake `app.contract` through `app.ts` into `OUT_DIR` at build time: the plan,
//! the module's Hermes bytecode, its identity and the compatibility id, for
//! `src/lib.rs` and `exact_linux::canvas_jni!` to embed, and the hatches'
//! install and the plan's uses (`contract::native::android_entry`). The bake is the Linux
//! host's (`apps/messages/linux/build.rs`): the Canvas host is that host's
//! presenter under Android's own Canvas, and has no update store either.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // The library is empty off Android (src/lib.rs): nothing to bake.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    exact_js_bake::build(std::path::Path::new(".."), "linux").expect("bake Lights");
    // The app's hatches and what its plan links (LLP 1115.001.000 D2, D4), from the plan the bake wrote.
    println!("cargo:rerun-if-changed=../app.json");
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let bytes = std::fs::read(out_dir.join("app.plan")).expect("the bake's plan");
    let plan =
        exact_plan::Plan::decode(&bytes).unwrap_or_else(|e| panic!("the bake's plan: {e:?}"));
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    contract::native::android_entry(&app_dir, &manifest, &plan, &out_dir)
        .unwrap_or_else(|e| panic!("android: {e}"));
}
