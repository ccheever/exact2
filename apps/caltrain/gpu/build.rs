//! Validate every shader under `shaders/` at build (LLP 1009 D5): a bad one
//! fails the build with its line and column, never a canvas at runtime.

fn main() {
    println!("cargo:rerun-if-changed=shaders");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("shaders/")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "wgsl"))
        .collect();
    entries.sort();
    for path in entries {
        println!("cargo:rerun-if-changed={}", path.display());
        let src = std::fs::read_to_string(&path).unwrap();
        let module = match naga::front::wgsl::parse_str(&src) {
            Ok(m) => m,
            Err(e) => panic!("{}: {}", path.display(), e.emit_to_string(&src)),
        };
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        if let Err(e) = validator.validate(&module) {
            panic!("{}: {}", path.display(), e.emit_to_string(&src));
        }
    }
}
