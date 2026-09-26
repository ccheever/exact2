//! Link the lean Hermes VM when a build of it is present, and compile the
//! test fixture (`tests/fixtures/caltrain.ts`) to bytecode with the bake's
//! own toolchain: Rolldown, then `hermesc` (LLP 1027 D5).
//!
//! The engine is the vanilla Hermes build the ibex repo produces
//! (`ios/Frameworks-vanilla/`, receipt beside it): the bytecode-only
//! `hermesvmlean` archive, JSI, and the headers. iOS uses matching lean CMake
//! builds under `target/hermes-ios` (EXACT_HERMES_IOS_DIR overrides; LLP 1027 D6).
//! All linked engine archives are captured in OUT_DIR for the bake receipt.
//! On macOS, iOS and Linux a missing engine is a build error naming how to
//! provision it; `EXACT_JS_ENGINE=stub` instead builds a stub whose
//! `Module::load` refuses by name. Other targets are always the stub.
//! `EXACT_HERMES_DIR`, `EXACT_HERMESC`, and `EXACT_ROLLDOWN` point at the
//! three tools when they are somewhere else; Linux also honors
//! `HERMES_INCLUDE_DIR` / `HERMES_LIB_DIR`.
//!
//! The pin is vanilla Hermes 260318099.0.0-stable, facebook/hermes
//! `6badada762121682b5481b6124e6c3a991ae6046` (ibex's
//! `ios/Frameworks-vanilla/hermes-input-receipt.json`). SHA-256 of the
//! provisioned inputs (2026-08-28):
//!
//! - macOS `libhermesvmlean_a.a` `494f925f1aa667ebbb622be3da156af9c75465971201d438bf82945b50d36aa6`
//! - macOS `libjsi.a` `b6a497618b6363fb1ed5ed0667b8441769cd232cdfaaa5ba9ed874c9fbb0fdde`
//! - macOS `libboost_context.a` `cb3ffcfa31e515ff03978425e8618992fd77362b92e0ed94487f3e2d531012cb`
//! - `hermesc-macos-arm64` `fa070c2feddee6968c6a5aef1c92bfb84c075be1471c4733ad0361b680d73132`
//! - iOS device `libhermesvmlean_a.a` `2e01ddf9646fdff235092752b53bee5f2f42b6f411c9999c5fd2c32407a39d75`
//! - iOS simulator `libhermesvmlean_a.a` `e78352300b330407a4ae6a94559d97e9f020ca766909999bed31329827ba5234`

use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(exact_js_engine)");
    for var in [
        "BUN",
        "EXACT_JS_ENGINE",
        "EXACT_HERMES_DIR",
        "EXACT_HERMES_IOS_DIR",
        "EXACT_HERMESC",
        "EXACT_ROLLDOWN",
        "HERMES_INCLUDE_DIR",
        "HERMES_LIB_DIR",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-changed=src/shim.cc");
    println!("cargo:rerun-if-changed=src/prelude.js");
    println!("cargo:rerun-if-changed=src/pure.js");

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // ibex2's sources are vendored; the engine builds stay in the ibex checkout.
    let ibex = manifest.join("../../ibex");
    let bindings = manifest.join("../vendor/ibex2");
    for file in [
        "include/ibex2_jsi.h",
        "src/engine/ibex2_jsi.cc",
        "src/bindings/harden.js",
        "src/bindings/sqlite.js",
        "src/bindings/url.js",
    ] {
        println!("cargo:rerun-if-changed={}", bindings.join(file).display());
    }
    let engine = env::var("EXACT_HERMES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| ibex.join("ios/Frameworks-vanilla"));
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target = env::var("TARGET").unwrap_or_default();
    // The iOS input is a pair of lean CMake builds, not the full framework
    // (which also contains a compiler). No engine bytes enter Rust-only apps.
    let ios = env::var_os("EXACT_HERMES_IOS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../target/hermes-ios"));
    let linux = ibex.join("linux-vanilla");
    let (headers, static_dir, engine_lib_name, extra_libs) = if target_os == "ios" {
        let static_dir = ios
            .join(
                if target.ends_with("-sim") || target.starts_with("x86_64-") {
                    "ios-simulator"
                } else {
                    "ios"
                },
            )
            .join("lib");
        let build = static_dir.parent().expect("iOS build directory");
        (
            engine.join("hermes-headers"),
            static_dir.clone(),
            "hermesvmlean_a".to_string(),
            vec![
                build.join("jsi/libjsi.a"),
                build.join("external/boost/boost_1_86_0/libs/context/libboost_context.a"),
            ],
        )
    } else if target_os == "linux" {
        let headers = env::var("HERMES_INCLUDE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| linux.join("hermes-headers"));
        let static_dir = env::var("HERMES_LIB_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| linux.join("lib"));
        let engine_lib_name = "hermesvmlean_a".to_string();
        (
            headers,
            static_dir.clone(),
            engine_lib_name,
            vec![
                static_dir.join("libjsi.a"),
                static_dir.join("libboost_context.a"),
            ],
        )
    } else {
        let static_dir = engine.join("macos-static");
        (
            engine.join("hermes-headers"),
            static_dir.clone(),
            "hermesvmlean_a".to_string(),
            vec![
                static_dir.join("libjsi.a"),
                static_dir.join("libboost_context.a"),
            ],
        )
    };
    // Provisioning an iOS engine after a stub build must invalidate it.
    // macOS's external SDK archives are captured in OUT_DIR below, not
    // traversed as repository source directories by the bake receipt.
    if target_os == "ios" {
        println!("cargo:rerun-if-changed={}", static_dir.display());
    }
    let engine_archive = static_dir.join(format!("lib{engine_lib_name}.a"));
    let hermes_target = matches!(target_os.as_str(), "macos" | "ios" | "linux");
    // The explicit selection wins before any engine input is inspected.
    let stub = env::var("EXACT_JS_ENGINE").as_deref() == Ok("stub");
    let provisioned =
        headers.is_dir() && engine_archive.is_file() && extra_libs.iter().all(|lib| lib.is_file());
    if !hermes_target || stub || !provisioned {
        if hermes_target {
            assert!(
                stub,
                "exact-js: no Hermes for {target_os} at {}. Provision the pinned engine (js/build.rs header): macOS, ibex ./scripts/build-hermes.sh --vanilla (EXACT_HERMES_DIR if elsewhere); iOS, lean builds under target/hermes-ios (LLP 1027 D6; EXACT_HERMES_IOS_DIR); Linux, ibex ./scripts/build-hermes-linux.sh --vanilla --release --intl (HERMES_LIB_DIR). Or set EXACT_JS_ENGINE=stub for an executor that refuses to load.",
                static_dir.display()
            );
            println!("cargo:warning=exact-js: EXACT_JS_ENGINE=stub; the executor refuses to load");
        }
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let mut library_paths = vec![engine_archive];
    library_paths.extend(extra_libs);
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
        .flag("-std=c++17");
    if target_os != "linux" {
        shim.flag("-stdlib=libc++");
    }
    shim.compile("exact_js_shim");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static={engine_lib_name}");
    println!("cargo:rustc-link-lib=static=jsi");
    println!("cargo:rustc-link-lib=static=boost_context");
    if target_os == "linux" {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=icui18n");
        println!("cargo:rustc-link-lib=icuuc");
        println!("cargo:rustc-link-lib=icudata");
    } else {
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
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
        .unwrap_or_else(|_| {
            if cfg!(target_os = "linux") {
                ibex.join(format!("tools/hermes-vanilla/hermesc-linux-{arch}"))
            } else {
                ibex.join(format!("tools/hermes-vanilla/hermesc-macos-{arch}"))
            }
        });
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
    // Async break checks in every loop and function: what lets another thread
    // interrupt a running call (LLP 1048.000 D10), as the bake compiles.
    let compile = |script: &PathBuf, bytecode: &PathBuf| {
        let status = Command::new(&hermesc)
            .args(["-O", "-emit-async-break-check", "-emit-binary", "-out"])
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
    // Pure class shapes share Ibex's URL implementation. All are baked;
    // the runtime only evaluates bytecode, before any application module.
    let prelude = [
        manifest.join("src/pure.js"),
        bindings.join("src/bindings/url.js"),
        manifest.join("src/prelude.js"),
    ]
    .iter()
    .map(|path| std::fs::read_to_string(path).expect("prelude source"))
    .collect::<Vec<_>>()
    .join("\n")
        + "\nglobalThis.__exact_finish_pure();\n";
    let prelude_path = out.join("prelude.js");
    std::fs::write(&prelude_path, prelude).expect("write combined prelude");
    compile(&prelude_path, &out.join("prelude.hbc"));
    compile(
        &bindings.join("src/bindings/harden.js"),
        &out.join("storage-harden.hbc"),
    );
    compile(
        &bindings.join("src/bindings/sqlite.js"),
        &out.join("storage-sqlite.hbc"),
    );
    for name in [
        "caltrain",
        "castle",
        "inputs",
        "ambient-init",
        "storage",
        "pure",
        "spin",
    ] {
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
            // `BUN` names the Bun to run it with, else the first on PATH.
            let mut command = Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()));
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
