//! Build Exact's small caller-owned Hermes shim and trusted prelude.
//!
//! Ibex owns its JSI adapter and compiled binding scripts. `hermes-lean-sys`
//! owns the matching headers, compiler, archive selection, and native link
//! lines; this build script consumes only the metadata those crates export.

mod engine_os;
mod package_tool;

use engine_os::ENGINE_OS;
use package_tool::package_tool;
use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(exact_js_engine)");
    for name in ["BUN", "EXACT_JS_ENGINE", "EXACT_ROLLDOWN"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for source in [
        "package_tool.rs",
        "engine_os.rs",
        "src/shim.cc",
        "src/pure.js",
        "src/standard.js",
        "src/prelude.js",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }

    if env::var("EXACT_JS_ENGINE").as_deref() == Ok("stub") {
        println!("cargo:warning=exact-js: EXACT_JS_ENGINE=stub; the executor refuses to load");
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if !ENGINE_OS.contains(&target_os.as_str()) {
        return;
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ibex = manifest.join("../vendor/ibex/crates/ibex2");
    let headers = required_path("DEP_HERMES_LEAN_INCLUDE_DIR");
    let hermesc = required_path("DEP_HERMES_LEAN_HERMESC_PATH");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    let mut shim = cc::Build::new();
    if target_os == "macos" {
        // An inherited SDKROOT from an Apple cross-build must not turn this
        // host-side shim into an iOS input.
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
        .include(ibex.join("include"))
        .include(headers)
        .std("c++17");
    if target_os == "windows" {
        shim.static_crt(false)
            .flag("/EHsc")
            .define("NOMINMAX", None)
            .define("_ITERATOR_DEBUG_LEVEL", "0");
    } else if target_os != "linux" {
        shim.flag("-stdlib=libc++");
    }
    shim.compile("exact_js_shim");
    println!("cargo:rustc-cfg=exact_js_engine");

    let compile = |script: &Path, bytecode: &Path| {
        let status = Command::new(&hermesc)
            .args(["-O", "-emit-async-break-check", "-emit-binary", "-out"])
            .arg(bytecode)
            .arg(script)
            .status()
            .unwrap_or_else(|error| panic!("exact-js: cannot run {}: {error}", hermesc.display()));
        assert!(
            status.success(),
            "exact-js: hermesc failed on {}",
            script.display()
        );
    };

    // Ibex installs PURE | CRYPTO | ABORT first. These are only Exact's
    // trusted policy and data-seam layers; application bytecode follows after
    // Adapter::capture_intrinsics() and Adapter::harden().
    let prelude = ["src/pure.js", "src/standard.js", "src/prelude.js"]
        .iter()
        .map(|source| std::fs::read_to_string(manifest.join(source)).expect("prelude source"))
        .collect::<Vec<_>>()
        .join("\n");
    let prelude = if target_os == "windows" {
        "globalThis.__exact_windows_storage = true;\n".to_owned() + &prelude
    } else {
        prelude
    };
    let leaked_abort_hooks =
        prelude.replacen("  delete global.__exact_ibex2_abort_hooks;\n", "", 1);
    assert_ne!(
        leaked_abort_hooks, prelude,
        "the abort-hook reachability fixture must omit the handoff delete"
    );
    let prelude_path = out.join("prelude.js");
    std::fs::write(&prelude_path, &prelude).expect("write combined prelude");
    compile(&prelude_path, &out.join("prelude.hbc"));
    let leaked_path = out.join("prelude-with-abort-hook.js");
    std::fs::write(&leaked_path, leaked_abort_hooks).expect("write abort-hook fixture prelude");
    compile(&leaked_path, &out.join("prelude-with-abort-hook.hbc"));

    let rolldown = env::var("EXACT_ROLLDOWN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| package_tool(&manifest.join(".."), "rolldown"));
    assert!(
        rolldown.is_file(),
        "exact-js: rolldown not found at {} (run `bun install`, or set EXACT_ROLLDOWN)",
        rolldown.display()
    );
    for name in [
        "caltrain",
        "castle",
        "entropy",
        "ecdsa",
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
            let mut command = Command::new(env::var_os("BUN").unwrap_or_else(|| "bun".into()));
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
            .unwrap_or_else(|error| panic!("exact-js: cannot run {}: {error}", rolldown.display()));
        assert!(
            status.success(),
            "exact-js: rolldown failed on {}",
            source.display()
        );
        compile(&script, &out.join(format!("{name}.hbc")));
    }
}

fn required_path(name: &str) -> PathBuf {
    PathBuf::from(
        env::var(name).unwrap_or_else(|_| panic!("{name} was not exported by hermes-lean-sys")),
    )
}
