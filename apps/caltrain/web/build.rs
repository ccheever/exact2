//! Compile and bake `app.contract` into `OUT_DIR/app.plan` at build time, so
//! the wasm carries its plan and the page fetches exactly one file.

fn main() {
    println!("cargo:rerun-if-changed=../app.contract");
    println!("cargo:rerun-if-changed=build.rs");
    let src = std::fs::read_to_string("../app.contract").expect("apps/caltrain/app.contract");
    let plan = match contract::compile(&src) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let baked =
        contract::bake(plan, caltrain_data::Caltrain).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("app.plan");
    std::fs::write(out, baked.encode()).unwrap();
}
