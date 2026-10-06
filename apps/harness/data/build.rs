//! Generate the seam's Rust types from `../shapes.contract` with
//! `contract rust` (LLP 1101.002 P9), into `OUT_DIR`, as that generator's
//! documentation says: a field added to a shape is a compile error here,
//! not a record built one position off. The declaration file needs a
//! component to compile; a bare one is added, and every declared shape is
//! generated whether or not it uses it.

fn main() {
    let path = std::path::Path::new("../shapes.contract");
    println!("cargo:rerun-if-changed={}", path.display());
    let shapes = std::fs::read_to_string(path).expect("../shapes.contract");
    let source = format!("{shapes}\ncomponent Seam\n  view\n    text \"\"\n");
    let plan = contract::compile(&source)
        .unwrap_or_else(|e| panic!("../shapes.contract does not compile: {e}"));
    let rust = contract::rust(&plan).expect("contract rust");
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("shapes.rs");
    std::fs::write(out, rust).expect("write shapes.rs");
}
