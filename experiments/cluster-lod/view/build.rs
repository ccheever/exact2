fn main() {
    let mut failures = Vec::new();
    let mut count = 0;
    for entry in std::fs::read_dir("shaders").expect("shader directory") {
        let path = entry.expect("shader entry").path();
        if path.extension().is_none_or(|x| x != "wgsl") {
            continue;
        }
        count += 1;
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path).expect("shader source");
        let result = naga::front::wgsl::parse_str(&text)
            .map_err(|e| e.emit_to_string(&text))
            .and_then(|module| {
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::empty(),
                )
                .validate(&module)
                .map(|_| ())
                .map_err(|e| e.to_string())
            });
        if let Err(error) = result {
            failures.push(format!("{}: {error}", path.display()));
        }
    }
    println!(
        "cargo:warning=WGSL validation: {count} files, {} failures",
        failures.len()
    );
    assert!(count > 0 && failures.is_empty(), "{}", failures.join("\n"));
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .opt_level(2)
            .debug(false)
            .include("../vendor/meshoptimizer/src")
            .files([
                "../vendor/meshoptimizer/src/allocator.cpp",
                "../vendor/meshoptimizer/src/vcacheoptimizer.cpp",
                "../vendor/meshoptimizer/src/vfetchoptimizer.cpp",
            ])
            .compile("clod_view_cache");
    }
}
