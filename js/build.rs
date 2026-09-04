//! Link the lean Hermes VM when a build of it is present, and compile the
//! test fixture (`tests/fixtures/caltrain.ts`) to bytecode with the bake's
//! own toolchain: Rolldown, then `hermesc` (LLP 1027 D5).
//!
//! The engine is the vanilla Hermes build the ibex repo produces
//! (`ios/Frameworks-vanilla/`, receipt beside it): the bytecode-only
//! `hermesvmlean` archive, JSI, and the headers. Without it — a Linux
//! builder today, or a checkout without `../ibex` — this crate still builds,
//! as a stub whose `Module::load` refuses by name, so `cargo build
//! --workspace` is green everywhere and the executor is honest about where
//! it can run. `EXACT_HERMES_DIR`, `EXACT_HERMESC`, and `EXACT_ROLLDOWN`
//! point at the three tools when they are somewhere else.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(exact_js_engine)");
    for var in ["EXACT_HERMES_DIR", "EXACT_HERMESC", "EXACT_ROLLDOWN"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-changed=src/shim.cc");
    println!("cargo:rerun-if-changed=src/prelude.js");

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ibex = manifest.join("../../ibex");
    let engine = env::var("EXACT_HERMES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| ibex.join("ios/Frameworks-vanilla"));
    let headers = engine.join("hermes-headers");
    let static_dir = engine.join("macos-static");
    let lean = static_dir.join("libhermesvmlean_a.a");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" || !headers.is_dir() || !lean.is_file() {
        println!(
            "cargo:warning=exact-js: no lean Hermes for {target_os} at {} — the executor is a stub that refuses to load (EXACT_HERMES_DIR points at a build; ibex: ./scripts/build-hermes.sh --vanilla)",
            engine.display()
        );
        return;
    }

    cc::Build::new()
        .cpp(true)
        .file("src/shim.cc")
        .include(&headers)
        .flag("-std=c++17")
        .flag("-stdlib=libc++")
        .compile("exact_js_shim");
    println!("cargo:rustc-link-search=native={}", static_dir.display());
    println!("cargo:rustc-link-lib=static=hermesvmlean_a");
    println!("cargo:rustc-link-lib=static=jsi");
    println!("cargo:rustc-link-lib=static=boost_context");
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-cfg=exact_js_engine");

    // The prelude and the fixtures: the prelude straight through hermesc;
    // each fixture TypeScript → one script (Rolldown) → bytecode (hermesc),
    // the bake's own two steps, into OUT_DIR for the crate and its tests.
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        _ => "x64",
    };
    let hermesc = env::var("EXACT_HERMESC")
        .map(PathBuf::from)
        .unwrap_or_else(|_| ibex.join(format!("tools/hermes-vanilla/hermesc-macos-{arch}")));
    let rolldown = env::var("EXACT_ROLLDOWN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest.join("../node_modules/.bin/rolldown"));
    assert!(
        hermesc.is_file(),
        "exact-js: hermesc not found at {} (EXACT_HERMESC, or ibex: ./scripts/build-hermes.sh --vanilla)",
        hermesc.display()
    );
    assert!(
        rolldown.is_file(),
        "exact-js: rolldown not found at {} (run `npm install` at the repo root, or set EXACT_ROLLDOWN)",
        rolldown.display()
    );
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let compile = |script: &PathBuf, bytecode: &PathBuf| {
        let status = Command::new(&hermesc)
            .args(["-O", "-emit-binary", "-out"])
            .arg(bytecode)
            .arg(script)
            .status()
            .unwrap_or_else(|e| panic!("exact-js: cannot run {}: {e}", hermesc.display()));
        assert!(
            status.success(),
            "exact-js: hermesc failed on {}",
            script.display()
        );
    };
    compile(&manifest.join("src/prelude.js"), &out.join("prelude.hbc"));
    for name in ["caltrain", "castle", "inputs", "ambient-init"] {
        let source = manifest.join(format!("tests/fixtures/{name}.ts"));
        println!("cargo:rerun-if-changed={}", source.display());
        let script = out.join(format!("{name}.js"));
        let status = Command::new(&rolldown)
            .arg(&source)
            .args(["--format", "iife", "--file"])
            .arg(&script)
            .status()
            .unwrap_or_else(|e| panic!("exact-js: cannot run {}: {e}", rolldown.display()));
        assert!(
            status.success(),
            "exact-js: rolldown failed on {}",
            source.display()
        );
        compile(&script, &out.join(format!("{name}.hbc")));
    }
}
