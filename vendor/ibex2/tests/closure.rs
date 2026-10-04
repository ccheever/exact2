//! The negative closure guarantees LLP 0067 §5 keeps from LLP 0058.000.001 §5
//! (tombstoned).
//!
//! A greenfield artifact's value is what it does **not** contain, and that is
//! exactly the kind of property that decays silently: nothing fails when a
//! forbidden dependency creeps back, it just stops being true. These are tests
//! rather than a script so they run with everything else.
//!
//! §5 closes with the reason: *"The clean closure is a security property;
//! disabling legacy code is not isolation evidence."*

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

const FORBIDDEN_LEGACY_PACKAGES: &[&str] = &[
    "ibex-runtime",
    "ibex_runtime",
    "capsec-semantics",
    "ibex-sfe-format",
    "ibex-sfe-catalog",
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_and_native_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("rs" | "cc" | "mm" | "h")
            ) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&crate_root().join("src"), &mut out);
    out
}

fn forbidden_resolved_dependencies(metadata: &serde_json::Value) -> Vec<String> {
    let packages = metadata["packages"]
        .as_array()
        .expect("package list in cargo metadata");
    let package_names: HashMap<&str, &str> = packages
        .iter()
        .map(|package| {
            (
                package["id"].as_str().expect("package id"),
                package["name"].as_str().expect("package name"),
            )
        })
        .collect();
    let root = packages
        .iter()
        .find(|package| package["name"] == "ibex2")
        .and_then(|package| package["id"].as_str())
        .expect("ibex2 package in cargo metadata");
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolved nodes in cargo metadata");
    let nodes_by_id: HashMap<&str, &serde_json::Value> = nodes
        .iter()
        .map(|node| (node["id"].as_str().expect("resolved node id"), node))
        .collect();
    assert!(nodes_by_id.contains_key(root), "ibex2 resolve node");

    let mut reachable = HashSet::new();
    let mut pending = vec![root];
    while let Some(package_id) = pending.pop() {
        if !reachable.insert(package_id) {
            continue;
        }
        let node = nodes_by_id
            .get(package_id)
            .unwrap_or_else(|| panic!("resolved node for package {package_id}"));
        for dependency in node["deps"].as_array().expect("resolved dependency list") {
            let is_product_edge = dependency["dep_kinds"]
                .as_array()
                .expect("resolved dependency kinds")
                .iter()
                .any(|kind| kind["kind"].is_null() || kind["kind"] == "build");
            if is_product_edge {
                pending.push(
                    dependency["pkg"]
                        .as_str()
                        .expect("resolved dependency package id"),
                );
            }
        }
    }

    let mut forbidden: Vec<String> = reachable
        .into_iter()
        .filter_map(|package_id| package_names.get(package_id).copied())
        .filter(|name| FORBIDDEN_LEGACY_PACKAGES.contains(name))
        .map(str::to_owned)
        .collect();
    forbidden.sort();
    forbidden.dedup();
    forbidden
}

/// §5.3 — the kernel may not depend on the legacy runtime or its authority
/// crates. Cargo metadata resolves package aliases, target tables and workspace
/// dependencies, so this checks the complete product graph rather than a
/// bypassable direct spelling in either Rust source or the manifest. Dev-only
/// edges are excluded because they do not enter the product/link closure.
#[test]
fn the_kernel_does_not_depend_on_the_legacy_runtime() {
    let manifest = crate_root().join("Cargo.toml");
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--all-features",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .expect("cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata JSON");
    let forbidden = forbidden_resolved_dependencies(&metadata);
    assert!(
        forbidden.is_empty(),
        "crates/ibex2's resolved normal/build dependency graph reaches forbidden legacy packages (§5.3): {forbidden:?}"
    );
}

#[test]
fn closure_walk_follows_renamed_transitive_and_build_dependencies() {
    let metadata = serde_json::json!({
        "packages": [
            { "id": "root", "name": "ibex2" },
            { "id": "bridge", "name": "workspace-bridge" },
            { "id": "legacy", "name": "ibex-runtime" }
        ],
        "resolve": {
            "nodes": [
                {
                    "id": "root",
                    "deps": [{
                        "name": "renamed_bridge",
                        "pkg": "bridge",
                        "dep_kinds": [{ "kind": null, "target": null }]
                    }]
                },
                {
                    "id": "bridge",
                    "deps": [{
                        "name": "innocent_alias",
                        "pkg": "legacy",
                        "dep_kinds": [{ "kind": "build", "target": null }]
                    }]
                },
                { "id": "legacy", "deps": [] }
            ]
        }
    });

    assert_eq!(
        forbidden_resolved_dependencies(&metadata),
        vec!["ibex-runtime"]
    );
}

#[test]
fn closure_walk_excludes_dev_only_dependencies() {
    let metadata = serde_json::json!({
        "packages": [
            { "id": "root", "name": "ibex2" },
            { "id": "legacy", "name": "ibex-runtime" }
        ],
        "resolve": {
            "nodes": [
                {
                    "id": "root",
                    "deps": [{
                        "name": "test_fixture",
                        "pkg": "legacy",
                        "dep_kinds": [{ "kind": "dev", "target": null }]
                    }]
                },
                { "id": "legacy", "deps": [] }
            ]
        }
    });

    assert!(forbidden_resolved_dependencies(&metadata).is_empty());
}

/// §5.4 — no patched-engine symbol or identity-reading helper appears in the
/// kernel's sources. These are the names the carried patch stack adds; reaching
/// for one is how the fork grows back.
#[test]
fn no_patched_engine_symbol_is_referenced_in_code() {
    let forbidden = [
        "ex_hermes_vm_",
        "currentPrincipalId",
        "exactCollectTypedPrincipalStack",
        "__exactSetCompartmentFor",
        "__exactDeepFreeze",
        "__exactNativeFreeze",
        "EXACT_HAVE_FRAME_ATTRIBUTION",
    ];
    let mut offenders = Vec::new();
    for path in rust_and_native_sources() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            // Comments may name these — the whole point of several of them is
            // to explain what is NOT used — so only code counts.
            if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("*") {
                continue;
            }
            for needle in forbidden {
                if line.contains(needle) {
                    offenders.push(format!("{}:{}: {needle}", path.display(), number + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "patched-engine reach (§5.4):\n  {}",
        offenders.join("\n  ")
    );
}

/// §5.2 — nothing below `patches/hermes/` is a build input.
#[test]
fn the_patch_series_is_not_a_build_input() {
    let build = std::fs::read_to_string(crate_root().join("build.rs")).expect("build.rs");
    assert!(
        !build.contains("patches/hermes"),
        "build.rs reads from patches/hermes (§5.2)"
    );
    // And the engine it points at is the vanilla tree, never the reviewed one.
    assert!(
        build.contains("Frameworks-vanilla"),
        "build.rs must select the vanilla engine"
    );
    assert!(
        !build.contains("\"ios/Frameworks\"") && !build.contains("join(\"ios/Frameworks\")"),
        "build.rs selects the patched engine"
    );
}

/// §5.5, as far as a test can reach it: the PRODUCT artifact carries no patched
/// export. The real check starts from the linker's object closure and belongs
/// to a post-link scanner that does not exist yet.
///
/// The `ibex2` binary, not this test binary: an integration test never calls
/// the engine, so the linker strips Hermes out of it entirely and the scan
/// would pass vacuously. The vacuity guard below is what caught that.
#[cfg(feature = "hermes")]
#[test]
fn the_product_artifact_carries_no_patched_export() {
    // current_exe is .../target/<profile>/deps/closure-<hash>
    let exe = std::env::current_exe().expect("current exe");
    let profile_dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("target/<profile>");
    let product = profile_dir.join("ibex2");
    if !product.exists() {
        // Built by `cargo build --features hermes --bin ibex2`; a test run that
        // has not built it has nothing to scan, and saying so beats passing.
        eprintln!("skipping: no {} to scan", product.display());
        return;
    }

    // The full symbol table, not `nm -gU`: a statically linked binary keeps
    // Hermes's symbols local, so the exported-only view is empty.
    let output = std::process::Command::new("nm")
        .arg(&product)
        .output()
        .expect("nm");
    let symbols = String::from_utf8_lossy(&output.stdout);

    let found: Vec<&str> = [
        "ex_hermes_vm_current_package_id",
        "ex_hermes_vm_collect_package_ids",
        "ex_hermes_vm_disable_eval",
        "ex_hermes_vm_set_pending_package_id",
        "ex_hermes_vm_set_job_scheduler_capture",
    ]
    .into_iter()
    .filter(|symbol| symbols.contains(symbol))
    .collect();

    assert!(
        found.is_empty(),
        "the product artifact exports patched-engine symbols (§5.5): {found:?}"
    );
    // A check that cannot fail is not a check: if the binary carries no Hermes
    // at all, the assertion above proves nothing.
    assert!(
        symbols.to_lowercase().contains("hermes"),
        "no Hermes symbols in {}; this check would pass vacuously",
        product.display()
    );
}
