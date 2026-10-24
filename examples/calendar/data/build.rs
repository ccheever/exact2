//! The module binding from the app's plan (`contract::module_rust`, LLP 1133
//! §5), into `OUT_DIR`: generated as each build compiles the Contract, never
//! committed. The data crate reads the person's calendars through it.

fn main() {
    let path = std::path::Path::new("../app.contract");
    contract::rerun_if_changed(path);
    println!("cargo:rerun-if-changed=../src");
    let plan = contract::compile_path(path).unwrap_or_else(|e| panic!("app.contract:{e}"));
    let rust = contract::module_rust(&plan).unwrap_or_else(|e| panic!("module_rust: {e}"));
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("modules.rs");
    std::fs::write(out, rust).expect("write modules.rs");
}
