#[allow(dead_code)]
#[path = "../build_support.rs"]
mod build_support;

use build_support::{
    acquire_bundle, apple_simulator_arch, digest_file, download_options_from_env,
    installer_command, parse_pin_sha256, pin_for_target,
    prepare_apple_simulator_link_archives_with_lipo, repository_install_root, rerun_paths,
    verify_and_extract_archive, watched_inputs, BundlePin, DownloadOptions, EngineInstall,
    LinkArchive, RELEASE_TAG,
};
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::thread;

const ASSET: &str = "hermes-vanilla-test-target.tar.gz";
const CACHE_ARCHIVE: &str = ".hermes-lean-sys-bundle.tar.gz";

#[test]
fn target_to_asset_mapping_includes_apple_simulator_aliases() {
    assert_eq!(
        pin_for_target("aarch64-apple-ios-sim")
            .expect("arm simulator pin")
            .asset,
        "hermes-vanilla-universal-apple-ios-simulator.tar.gz"
    );
    assert_eq!(
        pin_for_target("x86_64-apple-ios")
            .expect("x64 simulator pin")
            .asset,
        "hermes-vanilla-universal-apple-ios-simulator.tar.gz"
    );
    assert_eq!(
        pin_for_target("aarch64-apple-tvos")
            .expect("tvOS device pin")
            .asset,
        "hermes-vanilla-aarch64-apple-tvos.tar.gz"
    );
    assert_eq!(
        pin_for_target("aarch64-apple-tvos-sim")
            .expect("tvOS simulator pin")
            .asset,
        "hermes-vanilla-aarch64-apple-tvos-simulator.tar.gz"
    );
    let error = pin_for_target("riscv64-unknown-linux-gnu").expect_err("unsupported target");
    assert!(error.contains("HERMES_LEAN_SYS_DIR"), "{error}");
    // The Intel tvOS Simulator exists in Rust but has no bundle; it must never
    // fall through to the arm64 Simulator archive.
    let error = pin_for_target("x86_64-apple-tvos").expect_err("Intel tvOS Simulator");
    assert!(error.contains("HERMES_LEAN_SYS_DIR"), "{error}");
}

#[test]
fn repository_installs_keep_macos_and_tvos_targets_separate() {
    let temporary = tempfile::tempdir().expect("temporary repository");
    for relative in [
        "ios/Frameworks-vanilla",
        "tvos/Frameworks-vanilla",
        "tvos-simulator/Frameworks-vanilla",
    ] {
        fs::create_dir_all(temporary.path().join(relative)).expect("repository install");
    }
    assert_eq!(
        repository_install_root(temporary.path(), "aarch64-apple-darwin"),
        Some(temporary.path().join("ios/Frameworks-vanilla"))
    );
    assert_eq!(
        repository_install_root(temporary.path(), "aarch64-apple-tvos"),
        Some(temporary.path().join("tvos/Frameworks-vanilla"))
    );
    assert_eq!(
        repository_install_root(temporary.path(), "aarch64-apple-tvos-sim"),
        Some(temporary.path().join("tvos-simulator/Frameworks-vanilla"))
    );
}

#[test]
fn apple_simulator_targets_map_to_lipo_architectures() {
    assert_eq!(apple_simulator_arch("aarch64-apple-ios-sim"), Some("arm64"));
    assert_eq!(apple_simulator_arch("x86_64-apple-ios"), Some("x86_64"));
    assert_eq!(
        apple_simulator_arch("aarch64-apple-tvos-sim"),
        Some("arm64")
    );
    assert_eq!(apple_simulator_arch("aarch64-apple-ios"), None);
    assert_eq!(apple_simulator_arch("aarch64-apple-darwin"), None);
}

#[cfg(target_os = "macos")]
#[test]
fn real_universal_archive_is_thinned_and_rustc_links_both_simulator_architectures() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let root = temporary.path().join("bundle");
    let lib = root.join("lib");
    fs::create_dir_all(&lib).expect("library directory");
    let universal = make_universal_archive(&lib, "tiny", "tiny_answer", 42);
    let archive = LinkArchive {
        library: "tiny".to_owned(),
        source: universal,
    };
    let install = test_install(&root, std::slice::from_ref(&archive), true);

    for (target, arch) in [
        ("aarch64-apple-ios-sim", "arm64"),
        ("x86_64-apple-ios", "x86_64"),
    ] {
        let out = temporary.path().join(format!("out-{arch}"));
        let first = prepare_apple_simulator_link_archives_with_lipo(
            &install,
            target,
            &out,
            std::slice::from_ref(&archive),
            Path::new("lipo"),
        )
        .expect("thin real universal archive");
        let second = prepare_apple_simulator_link_archives_with_lipo(
            &install,
            target,
            &out,
            std::slice::from_ref(&archive),
            Path::new("lipo"),
        )
        .expect("repeat real thinning");
        assert_eq!(first, second, "fixed inputs produce the same closure");
        assert_eq!(lipo_archs(&first[0].linked), vec![arch]);
        assert_eq!(
            digest_file(&first[0].linked).expect("derivative digest"),
            first[0].derivative_digest.as_deref().unwrap()
        );
        // The derivative must define the native symbol, not merely mention it:
        // a staticlib may legally leave an extern unresolved.
        let symbols = Command::new("nm")
            .arg("-g")
            .arg(&first[0].linked)
            .output()
            .expect("run nm");
        assert!(symbols.status.success(), "nm failed");
        assert!(
            String::from_utf8_lossy(&symbols.stdout).contains("T _tiny_answer"),
            "derivative does not define tiny_answer: {}",
            String::from_utf8_lossy(&symbols.stdout)
        );
        // Only the current closure is left under OUT_DIR, so nothing else can
        // satisfy `-l tiny`; the unpublished snapshot is gone.
        let staging: Vec<_> = fs::read_dir(&out)
            .expect("staging parent")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .into_string()
                    .expect("utf8")
            })
            .collect();
        assert_eq!(staging.len(), 1, "only the closure remains: {staging:?}");
        rustc_links_staticlib(target, &first[0].linked, "tiny", "tiny_answer");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn fat_member_becoming_thin_cannot_select_the_stale_derivative() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let root = temporary.path().join("bundle");
    let lib = root.join("lib");
    fs::create_dir_all(&lib).expect("library directory");
    let vm = LinkArchive {
        library: "vm".to_owned(),
        source: make_universal_archive(&lib, "vm", "vm_answer", 1),
    };
    let member = LinkArchive {
        library: "member".to_owned(),
        source: make_universal_archive(&lib, "member", "member_answer", 2),
    };
    let archives = vec![vm, member];
    let mut install = test_install(&root, &archives, true);
    let out = temporary.path().join("out");
    let first = prepare_apple_simulator_link_archives_with_lipo(
        &install,
        "aarch64-apple-ios-sim",
        &out,
        &archives,
        Path::new("lipo"),
    )
    .expect("first fat closure");
    let first_derivative = fs::read(&first[1].linked).expect("old derivative");

    let replacement = make_thin_archive(&lib, "replacement", "member_answer", 99, "arm64");
    fs::copy(&replacement, &archives[1].source).expect("replace member with thin archive");
    install.authenticated_archive_digests.insert(
        archives[1].source.clone(),
        digest_file(&archives[1].source).expect("replacement digest"),
    );
    let second = prepare_apple_simulator_link_archives_with_lipo(
        &install,
        "aarch64-apple-ios-sim",
        &out,
        &archives,
        Path::new("lipo"),
    )
    .expect("fat plus thin closure");

    let first_dir = first[0].linked.parent().expect("first closure directory");
    let second_dir = second[0].linked.parent().expect("second closure directory");
    assert_ne!(first_dir, second_dir, "closure identity must change");
    assert!(second
        .iter()
        .all(|archive| archive.linked.parent() == Some(second_dir)));
    assert!(first[1].derivative_digest.is_some());
    assert_eq!(second[1].derivative_digest, None);
    assert_eq!(lipo_archs(&second[1].linked), vec!["arm64"]);
    assert_eq!(
        fs::read(&second[1].linked).expect("new linked member"),
        fs::read(&archives[1].source).expect("new source member")
    );
    assert_ne!(
        first_derivative,
        fs::read(&second[1].linked).expect("new thin member")
    );
    // Earlier closures and every snapshot are pruned; what links is read-only.
    assert!(!first_dir.exists(), "stale closure must be pruned");
    let staging: Vec<_> = fs::read_dir(&out)
        .expect("staging parent")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("utf8")
        })
        .filter(|name| name.starts_with("hermes-lean-sys-"))
        .collect();
    assert_eq!(staging.len(), 1, "one closure remains: {staging:?}");
    assert!(second.iter().all(|archive| fs::metadata(&archive.linked)
        .expect("linked metadata")
        .permissions()
        .readonly()));
}

#[cfg(target_os = "macos")]
#[test]
fn symlinked_and_non_regular_simulator_members_are_refused() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().expect("temporary directory");
    let root = temporary.path().join("bundle");
    let lib = root.join("lib");
    fs::create_dir_all(&lib).expect("library directory");
    let regular = make_thin_archive(&lib, "real", "tiny_answer", 42, "arm64");
    let symlink_path = lib.join("libsymlink.a");
    symlink(&regular, &symlink_path).expect("archive symlink");
    let symlink_archive = LinkArchive {
        library: "symlink".to_owned(),
        source: symlink_path,
    };
    let symlink_install = test_install(&root, std::slice::from_ref(&symlink_archive), true);
    let error = prepare_apple_simulator_link_archives_with_lipo(
        &symlink_install,
        "aarch64-apple-ios-sim",
        &temporary.path().join("symlink-out"),
        std::slice::from_ref(&symlink_archive),
        Path::new("lipo"),
    )
    .expect_err("symlinked archive must fail");
    assert!(error.contains("symlink"), "{error}");

    let directory_path = lib.join("libdirectory.a");
    fs::create_dir(&directory_path).expect("non-regular archive fixture");
    let directory_archive = LinkArchive {
        library: "directory".to_owned(),
        source: directory_path,
    };
    let directory_install = test_install(&root, std::slice::from_ref(&directory_archive), false);
    let error = prepare_apple_simulator_link_archives_with_lipo(
        &directory_install,
        "aarch64-apple-ios-sim",
        &temporary.path().join("directory-out"),
        std::slice::from_ref(&directory_archive),
        Path::new("lipo"),
    )
    .expect_err("non-regular archive must fail");
    assert!(error.contains("not a regular file"), "{error}");
}

#[cfg(target_os = "macos")]
#[test]
fn receipt_free_thin_simulator_override_still_links() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let root = temporary.path().join("override");
    let lib = root.join("lib");
    fs::create_dir_all(&lib).expect("library directory");
    let thin = make_thin_archive(&lib, "tiny-slice", "tiny_answer", 42, "arm64");
    let source = lib.join("libtiny.a");
    fs::copy(thin, &source).expect("legacy thin archive");
    let archive = LinkArchive {
        library: "tiny".to_owned(),
        source,
    };
    let install = test_install(&root, std::slice::from_ref(&archive), false);
    let prepared = prepare_apple_simulator_link_archives_with_lipo(
        &install,
        "aarch64-apple-ios-sim",
        &temporary.path().join("out"),
        std::slice::from_ref(&archive),
        Path::new("lipo"),
    )
    .expect("receipt-free thin override");

    assert_eq!(prepared[0].derivative_digest, None);
    assert_ne!(prepared[0].linked, archive.source);
    assert_eq!(lipo_archs(&prepared[0].linked), vec!["arm64"]);
    rustc_links_staticlib(
        "aarch64-apple-ios-sim",
        &prepared[0].linked,
        "tiny",
        "tiny_answer",
    );
}

#[cfg(target_os = "macos")]
fn make_universal_archive(directory: &Path, library: &str, symbol: &str, answer: i32) -> PathBuf {
    let arm64 = make_thin_archive(directory, library, symbol, answer, "arm64");
    let x86_64 = make_thin_archive(directory, library, symbol, answer, "x86_64");
    let universal = directory.join(format!("lib{library}.a"));
    checked_command(
        Command::new("lipo")
            .arg("-create")
            .arg(arm64)
            .arg(x86_64)
            .arg("-output")
            .arg(&universal),
        "create universal static archive",
    );
    assert_eq!(lipo_archs(&universal), vec!["x86_64", "arm64"]);
    universal
}

#[cfg(target_os = "macos")]
fn make_thin_archive(
    directory: &Path,
    library: &str,
    symbol: &str,
    answer: i32,
    arch: &str,
) -> PathBuf {
    let source = directory.join(format!("{library}-{arch}-{answer}.c"));
    let object = directory.join(format!("{library}-{arch}-{answer}.o"));
    let archive = directory.join(format!("lib{library}-{arch}-{answer}.a"));
    fs::write(
        &source,
        format!("int {symbol}(void) {{ return {answer}; }}\n"),
    )
    .expect("tiny C source");
    checked_command(
        Command::new("cc")
            .arg("-arch")
            .arg(arch)
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&object),
        "compile thin C object",
    );
    checked_command(
        Command::new("ar").arg("rcs").arg(&archive).arg(&object),
        "create thin static archive",
    );
    assert_eq!(lipo_archs(&archive), vec![arch]);
    archive
}

#[cfg(target_os = "macos")]
fn test_install(root: &Path, archives: &[LinkArchive], authenticate: bool) -> EngineInstall {
    let authenticated_archive_digests = if authenticate {
        archives
            .iter()
            .map(|archive| {
                (
                    archive.source.clone(),
                    digest_file(&archive.source).expect("source digest"),
                )
            })
            .collect()
    } else {
        Default::default()
    };
    EngineInstall {
        root: root.to_path_buf(),
        include_dir: root.join("include"),
        lib_root: root.join("lib"),
        vm_archive: archives[0].source.clone(),
        lean_vm_archive: None,
        icu_i18n_archive: None,
        icu_uc_archive: None,
        icu_data_archive: None,
        icu_full_data_archive: None,
        icu_en_data_archive: None,
        icu_en_filter: None,
        icu_trimmed_filter: None,
        hermesc: root.join("bin/hermesc"),
        authenticated_archive_digests,
    }
}

#[cfg(target_os = "macos")]
fn lipo_archs(archive: &Path) -> Vec<&str> {
    let output = checked_command(
        Command::new("lipo").arg("-archs").arg(archive),
        "inspect static archive architectures",
    );
    let stdout = String::from_utf8(output.stdout).expect("lipo output is UTF-8");
    stdout
        .split_whitespace()
        .map(|arch| match arch {
            "arm64" => "arm64",
            "x86_64" => "x86_64",
            other => panic!("unexpected lipo architecture {other}"),
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn rustc_links_staticlib(target: &str, archive: &Path, library: &str, symbol: &str) {
    let temporary = tempfile::tempdir().expect("rustc proof directory");
    let source = temporary.path().join("proof.rs");
    let output = temporary.path().join("libproof.a");
    fs::write(
        &source,
        format!(
            "extern \"C\" {{ fn {symbol}() -> i32; }}\n#[no_mangle]\npub extern \"C\" fn call_native() -> i32 {{ unsafe {{ {symbol}() }} }}\n"
        ),
    )
    .expect("Rust staticlib proof source");
    let link_dir = archive.parent().expect("prepared archive parent");
    checked_command(
        Command::new("rustc")
            .arg("--edition=2021")
            .arg("--crate-name=simulator_link_proof")
            .arg("--crate-type=staticlib")
            .arg("--target")
            .arg(target)
            .arg(&source)
            .arg("-L")
            .arg(format!("native={}", link_dir.display()))
            .arg("-l")
            .arg(format!("static={library}"))
            .arg("-o")
            .arg(&output),
        "link Rust staticlib for iOS Simulator",
    );
    let staticlib = fs::read(&output).expect("Rust staticlib output");
    assert!(
        staticlib
            .windows(symbol.len())
            .any(|candidate| candidate == symbol.as_bytes()),
        "Rust staticlib did not bundle {symbol}"
    );
}

#[cfg(target_os = "macos")]
fn checked_command(command: &mut Command, purpose: &str) -> std::process::Output {
    let output = command.output().unwrap_or_else(|error| {
        panic!("cannot {purpose} with {command:?}: {error}");
    });
    assert!(
        output.status.success(),
        "failed to {purpose} with {command:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[cfg(unix)]
#[test]
fn arm64_only_tvos_simulator_archive_is_not_thinned() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("libhermesvm_a.a");
    fs::write(&source, b"thin-arm64").expect("archive");
    let digest = digest_file(&source).expect("archive digest");
    let install = EngineInstall {
        root: temporary.path().to_path_buf(),
        include_dir: temporary.path().join("include"),
        lib_root: temporary.path().to_path_buf(),
        vm_archive: source.clone(),
        lean_vm_archive: None,
        icu_i18n_archive: None,
        icu_uc_archive: None,
        icu_data_archive: None,
        icu_full_data_archive: None,
        icu_en_data_archive: None,
        icu_en_filter: None,
        icu_trimmed_filter: None,
        hermesc: temporary.path().join("hermesc"),
        authenticated_archive_digests: [(source.clone(), digest.clone())].into_iter().collect(),
    };
    let fake_lipo = temporary.path().join("lipo");
    fs::write(
        &fake_lipo,
        "#!/bin/sh\n[ \"$1\" = \"-archs\" ] && printf 'arm64\\n' && exit 0\nexit 99\n",
    )
    .expect("fake lipo");
    let mut permissions = fs::metadata(&fake_lipo)
        .expect("fake lipo metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_lipo, permissions).expect("make fake lipo executable");

    let prepared = prepare_apple_simulator_link_archives_with_lipo(
        &install,
        "aarch64-apple-tvos-sim",
        &temporary.path().join("out"),
        &[LinkArchive {
            library: "hermesvm_a".to_owned(),
            source: source.clone(),
        }],
        &fake_lipo,
    )
    .expect("inspect thin archive");
    assert_ne!(prepared[0].linked, source);
    assert_eq!(
        fs::read(&prepared[0].linked).expect("staged thin archive"),
        b"thin-arm64"
    );
    assert_eq!(prepared[0].source_digest.as_deref(), Some(digest.as_str()));
    assert_eq!(prepared[0].derivative_digest, None);
}

#[cfg(unix)]
#[test]
fn simulator_thinning_refuses_an_unauthenticated_input_before_lipo() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("libhermesvm_a.a");
    fs::write(&source, b"archive").expect("archive");
    let install = EngineInstall {
        root: temporary.path().to_path_buf(),
        include_dir: temporary.path().join("include"),
        lib_root: temporary.path().to_path_buf(),
        vm_archive: source.clone(),
        lean_vm_archive: None,
        icu_i18n_archive: None,
        icu_uc_archive: None,
        icu_data_archive: None,
        icu_full_data_archive: None,
        icu_en_data_archive: None,
        icu_en_filter: None,
        icu_trimmed_filter: None,
        hermesc: temporary.path().join("hermesc"),
        authenticated_archive_digests: Default::default(),
    };
    let fake_lipo = temporary.path().join("lipo");
    fs::write(
        &fake_lipo,
        "#!/bin/sh\n[ \"$1\" = \"-archs\" ] && printf 'x86_64 arm64\\n' && exit 0\nexit 99\n",
    )
    .expect("fake lipo");
    let mut permissions = fs::metadata(&fake_lipo)
        .expect("fake lipo metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_lipo, permissions).expect("make fake lipo executable");
    let error = prepare_apple_simulator_link_archives_with_lipo(
        &install,
        "aarch64-apple-ios-sim",
        &temporary.path().join("out"),
        &[LinkArchive {
            library: "hermesvm_a".to_owned(),
            source,
        }],
        &fake_lipo,
    )
    .expect_err("receipt omission must fail first");
    assert!(error.contains("canonical receipt"), "{error}");
    assert!(error.contains("libhermesvm_a.a"), "{error}");
}

#[test]
fn missing_lipo_names_xcode_command_line_tools() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("libhermesvm_a.a");
    fs::write(&source, b"archive").expect("archive");
    let digest = digest_file(&source).expect("archive digest");
    let install = EngineInstall {
        root: temporary.path().to_path_buf(),
        include_dir: temporary.path().join("include"),
        lib_root: temporary.path().to_path_buf(),
        vm_archive: source.clone(),
        lean_vm_archive: None,
        icu_i18n_archive: None,
        icu_uc_archive: None,
        icu_data_archive: None,
        icu_full_data_archive: None,
        icu_en_data_archive: None,
        icu_en_filter: None,
        icu_trimmed_filter: None,
        hermesc: temporary.path().join("hermesc"),
        authenticated_archive_digests: [(source.clone(), digest)].into_iter().collect(),
    };
    let error = prepare_apple_simulator_link_archives_with_lipo(
        &install,
        "aarch64-apple-ios-sim",
        &temporary.path().join("out"),
        &[LinkArchive {
            library: "hermesvm_a".to_owned(),
            source,
        }],
        &temporary.path().join("missing-lipo"),
    )
    .expect_err("missing lipo must fail clearly");
    assert!(error.contains("Xcode command-line tools"), "{error}");
    assert!(error.contains("xcode-select --install"), "{error}");
}

#[test]
fn rerun_inputs_cover_receipt_headers_cache_and_link_archives() {
    let root = Path::new("/cache/entry");
    let install = EngineInstall {
        root: root.to_path_buf(),
        include_dir: root.join("include"),
        lib_root: root.join("lib"),
        vm_archive: root.join("lib/libhermesvm_a.a"),
        lean_vm_archive: Some(root.join("lib/libhermesvmlean_a.a")),
        icu_i18n_archive: Some(root.join("lib/libicui18n.a")),
        icu_uc_archive: Some(root.join("lib/libicuuc.a")),
        icu_data_archive: Some(root.join("lib/libicudata.a")),
        icu_en_data_archive: Some(root.join("lib/libicudata-en.a")),
        icu_full_data_archive: Some(root.join("lib/libicudata-full.a")),
        icu_trimmed_filter: Some(root.join("share/icu/filters-root-en.json")),
        icu_en_filter: Some(root.join("share/icu/filters-en-intl.json")),
        hermesc: root.join("bin/hermesc"),
        authenticated_archive_digests: Default::default(),
    };
    let paths = watched_inputs(&install, "x86_64-unknown-linux-gnu");
    for expected in [
        root.to_path_buf(),
        root.join("include"),
        root.join("hermes-input-receipt.json"),
        root.join(CACHE_ARCHIVE),
        root.join("bin/hermesc"),
        root.join("lib/libhermesvm_a.a"),
        root.join("lib/libhermesvmlean_a.a"),
        root.join("lib/libjsi.a"),
        root.join("lib/libboost_context.a"),
        root.join("lib/libicui18n.a"),
        root.join("lib/libicuuc.a"),
        root.join("lib/libicudata.a"),
        root.join("lib/libicudata-en.a"),
        root.join("lib/libicudata-full.a"),
        root.join("share/icu/filters-root-en.json"),
        root.join("share/icu/filters-en-intl.json"),
        root.join("lib/libtinfo.a"),
    ] {
        assert!(
            paths.contains(&expected),
            "missing watch for {}",
            expected.display()
        );
    }

    let windows = watched_inputs(&install, "x86_64-pc-windows-msvc");
    assert!(windows.contains(&root.join("lib/jsi.lib")));
    assert!(windows.contains(&root.join("lib/boost_context.lib")));
}

#[test]
fn pin_table_digest_parser_accepts_only_sha256_hex() {
    assert_eq!(
        parse_pin_sha256(&"A".repeat(64)).expect("valid SHA-256"),
        "a".repeat(64)
    );
    assert!(parse_pin_sha256("abc").is_err());
    assert!(parse_pin_sha256(&"g".repeat(64)).is_err());
    let placeholder = parse_pin_sha256("TODO_I3_V4_SHA256_TEST").expect_err("sentinel");
    assert!(
        placeholder.contains("awaiting publication"),
        "{placeholder}"
    );
}

#[test]
fn digest_mismatch_is_refused_before_extraction() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let archive = temporary.path().join("bundle.tar.gz");
    write_archive(&archive, &[ArchiveEntry::File("sentinel", b"contents")]);
    let destination = temporary.path().join("install");

    let error = verify_and_extract_archive(&archive, &"0".repeat(64), &destination)
        .expect_err("digest mismatch");
    assert!(error.contains("nothing was extracted"), "{error}");
    assert!(!destination.exists());
}

#[test]
fn traversal_and_link_entries_are_refused() {
    let temporary = tempfile::tempdir().expect("temporary directory");

    let traversal = temporary.path().join("traversal.tar.gz");
    write_archive(&traversal, &[ArchiveEntry::File("safe-name", b"no")]);
    replace_first_tar_path(&traversal, "../escape");
    let traversal_digest = sha256_file(&traversal);
    let error = verify_and_extract_archive(
        &traversal,
        &traversal_digest,
        &temporary.path().join("traversal-out"),
    )
    .expect_err("traversal must fail");
    assert!(
        error.contains("unsafe") || error.contains("path"),
        "{error}"
    );
    assert!(!temporary.path().join("escape").exists());

    let absolute = temporary.path().join("absolute.tar.gz");
    write_archive(&absolute, &[ArchiveEntry::File("safe-name", b"no")]);
    replace_first_tar_path(&absolute, "/absolute-escape");
    let absolute_digest = sha256_file(&absolute);
    let error = verify_and_extract_archive(
        &absolute,
        &absolute_digest,
        &temporary.path().join("absolute-out"),
    )
    .expect_err("absolute path must fail");
    assert!(
        error.contains("unsafe") || error.contains("path"),
        "{error}"
    );

    let links = temporary.path().join("links.tar.gz");
    write_archive(&links, &[ArchiveEntry::Symlink("link", "target")]);
    let link_digest = sha256_file(&links);
    let error =
        verify_and_extract_archive(&links, &link_digest, &temporary.path().join("link-out"))
            .expect_err("link must fail");
    assert!(
        error.contains("links and special files are forbidden"),
        "{error}"
    );
    assert!(!temporary.path().join("link-out").exists());
}

#[test]
fn host_independent_tar_names_and_collisions_are_refused() {
    fn refuses(entries: &[ArchiveEntry<'_>], expected: &str) {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let archive = temporary.path().join("fixture.tar.gz");
        write_archive(&archive, entries);
        let digest = sha256_file(&archive);
        let error = verify_and_extract_archive(&archive, &digest, &temporary.path().join("out"))
            .expect_err("unsafe archive must fail");
        assert!(
            error.contains(expected),
            "expected {expected:?} in {error:?}"
        );
    }

    refuses(&[ArchiveEntry::File(r"dir\file", b"no")], "backslash");
    refuses(&[ArchiveEntry::File("C:/file", b"no")], "forbidden");
    refuses(&[ArchiveEntry::File(r"\\?\C:\file", b"no")], "backslash");
    refuses(
        &[ArchiveEntry::File(r"\\server\share\file", b"no")],
        "backslash",
    );
    refuses(&[ArchiveEntry::File("dir/CON.txt", b"no")], "reserved");
    refuses(
        &[
            ArchiveEntry::File("same", b"first"),
            ArchiveEntry::File("same", b"second"),
        ],
        "duplicate normalized",
    );
    refuses(
        &[
            ArchiveEntry::File("Dir/one", b"first"),
            ArchiveEntry::File("dir/two", b"second"),
        ],
        "case-folded",
    );
    refuses(
        &[
            ArchiveEntry::File("parent", b"file"),
            ArchiveEntry::File("parent/child", b"child"),
        ],
        "file ancestor",
    );
}

#[test]
fn extraction_never_overwrites_an_existing_path() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let archive = temporary.path().join("bundle.tar.gz");
    write_archive(&archive, &[ArchiveEntry::File("sentinel", b"replacement")]);
    let digest = sha256_file(&archive);
    let destination = temporary.path().join("out");
    fs::create_dir(&destination).expect("destination");
    fs::write(destination.join("sentinel"), b"original").expect("existing file");

    let error = verify_and_extract_archive(&archive, &digest, &destination)
        .expect_err("overwrite must fail");
    assert!(error.contains("overwrite"), "{error}");
    assert_eq!(
        fs::read(destination.join("sentinel")).expect("existing file"),
        b"original"
    );
}

#[test]
fn canonical_directory_entries_extract_with_normalized_names() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let archive = temporary.path().join("bundle.tar.gz");
    write_archive(
        &archive,
        &[
            ArchiveEntry::Dir("include/"),
            ArchiveEntry::File("include/header.h", b"header"),
        ],
    );
    let digest = sha256_file(&archive);
    let destination = temporary.path().join("out");
    verify_and_extract_archive(&archive, &digest, &destination).expect("safe extraction");
    assert_eq!(
        fs::read(destination.join("include/header.h")).expect("header"),
        b"header"
    );
}

#[test]
fn offline_empty_cache_fails_with_recovery_instructions() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let digest = leak("1".repeat(64));
    let pin = BundlePin {
        target: "test-target",
        asset: ASSET,
        sha256: digest,
    };
    let options = DownloadOptions {
        cache_root: temporary.path().join("cache"),
        release_base_url: "http://127.0.0.1:1".to_owned(),
        offline: true,
        installer_manifest: Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates directory")
            .join("hermes-lean-sys-installer/Cargo.toml"),
    };

    let error = acquire_bundle(&pin, &options, |_| Ok(())).expect_err("empty offline cache");
    assert!(error.contains("offline mode is enabled"), "{error}");
    assert!(error.contains("HERMES_LEAN_SYS_DIR"), "{error}");
    assert!(
        error.contains(&format!(
            "this build pins {RELEASE_TAG}/{ASSET} at sha256-{digest}"
        )),
        "{error}"
    );
    assert!(
        error.contains(&format!(
            "{} --target test-target",
            installer_command(&options)
        )),
        "{error}"
    );
}

#[test]
fn warm_cache_is_reverified_against_its_retained_archive() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let archive = temporary.path().join(ASSET);
    write_archive(&archive, &[ArchiveEntry::File("sentinel", b"warm")]);
    let digest = sha256_file(&archive);
    let entry = temporary
        .path()
        .join("cache")
        .join(RELEASE_TAG)
        .join(&digest);
    fs::create_dir_all(&entry).expect("cache entry");
    verify_and_extract_archive(&archive, &digest, &entry).expect("verified extraction");
    fs::copy(&archive, entry.join(CACHE_ARCHIVE)).expect("retained archive");
    let pin = BundlePin {
        target: "test-target",
        asset: ASSET,
        sha256: leak(digest.clone()),
    };
    let options = DownloadOptions {
        cache_root: temporary.path().join("cache"),
        release_base_url: "http://127.0.0.1:1".to_owned(),
        offline: true,
        installer_manifest: Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates directory")
            .join("hermes-lean-sys-installer/Cargo.toml"),
    };

    let reused = acquire_bundle(&pin, &options, |_| Ok(())).expect("matching cache entry");
    assert_eq!(reused, entry);
    fs::write(reused.join("sentinel"), b"poisoned").expect("poison extracted file");
    let error = acquire_bundle(&pin, &options, |_| Ok(())).expect_err("changed extracted file");
    assert!(error.contains("archive digest"), "{error}");

    fs::write(reused.join("sentinel"), b"warm").expect("restore extracted file");
    fs::write(reused.join("extra"), b"poisoned").expect("add extra file");
    let error = acquire_bundle(&pin, &options, |_| Ok(())).expect_err("extra extracted file");
    assert!(error.contains("extra file"), "{error}");

    fs::remove_file(reused.join("extra")).expect("remove extra file");
    fs::write(reused.join(CACHE_ARCHIVE), b"poisoned archive").expect("poison archive");
    let error = acquire_bundle(&pin, &options, |_| Ok(())).expect_err("changed retained archive");
    assert!(error.contains("not pinned"), "{error}");
}

#[cfg(unix)]
#[test]
fn warm_cache_with_a_non_executable_compiler_is_stale() {
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempfile::tempdir().expect("temporary directory");
    let archive = temporary.path().join(ASSET);
    write_archive(
        &archive,
        &[ArchiveEntry::Executable("bin/hermesc", b"compiler")],
    );
    let digest = sha256_file(&archive);
    let entry = temporary
        .path()
        .join("cache")
        .join(RELEASE_TAG)
        .join(&digest);
    fs::create_dir_all(&entry).expect("cache entry");
    verify_and_extract_archive(&archive, &digest, &entry).expect("verified extraction");
    fs::copy(&archive, entry.join(CACHE_ARCHIVE)).expect("retained archive");
    let pin = BundlePin {
        target: "test-target",
        asset: ASSET,
        sha256: leak(digest.clone()),
    };
    let options = DownloadOptions {
        cache_root: temporary.path().join("cache"),
        release_base_url: "http://127.0.0.1:1".to_owned(),
        offline: true,
        installer_manifest: Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates directory")
            .join("hermes-lean-sys-installer/Cargo.toml"),
    };
    acquire_bundle(&pin, &options, |_| Ok(())).expect("executable compiler admitted");

    let compiler = entry.join("bin/hermesc");
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o644)).expect("chmod -x");
    let error = acquire_bundle(&pin, &options, |_| Ok(())).expect_err("non-executable compiler");
    assert!(error.contains("bin/hermesc"), "{error}");
}

#[test]
fn rerun_paths_name_only_existing_inputs_and_the_root() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let root = temporary.path().join("install");
    fs::create_dir_all(root.join("include")).expect("include");
    fs::create_dir_all(root.join("lib")).expect("lib");
    fs::write(root.join("lib/libhermesvm_a.a"), b"engine").expect("engine");
    fs::create_dir_all(root.join("bin")).expect("bin");
    fs::write(root.join("bin/hermesc"), b"compiler").expect("compiler");
    let install = EngineInstall {
        root: root.clone(),
        include_dir: root.join("include"),
        lib_root: root.join("lib"),
        vm_archive: root.join("lib/libhermesvm_a.a"),
        lean_vm_archive: None,
        icu_i18n_archive: None,
        icu_uc_archive: None,
        icu_data_archive: None,
        icu_en_data_archive: None,
        icu_full_data_archive: None,
        icu_trimmed_filter: None,
        icu_en_filter: None,
        hermesc: root.join("bin/hermesc"),
        authenticated_archive_digests: Default::default(),
    };
    let paths = rerun_paths(&install, "aarch64-apple-darwin");
    for expected in [
        root.clone(),
        root.join("include"),
        root.join("lib/libhermesvm_a.a"),
        root.join("bin/hermesc"),
    ] {
        assert!(paths.contains(&expected), "missing {}", expected.display());
    }
    assert!(!paths.contains(&root.join(CACHE_ARCHIVE)));
    assert!(!paths.contains(&root.join("hermes-input-receipt.json")));
    assert!(!paths.contains(&root.join("lib/libjsi.a")));
}

#[test]
fn local_http_mirror_bundle_is_verified_and_cached() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let repository = temporary.path().join("repository");
    fs::create_dir_all(repository.join("ios/Frameworks-vanilla/hermes-headers"))
        .expect("repository headers");
    fs::create_dir_all(repository.join("ios/Frameworks-vanilla/macos-static"))
        .expect("macOS repository libraries");
    assert!(
        repository_install_root(&repository, "aarch64-apple-ios").is_none(),
        "an iOS target must not select the repository's macOS-only layout"
    );
    let archive = temporary.path().join(ASSET);
    write_archive(
        &archive,
        &[
            ArchiveEntry::File("include/hermes/hermes.h", b"header"),
            ArchiveEntry::File("lib/libhermesvm_a.a", b"engine"),
            ArchiveEntry::File("bin/hermesc", b"compiler"),
        ],
    );
    let bytes = fs::read(&archive).expect("archive bytes");
    let digest = sha256_bytes(&bytes);
    let listener = TcpListener::bind("127.0.0.1:0").expect("local listener");
    let address = listener.local_addr().expect("listener address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("mirror request");
        let mut request = [0_u8; 4096];
        let count = stream.read(&mut request).expect("read request");
        let request = String::from_utf8_lossy(&request[..count]);
        assert!(
            request.starts_with(&format!("GET /{RELEASE_TAG}/{ASSET} ")),
            "{request}"
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .expect("response header");
        stream.write_all(&bytes).expect("response body");
    });

    let _mirror = EnvGuard::set("HERMES_LEAN_SYS_MIRROR", &format!("http://{address}"));
    let mut options = download_options_from_env(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("environment options");
    options.cache_root = temporary.path().join("cache");
    options.offline = false;
    let pin = BundlePin {
        target: "aarch64-apple-ios",
        asset: ASSET,
        sha256: leak(digest.clone()),
    };
    let partial = options.cache_root.join(RELEASE_TAG).join(&digest);
    fs::create_dir_all(&partial).expect("partial cache entry");
    fs::write(partial.join("partial"), b"incomplete").expect("partial cache file");
    let installed = acquire_bundle(&pin, &options, |_| Ok(())).expect("downloaded bundle");
    server.join().expect("mirror server");

    assert_eq!(
        fs::read(installed.join("lib/libhermesvm_a.a")).expect("engine"),
        b"engine"
    );
    assert!(
        installed.starts_with(&options.cache_root),
        "the iOS fallthrough selects the pinned bundle cache, not the macOS repository layout"
    );
    assert_eq!(
        sha256_file(&installed.join(CACHE_ARCHIVE)),
        digest,
        "the verified archive is retained in the cache entry"
    );
    assert!(!installed.join("partial").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_cache_entry_is_rejected_before_use() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().expect("temporary directory");
    let digest = "4".repeat(64);
    let entry = temporary
        .path()
        .join("cache")
        .join(RELEASE_TAG)
        .join(&digest);
    fs::create_dir_all(entry.parent().expect("tag directory")).expect("tag directory");
    let attacker = temporary.path().join("attacker");
    fs::create_dir(&attacker).expect("attacker directory");
    symlink(&attacker, &entry).expect("cache symlink");
    let pin = BundlePin {
        target: "test-target",
        asset: ASSET,
        sha256: leak(digest),
    };
    let options = DownloadOptions {
        cache_root: temporary.path().join("cache"),
        release_base_url: "http://127.0.0.1:1".to_owned(),
        offline: true,
        installer_manifest: Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates directory")
            .join("hermes-lean-sys-installer/Cargo.toml"),
    };

    let error = acquire_bundle(&pin, &options, |_| Ok(())).expect_err("symlinked cache entry");
    assert!(error.contains("symlink"), "{error}");
}

#[test]
fn non_directory_cache_entry_is_rejected_before_use() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let digest = "5".repeat(64);
    let entry = temporary
        .path()
        .join("cache")
        .join(RELEASE_TAG)
        .join(&digest);
    fs::create_dir_all(entry.parent().expect("tag directory")).expect("tag directory");
    fs::write(&entry, b"not a cache directory").expect("cache file");
    let pin = BundlePin {
        target: "test-target",
        asset: ASSET,
        sha256: leak(digest),
    };
    let options = DownloadOptions {
        cache_root: temporary.path().join("cache"),
        release_base_url: "http://127.0.0.1:1".to_owned(),
        offline: true,
        installer_manifest: Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates directory")
            .join("hermes-lean-sys-installer/Cargo.toml"),
    };

    let error = acquire_bundle(&pin, &options, |_| Ok(())).expect_err("non-directory cache entry");
    assert!(error.contains("not a directory"), "{error}");
}

enum ArchiveEntry<'a> {
    Dir(&'a str),
    File(&'a str, &'a [u8]),
    Executable(&'a str, &'a [u8]),
    Symlink(&'a str, &'a str),
}

fn write_archive(path: &Path, entries: &[ArchiveEntry<'_>]) {
    let file = fs::File::create(path).expect("archive file");
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = tar::Builder::new(encoder);
    for entry in entries {
        match entry {
            ArchiveEntry::Dir(name) => {
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Directory);
                header.set_mode(0o755);
                header.set_size(0);
                header.set_cksum();
                archive
                    .append_data(&mut header, name, &[][..])
                    .expect("directory entry");
            }
            ArchiveEntry::File(name, contents) => {
                let mut header = tar::Header::new_gnu();
                header.set_mode(0o644);
                header.set_size(contents.len() as u64);
                header.set_cksum();
                archive
                    .append_data(&mut header, name, *contents)
                    .expect("regular entry");
            }
            ArchiveEntry::Executable(name, contents) => {
                let mut header = tar::Header::new_gnu();
                header.set_mode(0o755);
                header.set_size(contents.len() as u64);
                header.set_cksum();
                archive
                    .append_data(&mut header, name, *contents)
                    .expect("executable entry");
            }
            ArchiveEntry::Symlink(name, target) => {
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_mode(0o777);
                header.set_size(0);
                header.set_link_name(target).expect("link target");
                header.set_cksum();
                archive
                    .append_data(&mut header, name, &[][..])
                    .expect("link entry");
            }
        }
    }
    let encoder = archive.into_inner().expect("finish tar");
    encoder.finish().expect("finish gzip");
}

fn replace_first_tar_path(path: &Path, replacement: &str) {
    let compressed = fs::read(path).expect("compressed archive");
    let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
    let mut tar_bytes = Vec::new();
    decoder.read_to_end(&mut tar_bytes).expect("decode archive");
    assert!(replacement.len() < 100);
    tar_bytes[..100].fill(0);
    tar_bytes[..replacement.len()].copy_from_slice(replacement.as_bytes());
    tar_bytes[148..156].fill(b' ');
    let checksum: u32 = tar_bytes[..512].iter().map(|byte| u32::from(*byte)).sum();
    let field = format!("{checksum:06o}\0 ");
    tar_bytes[148..156].copy_from_slice(field.as_bytes());
    let file = fs::File::create(path).expect("rewritten archive");
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(&tar_bytes).expect("rewritten tar");
    encoder.finish().expect("finish rewritten gzip");
}

fn sha256_file(path: &Path) -> String {
    sha256_bytes(&fs::read(path).expect("digest input"))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = env::var_os(name);
        env::set_var(name, value);
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            env::set_var(self.name, previous);
        } else {
            env::remove_var(self.name);
        }
    }
}

#[test]
fn a_vendored_copy_without_the_installer_says_how_to_get_it() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let options = DownloadOptions {
        cache_root: temporary.path().join("cache"),
        release_base_url: "http://127.0.0.1:1".to_owned(),
        offline: true,
        installer_manifest: temporary
            .path()
            .join("vendor/hermes-lean-sys-installer/Cargo.toml"),
    };
    let command = installer_command(&options);
    assert!(!command.starts_with("cargo run"), "{command}");
    assert!(
        command.contains("vendor crates/hermes-lean-sys-installer"),
        "{command}"
    );

    let manifest = temporary
        .path()
        .join("ibex/crates/hermes-lean-sys-installer/Cargo.toml");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    std::fs::write(&manifest, "[package]\n").unwrap();
    let present = DownloadOptions {
        installer_manifest: manifest,
        ..options
    };
    assert!(installer_command(&present).starts_with("cargo run --manifest-path"));
}
