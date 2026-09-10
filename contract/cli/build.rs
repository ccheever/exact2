//! Keep the compiler engine-free while consuming Ibex's canonical declarations.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("storage.d.ts"), ibex2::bindings::TYPESCRIPT)
        .expect("write Ibex storage declarations");
}
