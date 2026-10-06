#![cfg(unix)]

use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const RELEASE_TAG: &str = "hermes-vanilla-d412d3bd8512-v4";
const HERMES_SOURCE_COMMIT: &str = "d412d3bd851278712c20cca25d094e32641a0465";

#[test]
fn install_once_then_build_offline_and_report_an_actionable_miss() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let cargo_home = temporary.path().join("cargo-home");
    seed_cargo_registry(&cargo_home);

    let host = rustc_host();
    let asset = format!("hermes-vanilla-test-{host}.tar.gz");
    let archive = temporary.path().join(&asset);
    write_test_bundle(&archive, &host);
    let archive_bytes = fs::read(&archive).expect("test bundle");
    let archive_digest = sha256(&archive_bytes);
    let (mirror, mirror_server) = serve_once(asset.clone(), archive_bytes);

    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("installer lives below the repository root");
    let installer_manifest = repository.join("crates/hermes-lean-sys-installer/Cargo.toml");
    let outside_workspace = temporary.path().join("consumer");
    fs::create_dir(&outside_workspace).expect("consumer directory");
    let tool_target = temporary.path().join("tool-target");
    let install = Command::new(cargo())
        .args(["run", "--offline", "--locked", "--manifest-path"])
        .arg(&installer_manifest)
        .args(["--", "--test-pin", &host, &asset, &archive_digest])
        .current_dir(&outside_workspace)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_TARGET_DIR", &tool_target)
        .env("HERMES_LEAN_SYS_MIRROR", &mirror)
        .env_remove("CARGO_NET_OFFLINE")
        // A consumer's forced [env] offline switch reaches the installer too;
        // the explicit install step must still download.
        .env("HERMES_LEAN_SYS_OFFLINE", "1")
        .output()
        .expect("run installer through Cargo");
    mirror_server.join().expect("mirror server");
    assert_success("installer", &install);

    let printed_command = format!(
        "cargo run --manifest-path {:?} -- --help",
        installer_manifest
    );
    let help = Command::new("sh")
        .args(["-c", &printed_command])
        .current_dir(&outside_workspace)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_TARGET_DIR", &tool_target)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run the printed command outside the Ibex workspace");
    assert_success("printed manifest-path command", &help);

    let entry = cargo_home
        .join("hermes-lean-sys")
        .join(RELEASE_TAG)
        .join(&archive_digest);
    assert!(entry.join(".hermes-lean-sys-bundle.tar.gz").is_file());
    assert!(entry.join("hermes-input-receipt.json").is_file());

    let (offline_mirror, stop, connections, watcher) = watch_connections();
    let success_fixture = temporary.path().join("offline-success");
    write_build_fixture(&success_fixture, repository);
    let success = cargo_build(
        &success_fixture,
        &cargo_home,
        &temporary.path().join("success-target"),
        &offline_mirror,
        &host,
        &asset,
        &archive_digest,
    );
    assert_success("offline build with installed cache", &success);

    // `--check` accepts the installed bundle without the network, and refuses a
    // pin whose bundle is absent, naming the install command.
    let check = |pin_asset: &str, pin_digest: &str| {
        Command::new(cargo())
            .args(["run", "--offline", "--locked", "--manifest-path"])
            .arg(&installer_manifest)
            .args(["--", "--check", "--test-pin", &host, pin_asset, pin_digest])
            .current_dir(&outside_workspace)
            .env("CARGO_HOME", &cargo_home)
            .env("CARGO_TARGET_DIR", &tool_target)
            .env("HERMES_LEAN_SYS_MIRROR", &offline_mirror)
            .env_remove("HERMES_LEAN_SYS_OFFLINE")
            .output()
            .expect("run installer --check through Cargo")
    };
    let checked = check(&asset, &archive_digest);
    assert_success("installer --check with installed cache", &checked);
    assert!(
        output_text(&checked).contains("Verified"),
        "--check did not report verification:\n{}",
        output_text(&checked)
    );
    let unchecked = check("hermes-vanilla-missing.tar.gz", &"0".repeat(64));
    assert!(
        !unchecked.status.success(),
        "--check must fail on a missing bundle"
    );
    assert!(
        output_text(&unchecked).contains("cargo run --manifest-path"),
        "--check miss does not name the install command:\n{}",
        output_text(&unchecked)
    );

    let missing_fixture = temporary.path().join("offline-missing");
    write_build_fixture(&missing_fixture, repository);
    let missing_digest = "0".repeat(64);
    let missing = cargo_build(
        &missing_fixture,
        &cargo_home,
        &temporary.path().join("missing-target"),
        &offline_mirror,
        &host,
        "hermes-vanilla-missing.tar.gz",
        &missing_digest,
    );
    assert!(!missing.status.success(), "empty offline cache must fail");
    let missing_output = output_text(&missing);
    let recovery_command = format!(
        "cargo run --manifest-path {:?} -- --target",
        installer_manifest
    );
    assert!(
        missing_output.contains(&recovery_command),
        "missing install command in:\n{missing_output}"
    );
    assert!(
        missing_output.contains("offline mode is enabled"),
        "missing offline diagnosis in:\n{missing_output}"
    );

    stop.store(true, Ordering::SeqCst);
    watcher.join().expect("offline network watcher");
    assert_eq!(
        connections.load(Ordering::SeqCst),
        0,
        "offline Cargo builds attempted to contact the configured mirror"
    );
}

#[test]
fn invalid_receipt_is_refused_before_the_cache_entry_is_published() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let cargo_home = temporary.path().join("cargo-home");
    fs::create_dir(&cargo_home).expect("Cargo home");
    let host = rustc_host();
    let asset = format!("hermes-vanilla-invalid-{host}.tar.gz");
    let archive = temporary.path().join(&asset);
    write_test_bundle_with_source_commit(&archive, &host, &"0".repeat(40));
    let archive_bytes = fs::read(&archive).expect("invalid test bundle");
    let archive_digest = sha256(&archive_bytes);
    let (mirror, mirror_server) = serve_once(asset.clone(), archive_bytes);

    let install = Command::new(env!("CARGO_BIN_EXE_hermes-lean-sys-installer"))
        .args(["--test-pin", &host, &asset, &archive_digest])
        .current_dir(temporary.path())
        .env("CARGO_HOME", &cargo_home)
        .env("HERMES_LEAN_SYS_MIRROR", &mirror)
        .env_remove("CARGO_NET_OFFLINE")
        .env_remove("HERMES_LEAN_SYS_OFFLINE")
        .output()
        .expect("run installer against invalid receipt");
    mirror_server.join().expect("mirror server");

    assert!(!install.status.success(), "invalid receipt must fail");
    let output = output_text(&install);
    assert!(output.contains("invalid canonical receipt"), "{output}");
    assert!(output.contains("sourceCommit"), "{output}");
    let entry = cargo_home
        .join("hermes-lean-sys")
        .join(RELEASE_TAG)
        .join(archive_digest);
    assert!(
        !entry.exists(),
        "invalid receipt became visible at {}",
        entry.display()
    );
}

fn seed_cargo_registry(cargo_home: &Path) {
    fs::create_dir(cargo_home).expect("fresh Cargo home");
    let source_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .expect("source Cargo home");
    for directory in ["registry", "git"] {
        let source = source_home.join(directory);
        if source.exists() {
            symlink(&source, cargo_home.join(directory)).expect("seed Cargo dependency cache");
        }
    }
}

fn cargo() -> PathBuf {
    env::var_os("CARGO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"))
}

fn rustc_host() -> String {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("-vV").output().expect("rustc -vV");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("UTF-8 rustc output")
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("rustc host")
        .to_owned()
}

fn write_test_bundle(path: &Path, target: &str) {
    write_test_bundle_with_source_commit(path, target, HERMES_SOURCE_COMMIT);
}

fn write_test_bundle_with_source_commit(path: &Path, target: &str, source_commit: &str) {
    let linux = target.ends_with("-unknown-linux-gnu");
    let compiler = b"#!/bin/sh\necho 'HBC bytecode version: 99'\n";
    let full = b"test full VM archive";
    let lean = b"test lean VM archive";
    let icu_full_data = b"test full ICU data archive";
    let icu_en_data = b"test English-Intl ICU data archive";
    let icu_trimmed_data = b"test trimmed ICU data archive";
    let icu_i18n = b"test ICU i18n code archive";
    let icu_uc = b"test ICU Unicode code archive";
    let icu_filter = include_bytes!("../../../scripts/icu74-filter-root-en.json");
    let icu_en_filter = include_bytes!("../../../scripts/icu74-filter-en-intl.json");
    let jsi = b"test JSI archive";
    let header = b"// test JSI header";
    let mut build_flags = vec![json!("-DHERMES_ENABLE_DEBUGGER=false")];
    let mut archive_entries = vec![
        json!({ "path": "lib/libhermesvm_a.a", "digest": format!("sha256-{}", sha256(full)) }),
        json!({ "path": "lib/libhermesvmlean_a.a", "digest": format!("sha256-{}", sha256(lean)) }),
        json!({ "path": "lib/libjsi.a", "digest": format!("sha256-{}", sha256(jsi)) }),
    ];
    if linux {
        build_flags.extend([
            json!("-DHERMES_ENABLE_INTL=false"),
            json!("-DHERMES_UNICODE_LITE=false"),
        ]);
        archive_entries.extend([
            json!({ "path": "lib/libicudata-en.a", "digest": format!("sha256-{}", sha256(icu_en_data)) }),
            json!({ "path": "lib/libicudata-full.a", "digest": format!("sha256-{}", sha256(icu_full_data)) }),
            json!({ "path": "lib/libicudata.a", "digest": format!("sha256-{}", sha256(icu_trimmed_data)) }),
            json!({ "path": "lib/libicui18n.a", "digest": format!("sha256-{}", sha256(icu_i18n)) }),
            json!({ "path": "lib/libicuuc.a", "digest": format!("sha256-{}", sha256(icu_uc)) }),
        ]);
    }
    archive_entries.sort_by(|left, right| {
        left["path"]
            .as_str()
            .expect("archive path")
            .cmp(right["path"].as_str().expect("archive path"))
    });
    let mut receipt = json!({
        "schema": "ibex/hermes-upstream-pinned-receipt/2",
        "upstream": {
            "artifact": "facebook/hermes",
            "sourceCommit": source_commit,
            "sourceRef": "hermes-v260318099.0.4",
            "sourceVersion": "260318099.0.4"
        },
        "patchSet": {
            "digest": "sha256-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "applied": []
        },
        "target": target,
        "profile": "release",
        "build": { "flags": build_flags },
        "bytecode": { "version": 99 },
        "compiler": {
            "binary": "bin/hermesc",
            "digest": format!("sha256-{}", sha256(compiler))
        },
        "engine": {
            "binary": "lib/libhermesvm_a.a",
            "binaryDigest": format!("sha256-{}", sha256(full)),
            "variant": "release"
        },
        "archives": archive_entries,
        "headers": [
            { "path": "include/jsi/jsi.h", "digest": format!("sha256-{}", sha256(header)) }
        ],
        "linkDirectives": [
            "rustc-link-search=native=lib",
            "rustc-link-lib=static=hermesvm_a"
        ]
    });
    if linux {
        receipt["icu"] = json!({
            "upstream": {
                "artifact": "unicode-org/icu",
                "sourceCommit": "2d029329c82c7792b985024b2bdab5fc7278fbc8",
                "sourceRef": "release-74-2",
                "sourceVersion": "74.2"
            },
            "codeArchives": ["lib/libicui18n.a", "lib/libicuuc.a"],
            "data": {
                "trimmed": {
                    "archive": "lib/libicudata.a",
                    "filter": {
                        "path": "share/icu/filters-root-en.json",
                        "digest": format!("sha256-{}", sha256(icu_filter))
                    }
                },
                "en": {
                    "archive": "lib/libicudata-en.a",
                    "filter": {
                        "path": "share/icu/filters-en-intl.json",
                        "digest": format!("sha256-{}", sha256(icu_en_filter))
                    }
                },
                "full": { "archive": "lib/libicudata-full.a" }
            }
        });
        receipt["linkDirectives"] = json!([
            "rustc-link-search=native=lib",
            "rustc-link-lib=static=hermesvm_a",
            "rustc-link-lib=static=icui18n",
            "rustc-link-lib=static=icuuc",
            "rustc-link-lib=static=icudata"
        ]);
    }
    let receipt = serde_json::to_vec_pretty(&receipt).expect("receipt JSON");

    let file = fs::File::create(path).expect("test bundle file");
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = tar::Builder::new(encoder);
    append(&mut archive, "bin/hermesc", compiler, 0o755);
    append(&mut archive, "include/jsi/jsi.h", header, 0o644);
    append(&mut archive, "lib/libhermesvm_a.a", full, 0o644);
    append(&mut archive, "lib/libhermesvmlean_a.a", lean, 0o644);
    append(&mut archive, "lib/libjsi.a", jsi, 0o644);
    if linux {
        append(&mut archive, "lib/libicudata-en.a", icu_en_data, 0o644);
        append(&mut archive, "lib/libicudata-full.a", icu_full_data, 0o644);
        append(&mut archive, "lib/libicudata.a", icu_trimmed_data, 0o644);
        append(&mut archive, "lib/libicui18n.a", icu_i18n, 0o644);
        append(&mut archive, "lib/libicuuc.a", icu_uc, 0o644);
        append(
            &mut archive,
            "share/icu/filters-root-en.json",
            icu_filter,
            0o644,
        );
        append(
            &mut archive,
            "share/icu/filters-en-intl.json",
            icu_en_filter,
            0o644,
        );
    }
    append(&mut archive, "hermes-input-receipt.json", &receipt, 0o644);
    let encoder = archive.into_inner().expect("finish tar");
    encoder.finish().expect("finish gzip");
}

fn append(archive: &mut tar::Builder<GzEncoder<fs::File>>, path: &str, bytes: &[u8], mode: u32) {
    let mut header = tar::Header::new_gnu();
    header.set_mode(mode);
    header.set_size(bytes.len() as u64);
    header.set_cksum();
    archive
        .append_data(&mut header, path, bytes)
        .expect("append bundle member");
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn serve_once(asset: String, bytes: Vec<u8>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local mirror");
    let address = listener.local_addr().expect("mirror address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("mirror request");
        let mut request = [0_u8; 4096];
        let count = stream.read(&mut request).expect("read mirror request");
        let request = String::from_utf8_lossy(&request[..count]);
        assert!(
            request.starts_with(&format!("GET /{RELEASE_TAG}/{asset} ")),
            "{request}"
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .expect("mirror response header");
        stream.write_all(&bytes).expect("mirror response body");
    });
    (format!("http://{address}"), server)
}

fn watch_connections() -> (
    String,
    Arc<AtomicBool>,
    Arc<AtomicUsize>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("offline mirror watcher");
    listener.set_nonblocking(true).expect("nonblocking watcher");
    let address = listener.local_addr().expect("watcher address");
    let stop = Arc::new(AtomicBool::new(false));
    let connections = Arc::new(AtomicUsize::new(0));
    let thread_stop = Arc::clone(&stop);
    let thread_connections = Arc::clone(&connections);
    let watcher = thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    thread_connections.fetch_add(1, Ordering::SeqCst);
                    let _ = stream.write_all(
                        b"HTTP/1.1 500 Offline test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("offline mirror watcher failed: {error}"),
            }
        }
    });
    (format!("http://{address}"), stop, connections, watcher)
}

fn write_build_fixture(root: &Path, repository: &Path) {
    fs::create_dir_all(root.join("src")).expect("fixture source directory");
    fs::create_dir_all(root.join(".cargo")).expect("fixture Cargo config directory");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "hermes-offline-build-fixture"
version = "0.0.0"
edition = "2021"
publish = false
build = "build.rs"

[workspace]

[build-dependencies]
flate2 = "1"
libc = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
tar = "0.4"
tempfile = "3"
ureq = { version = "=3.4.0", default-features = false, features = ["rustls"] }
"#,
    )
    .expect("fixture manifest");
    fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("fixture library");
    fs::write(
        root.join(".cargo/config.toml"),
        "[env]\nHERMES_LEAN_SYS_OFFLINE = { value = \"1\", force = true }\n",
    )
    .expect("fixture Cargo config");
    let support = repository.join("crates/hermes-lean-sys/build_support.rs");
    let hermes_manifest_dir = repository.join("crates/hermes-lean-sys");
    fs::write(
        root.join("build.rs"),
        format!(
            r#"#[allow(dead_code)]
#[path = {support:?}]
mod build_support;

use build_support::{{
    acquire_bundle, download_options_from_env, validate_host_bundle, BundlePin,
}};

fn main() {{
    let target = leak(std::env::var("TEST_HERMES_TARGET").expect("test target"));
    let asset = leak(std::env::var("TEST_HERMES_ASSET").expect("test asset"));
    let sha256 = leak(std::env::var("TEST_HERMES_SHA256").expect("test digest"));
    let pin = BundlePin {{ target, asset, sha256 }};
    let options = download_options_from_env(std::path::Path::new({hermes_manifest_dir:?}))
        .expect("download options");
    assert!(options.offline, "fixture must force hermes-lean-sys offline mode");
    let root = acquire_bundle(&pin, &options, |candidate| {{
        validate_host_bundle(candidate.to_path_buf(), target, false).map(drop)
    }})
        .unwrap_or_else(|error| panic!("Hermes engine resolution failed: {{error}}"));
    validate_host_bundle(root, target, false)
        .unwrap_or_else(|error| panic!("Hermes engine validation failed: {{error}}"));
}}

fn leak(value: String) -> &'static str {{
    Box::leak(value.into_boxed_str())
}}
"#,
            support = support,
            hermes_manifest_dir = hermes_manifest_dir,
        ),
    )
    .expect("fixture build script");
}

fn cargo_build(
    fixture: &Path,
    cargo_home: &Path,
    target_dir: &Path,
    mirror: &str,
    target: &str,
    asset: &str,
    digest: &str,
) -> Output {
    Command::new(cargo())
        .args(["build", "--offline"])
        .current_dir(fixture)
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("HERMES_LEAN_SYS_MIRROR", mirror)
        .env("TEST_HERMES_TARGET", target)
        .env("TEST_HERMES_ASSET", asset)
        .env("TEST_HERMES_SHA256", digest)
        .output()
        .expect("offline Cargo build")
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed:\n{}",
        output_text(output)
    );
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
