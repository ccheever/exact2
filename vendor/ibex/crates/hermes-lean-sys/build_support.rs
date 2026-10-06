use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

#[path = "receipt_schema.rs"]
mod receipt_schema;

pub(crate) const RELEASE_TAG: &str = "hermes-vanilla-d412d3bd8512-v3";
const DEFAULT_RELEASE_BASE_URL: &str = "https://github.com/expo/ibex/releases/download";
pub(crate) const CACHE_ARCHIVE: &str = ".hermes-lean-sys-bundle.tar.gz";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BundlePin {
    pub target: &'static str,
    pub asset: &'static str,
    pub sha256: &'static str,
}

// @ref LLP 0057.000#l1--the-bindings-door — this table is the trust root for
// the compiler/VM identity shared by the bindings and the owning runtime.
// The v3 release is not published yet, so every digest is a deliberately
// rejecting sentinel. They MUST be replaced with the v3 asset digests only
// after the immutable release and its Sigstore attestations are verified; see
// scripts/update-hermes-lean-sys-pins.mjs.
pub(crate) const PINNED_BUNDLES: &[BundlePin] = &[
    BundlePin {
        target: "aarch64-apple-darwin",
        asset: "hermes-vanilla-aarch64-apple-darwin.tar.gz",
        sha256: "b2789fd4aa33b711b3a86525ad562e7abb65375e8b728ead03eebc8d4c386544",
    },
    BundlePin {
        target: "x86_64-apple-darwin",
        asset: "hermes-vanilla-x86_64-apple-darwin.tar.gz",
        sha256: "c33c595960e75569ad8f623f8f1db48cd65d0a296a0ed4e98abce0989593e175",
    },
    BundlePin {
        target: "aarch64-apple-ios",
        asset: "hermes-vanilla-aarch64-apple-ios.tar.gz",
        sha256: "3f5a57f7ecd8382144a155352a63ff23b10f5e23648967487fb6ebdd62640c17",
    },
    BundlePin {
        target: "aarch64-apple-ios-sim",
        asset: "hermes-vanilla-universal-apple-ios-simulator.tar.gz",
        sha256: "d48df1a3d49b8815a55e332d39eb6a8881b1217ae2362558998d9ade41a1d9d9",
    },
    BundlePin {
        target: "x86_64-apple-ios",
        asset: "hermes-vanilla-universal-apple-ios-simulator.tar.gz",
        sha256: "d48df1a3d49b8815a55e332d39eb6a8881b1217ae2362558998d9ade41a1d9d9",
    },
    BundlePin {
        target: "x86_64-unknown-linux-gnu",
        asset: "hermes-vanilla-x86_64-unknown-linux-gnu.tar.gz",
        sha256: "b566a7493ee71163d0c4aa792342407b51f2caf27017f3c010720d94d1618bb7",
    },
    BundlePin {
        target: "aarch64-unknown-linux-gnu",
        asset: "hermes-vanilla-aarch64-unknown-linux-gnu.tar.gz",
        sha256: "a5b4fe404eb606bd3e021273573c21217a0f614c808abe48ab67bc6d2315608d",
    },
    BundlePin {
        target: "x86_64-pc-windows-msvc",
        asset: "hermes-vanilla-x86_64-pc-windows-msvc.tar.gz",
        sha256: "607f78b14ece437c6069869ff6a57cd0fc4ae0be040d91824049ce3c69f7c38b",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstallOrigin {
    Override,
    Repository,
    Bundle,
}

#[derive(Debug)]
struct InstallLayout {
    root: PathBuf,
    include_dir: PathBuf,
    lib_root: PathBuf,
    vm_archive: PathBuf,
    lean_vm_archive: PathBuf,
    icu_i18n_archive: Option<PathBuf>,
    icu_uc_archive: Option<PathBuf>,
    icu_data_archive: Option<PathBuf>,
    icu_full_data_archive: Option<PathBuf>,
    icu_trimmed_filter: Option<PathBuf>,
    origin: InstallOrigin,
    requires_receipt: bool,
    target: String,
}

#[derive(Debug)]
pub(crate) struct EngineInstall {
    pub root: PathBuf,
    pub include_dir: PathBuf,
    pub lib_root: PathBuf,
    pub vm_archive: PathBuf,
    pub lean_vm_archive: Option<PathBuf>,
    pub icu_i18n_archive: Option<PathBuf>,
    pub icu_uc_archive: Option<PathBuf>,
    pub icu_data_archive: Option<PathBuf>,
    pub icu_full_data_archive: Option<PathBuf>,
    pub icu_trimmed_filter: Option<PathBuf>,
    pub hermesc: PathBuf,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct ValidatedHostBundle {
    pub root: PathBuf,
    pub hermesc: PathBuf,
    pub bytecode_version: String,
}

pub(crate) fn watched_inputs(install: &EngineInstall, target: &str) -> Vec<PathBuf> {
    let windows = target.ends_with("-pc-windows-msvc");
    let mut paths = BTreeSet::from([
        install.root.clone(),
        install.include_dir.clone(),
        install.vm_archive.clone(),
        install.hermesc.clone(),
        install.root.join("hermes-input-receipt.json"),
        install.root.join(CACHE_ARCHIVE),
    ]);
    if let Some(lean_vm_archive) = &install.lean_vm_archive {
        paths.insert(lean_vm_archive.clone());
    }
    for archive in if windows {
        ["jsi.lib", "boost_context.lib"]
    } else {
        ["libjsi.a", "libboost_context.a"]
    } {
        paths.insert(install.lib_root.join(archive));
    }
    for path in [
        &install.icu_i18n_archive,
        &install.icu_uc_archive,
        &install.icu_data_archive,
        &install.icu_full_data_archive,
        &install.icu_trimmed_filter,
    ]
    .into_iter()
    .flatten()
    {
        paths.insert(path.clone());
    }
    if target.ends_with("-unknown-linux-gnu") {
        paths.insert(install.lib_root.join("libtinfo.a"));
    }
    paths.into_iter().collect()
}

/// The inputs to name in `cargo:rerun-if-changed`. Cargo treats a watched path
/// that doesn't exist as permanently stale, which would rerun this script (and
/// rehash the engine) on every build of a layout that lacks, say, the retained
/// cache archive. The root is always watched, and Cargo scans a watched
/// directory recursively, so a file created later still triggers a rerun.
pub(crate) fn rerun_paths(install: &EngineInstall, target: &str) -> Vec<PathBuf> {
    watched_inputs(install, target)
        .into_iter()
        .filter(|path| *path == install.root || path.exists())
        .collect()
}

#[derive(Debug)]
pub(crate) struct DownloadOptions {
    pub cache_root: PathBuf,
    pub release_base_url: String,
    pub offline: bool,
    pub installer_manifest: PathBuf,
}

#[derive(Deserialize)]
struct Receipt {
    engine: ReceiptEngine,
    #[serde(default)]
    compiler: Option<ReceiptCompiler>,
    #[serde(default)]
    bytecode: Option<ReceiptBytecode>,
}

#[derive(Deserialize)]
struct ReceiptEngine {
    binary: String,
    #[serde(rename = "binaryDigest")]
    binary_digest: String,
}

#[derive(Deserialize)]
struct ReceiptCompiler {
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Deserialize)]
struct ReceiptBytecode {
    version: serde_json::Value,
}

struct ReceiptClaims {
    engine_binary: String,
    engine_digest: String,
    compiler_digest: Option<String>,
    bytecode_version: Option<String>,
    archive_digests: Option<BTreeMap<String, String>>,
    icu: Option<receipt_schema::CanonicalIcuReceipt>,
}

pub(crate) fn pin_for_target(target: &str) -> Result<&'static BundlePin, String> {
    PINNED_BUNDLES
        .iter()
        .find(|pin| pin.target == target)
        .ok_or_else(|| {
            format!(
                "unsupported Hermes target {target}; set HERMES_LEAN_SYS_DIR to a complete local install"
            )
        })
}

pub(crate) fn parse_pin_sha256(value: &str) -> Result<String, String> {
    if value.starts_with("TODO_L1G_SHA256_") {
        return Err(format!(
            "the Hermes bundle digest pin {value} is awaiting publication; set HERMES_LEAN_SYS_DIR to a complete local install"
        ));
    }
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "invalid pinned Hermes archive SHA-256 {value:?}; expected 64 hexadecimal digits"
        ));
    }
    Ok(value.to_ascii_lowercase())
}

pub(crate) fn download_options_from_env(manifest_dir: &Path) -> Result<DownloadOptions, String> {
    let cargo_home = match env::var_os("CARGO_HOME") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => default_cargo_home()?,
    };
    let crates_dir = manifest_dir.parent().ok_or_else(|| {
        format!(
            "cannot locate the Ibex crates directory above {}",
            manifest_dir.display()
        )
    })?;
    Ok(DownloadOptions {
        cache_root: cargo_home.join("hermes-lean-sys"),
        release_base_url: env::var("HERMES_LEAN_SYS_MIRROR")
            .unwrap_or_else(|_| DEFAULT_RELEASE_BASE_URL.to_owned()),
        offline: env_truthy("CARGO_NET_OFFLINE") || env_truthy("HERMES_LEAN_SYS_OFFLINE"),
        installer_manifest: crates_dir
            .join("hermes-lean-sys-installer")
            .join("Cargo.toml"),
    })
}

pub(crate) fn installer_command(options: &DownloadOptions) -> String {
    if options.installer_manifest.is_file() {
        format!(
            "cargo run --manifest-path {:?} --",
            options.installer_manifest
        )
    } else {
        // A vendored copy of hermes-lean-sys alone has no sibling installer.
        format!(
            "hermes-lean-sys-installer (not found at {:?}; vendor crates/hermes-lean-sys-installer \
             beside hermes-lean-sys from the same Ibex revision, or run it from an Ibex checkout \
             whose hermes-lean-sys pins the same release tag and digest)",
            options.installer_manifest
        )
    }
}

fn default_cargo_home() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .or_else(|| env::var_os("USERPROFILE").filter(|path| !path.is_empty()))
        .map(|home| PathBuf::from(home).join(".cargo"))
        .ok_or_else(|| {
            "cannot determine Cargo home; set CARGO_HOME or HERMES_LEAN_SYS_DIR".to_owned()
        })
}

fn env_truthy(name: &str) -> bool {
    env::var(name)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub(crate) fn resolve_engine_directory(
    repo_root: &Path,
    target: &str,
    host: &str,
    require_lean: bool,
) -> Result<EngineInstall, String> {
    let manifest_dir = repo_root.join("crates").join("hermes-lean-sys");
    let mut validated_bundle_compiler = None;
    let target_layout = if let Some(overridden) = env::var_os("HERMES_LEAN_SYS_DIR") {
        install_layout(PathBuf::from(overridden), target, InstallOrigin::Override)
    } else {
        let pin = pin_for_target(target)?;
        if let Some(root) = repository_install_root(repo_root, target) {
            install_layout(root, target, InstallOrigin::Repository)
        } else {
            let options = download_options_from_env(&manifest_dir)?;
            let root = if target == host {
                let bundle = acquire_validated_host_bundle(pin, &options, target, require_lean)?;
                let root = bundle.root.clone();
                validated_bundle_compiler = Some(bundle);
                root
            } else {
                let host_pin = pin_for_target(host).map_err(|_| {
                    format!(
                        "unsupported Hermes compiler host {host}; set HERMES_LEAN_SYS_DIR cannot replace the required pinned host compiler bundle for cross compilation"
                    )
                })?;
                let host_bundle = acquire_validated_host_bundle(host_pin, &options, host, false)?;
                let root = acquire_validated_target_bundle(
                    pin,
                    &options,
                    target,
                    &host_bundle,
                    require_lean,
                )?;
                validated_bundle_compiler = Some(host_bundle);
                root
            };
            install_layout(root, target, InstallOrigin::Bundle)
        }
    };

    validate_layout(&target_layout, require_lean)?;
    let (hermesc, bytecode_version) = if let Some(bundle) = validated_bundle_compiler {
        (bundle.hermesc, bundle.bytecode_version)
    } else if target != host {
        let host_pin = pin_for_target(host).map_err(|_| {
            format!(
                "unsupported Hermes compiler host {host}; set HERMES_LEAN_SYS_DIR cannot replace the required pinned host compiler bundle for cross compilation"
            )
        })?;
        let options = download_options_from_env(&manifest_dir)?;
        let bundle = acquire_validated_host_bundle(host_pin, &options, host, false)?;
        (bundle.hermesc, bundle.bytecode_version)
    } else if target_layout.origin == InstallOrigin::Repository {
        let compiler = repository_hermesc(repo_root, host)?;
        let version = validate_compiler_bundle(&target_layout, &compiler)?;
        (compiler, version)
    } else {
        let local = compiler_in_bundle_or_install(&target_layout.root, host);
        let compiler = if local.is_file() || target_layout.origin != InstallOrigin::Override {
            local
        } else if repository_install_root(repo_root, target)
            .as_deref()
            .is_some_and(|repository_root| same_location(repository_root, &target_layout.root))
        {
            repository_hermesc(repo_root, host)?
        } else {
            local
        };
        let version = validate_compiler_bundle(&target_layout, &compiler)?;
        (compiler, version)
    };

    if !hermesc.is_file() {
        return Err(format!(
            "hermesc not found at {}; set HERMES_LEAN_SYS_DIR to a complete local install",
            hermesc.display()
        ));
    }
    let engine_digest = digest_file(&target_layout.vm_archive)?;
    validate_receipt(
        &target_layout,
        &hermesc,
        target == host,
        &engine_digest,
        &bytecode_version,
    )?;

    let lean_vm_archive = authenticated_lean_archive(&target_layout);

    Ok(EngineInstall {
        root: target_layout.root,
        include_dir: target_layout.include_dir,
        lib_root: target_layout.lib_root,
        vm_archive: target_layout.vm_archive,
        lean_vm_archive,
        icu_i18n_archive: target_layout.icu_i18n_archive,
        icu_uc_archive: target_layout.icu_uc_archive,
        icu_data_archive: target_layout.icu_data_archive,
        icu_full_data_archive: target_layout.icu_full_data_archive,
        icu_trimmed_filter: target_layout.icu_trimmed_filter,
        hermesc,
    })
}

pub(crate) fn repository_install_root(repo_root: &Path, target: &str) -> Option<PathBuf> {
    let root = if matches!(target, "aarch64-apple-darwin" | "x86_64-apple-darwin") {
        repo_root.join("ios/Frameworks-vanilla")
    } else if target.ends_with("-pc-windows-msvc") {
        let arch = if target.starts_with("x86_64-") {
            "x64"
        } else if target.starts_with("aarch64-") {
            "arm64"
        } else {
            return None;
        };
        repo_root.join(format!("tools/hermes-vanilla/windows-{arch}"))
    } else if target.ends_with("-unknown-linux-gnu") {
        repo_root.join("linux/Frameworks-vanilla")
    } else {
        return None;
    };
    root.is_dir().then_some(root)
}

fn install_layout(root: PathBuf, target: &str, origin: InstallOrigin) -> InstallLayout {
    let bundle_layout = root.join("include").is_dir() || root.join("lib").is_dir();
    let (include_dir, lib_root) = if bundle_layout {
        (root.join("include"), root.join("lib"))
    } else {
        let lib_dir = if target.contains("-apple-") {
            "macos-static"
        } else if target.ends_with("-pc-windows-msvc") {
            "windows-static"
        } else {
            "linux-static"
        };
        (root.join("hermes-headers"), root.join(lib_dir))
    };
    let (archive, lean_archive) = if target.ends_with("-pc-windows-msvc") {
        ("hermesvm_a.lib", "hermesvmlean_a.lib")
    } else {
        ("libhermesvm_a.a", "libhermesvmlean_a.a")
    };
    let linux_icu = target.ends_with("-unknown-linux-gnu");
    InstallLayout {
        icu_i18n_archive: linux_icu.then(|| lib_root.join("libicui18n.a")),
        icu_uc_archive: linux_icu.then(|| lib_root.join("libicuuc.a")),
        icu_data_archive: linux_icu.then(|| lib_root.join("libicudata.a")),
        icu_full_data_archive: linux_icu.then(|| lib_root.join("libicudata-full.a")),
        icu_trimmed_filter: linux_icu
            .then(|| root.join("share").join("icu").join("filters-root-en.json")),
        root,
        include_dir,
        vm_archive: lib_root.join(archive),
        lean_vm_archive: lib_root.join(lean_archive),
        lib_root,
        origin,
        requires_receipt: bundle_layout,
        target: target.to_owned(),
    }
}

fn repository_hermesc(repo_root: &Path, host: &str) -> Result<PathBuf, String> {
    let (os, arch, suffix) = if host == "aarch64-apple-darwin" {
        ("macos", "arm64", "")
    } else if host == "x86_64-apple-darwin" {
        ("macos", "x64", "")
    } else if host == "aarch64-unknown-linux-gnu" {
        ("linux", "arm64", "")
    } else if host == "x86_64-unknown-linux-gnu" {
        ("linux", "x64", "")
    } else if host == "x86_64-pc-windows-msvc" {
        ("windows", "x64", ".exe")
    } else {
        return Err(format!(
            "unsupported Hermes compiler host {host}; set HERMES_LEAN_SYS_DIR does not change which host can execute hermesc"
        ));
    };
    Ok(repo_root
        .join("tools/hermes-vanilla")
        .join(format!("hermesc-{os}-{arch}{suffix}")))
}

fn compiler_in_bundle_or_install(root: &Path, host: &str) -> PathBuf {
    let name = if host.ends_with("-pc-windows-msvc") {
        "hermesc.exe"
    } else {
        "hermesc"
    };
    [root.join("bin").join(name), root.join(name)]
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| root.join("bin").join(name))
}

fn same_location(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn validate_layout(layout: &InstallLayout, require_lean: bool) -> Result<(), String> {
    let mut required = vec![
        ("Hermes headers", &layout.include_dir, true),
        ("Hermes library directory", &layout.lib_root, true),
        ("Hermes VM archive", &layout.vm_archive, false),
    ];
    if require_lean {
        required.push(("lean Hermes VM archive", &layout.lean_vm_archive, false));
    }
    for (label, path) in [
        ("ICU i18n code archive", &layout.icu_i18n_archive),
        ("ICU Unicode code archive", &layout.icu_uc_archive),
        ("trimmed ICU data archive", &layout.icu_data_archive),
        ("full ICU data archive", &layout.icu_full_data_archive),
        ("trimmed ICU data filter", &layout.icu_trimmed_filter),
    ] {
        if let Some(path) = path {
            required.push((label, path, false));
        }
    }
    for (label, path, want_dir) in required {
        let exists = if want_dir {
            path.is_dir()
        } else {
            path.is_file()
        };
        if !exists {
            return Err(format!(
                "{label} not found at {}; set HERMES_LEAN_SYS_DIR to a complete local install{} or populate the pinned bundle cache",
                path.display(),
                if require_lean && label == "lean Hermes VM archive" {
                    " containing the lean archive"
                } else {
                    ""
                },
            ));
        }
    }
    Ok(())
}

fn validate_compiler_bundle(layout: &InstallLayout, compiler: &Path) -> Result<String, String> {
    if !compiler.is_file() {
        return Err(format!("hermesc not found at {}", compiler.display()));
    }
    authenticate_compiler(layout, compiler)?;
    let engine_digest = digest_file(&layout.vm_archive)?;
    let bytecode_version = hermesc_bytecode_version(compiler)?;
    validate_receipt(layout, compiler, true, &engine_digest, &bytecode_version)?;
    Ok(bytecode_version)
}

/// Apply the same layout, compiler, receipt, and archive checks that the build
/// resolver applies to a native bundle, without consulting repository layouts.
/// Both the build resolver and explicit installer use this while a downloaded
/// bundle is still staged, before `acquire_bundle` publishes its cache entry.
#[allow(dead_code)]
pub(crate) fn validate_host_bundle(
    root: PathBuf,
    target: &str,
    require_lean: bool,
) -> Result<ValidatedHostBundle, String> {
    let layout = install_layout(root, target, InstallOrigin::Bundle);
    validate_layout(&layout, require_lean)?;
    let hermesc = compiler_in_bundle_or_install(&layout.root, target);
    let bytecode_version = validate_compiler_bundle(&layout, &hermesc)?;
    Ok(ValidatedHostBundle {
        root: layout.root,
        hermesc,
        bytecode_version,
    })
}

/// Apply the build resolver's target-side checks using the already
/// authenticated host compiler identity and HBC version. Cross bundles carry
/// their own compiler, but Cargo never executes it on the host and therefore
/// does not require it to have the host compiler's independently built digest.
#[allow(dead_code)]
pub(crate) fn validate_target_bundle(
    root: PathBuf,
    target: &str,
    host: &ValidatedHostBundle,
    require_lean: bool,
) -> Result<(), String> {
    let layout = install_layout(root, target, InstallOrigin::Bundle);
    validate_layout(&layout, require_lean)?;
    let engine_digest = digest_file(&layout.vm_archive)?;
    validate_receipt(
        &layout,
        &host.hermesc,
        false,
        &engine_digest,
        &host.bytecode_version,
    )
}

pub(crate) fn acquire_validated_host_bundle(
    pin: &BundlePin,
    options: &DownloadOptions,
    target: &str,
    require_lean: bool,
) -> Result<ValidatedHostBundle, String> {
    let bytecode_version = RefCell::new(None);
    let root = acquire_bundle(pin, options, |candidate| {
        let bundle = validate_host_bundle(candidate.to_path_buf(), target, require_lean)?;
        *bytecode_version.borrow_mut() = Some(bundle.bytecode_version);
        Ok(())
    })?;
    let bytecode_version = bytecode_version
        .into_inner()
        .expect("successful admission ran the host bundle validator");
    let layout = install_layout(root, target, InstallOrigin::Bundle);
    let hermesc = compiler_in_bundle_or_install(&layout.root, target);
    Ok(ValidatedHostBundle {
        root: layout.root,
        hermesc,
        bytecode_version,
    })
}

pub(crate) fn acquire_validated_target_bundle(
    pin: &BundlePin,
    options: &DownloadOptions,
    target: &str,
    host: &ValidatedHostBundle,
    require_lean: bool,
) -> Result<PathBuf, String> {
    acquire_bundle(pin, options, |candidate| {
        validate_target_bundle(candidate.to_path_buf(), target, host, require_lean)
    })
}

fn authenticate_compiler(layout: &InstallLayout, compiler: &Path) -> Result<(), String> {
    let Some(receipt) = read_receipt_claims(layout)? else {
        return Ok(());
    };
    let receipt_path = layout.root.join("hermes-input-receipt.json");
    let expected = receipt.compiler_digest.ok_or_else(|| {
        format!(
            "{} has no compiler digest, so {} cannot be authenticated before execution",
            receipt_path.display(),
            compiler.display()
        )
    })?;
    let actual = digest_file(compiler)?;
    if expected != actual {
        return Err(format!(
            "{} records compiler digest {}, but selected hermesc {} has {}; refusing to execute an unauthenticated compiler",
            receipt_path.display(),
            expected,
            compiler.display(),
            actual
        ));
    }
    Ok(())
}

/// The lean VM archive is offered only when the install's receipt has
/// authenticated it. `validate_receipt` (already run for this layout) refuses a
/// receipt that doesn't bind a present lean archive, so a receipt plus a lean
/// archive means an authenticated one. A receipt-free legacy layout keeps
/// working for the full VM, but exports no lean identity and can't satisfy
/// `link-lean`: its lean archive would be a self-measured, unattested digest.
fn authenticated_lean_archive(layout: &InstallLayout) -> Option<PathBuf> {
    (layout.root.join("hermes-input-receipt.json").is_file() && layout.lean_vm_archive.is_file())
        .then(|| layout.lean_vm_archive.clone())
}

fn validate_receipt(
    layout: &InstallLayout,
    compiler: &Path,
    validate_compiler_digest: bool,
    engine_digest: &str,
    bytecode_version: &str,
) -> Result<(), String> {
    let receipt_path = layout.root.join("hermes-input-receipt.json");
    let Some(receipt) = read_receipt_claims(layout)? else {
        if layout.target.ends_with("-unknown-linux-gnu") {
            return Err(format!(
                "Linux Hermes install {} has no canonical receipt binding its ICU code and trimmed/full data archives",
                layout.root.display()
            ));
        }
        return Ok(());
    };
    if layout.target.ends_with("-unknown-linux-gnu") {
        validate_linux_icu_receipt(layout, &receipt, &receipt_path)?;
    }
    let receipt_archive = safe_relative_path(&layout.root, Path::new(&receipt.engine_binary))?;
    let selected_archive = fs::canonicalize(&layout.vm_archive).map_err(|error| {
        format!(
            "cannot resolve selected Hermes archive {}: {error}",
            layout.vm_archive.display()
        )
    })?;
    let described_archive = fs::canonicalize(&receipt_archive).map_err(|error| {
        format!(
            "receipt engine archive {} does not exist: {error}",
            receipt_archive.display()
        )
    })?;
    if selected_archive != described_archive {
        return Err(format!(
            "{} describes engine archive {}, but hermes-lean-sys selected {}",
            receipt_path.display(),
            receipt.engine_binary,
            layout.vm_archive.display()
        ));
    }
    if receipt.engine_digest != engine_digest {
        return Err(format!(
            "{} records engine digest {}, but ENGINE_DIGEST is {}",
            receipt_path.display(),
            receipt.engine_digest,
            engine_digest
        ));
    }
    // A receipt governs every VM archive present in its install, independent
    // of which link feature this compilation enables. This prevents a
    // build-dependency instance from exporting an unauthenticated lean
    // identity for a sibling runtime instance to trust.
    if layout.lean_vm_archive.is_file() {
        let lean_digest = digest_file(&layout.lean_vm_archive)?;
        let relative = layout
            .lean_vm_archive
            .strip_prefix(&layout.root)
            .map_err(|_| {
                format!(
                    "selected lean Hermes archive {} is outside engine root {}",
                    layout.lean_vm_archive.display(),
                    layout.root.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let manifest = receipt.archive_digests.as_ref().ok_or_else(|| {
            format!(
                "{} has no archive manifest, so it cannot authenticate selected lean Hermes archive {}",
                receipt_path.display(),
                layout.lean_vm_archive.display()
            )
        })?;
        let recorded = manifest.get(&relative).ok_or_else(|| {
            format!(
                "{} does not bind selected lean Hermes archive {} in its archive manifest",
                receipt_path.display(),
                relative
            )
        })?;
        if recorded != &lean_digest {
            return Err(format!(
                "{} records lean engine digest {}, but LEAN_ENGINE_DIGEST is {}",
                receipt_path.display(),
                recorded,
                lean_digest
            ));
        }
    }
    if validate_compiler_digest {
        if let Some(compiler_digest) = receipt.compiler_digest {
            let selected_compiler_digest = digest_file(compiler)?;
            if compiler_digest != selected_compiler_digest {
                return Err(format!(
                    "{} records compiler digest {}, but selected hermesc {} has {}; the receipt describes a different compiler than this build uses",
                    receipt_path.display(),
                    compiler_digest,
                    compiler.display(),
                    selected_compiler_digest
                ));
            }
        }
    }
    if let Some(receipt_version) = receipt.bytecode_version {
        if receipt_version != bytecode_version {
            return Err(format!(
                "{} records HBC bytecode version {}, but selected hermesc {} reports {}",
                receipt_path.display(),
                receipt_version,
                compiler.display(),
                bytecode_version
            ));
        }
    }
    Ok(())
}

fn validate_linux_icu_receipt(
    layout: &InstallLayout,
    receipt: &ReceiptClaims,
    receipt_path: &Path,
) -> Result<(), String> {
    let icu = receipt.icu.as_ref().ok_or_else(|| {
        format!(
            "{} has no pinned Linux ICU metadata",
            receipt_path.display()
        )
    })?;
    let manifest = receipt.archive_digests.as_ref().ok_or_else(|| {
        format!(
            "{} has no archive manifest for Linux ICU",
            receipt_path.display()
        )
    })?;
    let required = [
        (
            "ICU i18n code archive",
            layout.icu_i18n_archive.as_ref(),
            icu.code_archives.first(),
        ),
        (
            "ICU Unicode code archive",
            layout.icu_uc_archive.as_ref(),
            icu.code_archives.get(1),
        ),
        (
            "trimmed ICU data archive",
            layout.icu_data_archive.as_ref(),
            Some(&icu.trimmed_data_archive),
        ),
        (
            "full ICU data archive",
            layout.icu_full_data_archive.as_ref(),
            Some(&icu.full_data_archive),
        ),
    ];
    for (label, selected, claimed) in required {
        let selected = selected.ok_or_else(|| format!("Linux layout has no {label}"))?;
        let claimed =
            claimed.ok_or_else(|| format!("{} has no {label}", receipt_path.display()))?;
        let relative = relative_install_path(&layout.root, selected)?;
        if &relative != claimed {
            return Err(format!(
                "{} records {label} {}, but hermes-lean-sys selected {}",
                receipt_path.display(),
                claimed,
                relative
            ));
        }
        let recorded = manifest.get(&relative).ok_or_else(|| {
            format!(
                "{} does not bind selected {label} {}",
                receipt_path.display(),
                relative
            )
        })?;
        let actual = digest_file(selected)?;
        if recorded != &actual {
            return Err(format!(
                "{} records {label} digest {}, but selected archive has {}",
                receipt_path.display(),
                recorded,
                actual
            ));
        }
    }

    let filter = layout
        .icu_trimmed_filter
        .as_ref()
        .ok_or("Linux layout has no trimmed ICU data filter")?;
    let relative = relative_install_path(&layout.root, filter)?;
    if relative != icu.trimmed_filter_path {
        return Err(format!(
            "{} records trimmed ICU filter {}, but hermes-lean-sys selected {}",
            receipt_path.display(),
            icu.trimmed_filter_path,
            relative
        ));
    }
    let actual = digest_file(filter)?;
    if actual != icu.trimmed_filter_digest {
        return Err(format!(
            "{} records trimmed ICU filter digest {}, but selected filter has {}",
            receipt_path.display(),
            icu.trimmed_filter_digest,
            actual
        ));
    }
    Ok(())
}

fn relative_install_path(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            format!(
                "selected Hermes input {} is outside install root {}",
                path.display(),
                root.display()
            )
        })
}

fn read_receipt_claims(layout: &InstallLayout) -> Result<Option<ReceiptClaims>, String> {
    let receipt_path = layout.root.join("hermes-input-receipt.json");
    if !receipt_path.is_file() {
        if layout.requires_receipt {
            return Err(format!(
                "pinned Hermes bundle is missing {}",
                receipt_path.display()
            ));
        }
        return Ok(None);
    }
    let bytes = fs::read(&receipt_path)
        .map_err(|error| format!("cannot read {}: {error}", receipt_path.display()))?;
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", receipt_path.display()))?;
    if document.get("schema").and_then(serde_json::Value::as_str) == Some(receipt_schema::SCHEMA) {
        let receipt_target = receipt_schema::bundle_target_for_rust_target(&layout.target);
        let receipt =
            receipt_schema::validate(&document, Some(receipt_target)).map_err(|error| {
                format!(
                    "invalid canonical receipt {}: {error}",
                    receipt_path.display()
                )
            })?;
        debug_assert_eq!(receipt.target, receipt_target);
        let archive_digests = document["archives"]
            .as_array()
            .expect("canonical validator requires archives")
            .iter()
            .map(|entry| {
                let path = entry["path"]
                    .as_str()
                    .expect("canonical validator requires archive paths")
                    .to_owned();
                let digest = receipt
                    .archive_digests()
                    .get(&path)
                    .expect("canonical validator parsed every archive")
                    .to_owned();
                (path, digest)
            })
            .collect();
        return Ok(Some(ReceiptClaims {
            engine_binary: receipt.engine_binary,
            engine_digest: receipt.engine_digest,
            compiler_digest: Some(receipt.compiler_digest),
            bytecode_version: Some(receipt.bytecode_version.to_string()),
            archive_digests: Some(archive_digests),
            icu: receipt.icu,
        }));
    }
    if layout.requires_receipt {
        return Err(format!(
            "pinned Hermes bundle receipt {} is not schema {}",
            receipt_path.display(),
            receipt_schema::SCHEMA
        ));
    }

    let receipt: Receipt = serde_json::from_value(document)
        .map_err(|error| format!("cannot parse legacy {}: {error}", receipt_path.display()))?;
    let bytecode_version = match receipt.bytecode.map(|bytecode| bytecode.version) {
        None => None,
        Some(serde_json::Value::Number(number)) => Some(number.to_string()),
        Some(serde_json::Value::String(text)) => Some(text),
        Some(other) => {
            return Err(format!(
                "{} has invalid bytecode.version {other}",
                receipt_path.display()
            ));
        }
    };
    Ok(Some(ReceiptClaims {
        engine_binary: receipt.engine.binary,
        engine_digest: receipt.engine.binary_digest,
        compiler_digest: receipt.compiler.and_then(|compiler| compiler.digest),
        bytecode_version,
        archive_digests: None,
        icu: None,
    }))
}

fn safe_relative_path(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(format!(
            "unsafe absolute or empty path in Hermes receipt: {}",
            relative.display()
        ));
    }
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(format!(
                "unsafe path in Hermes receipt: {}",
                relative.display()
            ));
        }
    }
    Ok(root.join(relative))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArchiveEntryKind {
    Directory,
    File,
}

#[derive(Default)]
struct ArchivePathValidator {
    entries: BTreeMap<String, ArchiveEntryKind>,
    folded_namespace: BTreeMap<String, String>,
}

impl ArchivePathValidator {
    fn admit(&mut self, raw: &[u8], kind: ArchiveEntryKind) -> Result<PathBuf, String> {
        let normalized = normalize_archive_path(raw, kind)?;
        if self.entries.contains_key(&normalized) {
            return Err(format!("duplicate normalized tar path {normalized:?}"));
        }

        let components: Vec<&str> = normalized.split('/').collect();
        for index in 1..=components.len() {
            let prefix = components[..index].join("/");
            let folded = prefix.to_lowercase();
            if let Some(existing) = self.folded_namespace.get(&folded) {
                if existing != &prefix {
                    return Err(format!(
                        "case-folded tar path collision between {existing:?} and {prefix:?}"
                    ));
                }
            } else {
                self.folded_namespace.insert(folded, prefix);
            }
        }

        for index in 1..components.len() {
            let ancestor = components[..index].join("/").to_lowercase();
            if self.entries.iter().any(|(path, entry_kind)| {
                path.to_lowercase() == ancestor && *entry_kind == ArchiveEntryKind::File
            }) {
                return Err(format!(
                    "tar path {normalized:?} has a file ancestor {}",
                    components[..index].join("/")
                ));
            }
        }
        if kind == ArchiveEntryKind::File {
            let descendant_prefix = format!("{}/", normalized.to_lowercase());
            if let Some(descendant) = self
                .entries
                .keys()
                .find(|path| path.to_lowercase().starts_with(&descendant_prefix))
            {
                return Err(format!(
                    "tar file path {normalized:?} conflicts with descendant {descendant:?}"
                ));
            }
        }

        self.entries.insert(normalized.clone(), kind);
        Ok(PathBuf::from(normalized))
    }
}

fn normalize_archive_path(raw: &[u8], kind: ArchiveEntryKind) -> Result<String, String> {
    let original = std::str::from_utf8(raw).map_err(|_| "tar path is not UTF-8".to_string())?;
    let path = if kind == ArchiveEntryKind::Directory {
        original.strip_suffix('/').unwrap_or(original)
    } else {
        original
    };
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return Err(format!(
            "unsafe absolute, empty, or backslash tar path {path:?}"
        ));
    }
    let mut normalized = Vec::new();
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(format!("unsafe tar path component in {path:?}"));
        }
        if component.ends_with('.') || component.ends_with(' ') {
            return Err(format!(
                "Windows-aliased trailing dot or space in tar path {path:?}"
            ));
        }
        if component
            .bytes()
            .any(|byte| matches!(byte, b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*' | 0))
        {
            return Err(format!(
                "Windows prefix or forbidden character in tar path {path:?}"
            ));
        }
        let stem = component
            .split_once('.')
            .map_or(component, |(stem, _)| stem)
            .to_ascii_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
        ) || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|number| number.len() == 1 && matches!(number.as_bytes()[0], b'1'..=b'9'));
        if reserved {
            return Err(format!("Windows reserved name in tar path {path:?}"));
        }
        normalized.push(component);
    }
    Ok(normalized.join("/"))
}

pub(crate) fn digest_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut DigestWriter(&mut digest))
        .map_err(|error| format!("cannot hash {}: {error}", path.display()))?;
    Ok(format!("sha256-{:x}", digest.finalize()))
}

struct DigestWriter<'a>(&'a mut Sha256);

impl Write for DigestWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn hermesc_bytecode_version(hermesc: &Path) -> Result<String, String> {
    let output = std::process::Command::new(hermesc)
        .arg("-version")
        .output()
        .map_err(|error| format!("cannot run {}: {error}", hermesc.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} -version failed with {}",
            hermesc.display(),
            output.status
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    text.lines()
        .find_map(|line| line.trim().strip_prefix("HBC bytecode version:"))
        .map(str::trim)
        .filter(|version| !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "{} did not report an HBC bytecode version",
                hermesc.display()
            )
        })
}

/// Acquire a pinned bundle and publish it only after both archive/tree checks
/// and the caller's complete receipt/compiler/pairing validator succeed.
pub(crate) fn acquire_bundle<F>(
    pin: &BundlePin,
    options: &DownloadOptions,
    validate_bundle: F,
) -> Result<PathBuf, String>
where
    F: Fn(&Path) -> Result<(), String>,
{
    let expected_digest = parse_pin_sha256(pin.sha256)?;
    let recovery = format!(
        "this build pins {RELEASE_TAG}/{} at sha256-{expected_digest}; while online run `{} --target {}`",
        pin.asset,
        installer_command(options),
        pin.target,
    );
    let entry = options.cache_root.join(RELEASE_TAG).join(&expected_digest);
    if fs::symlink_metadata(&entry).is_ok() {
        match validate_cache_entry(&entry, &expected_digest).and_then(|()| validate_bundle(&entry))
        {
            Ok(()) => return Ok(entry),
            Err(error) if options.offline => {
                return Err(format!(
                    "cached Hermes bundle {} is invalid and offline mode is enabled: {error}; {recovery} to replace it, or set HERMES_LEAN_SYS_DIR to a complete local install",
                    entry.display(),
                ));
            }
            Err(_) => remove_cache_entry(&entry)?,
        }
    }
    if options.offline {
        return Err(format!(
            "Hermes bundle {} is not cached at {} and offline mode is enabled; {recovery} to install it, or set HERMES_LEAN_SYS_DIR to a complete local install",
            pin.asset,
            entry.display(),
        ));
    }

    let tag_dir = options.cache_root.join(RELEASE_TAG);
    fs::create_dir_all(&tag_dir)
        .map_err(|error| format!("cannot create Hermes cache {}: {error}", tag_dir.display()))?;
    let staging = tempfile::Builder::new()
        .prefix(".download-")
        .tempdir_in(&tag_dir)
        .map_err(|error| format!("cannot create a Hermes cache staging directory: {error}"))?;
    let archive_path = staging.path().join(CACHE_ARCHIVE);
    let url = format!(
        "{}/{}/{}",
        options.release_base_url.trim_end_matches('/'),
        RELEASE_TAG,
        pin.asset
    );
    download_archive(&url, &archive_path, &expected_digest)?;

    let extracted = staging.path().join("install");
    fs::create_dir(&extracted)
        .map_err(|error| format!("cannot create {}: {error}", extracted.display()))?;
    extract_archive_safely(&archive_path, &extracted)?;
    fs::rename(&archive_path, extracted.join(CACHE_ARCHIVE)).map_err(|error| {
        format!(
            "cannot retain the verified Hermes archive in {}: {error}",
            extracted.display()
        )
    })?;
    validate_cache_entry(&extracted, &expected_digest)?;
    validate_bundle(&extracted)?;

    match fs::rename(&extracted, &entry) {
        Ok(()) => Ok(entry),
        Err(error) if fs::symlink_metadata(&entry).is_ok() => {
            match validate_cache_entry(&entry, &expected_digest)
                .and_then(|()| validate_bundle(&entry))
            {
                Ok(()) => Ok(entry),
                Err(cache_error) => {
                    remove_cache_entry(&entry)?;
                    fs::rename(&extracted, &entry).map_err(|retry_error| {
                        format!(
                            "parallel Hermes cache install lost a race ({error}); the winning entry was invalid ({cache_error}) and replacing it failed: {retry_error}"
                        )
                    })?;
                    validate_cache_entry(&entry, &expected_digest)?;
                    validate_bundle(&entry)?;
                    Ok(entry)
                }
            }
        }
        Err(error) => Err(format!(
            "cannot atomically install Hermes bundle into {}: {error}",
            entry.display()
        )),
    }
}

fn remove_cache_entry(entry: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(entry).map_err(|error| {
        format!(
            "cannot inspect invalid cache entry {}: {error}",
            entry.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        fs::remove_file(entry).map_err(|error| {
            format!(
                "cannot remove invalid cache entry {}: {error}",
                entry.display()
            )
        })
    } else {
        fs::remove_dir_all(entry).map_err(|error| {
            format!(
                "cannot remove invalid cache entry {}: {error}",
                entry.display()
            )
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ArchiveTree {
    files: BTreeMap<PathBuf, String>,
    directories: BTreeSet<PathBuf>,
}

fn validate_cache_entry(entry: &Path, expected_digest: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(entry)
        .map_err(|error| format!("cannot inspect cache entry {}: {error}", entry.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("cache entry {} is a symlink", entry.display()));
    }
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "cache entry {} is not a directory",
            entry.display()
        ));
    }

    let archive_path = entry.join(CACHE_ARCHIVE);
    let archive_metadata = fs::symlink_metadata(&archive_path).map_err(|error| {
        format!(
            "cached archive {} is missing or unreadable: {error}",
            archive_path.display()
        )
    })?;
    if !archive_metadata.file_type().is_file() {
        return Err(format!(
            "cached archive {} is not a regular file",
            archive_path.display()
        ));
    }
    let expected = archive_tree(&archive_path, expected_digest)?;
    let actual = extracted_tree(entry)?;
    if expected == actual {
        return Ok(());
    }
    for (path, digest) in &expected.files {
        match actual.files.get(path) {
            None => {
                return Err(format!(
                    "cached extraction is missing file {}",
                    path.display()
                ))
            }
            Some(actual_digest) if actual_digest != digest => {
                return Err(format!(
                    "cached extraction file {} has {}, not archive digest {}",
                    path.display(),
                    actual_digest,
                    digest
                ));
            }
            Some(_) => {}
        }
    }
    if let Some(path) = actual
        .files
        .keys()
        .find(|path| !expected.files.contains_key(*path))
    {
        return Err(format!(
            "cached extraction contains extra file {}",
            path.display()
        ));
    }
    if let Some(path) = expected
        .directories
        .iter()
        .find(|path| !actual.directories.contains(*path))
    {
        return Err(format!(
            "cached extraction is missing directory {}",
            path.display()
        ));
    }
    if let Some(path) = actual
        .directories
        .iter()
        .find(|path| !expected.directories.contains(*path))
    {
        return Err(format!(
            "cached extraction contains extra directory {}",
            path.display()
        ));
    }
    Err("cached extraction does not match its retained archive".to_string())
}

fn archive_tree(archive_path: &Path, expected_digest: &str) -> Result<ArchiveTree, String> {
    let mut file = File::open(archive_path)
        .map_err(|error| format!("cannot open {}: {error}", archive_path.display()))?;
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut DigestWriter(&mut digest))
        .map_err(|error| format!("cannot hash {}: {error}", archive_path.display()))?;
    let actual_digest = format!("{:x}", digest.finalize());
    if actual_digest != expected_digest {
        return Err(format!(
            "cached archive {} has sha256-{}, not pinned sha256-{}",
            archive_path.display(),
            actual_digest,
            expected_digest
        ));
    }
    file.rewind()
        .map_err(|error| format!("cannot rewind {}: {error}", archive_path.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let mut tree = ArchiveTree {
        files: BTreeMap::new(),
        directories: BTreeSet::new(),
    };
    let mut paths = ArchivePathValidator::default();
    {
        let entries = archive
            .entries()
            .map_err(|error| format!("cannot read {}: {error}", archive_path.display()))?;
        for entry in entries {
            let mut entry = entry.map_err(|error| {
                format!(
                    "cannot read an entry from {}: {error}",
                    archive_path.display()
                )
            })?;
            let entry_type = entry.header().entry_type();
            let kind = if entry_type.is_dir() {
                ArchiveEntryKind::Directory
            } else if entry_type.is_file() {
                ArchiveEntryKind::File
            } else {
                return Err(format!(
                    "refusing non-file tar entry {:?}; links and special files are forbidden",
                    entry_type
                ));
            };
            let path = paths
                .admit(entry.path_bytes().as_ref(), kind)
                .map_err(|error| {
                    format!(
                        "refusing unsafe entry in {}: {error}",
                        archive_path.display()
                    )
                })?;
            if path == Path::new(CACHE_ARCHIVE) {
                return Err(format!(
                    "archive entry {} collides with cache metadata",
                    path.display()
                ));
            }
            add_parent_directories(&mut tree.directories, &path);
            if entry_type.is_dir() {
                tree.directories.insert(path);
            } else if entry_type.is_file() {
                let executable = entry.header().mode().unwrap_or(0o644) & 0o111 != 0;
                let mut digest = Sha256::new();
                io::copy(&mut entry, &mut DigestWriter(&mut digest)).map_err(|error| {
                    format!(
                        "cannot hash archive member {} from {}: {error}",
                        path.display(),
                        archive_path.display()
                    )
                })?;
                tree.files.insert(
                    path,
                    with_mode(format!("sha256-{:x}", digest.finalize()), executable),
                );
            }
        }
    }
    Ok(tree)
}

/// A cached file's identity is its digest plus, where the host has one, its
/// executable bit: extraction applies the archive's mode, so a cache entry whose
/// `bin/hermesc` lost its execute bit is stale and is refetched rather than
/// admitted and then failing with "permission denied" forever.
fn with_mode(digest: String, executable: bool) -> String {
    if cfg!(unix) && executable {
        format!("{digest} (executable)")
    } else {
        digest
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

fn add_parent_directories(directories: &mut BTreeSet<PathBuf>, path: &Path) {
    let mut parent = path.parent();
    while let Some(directory) = parent {
        if directory.as_os_str().is_empty() {
            break;
        }
        directories.insert(directory.to_path_buf());
        parent = directory.parent();
    }
}

fn extracted_tree(root: &Path) -> Result<ArchiveTree, String> {
    fn visit(root: &Path, directory: &Path, tree: &mut ArchiveTree) -> Result<(), String> {
        let entries = fs::read_dir(directory).map_err(|error| {
            format!(
                "cannot read cache directory {}: {error}",
                directory.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "cannot read an entry under {}: {error}",
                    directory.display()
                )
            })?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|error| {
                format!("cannot make {} cache-relative: {error}", path.display())
            })?;
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
            if relative == Path::new(CACHE_ARCHIVE) {
                if !metadata.file_type().is_file() {
                    return Err(format!(
                        "cached archive {} is not a regular file",
                        path.display()
                    ));
                }
                continue;
            }
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "cached extraction contains symlink {}",
                    path.display()
                ));
            }
            if metadata.file_type().is_dir() {
                tree.directories.insert(relative.to_path_buf());
                visit(root, &path, tree)?;
            } else if metadata.file_type().is_file() {
                tree.files.insert(
                    relative.to_path_buf(),
                    with_mode(digest_file(&path)?, is_executable(&metadata)),
                );
            } else {
                return Err(format!(
                    "cached extraction contains special file {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    let mut tree = ArchiveTree {
        files: BTreeMap::new(),
        directories: BTreeSet::new(),
    };
    visit(root, root, &mut tree)?;
    Ok(tree)
}

fn download_archive(url: &str, destination: &Path, expected_digest: &str) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|error| format!("cannot download pinned Hermes bundle {url}: {error}"))?;
    let mut reader = response.into_body().into_reader();
    let mut output = File::create(destination)
        .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("cannot read pinned Hermes bundle {url}: {error}"))?;
        if count == 0 {
            break;
        }
        output
            .write_all(&buffer[..count])
            .map_err(|error| format!("cannot write {}: {error}", destination.display()))?;
        digest.update(&buffer[..count]);
    }
    output
        .sync_all()
        .map_err(|error| format!("cannot finish {}: {error}", destination.display()))?;
    let actual = format!("{:x}", digest.finalize());
    if actual != expected_digest {
        return Err(format!(
            "refusing Hermes bundle from {url}: SHA-256 mismatch (pinned {expected_digest}, downloaded {actual}); nothing was extracted"
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn verify_and_extract_archive(
    archive_path: &Path,
    expected_digest: &str,
    destination: &Path,
) -> Result<(), String> {
    let expected_digest = parse_pin_sha256(expected_digest)?;
    let actual = digest_file(archive_path)?
        .strip_prefix("sha256-")
        .expect("digest_file prefixes SHA-256")
        .to_owned();
    if actual != expected_digest {
        return Err(format!(
            "refusing Hermes bundle {}: SHA-256 mismatch (pinned {}, downloaded {}); nothing was extracted",
            archive_path.display(),
            expected_digest,
            actual
        ));
    }
    extract_archive_safely(archive_path, destination)
}

fn extract_archive_safely(archive_path: &Path, destination: &Path) -> Result<(), String> {
    validate_archive_entries(archive_path)?;
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(format!(
                "archive destination {} is not a directory",
                destination.display()
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(destination)
            .map_err(|error| format!("cannot create {}: {error}", destination.display()))?,
        Err(error) => {
            return Err(format!(
                "cannot inspect archive destination {}: {error}",
                destination.display()
            ));
        }
    }
    let file = File::open(archive_path)
        .map_err(|error| format!("cannot open {}: {error}", archive_path.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| format!("cannot read {}: {error}", archive_path.display()))?;
    let mut paths = ArchivePathValidator::default();
    let mut created_directories = BTreeSet::new();
    for entry in entries {
        let mut entry = entry.map_err(|error| {
            format!(
                "cannot read an entry from {}: {error}",
                archive_path.display()
            )
        })?;
        let entry_type = entry.header().entry_type();
        let kind = if entry_type.is_dir() {
            ArchiveEntryKind::Directory
        } else if entry_type.is_file() {
            ArchiveEntryKind::File
        } else {
            unreachable!("archive entry types were validated before extraction");
        };
        let path = paths
            .admit(entry.path_bytes().as_ref(), kind)
            .expect("archive paths were validated before extraction");
        let output_path = destination.join(&path);
        if entry_type.is_dir() {
            create_extracted_directories(destination, &path, &mut created_directories)?;
        } else if entry_type.is_file() {
            if let Some(parent) = path.parent() {
                create_extracted_directories(destination, parent, &mut created_directories)?;
            }
            let mode = entry.header().mode().unwrap_or(0o644) & 0o777;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output_path)
                .map_err(|error| {
                    format!(
                        "refusing to overwrite extracted path {}: {error}",
                        output_path.display()
                    )
                })?;
            if let Err(error) = io::copy(&mut entry, &mut output) {
                drop(output);
                let _ = fs::remove_file(&output_path);
                return Err(format!(
                    "cannot extract regular file {}: {error}",
                    output_path.display()
                ));
            }
            set_extracted_permissions(&output_path, mode)?;
        } else {
            unreachable!("archive entry types were validated before extraction");
        }
    }
    Ok(())
}

fn create_extracted_directories(
    destination: &Path,
    relative: &Path,
    created: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    let mut current_relative = PathBuf::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            unreachable!("normalized archive path has only normal components");
        };
        current_relative.push(component);
        if created.contains(&current_relative) {
            continue;
        }
        let path = destination.join(&current_relative);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&path).map_err(|error| {
                    format!(
                        "cannot create extracted directory {}: {error}",
                        path.display()
                    )
                })?;
                created.insert(current_relative.clone());
            }
            Ok(_) => {
                return Err(format!(
                    "refusing to overwrite extracted path {}",
                    path.display()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "cannot inspect extracted path {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_extracted_permissions(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("cannot set permissions on {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn set_extracted_permissions(_path: &Path, _mode: u32) -> Result<(), String> {
    Ok(())
}

fn validate_archive_entries(archive_path: &Path) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|error| format!("cannot open {}: {error}", archive_path.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| format!("cannot read {}: {error}", archive_path.display()))?;
    let mut paths = ArchivePathValidator::default();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "cannot read an entry from {}: {error}",
                archive_path.display()
            )
        })?;
        let entry_type = entry.header().entry_type();
        let kind = if entry_type.is_dir() {
            ArchiveEntryKind::Directory
        } else if entry_type.is_file() {
            ArchiveEntryKind::File
        } else {
            return Err(format!(
                "refusing non-file tar entry type {:?}; links and special files are forbidden",
                entry_type
            ));
        };
        paths
            .admit(entry.path_bytes().as_ref(), kind)
            .map_err(|error| {
                format!(
                    "refusing unsafe entry in {}: {error}",
                    archive_path.display()
                )
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod internal_tests {
    use super::*;

    #[test]
    fn a_receipt_free_layout_offers_no_lean_archive() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("legacy");
        fs::create_dir_all(root.join("hermes-headers")).expect("headers");
        fs::create_dir_all(root.join("macos-static")).expect("lib");
        fs::write(root.join("macos-static/libhermesvm_a.a"), b"full").expect("full");
        fs::write(root.join("macos-static/libhermesvmlean_a.a"), b"lean").expect("lean");
        let layout = install_layout(
            root.clone(),
            "aarch64-apple-darwin",
            InstallOrigin::Override,
        );
        assert_eq!(authenticated_lean_archive(&layout), None);

        fs::write(root.join("hermes-input-receipt.json"), b"{}").expect("receipt marker");
        assert_eq!(
            authenticated_lean_archive(&layout),
            Some(root.join("macos-static/libhermesvmlean_a.a")),
            "with a receipt, validate_receipt has authenticated the lean archive"
        );
    }

    fn write_v2_receipt(
        root: &Path,
        target: &str,
        engine_binary: &str,
        engine_digest: &str,
        compiler_digest: &str,
        bytecode_version: u64,
    ) {
        let mut receipt: serde_json::Value =
            serde_json::from_str(include_str!("testdata/receipt-v2-valid.json"))
                .expect("shared receipt fixture");
        receipt["target"] = serde_json::Value::String(target.to_owned());
        receipt["engine"]["binary"] = serde_json::Value::String(engine_binary.to_owned());
        receipt["engine"]["binaryDigest"] = serde_json::Value::String(engine_digest.to_owned());
        receipt["compiler"]["digest"] = serde_json::Value::String(compiler_digest.to_owned());
        receipt["bytecode"]["version"] = serde_json::Value::from(bytecode_version);
        receipt["archives"] = serde_json::json!([
            { "path": engine_binary, "digest": engine_digest }
        ]);
        fs::write(
            root.join("hermes-input-receipt.json"),
            serde_json::to_vec_pretty(&receipt).expect("receipt JSON"),
        )
        .expect("receipt");
    }

    fn write_linux_v3_fixture(root: &Path) -> (InstallLayout, String) {
        for directory in ["include", "lib", "bin", "share/icu"] {
            fs::create_dir_all(root.join(directory)).expect("fixture directory");
        }
        let files = [
            ("lib/libhermesvm_a.a", b"engine".as_slice()),
            ("lib/libicudata-full.a", b"full data".as_slice()),
            ("lib/libicudata.a", b"trimmed data".as_slice()),
            ("lib/libicui18n.a", b"i18n code".as_slice()),
            ("lib/libicuuc.a", b"unicode code".as_slice()),
            ("bin/hermesc", b"compiler".as_slice()),
            ("include/sentinel.h", b"header".as_slice()),
        ];
        for (path, bytes) in files {
            fs::write(root.join(path), bytes).expect("fixture file");
        }
        fs::write(
            root.join("share/icu/filters-root-en.json"),
            include_bytes!("../../scripts/icu74-filter-root-en.json"),
        )
        .expect("pinned filter");

        let digest = |path: &str| digest_file(&root.join(path)).expect("fixture digest");
        let engine_digest = digest("lib/libhermesvm_a.a");
        let receipt = serde_json::json!({
            "schema": receipt_schema::SCHEMA,
            "upstream": {
                "artifact": "facebook/hermes",
                "sourceCommit": receipt_schema::SOURCE_COMMIT,
                "sourceRef": "hermes-v260318099.0.4",
                "sourceVersion": "260318099.0.4"
            },
            "patchSet": {
                "digest": receipt_schema::EMPTY_PATCH_SET,
                "applied": []
            },
            "target": "aarch64-unknown-linux-gnu",
            "profile": "release",
            "build": { "flags": [
                "-DHERMES_ENABLE_INTL=false",
                "-DHERMES_UNICODE_LITE=false"
            ] },
            "bytecode": { "version": 99 },
            "compiler": {
                "binary": "bin/hermesc",
                "digest": digest("bin/hermesc")
            },
            "engine": {
                "binary": "lib/libhermesvm_a.a",
                "binaryDigest": engine_digest,
                "variant": "release"
            },
            "archives": [
                { "path": "lib/libhermesvm_a.a", "digest": digest("lib/libhermesvm_a.a") },
                { "path": "lib/libicudata-full.a", "digest": digest("lib/libicudata-full.a") },
                { "path": "lib/libicudata.a", "digest": digest("lib/libicudata.a") },
                { "path": "lib/libicui18n.a", "digest": digest("lib/libicui18n.a") },
                { "path": "lib/libicuuc.a", "digest": digest("lib/libicuuc.a") }
            ],
            "headers": [
                { "path": "include/sentinel.h", "digest": digest("include/sentinel.h") }
            ],
            "icu": {
                "upstream": {
                    "artifact": "unicode-org/icu",
                    "sourceCommit": receipt_schema::ICU_SOURCE_COMMIT,
                    "sourceRef": "release-74-2",
                    "sourceVersion": "74.2"
                },
                "codeArchives": ["lib/libicui18n.a", "lib/libicuuc.a"],
                "data": {
                    "trimmed": {
                        "archive": "lib/libicudata.a",
                        "filter": {
                            "path": "share/icu/filters-root-en.json",
                            "digest": receipt_schema::ICU_TRIMMED_FILTER_DIGEST
                        }
                    },
                    "full": { "archive": "lib/libicudata-full.a" }
                }
            },
            "linkDirectives": [
                "rustc-link-search=native=lib",
                "rustc-link-lib=static=hermesvm_a",
                "rustc-link-lib=static=icui18n",
                "rustc-link-lib=static=icuuc",
                "rustc-link-lib=static=icudata"
            ]
        });
        fs::write(
            root.join("hermes-input-receipt.json"),
            serde_json::to_vec_pretty(&receipt).expect("receipt JSON"),
        )
        .expect("receipt");
        (
            install_layout(
                root.to_path_buf(),
                "aarch64-unknown-linux-gnu",
                InstallOrigin::Bundle,
            ),
            engine_digest,
        )
    }

    #[test]
    fn linux_layout_and_receipt_require_and_authenticate_both_icu_data_variants() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let (layout, engine_digest) = write_linux_v3_fixture(temporary.path());
        validate_layout(&layout, false).expect("complete Linux layout");
        validate_receipt(
            &layout,
            &temporary.path().join("bin/hermesc"),
            true,
            &engine_digest,
            "99",
        )
        .expect("receipt authenticates both ICU data variants");

        fs::write(
            temporary.path().join("lib/libicudata-full.a"),
            b"tampered full data",
        )
        .expect("tamper full data");
        let error = validate_receipt(
            &layout,
            &temporary.path().join("bin/hermesc"),
            true,
            &engine_digest,
            "99",
        )
        .expect_err("tampered full data is refused even when trimmed is selected");
        assert!(error.contains("full ICU data archive digest"), "{error}");

        fs::remove_file(temporary.path().join("lib/libicudata-full.a")).expect("remove full data");
        let error = validate_layout(&layout, false).expect_err("both data variants are required");
        assert!(error.contains("full ICU data archive"), "{error}");
    }

    #[test]
    fn published_layout_requires_a_v2_receipt_even_for_an_override() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir_all(root.join("include")).expect("include directory");
        fs::create_dir_all(root.join("lib")).expect("lib directory");
        fs::write(root.join("lib/libhermesvm_a.a"), b"engine").expect("engine archive");
        let layout = install_layout(
            root.to_path_buf(),
            "aarch64-apple-darwin",
            InstallOrigin::Override,
        );

        let error = validate_receipt(
            &layout,
            &root.join("bin/hermesc"),
            true,
            "sha256-unused",
            "96",
        )
        .expect_err("published layout without a receipt must fail");
        assert!(error.contains("missing"), "{error}");
    }

    #[test]
    fn receipt_refuses_a_different_selected_compiler() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir_all(root.join("include")).expect("include directory");
        fs::create_dir_all(root.join("lib")).expect("lib directory");
        fs::create_dir_all(root.join("bin")).expect("bin directory");
        fs::write(root.join("lib/libhermesvm_a.a"), b"engine").expect("engine archive");
        fs::write(root.join("bin/hermesc"), b"selected compiler").expect("compiler");
        let engine_digest = digest_file(&root.join("lib/libhermesvm_a.a")).expect("digest");
        write_v2_receipt(
            root,
            "aarch64-apple-darwin",
            "lib/libhermesvm_a.a",
            &engine_digest,
            &format!("sha256-{}", "0".repeat(64)),
            96,
        );
        let layout = install_layout(
            root.to_path_buf(),
            "aarch64-apple-darwin",
            InstallOrigin::Bundle,
        );

        let error = validate_receipt(
            &layout,
            &root.join("bin/hermesc"),
            true,
            &engine_digest,
            "96",
        )
        .expect_err("compiler mismatch must fail");
        assert!(error.contains("different compiler"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn compiler_digest_is_checked_before_hermesc_executes() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir_all(root.join("include")).expect("include directory");
        fs::create_dir_all(root.join("lib")).expect("library directory");
        fs::create_dir_all(root.join("bin")).expect("compiler directory");
        fs::write(root.join("lib/libhermesvm_a.a"), b"engine").expect("engine archive");
        let marker = root.join("executed");
        let compiler = root.join("bin/hermesc");
        fs::write(
            &compiler,
            format!(
                "#!/bin/sh\ntouch '{}'\necho 'HBC bytecode version: 96'\n",
                marker.display()
            ),
        )
        .expect("compiler script");
        let mut permissions = fs::metadata(&compiler).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&compiler, permissions).expect("executable compiler");
        let engine_digest = digest_file(&root.join("lib/libhermesvm_a.a")).expect("digest");
        write_v2_receipt(
            root,
            "aarch64-apple-darwin",
            "lib/libhermesvm_a.a",
            &engine_digest,
            &format!("sha256-{}", "0".repeat(64)),
            96,
        );
        let layout = install_layout(
            root.to_path_buf(),
            "aarch64-apple-darwin",
            InstallOrigin::Bundle,
        );

        let error = validate_compiler_bundle(&layout, &compiler)
            .expect_err("unauthenticated compiler must fail");
        assert!(error.contains("refusing to execute"), "{error}");
        assert!(
            !marker.exists(),
            "hermesc ran before its digest was authenticated"
        );
    }

    #[test]
    fn receipt_requires_the_selected_engine_archive() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir_all(root.join("lib")).expect("lib directory");
        fs::create_dir_all(root.join("bin")).expect("bin directory");
        fs::write(root.join("lib/libhermesvm_a.a"), b"selected").expect("selected archive");
        fs::create_dir(root.join("decoy")).expect("decoy directory");
        fs::write(root.join("decoy/libhermesvm_a.a"), b"decoy").expect("decoy archive");
        fs::write(root.join("bin/hermesc"), b"compiler").expect("compiler");
        let engine_digest = digest_file(&root.join("lib/libhermesvm_a.a")).expect("digest");
        write_v2_receipt(
            root,
            "aarch64-apple-darwin",
            "decoy/libhermesvm_a.a",
            &engine_digest,
            &digest_file(&root.join("bin/hermesc")).expect("compiler digest"),
            96,
        );
        let layout = install_layout(
            root.to_path_buf(),
            "aarch64-apple-darwin",
            InstallOrigin::Bundle,
        );

        let error = validate_receipt(
            &layout,
            &root.join("bin/hermesc"),
            true,
            &engine_digest,
            "96",
        )
        .expect_err("decoy archive must fail");
        assert!(error.contains("selected"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn cross_bundle_pairing_uses_the_host_compiler_receipt_and_hbc_version() {
        use std::os::unix::fs::PermissionsExt;

        fn bundle(root: &Path, compiler: &[u8], version: u64) -> (InstallLayout, PathBuf) {
            fs::create_dir_all(root.join("include")).expect("include directory");
            fs::create_dir_all(root.join("lib")).expect("library directory");
            fs::create_dir_all(root.join("bin")).expect("compiler directory");
            fs::write(
                root.join("lib/libhermesvm_a.a"),
                root.as_os_str().as_encoded_bytes(),
            )
            .expect("engine archive");
            let compiler_path = root.join("bin/hermesc");
            fs::write(&compiler_path, compiler).expect("compiler");
            let mut permissions = fs::metadata(&compiler_path)
                .expect("metadata")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&compiler_path, permissions).expect("executable compiler");
            let engine_digest =
                digest_file(&root.join("lib/libhermesvm_a.a")).expect("engine digest");
            let compiler_digest = digest_file(&compiler_path).expect("compiler digest");
            write_v2_receipt(
                root,
                "aarch64-apple-darwin",
                "lib/libhermesvm_a.a",
                &engine_digest,
                &compiler_digest,
                version,
            );
            (
                install_layout(
                    root.to_path_buf(),
                    "aarch64-apple-darwin",
                    InstallOrigin::Bundle,
                ),
                compiler_path,
            )
        }

        let temporary = tempfile::tempdir().expect("temporary directory");
        let target_script = b"#!/bin/sh\necho 'HBC bytecode version: 96'\n# target compiler\n";
        let host_script =
            b"#!/bin/sh\necho 'HBC bytecode version: 96'\n# different host compiler\n";
        let (target, _) = bundle(&temporary.path().join("target"), target_script, 96);
        let (host, host_compiler) = bundle(&temporary.path().join("host"), host_script, 96);

        let host_version = validate_compiler_bundle(&host, &host_compiler)
            .expect("host compiler is authenticated by the host receipt");
        assert_ne!(
            digest_file(&temporary.path().join("target/bin/hermesc")).unwrap(),
            digest_file(&host_compiler).unwrap(),
            "fixture compiler digests must differ"
        );
        let target_engine_digest = digest_file(&target.vm_archive).expect("target engine digest");
        validate_receipt(
            &target,
            &host_compiler,
            false,
            &target_engine_digest,
            &host_version,
        )
        .expect("matching HBC versions permit a cross-bundle pairing");

        let mut mismatched: serde_json::Value = serde_json::from_slice(
            &fs::read(target.root.join("hermes-input-receipt.json")).expect("target receipt"),
        )
        .expect("target receipt JSON");
        mismatched["bytecode"]["version"] = serde_json::Value::from(97);
        fs::write(
            target.root.join("hermes-input-receipt.json"),
            serde_json::to_vec_pretty(&mismatched).expect("mismatched receipt JSON"),
        )
        .expect("mismatched target receipt");
        let error = validate_receipt(
            &target,
            &host_compiler,
            false,
            &target_engine_digest,
            &host_version,
        )
        .expect_err("different HBC versions must fail");
        assert!(error.contains("HBC bytecode version"), "{error}");
    }

    #[test]
    fn a_missing_lean_archive_fails_only_when_lean_is_requested() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir_all(root.join("include")).expect("include directory");
        fs::create_dir_all(root.join("lib")).expect("lib directory");
        fs::write(root.join("lib/libhermesvm_a.a"), b"full").expect("full archive");
        let layout = install_layout(
            root.to_path_buf(),
            "aarch64-apple-darwin",
            InstallOrigin::Override,
        );

        validate_layout(&layout, false).expect("full selection accepts the old local layout");
        let error = validate_layout(&layout, true).expect_err("lean selection needs lean bytes");
        assert!(error.contains("lean Hermes VM archive"), "{error}");
        assert!(error.contains("containing the lean archive"), "{error}");
    }

    #[test]
    fn every_present_lean_archive_is_bound_by_the_receipt_archive_manifest() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir_all(root.join("include")).expect("include directory");
        fs::create_dir_all(root.join("lib")).expect("lib directory");
        fs::create_dir_all(root.join("bin")).expect("bin directory");
        fs::write(root.join("lib/libhermesvm_a.a"), b"full").expect("full archive");
        fs::write(root.join("lib/libhermesvmlean_a.a"), b"lean").expect("lean archive");
        fs::write(root.join("bin/hermesc"), b"compiler").expect("compiler");
        let full_digest = digest_file(&root.join("lib/libhermesvm_a.a")).expect("full digest");
        let lean_digest = digest_file(&root.join("lib/libhermesvmlean_a.a")).expect("lean digest");
        let compiler_digest = digest_file(&root.join("bin/hermesc")).expect("compiler digest");
        write_v2_receipt(
            root,
            "aarch64-apple-darwin",
            "lib/libhermesvm_a.a",
            &full_digest,
            &compiler_digest,
            96,
        );
        let layout = install_layout(
            root.to_path_buf(),
            "aarch64-apple-darwin",
            InstallOrigin::Bundle,
        );
        let error = validate_receipt(&layout, &root.join("bin/hermesc"), true, &full_digest, "96")
            .expect_err("a present lean archive must be bound even without link-lean");
        assert!(error.contains("does not bind"), "{error}");

        let receipt_path = root.join("hermes-input-receipt.json");
        let mut receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(&receipt_path).expect("receipt")).expect("JSON");
        receipt["archives"] = serde_json::json!([
            { "path": "lib/libhermesvm_a.a", "digest": full_digest },
            { "path": "lib/libhermesvmlean_a.a", "digest": lean_digest }
        ]);
        fs::write(
            &receipt_path,
            serde_json::to_vec_pretty(&receipt).expect("receipt JSON"),
        )
        .expect("receipt");
        validate_receipt(&layout, &root.join("bin/hermesc"), true, &full_digest, "96")
            .expect("manifest authenticates the selected lean archive");

        receipt["archives"][1]["digest"] =
            serde_json::Value::String(format!("sha256-{}", "0".repeat(64)));
        fs::write(
            &receipt_path,
            serde_json::to_vec_pretty(&receipt).expect("receipt JSON"),
        )
        .expect("receipt");
        let error = validate_receipt(&layout, &root.join("bin/hermesc"), true, &full_digest, "96")
            .expect_err("changed lean identity must fail");
        assert!(error.contains("LEAN_ENGINE_DIGEST"), "{error}");
    }
}
