//! Build the engine-free Rust library and, when requested, its JSI bindings.

use std::path::{Path, PathBuf};

fn main() {
    let target_vendor = std::env::var("CARGO_CFG_TARGET_VENDOR").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let is_apple = target_vendor == "apple";
    let has_intl = std::env::var_os("CARGO_FEATURE_INTL").is_some();

    build_platform_backends(is_apple);
    if std::env::var_os("CARGO_FEATURE_BINDINGS").is_none() {
        return;
    }
    println!("cargo:rerun-if-env-changed=EXACT_JS_ENGINE");
    if std::env::var("EXACT_JS_ENGINE").as_deref() == Ok("stub") {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        for name in [
            "headers",
            "timers",
            "url",
            "domexception",
            "crypto",
            "events",
            "abort",
            "websocket",
            "blob",
            "fetch",
            "sqlite",
            "structured_clone",
            "harden",
        ] {
            std::fs::write(out_dir.join(format!("{name}.hbc")), [])
                .expect("write stub binding bytecode");
        }
        let harden = out_dir.join("harden.hbc");
        let harden_source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bindings/harden.js");
        println!(
            "cargo:rustc-env=IBEX2_HARDEN_SOURCE_PATH={}",
            harden_source.display()
        );
        println!(
            "cargo:rustc-env=IBEX2_HARDEN_BYTECODE_PATH={}",
            harden.display()
        );
        println!("cargo:rustc-env=IBEX2_BINDINGS_ENGINE_DIGEST=");
        println!("cargo:rustc-env=IBEX2_BINDINGS_BYTECODE_VERSION=0");
        return;
    }

    let headers = required_path("DEP_HERMES_LEAN_INCLUDE_DIR");
    println!("cargo:rerun-if-changed=src/bindings/install.cc");
    println!("cargo:rerun-if-changed=include/ibex2_jsi.h");
    let mut installer = cc::Build::new();
    installer
        .cpp(true)
        .file("src/bindings/install.cc")
        .include(&headers)
        .std("c++17");
    if target_os == "windows" {
        installer.flag("/EHsc").define("NOMINMAX", None);
    }
    // @ref LLP 0057.000#51-included-gated-or-a-crate — D5/D6 require this
    // over-budget family to be absent unless the consumer opts in.
    // Windows compiles the same C-API shims against the SDK's <icu.h>
    // (LLP 0057.000 §5.1.1); its icuuc/icuin link lines also come from
    // hermes-lean-sys `icu`, and the icu.dll-only entry points are bound at
    // run time by intl_icu_windows.cc, so no link argument is needed here.
    let intl_shims = has_intl && (target_os == "linux" || target_os == "windows");
    if intl_shims {
        if target_os == "windows" {
            println!("cargo:rerun-if-changed=src/bindings/intl_icu_windows.h");
            println!("cargo:rerun-if-changed=src/bindings/intl_icu_windows.cc");
            installer.file("src/bindings/intl_icu_windows.cc");
        }
        for source in [
            "src/bindings/intl_number_format.cc",
            "src/bindings/intl_icu.cc",
            "src/bindings/intl_case_icu.cc",
            "src/bindings/intl_datetime_icu.cc",
        ] {
            println!("cargo:rerun-if-changed={source}");
            installer.file(source);
        }
        installer.define("IBEX2_JSI_HAS_INTL", None);
    }
    if is_apple {
        installer.flag("-stdlib=libc++");
    }
    // ICU for the Linux Intl shims is linked by hermes-lean-sys's ICU
    // features, never here, so a runtime graph carries it exactly once.
    installer.compile("ibex2_bindings_install");

    let hermesc = required_path("DEP_HERMES_LEAN_HERMESC_PATH");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let mut scripts = vec![
        "headers",
        "timers",
        "url",
        "domexception",
        "crypto",
        "events",
        "abort",
        "websocket",
        "blob",
        "fetch",
        "sqlite",
    ];
    if intl_shims {
        scripts.extend(["intl_number_format", "intl_case", "intl_datetime"]);
    }
    scripts.push("structured_clone");
    for name in scripts {
        let source = format!("src/bindings/{name}.js");
        println!("cargo:rerun-if-changed={source}");
        compile_javascript(
            &hermesc,
            Path::new(&source),
            &out_dir.join(format!("{name}.hbc")),
        );
    }

    // Decision C makes hardening part of the bindings contract: every
    // embedder, including ibex2-runtime, consumes these exact source/bytes.
    let harden_source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bindings/harden.js");
    println!("cargo:rerun-if-changed={}", harden_source.display());
    let harden_bytecode = out_dir.join("harden.hbc");
    compile_javascript(&hermesc, &harden_source, &harden_bytecode);
    println!(
        "cargo:rustc-env=IBEX2_HARDEN_SOURCE_PATH={}",
        harden_source.display()
    );
    println!(
        "cargo:rustc-env=IBEX2_HARDEN_BYTECODE_PATH={}",
        harden_bytecode.display()
    );
    let bytecode_version = required("DEP_HERMES_LEAN_BYTECODE_VERSION");
    let lean_bytecode_version = std::env::var("DEP_HERMES_LEAN_LEAN_BYTECODE_VERSION").ok();
    if let Some(lean_bytecode_version) = &lean_bytecode_version {
        assert_eq!(
            &bytecode_version, lean_bytecode_version,
            "full and lean Hermes VMs from one resolution must consume the same HBC version"
        );
    }
    println!(
        "cargo:rustc-env=IBEX2_BINDINGS_ENGINE_DIGEST={}",
        required("DEP_HERMES_LEAN_ENGINE_DIGEST")
    );
    if target_os == "linux" {
        // @ref LLP 0057.000#l1--the-bindings-door — this build-dependency
        // context links no VM or ICU data, so it exports both receipt-bound
        // available identities and never invents a selected one from `intl`.
        // Only the normal dependency that emits link lines exports LINKED_*.
        for (source, destination) in [
            ("DEP_HERMES_LEAN_ICU_DATA", "IBEX2_BINDINGS_ICU_DATA"),
            (
                "DEP_HERMES_LEAN_ICU_FULL_DATA",
                "IBEX2_BINDINGS_ICU_FULL_DATA",
            ),
        ] {
            println!(
                "cargo:rustc-env={destination}_ARCHIVE={}",
                required(&format!("{source}_ARCHIVE"))
            );
            println!(
                "cargo:rustc-env={destination}_DIGEST={}",
                required(&format!("{source}_DIGEST"))
            );
        }
    }
    if let Ok(digest) = std::env::var("DEP_HERMES_LEAN_LEAN_ENGINE_DIGEST") {
        println!("cargo:rustc-env=IBEX2_BINDINGS_LEAN_ENGINE_DIGEST={digest}");
    }
    println!("cargo:rustc-env=IBEX2_BINDINGS_BYTECODE_VERSION={bytecode_version}");
    if let Some(lean_bytecode_version) = lean_bytecode_version {
        println!("cargo:rustc-env=IBEX2_BINDINGS_LEAN_BYTECODE_VERSION={lean_bytecode_version}");
    }
}

fn build_platform_backends(is_apple: bool) {
    if !is_apple {
        return;
    }
    for source in [
        "src/transport/darwin_http.mm",
        "src/secrets/darwin_keychain.mm",
    ] {
        println!("cargo:rerun-if-changed={source}");
        compile_objc(source);
    }
    if std::env::var_os("CARGO_FEATURE_WEBSOCKET").is_some() {
        let source = "src/transport/darwin_websocket.mm";
        println!("cargo:rerun-if-changed={source}");
        compile_objc(source);
    }
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=Security");
}

fn compile_objc(source: &str) {
    let stem = Path::new(source)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("Objective-C++ source has a stem");
    cc::Build::new()
        .cpp(true)
        .file(source)
        .flag("-std=c++17")
        .flag("-stdlib=libc++")
        .flag("-fobjc-arc")
        .flag("-x")
        .flag("objective-c++")
        .compile(&format!("ibex2_{stem}"));
}

fn compile_javascript(hermesc: &Path, source: &Path, output: &Path) {
    let status = std::process::Command::new(hermesc)
        .args(["-emit-binary", "-O", "-Xes6-block-scoping", "-out"])
        .arg(output)
        .arg(source)
        .status()
        .unwrap_or_else(|error| panic!("cannot run {}: {error}", hermesc.display()));
    assert!(status.success(), "hermesc failed on {}", source.display());
}

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} was not exported by hermes-lean-sys"))
}

fn required_path(name: &str) -> PathBuf {
    PathBuf::from(required(name))
}
