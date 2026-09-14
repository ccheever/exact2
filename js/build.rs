//! Link the lean Hermes VM when a build of it is present, and compile the
//! test fixture (`tests/fixtures/caltrain.ts`) to bytecode with the bake's
//! own toolchain: Rolldown, then `hermesc` (LLP 1027 D5).
//!
//! The engine is the vanilla Hermes build the ibex repo produces
//! (`ios/Frameworks-vanilla/`, receipt beside it): the bytecode-only
//! `hermesvmlean` archive, JSI, and the headers. iOS uses matching lean CMake
//! builds under `target/hermes-ios` (EXACT_HERMES_IOS_DIR overrides; LLP 1027 D6).
//! All linked engine archives are captured in OUT_DIR for the bake receipt.
//! Without the target archives — a Linux
//! builder today, or a checkout without `../ibex` — this crate still builds,
//! as a stub whose `Module::load` refuses by name, so `cargo build
//! --workspace` is green everywhere and the executor is honest about where
//! it can run. `EXACT_HERMES_DIR`, `EXACT_HERMESC`, and `EXACT_ROLLDOWN`
//! point at the three tools when they are somewhere else.

use std::env;
use std::io::Read;
use std::path::PathBuf;
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
    let ibex = manifest.join("../../ibex");
    let bindings = ibex.join("crates/ibex2");
    for file in [
        "include/ibex2_jsi.h",
        "src/engine/ibex2_jsi.cc",
        "src/bindings/harden.js",
        "src/bindings/sqlite.js",
    ] {
        println!("cargo:rerun-if-changed={}", bindings.join(file).display());
    }
    let engine = env::var("EXACT_HERMES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| ibex.join("ios/Frameworks-vanilla"));
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
    // Provisioning an iOS engine after a stub build must invalidate it.
    // macOS's external SDK archives are captured in OUT_DIR below, not
    // traversed as repository source directories by the bake receipt.
    if target_os == "ios" {
        println!("cargo:rerun-if-changed={}", static_dir.display());
    }
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
        if target_os == "ios" {
            println!("cargo:rerun-if-changed={}", source.display());
        }
        std::fs::copy(&source, out.join(source.file_name().expect("archive name")))
            .unwrap_or_else(|e| panic!("cannot capture {}: {e}", source.display()));
    }

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
        .file(bindings.join("src/engine/ibex2_jsi.cc"))
        .include(bindings.join("include"))
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
    let arch = match env::consts::ARCH {
        "aarch64" => "arm64",
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
        "exact-js: rolldown not found at {} (run `bun install` at the repo root, or set EXACT_ROLLDOWN)",
        rolldown.display()
    );
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
    compile(
        &bindings.join("src/bindings/harden.js"),
        &out.join("storage-harden.hbc"),
    );
    compile(
        &bindings.join("src/bindings/sqlite.js"),
        &out.join("storage-sqlite.hbc"),
    );
    for name in ["caltrain", "castle", "inputs", "ambient-init", "storage"] {
        let source = manifest.join(format!("tests/fixtures/{name}.ts"));
        println!("cargo:rerun-if-changed={}", source.display());
        let script = out.join(format!("{name}.js"));
        let mut header = [0; 128];
        let len = std::fs::File::open(&rolldown)
            .and_then(|mut file| file.read(&mut header))
            .unwrap_or(0);
        let shebang = String::from_utf8_lossy(&header[..len]);
        let javascript = shebang.lines().next().is_some_and(|line| {
            line.starts_with("#!")
                && line
                    .split_whitespace()
                    .any(|word| matches!(word.rsplit('/').next(), Some("node" | "bun")))
        });
        let mut bundler = if javascript {
            let mut command = Command::new("bun");
            command.arg(&rolldown);
            command
        } else {
            Command::new(&rolldown)
        };
        let status = bundler
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
