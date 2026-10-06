//! Resolve, verify, cache, and link the pinned unmodified Hermes engine.

mod build_support;

use build_support::{digest_file, hermesc_bytecode_version, rerun_paths, resolve_engine_directory};
use std::path::Path;

const LINUX_ICU_I18N: &str = "icui18n";
const LINUX_ICU_UC: &str = "icuuc";
const LINUX_ICU_DATA: &str = "icudata";
const LINUX_ICU_FULL_DATA: &str = "icudata-full";
/// The Windows SDK's frozen ICU import libraries (`icuuc.dll`/`icuin.dll`,
/// forwarders into the OS `icu.dll`) and the OS ICU DLL itself.
const WINDOWS_ICU_UC: &str = "icuuc";
const WINDOWS_ICU_I18N: &str = "icuin";
const WINDOWS_OS_ICU_DLL: &str = "icu.dll";

fn main() {
    for name in [
        "HERMES_LEAN_SYS_DIR",
        "HERMES_LEAN_SYS_MIRROR",
        "HERMES_LEAN_SYS_OFFLINE",
        "EXACT_JS_ENGINE",
        "CARGO_NET_OFFLINE",
        "CARGO_HOME",
        "HOME",
        "USERPROFILE",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Exact's explicit refusing executor is used by Hermes-less CI and E1's
    // not-yet-supported targets. Keep the Cargo graph stable while selecting
    // no compiler, headers, archive, or native link line.
    if std::env::var("EXACT_JS_ENGINE").as_deref() == Ok("stub") {
        for key in [
            "include_dir",
            "hermesc_path",
            "lib_root",
            "archive",
            "engine_digest",
            "bytecode_version",
        ] {
            metadata(key, "");
        }
        for (key, value) in [
            ("HERMES_LEAN_ARCHIVE", ""),
            ("HERMES_LEAN_ENGINE_DIGEST", ""),
            ("HERMES_LEAN_BYTECODE_VERSION", "0"),
            ("HERMES_LEAN_HERMESC_PATH", ""),
        ] {
            println!("cargo:rustc-env={key}={value}");
        }
        return;
    }
    let repo_root = manifest_dir
        .ancestors()
        .nth(2)
        .expect("crate lives two levels below the repository root");
    let target = std::env::var("TARGET").expect("Cargo supplies TARGET");
    let host = std::env::var("HOST").expect("Cargo supplies HOST");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_vendor = std::env::var("CARGO_CFG_TARGET_VENDOR").unwrap_or_default();
    let links_full_runtime = std::env::var_os("CARGO_FEATURE_LINK").is_some();
    let links_lean_runtime = std::env::var_os("CARGO_FEATURE_LINK_LEAN").is_some();
    let links_icu = std::env::var_os("CARGO_FEATURE_ICU").is_some();
    let links_full_icu_data = std::env::var_os("CARGO_FEATURE_ICU_FULL_DATA").is_some();

    // @ref LLP 0057.000#l1--the-bindings-door — R-e permits exactly one
    // linked archive identity in a process.
    if links_full_runtime && links_lean_runtime {
        panic!("hermes-lean-sys features `link` and `link-lean` are mutually exclusive; select exactly one VM archive");
    }

    let install = resolve_engine_directory(repo_root, &target, &host, links_lean_runtime)
        .unwrap_or_else(|error| panic!("Hermes engine resolution failed: {error}"));

    for path in rerun_paths(&install, &target) {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let engine_digest = digest_file(&install.vm_archive)
        .unwrap_or_else(|error| panic!("cannot hash selected Hermes engine: {error}"));
    let lean_engine_digest = install.lean_vm_archive.as_ref().map(|lean_vm_archive| {
        digest_file(lean_vm_archive)
            .unwrap_or_else(|error| panic!("cannot hash selected lean Hermes engine: {error}"))
    });
    let icu_data_digest = install.icu_data_archive.as_ref().map(|archive| {
        digest_file(archive)
            .unwrap_or_else(|error| panic!("cannot hash trimmed ICU data archive: {error}"))
    });
    let icu_full_data_digest = install.icu_full_data_archive.as_ref().map(|archive| {
        digest_file(archive)
            .unwrap_or_else(|error| panic!("cannot hash full ICU data archive: {error}"))
    });
    let bytecode_version = hermesc_bytecode_version(&install.hermesc)
        .unwrap_or_else(|error| panic!("cannot inspect selected Hermes compiler: {error}"));
    metadata("include_dir", &install.include_dir.display().to_string());
    metadata("hermesc_path", &install.hermesc.display().to_string());
    println!(
        "cargo:rustc-env=HERMES_LEAN_HERMESC_PATH={}",
        install.hermesc.display()
    );
    metadata("lib_root", &install.lib_root.display().to_string());
    metadata("archive", &install.vm_archive.display().to_string());
    metadata("engine_digest", &engine_digest);
    if let Some(lean_vm_archive) = &install.lean_vm_archive {
        metadata("lean_archive", &lean_vm_archive.display().to_string());
    }
    if let Some(digest) = &lean_engine_digest {
        metadata("lean_engine_digest", digest);
        metadata("lean_bytecode_version", &bytecode_version);
    }
    metadata("bytecode_version", &bytecode_version);
    metadata("engine_dir", &install.root.display().to_string());
    if let (Some(archive), Some(digest)) = (&install.icu_data_archive, &icu_data_digest) {
        metadata("icu_data_archive", &archive.display().to_string());
        metadata("icu_data_digest", digest);
        println!(
            "cargo:rustc-env=HERMES_LEAN_ICU_DATA_ARCHIVE={}",
            archive.display()
        );
        println!("cargo:rustc-env=HERMES_LEAN_ICU_DATA_DIGEST={digest}");
    }
    if let (Some(archive), Some(digest)) = (&install.icu_full_data_archive, &icu_full_data_digest) {
        metadata("icu_full_data_archive", &archive.display().to_string());
        metadata("icu_full_data_digest", digest);
        println!(
            "cargo:rustc-env=HERMES_LEAN_ICU_FULL_DATA_ARCHIVE={}",
            archive.display()
        );
        println!("cargo:rustc-env=HERMES_LEAN_ICU_FULL_DATA_DIGEST={digest}");
    }
    println!("cargo:rustc-env=HERMES_LEAN_ENGINE_DIGEST={engine_digest}");
    println!(
        "cargo:rustc-env=HERMES_LEAN_ARCHIVE={}",
        install.vm_archive.display()
    );
    println!("cargo:rustc-env=HERMES_LEAN_BYTECODE_VERSION={bytecode_version}");
    if let Some(lean_vm_archive) = &install.lean_vm_archive {
        println!(
            "cargo:rustc-env=HERMES_LEAN_LEAN_ARCHIVE={}",
            lean_vm_archive.display()
        );
    }
    if let Some(digest) = &lean_engine_digest {
        println!("cargo:rustc-env=HERMES_LEAN_LEAN_ENGINE_DIGEST={digest}");
        println!("cargo:rustc-env=HERMES_LEAN_LEAN_BYTECODE_VERSION={bytecode_version}");
    }

    let linked = if links_full_runtime {
        Some((&install.vm_archive, engine_digest.as_str()))
    } else if links_lean_runtime {
        Some((
            install.lean_vm_archive.as_ref().unwrap_or_else(|| {
                panic!(
                    "link-lean requires a lean VM archive authenticated by the install's \
                         hermes-input-receipt.json; {} has no receipt or no lean archive \
                         (use the pinned bundle, or a receipted HERMES_LEAN_SYS_DIR install)",
                    install.root.display()
                )
            }),
            lean_engine_digest
                .as_deref()
                .expect("link-lean requires a lean VM digest"),
        ))
    } else {
        None
    };
    if let Some((archive, digest)) = linked {
        metadata("linked_archive", &archive.display().to_string());
        metadata("linked_engine_digest", digest);
        println!(
            "cargo:rustc-env=HERMES_LEAN_LINKED_ARCHIVE={}",
            archive.display()
        );
        println!("cargo:rustc-env=HERMES_LEAN_LINKED_ENGINE_DIGEST={digest}");
        emit_link_lines(&target_os, &target_vendor, &install.lib_root, archive);
    }
    if links_icu && target_os == "linux" {
        // @ref LLP 0057.000#l1--the-bindings-door — the ICU-data identity is
        // separate from the VM identity so R-e names the exact selected data
        // archive while the common ICU code remains receipt-bound.
        let (archive, digest, library) = if links_full_icu_data {
            (
                install
                    .icu_full_data_archive
                    .as_ref()
                    .expect("Linux install has full ICU data"),
                icu_full_data_digest
                    .as_deref()
                    .expect("Linux install has a full ICU data digest"),
                LINUX_ICU_FULL_DATA,
            )
        } else {
            (
                install
                    .icu_data_archive
                    .as_ref()
                    .expect("Linux install has trimmed ICU data"),
                icu_data_digest
                    .as_deref()
                    .expect("Linux install has a trimmed ICU data digest"),
                LINUX_ICU_DATA,
            )
        };
        metadata("linked_icu_data_archive", &archive.display().to_string());
        metadata("linked_icu_data_digest", digest);
        println!(
            "cargo:rustc-env=HERMES_LEAN_LINKED_ICU_DATA_ARCHIVE={}",
            archive.display()
        );
        println!("cargo:rustc-env=HERMES_LEAN_LINKED_ICU_DATA_DIGEST={digest}");
        emit_linux_icu_link_lines(&install.lib_root, library);
    }
    if links_icu && target_os == "windows" {
        // @ref LLP 0057.000#511-windows-intl-uses-the-os-icu — Windows has no
        // bundled ICU and no data archive, so there is no LINKED_ICU_DATA_*
        // identity to claim. `icu` links the SDK's frozen `icuuc`/`icuin`
        // import libraries, the same ones the Hermes VM imports (above), so
        // Ibex's Intl shims also link without a VM in the graph. Nothing links
        // `icu.lib`: the `unumf_*` entry points only `icu.dll` exports are
        // bound at run time by `ibex2` from System32's `icu.dll` by full path,
        // so no import names `icu.dll` and an embedder needs no linker flag.
        // Unused import libraries add no import. `icu-full-data` selects
        // nothing more.
        println!("cargo:rustc-env=HERMES_LEAN_LINKED_OS_ICU={WINDOWS_OS_ICU_DLL}");
        println!("cargo:rustc-link-lib=dylib={WINDOWS_ICU_UC}");
        println!("cargo:rustc-link-lib=dylib={WINDOWS_ICU_I18N}");
    }
}

fn metadata(key: &str, value: &str) {
    println!("cargo::metadata={key}={value}");
}

fn emit_link_lines(target_os: &str, target_vendor: &str, lib_root: &Path, vm_archive: &Path) {
    println!("cargo:rustc-link-search=native={}", lib_root.display());
    let vm = vm_archive
        .file_stem()
        .and_then(|name| name.to_str())
        .expect("Hermes archive has a UTF-8 stem")
        .trim_start_matches("lib");
    println!("cargo:rustc-link-lib=static={vm}");
    println!("cargo:rustc-link-lib=static=jsi");
    println!("cargo:rustc-link-lib=static=boost_context");
    if target_vendor == "apple" {
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
    } else if target_os == "windows" {
        println!("cargo:rustc-link-lib=icuuc");
        println!("cargo:rustc-link-lib=icuin");
        println!("cargo:rustc-link-lib=dbghelp");
        println!("cargo:rustc-link-lib=version");
        println!("cargo:rustc-link-lib=psapi");
        println!("cargo:rustc-link-lib=winmm");
    } else {
        // ICU comes from the `icu` feature selected by either Linux VM link
        // feature. Full Intl support independently swaps the data archive.
        println!("cargo:rustc-link-lib=static=tinfo");
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
    }
}

fn emit_linux_icu_link_lines(lib_root: &Path, data_library: &str) {
    println!("cargo:rustc-link-search=native={}", lib_root.display());
    println!("cargo:rustc-link-lib=static={LINUX_ICU_I18N}");
    println!("cargo:rustc-link-lib=static={LINUX_ICU_UC}");
    println!("cargo:rustc-link-lib=static={data_library}");
}
