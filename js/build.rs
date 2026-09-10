//! Link the lean Hermes VM when a build of it is present, and compile the
//! test fixture (`tests/fixtures/caltrain.ts`) to bytecode with the bake's
//! own toolchain: Rolldown, then `hermesc` (LLP 1027 D5).
//!
//! Provision the vanilla bytecode-only Hermes archives and headers with
//! `EXACT_HERMES_DIR`, and its matching compiler with `EXACT_HERMESC`.
//! iOS uses matching lean CMake builds under `target/hermes-ios`
//! (`EXACT_HERMES_IOS_DIR` overrides; LLP 1027 D6). Linked archives are
//! captured in OUT_DIR for the bake receipt. Without the target archives this
//! crate builds as a stub whose `Module::load` refuses by name. Ibex binding
//! sources come from the pinned Cargo dependency, never a sibling checkout.

use ibex2::bindings::{HARDEN_SOURCE, JSI_HEADER, JSI_SOURCE, SQLITE_SOURCE, TYPESCRIPT};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(exact_js_engine)");
    for var in [
        "EXACT_HERMES_DIR",
        "EXACT_HERMES_IOS_DIR",
        "EXACT_HERMESC",
        "EXACT_ROLLDOWN",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-changed=src/shim.cc");
    println!("cargo:rerun-if-changed=src/prelude.js");

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for file in [JSI_HEADER, JSI_SOURCE, HARDEN_SOURCE, SQLITE_SOURCE] {
        println!("cargo:rerun-if-changed={file}");
    }
    let Some(engine) = env::var_os("EXACT_HERMES_DIR").map(PathBuf::from) else {
        println!("cargo:warning=exact-js: EXACT_HERMES_DIR is unset — the executor is a stub that refuses to load");
        return;
    };
    let headers = engine.join("hermes-headers");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target = env::var("TARGET").unwrap_or_default();
    // The iOS input is a pair of lean CMake builds, not the full framework
    // (which also contains a compiler). No engine bytes enter Rust-only apps.
    let ios = env::var_os("EXACT_HERMES_IOS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../target/hermes-ios"));
    let static_dir = if target_os == "ios" {
        ios.join(
            if target.ends_with("-sim") || target.starts_with("x86_64-") {
                "ios-simulator"
            } else {
                "ios"
            },
        )
        .join("lib")
    } else {
        engine.join("macos-static")
    };
    let lean = static_dir.join("libhermesvmlean_a.a");
    if !matches!(target_os.as_str(), "macos" | "ios") || !headers.is_dir() || !lean.is_file() {
        println!(
            "cargo:warning=exact-js: no lean Hermes for {target_os} at {} — the executor is a stub that refuses to load (EXACT_HERMES_DIR supplies headers/macOS; EXACT_HERMES_IOS_DIR supplies iOS builds; see LLP 1027 D6)",
            static_dir.display()
        );
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let library_paths = if target_os == "ios" {
        let build = static_dir.parent().expect("iOS build directory");
        vec![
            lean,
            build.join("jsi/libjsi.a"),
            build.join("external/boost/boost_1_86_0/libs/context/libboost_context.a"),
        ]
    } else {
        vec![
            lean,
            static_dir.join("libjsi.a"),
            static_dir.join("libboost_context.a"),
        ]
    };
    // The normal bake receipt inventories OUT_DIR archives. Capture all three
    // actual linked inputs there, including the engine, not just our shim.
    for source in library_paths {
        println!("cargo:rerun-if-changed={}", source.display());
        std::fs::copy(&source, out.join(source.file_name().expect("archive name")))
            .unwrap_or_else(|e| panic!("cannot capture {}: {e}", source.display()));
    }

    println!("cargo:rerun-if-changed={}", headers.display());
    let mut shim = cc::Build::new();
    if target_os == "macos" {
        // Cargo builds the bake's host dependency in the iOS invocation too.
        // Its inherited SDKROOT must not turn the host shim into an iOS input.
        let sdk = Command::new("xcrun")
            .args(["--sdk", "macosx", "--show-sdk-path"])
            .output()
            .expect("macOS SDK");
        assert!(sdk.status.success(), "macOS SDK unavailable");
        shim.flag("-isysroot")
            .flag(String::from_utf8(sdk.stdout).expect("SDK path").trim());
    }
    shim.cpp(true)
        .file("src/shim.cc")
        .file(JSI_SOURCE)
        .include(
            Path::new(JSI_HEADER)
                .parent()
                .expect("JSI include directory"),
        )
        .include(&headers)
        .flag("-std=c++17")
        .flag("-stdlib=libc++")
        .compile("exact_js_shim");
    println!("cargo:rustc-link-search=native={}", out.display());
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
    let hermesc = PathBuf::from(
        env::var_os("EXACT_HERMESC")
            .expect("exact-js: set EXACT_HERMESC to the compiler matching EXACT_HERMES_DIR"),
    );
    let rolldown = env::var("EXACT_ROLLDOWN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest.join("../node_modules/.bin/rolldown"));
    assert!(
        hermesc.is_file(),
        "exact-js: hermesc not found at {} (set EXACT_HERMESC to the matching Hermes compiler)",
        hermesc.display()
    );
    assert!(
        rolldown.is_file(),
        "exact-js: rolldown not found at {} (run `npm install` at the repo root, or set EXACT_ROLLDOWN)",
        rolldown.display()
    );
    // The fixture bundler runs from its staging directory so emitted source
    // labels do not contain Cargo's OUT_DIR hash or checkout location.
    let rolldown = rolldown.canonicalize().expect("Rolldown executable path");
    println!("cargo:rerun-if-changed={}", hermesc.display());
    let compile = |script: &Path, bytecode: &Path| {
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
    compile(Path::new(HARDEN_SOURCE), &out.join("storage-harden.hbc"));
    compile(Path::new(SQLITE_SOURCE), &out.join("storage-sqlite.hbc"));
    // Stage fixtures beside the public binding declarations. Type-only imports
    // resolve to the same Ibex revision as the shim; no generated source-tree files.
    let fixtures = out.join("fixtures");
    std::fs::create_dir_all(&fixtures).expect("fixture directory");
    std::fs::write(fixtures.join("ibex-storage.d.ts"), TYPESCRIPT)
        .expect("write Ibex fixture declarations");
    let names = ["caltrain", "castle", "inputs", "ambient-init", "storage"];
    for name in names {
        let source = manifest.join(format!("tests/fixtures/{name}.ts"));
        println!("cargo:rerun-if-changed={}", source.display());
        std::fs::copy(&source, fixtures.join(format!("{name}.ts")))
            .expect("stage TypeScript fixture");
    }
    for name in names {
        let source = fixtures.join(format!("{name}.ts"));
        let script = out.join(format!("{name}.js"));
        let status = Command::new(&rolldown)
            .current_dir(&fixtures)
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
