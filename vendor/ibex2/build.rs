//! Build Ibex 2 against **vanilla** Hermes.
//!
//! The engine is opt-in behind the `hermes` feature so the workspace stays
//! green on a machine that has not built one. Produce it with:
//!
//!     ./scripts/build-hermes.sh --vanilla          # Apple
//!     ./scripts/build-hermes-linux.sh --vanilla    # Linux
//!     ./scripts/build-hermes-windows.ps1 -Vanilla  # Windows (MSVC shell)
//!
//! This deliberately points at the platform's `Frameworks-vanilla/`, never at
//! the legacy patched install. Linking the reviewed patched engine here would
//! silently reintroduce the fork LLP 0060 D3 retires.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/engine/hermes_shim.cc");
    println!("cargo:rerun-if-changed=src/engine/intl_number_format.cc");
    println!("cargo:rerun-if-changed=src/engine/intl_icu.cc");
    println!("cargo:rerun-if-changed=src/engine/intl_case_icu.cc");
    println!("cargo:rerun-if-changed=src/engine/intl_datetime_icu.cc");
    println!("cargo:rerun-if-changed=tests/embedding.cc");
    println!("cargo:rerun-if-changed=src/engine/ibex2_jsi.cc");
    println!("cargo:rerun-if-changed=include/ibex2_jsi.h");
    println!("cargo:rerun-if-env-changed=IBEX2_VANILLA_HERMES_DIR");
    println!("cargo:rerun-if-env-changed=IBEX2_HERMESC");

    let target_vendor = std::env::var("CARGO_CFG_TARGET_VENDOR").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let is_apple = target_vendor == "apple";
    let is_windows = target_os == "windows";

    // The Apple platform transport (LLP 0057 §3). Objective-C++ with ARC, so
    //
    // Compiled whether or not there is an engine: a Rust consumer of the
    // standard library (LLP 0068) has no engine in the process and still
    // needs the platform's `fetch` underneath it.
    // NSURLSession manages its own object graph and this file does not.
    if is_apple {
        println!("cargo:rerun-if-changed=src/engine/darwin_http.mm");
        cc::Build::new()
            .cpp(true)
            .file("src/engine/darwin_http.mm")
            .flag("-std=c++17")
            .flag("-stdlib=libc++")
            .flag("-fobjc-arc")
            .flag("-x")
            .flag("objective-c++")
            .compile("ibex2_darwin_http");
        // A listening WebSocket (LLP 0059.000 §3.12), the same way.
        println!("cargo:rerun-if-changed=src/engine/darwin_websocket.mm");
        cc::Build::new()
            .cpp(true)
            .file("src/engine/darwin_websocket.mm")
            .flag("-std=c++17")
            .flag("-stdlib=libc++")
            .flag("-fobjc-arc")
            .flag("-x")
            .flag("objective-c++")
            .compile("ibex2_darwin_websocket");
    }
    // The Keychain behind `SecretStore` (LLP 0069 §3): the same shape, one
    // more framework.
    if is_apple {
        println!("cargo:rerun-if-changed=src/engine/darwin_keychain.mm");
        cc::Build::new()
            .cpp(true)
            .file("src/engine/darwin_keychain.mm")
            .flag("-std=c++17")
            .flag("-stdlib=libc++")
            .flag("-fobjc-arc")
            .flag("-x")
            .flag("objective-c++")
            .compile("ibex2_darwin_keychain");
    }
    if is_apple {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Security");
    }

    if std::env::var("CARGO_FEATURE_HERMES").is_err() {
        return;
    }
    assert!(
        is_apple || target_os == "linux" || is_windows,
        "unsupported Hermes platform: {target_os}"
    );
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x64",
        other => panic!("unsupported Hermes architecture: {other:?}"),
    };
    assert!(
        !is_windows || arch == "x64",
        "Windows Hermes currently supports x64 only"
    );

    // canonicalize produces a verbatim Windows path that MSVC's include
    // search does not accept. Cargo already supplies an absolute path.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .ancestors()
        .nth(2)
        .expect("crate lives two levels below the repo root")
        .to_path_buf();

    let engine_dir = std::env::var("IBEX2_VANILLA_HERMES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if is_apple {
                repo_root.join("ios/Frameworks-vanilla")
            } else if is_windows {
                repo_root.join(format!("tools/hermes-vanilla/windows-{arch}"))
            } else {
                repo_root.join("linux/Frameworks-vanilla")
            }
        });

    let headers = engine_dir.join("hermes-headers");
    let static_dir = engine_dir.join(if is_apple {
        "macos-static"
    } else if is_windows {
        "windows-static"
    } else {
        "linux-static"
    });

    // Fail loud rather than emitting a link line that dies at the last step
    // with an unresolved-symbol wall: the actionable message is here.
    for required in [&headers, &static_dir] {
        assert!(
            required.exists(),
            "vanilla Hermes not found at {}\n\
             build it with the platform's build-hermes script and --vanilla\n\
             (or point IBEX2_VANILLA_HERMES_DIR at an existing one)",
            required.display()
        );
    }

    let mut shim = cc::Build::new();
    shim.cpp(true)
        .file("src/engine/hermes_shim.cc")
        .file("src/engine/ibex2_jsi.cc")
        .file("tests/embedding.cc")
        .include(&headers)
        .std("c++17");
    // @ref LLP 0068#windows-host-and-engine — static MSVC embedding uses
    // ordinary C++ exceptions at the JSI boundary, never DLL imports.
    if is_windows {
        shim.flag("/EHsc").define("NOMINMAX", None);
    }
    if target_os == "linux" {
        shim.file("src/engine/intl_number_format.cc")
            .file("src/engine/intl_icu.cc")
            .file("src/engine/intl_case_icu.cc")
            .file("src/engine/intl_datetime_icu.cc")
            .define("IBEX2_JSI_HAS_INTL", None);
    }
    if is_apple {
        shim.flag("-stdlib=libc++");
    }
    shim.compile("ibex2_hermes_shim");

    // The engine this binary links, as a digest baked into it. Artifacts are
    // keyed by it at build time and the manifest is checked against it at run
    // time, so the runtime never hashes anything to know which engine it is —
    // the previous design SHA-256'd the framework dylib on disk at every start,
    // 25 ms of a 30 ms budget, to verify a file it does not even run
    // (issues/20260829-run-hashes-the-engine-on-every-start.md). Hashed here,
    // once per link, of the archive actually linked.
    let archive = static_dir.join(if is_windows {
        "hermesvm_a.lib"
    } else {
        "libhermesvm_a.a"
    });
    println!("cargo:rerun-if-changed={}", archive.display());
    let archive_bytes = std::fs::read(&archive)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", archive.display()));
    let digest = <sha2::Sha256 as sha2::Digest>::digest(&archive_bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    println!("cargo:rustc-env=IBEX2_LINKED_ENGINE_DIGEST=sha256-{hex}");

    // The runtime's own JavaScript — the ESM helpers, Headers, timers, URL,
    // and the freeze — compiled to bytecode here so a runtime never parses
    // them: rules/RULES.md says the boot path compiles nothing, and that was
    // true of application code while the bindings were still evaluated from
    // source at every start (~0.7 ms of a 1.8 ms floor, LLP 0063 §2). The
    // hermesc is the one beside the vanilla engine, from the same install as
    // the archive linked above, so the bytecode version matches the VM.
    let hermesc = std::env::var("IBEX2_HERMESC")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let suffix = if is_windows { ".exe" } else { "" };
            repo_root.join(format!(
                "tools/hermes-vanilla/hermesc-{target_os}-{arch}{suffix}"
            ))
        });
    assert!(
        hermesc.exists(),
        "hermesc not found at {}\n\
         build it with the platform's build-hermes script and --vanilla\n\
         (or point IBEX2_HERMESC at one)",
        hermesc.display()
    );
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let mut bindings = vec![
        "esm",
        "headers",
        "timers",
        "url",
        "domexception",
        "crypto",
        "abort",
        "fetch",
        "sqlite",
        "harden",
    ];
    if target_os == "linux" {
        bindings.push("intl_number_format");
        bindings.push("intl_case");
        bindings.push("intl_datetime");
    }
    // It installs last even though compilation order is not observable; keep
    // this inventory in the same conceptual order as bindings::scripts.
    bindings.push("structured_clone");
    for name in bindings {
        let source = format!("src/bindings/{name}.js");
        println!("cargo:rerun-if-changed={source}");
        let artifact = out_dir.join(format!("{name}.hbc"));
        let status = std::process::Command::new(&hermesc)
            .args(["-emit-binary", "-O", "-out"])
            .arg(&artifact)
            .arg(&source)
            .status()
            .unwrap_or_else(|e| panic!("cannot run {}: {e}", hermesc.display()));
        assert!(status.success(), "hermesc failed on {source}");
    }

    println!("cargo:rustc-link-search=native={}", static_dir.display());
    println!("cargo:rustc-link-lib=static=hermesvm_a");
    println!("cargo:rustc-link-lib=static=jsi");
    println!("cargo:rustc-link-lib=static=boost_context");
    if is_apple {
        println!("cargo:rustc-link-lib=c++");
        // Hermes's Apple platform-unicode path (PlatformUnicodeCF.cpp) calls
        // CFLocale/CFString directly for case conversion and normalization.
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
    } else if is_windows {
        println!("cargo:rustc-link-lib=icuuc");
        println!("cargo:rustc-link-lib=icuin");
        println!("cargo:rustc-link-lib=dbghelp");
        println!("cargo:rustc-link-lib=version");
        println!("cargo:rustc-link-lib=psapi");
        println!("cargo:rustc-link-lib=winmm");
    } else {
        // The Linux vanilla build enables Intl. Close ICU statically so a
        // published executable does not acquire an undeclared libicu runtime
        // dependency; libstdc++/dl/pthread/m are ordinary system ABI edges.
        println!("cargo:rustc-link-lib=static=icui18n");
        println!("cargo:rustc-link-lib=static=icuuc");
        println!("cargo:rustc-link-lib=static=icudata");
        println!("cargo:rustc-link-lib=static=tinfo");
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
    }
}
