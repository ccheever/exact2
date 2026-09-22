//! WPT `fetch/api/headers/*`, run unmodified against our `Headers`.
//!
//! These need no server and no browser: they exercise the class in isolation,
//! which is why they are the first `fetch/` suite reachable. They are gated by
//! the binding layer rather than the transport.

#![cfg(feature = "hermes")]

use ibex2::engine::hermes::{DynamicCode, Hermes};

fn run_file(name: &str) -> Vec<(String, bool, String)> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/wpt/fetch/api/headers")
        .join(name);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("vendored {}: {e}", path.display()));

    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_stdlib(), "stdlib");
    rt.install_bindings().expect("bindings");
    rt.install_test_harness().expect("test harness");
    rt.eval("__ibex2_reset_results()").expect("reset");

    // The file itself, unmodified. A syntax or top-level error is a failure of
    // the binding, not of the test.
    if let Err(e) = rt.eval(&source) {
        panic!("{name} failed to evaluate: {e}");
    }

    let raw = rt.eval("__ibex2_test_results()").expect("results");
    serde_json::from_str::<Vec<serde_json::Value>>(&raw)
        .expect("results json")
        .into_iter()
        .map(|v| {
            (
                v["name"].as_str().unwrap_or("").to_string(),
                v["ok"].as_bool().unwrap_or(false),
                v["message"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

#[test]
#[ignore]
fn wpt_headers_report() {
    let files = [
        "headers-basic.any.js",
        "headers-casing.any.js",
        "headers-normalize.any.js",
        "headers-errors.any.js",
    ];
    let (mut total, mut passed) = (0usize, 0usize);
    println!("\n=== WPT fetch/api/headers ===");
    for file in files {
        let results = run_file(file);
        let ok = results.iter().filter(|(_, ok, _)| *ok).count();
        total += results.len();
        passed += ok;
        println!("  {file:32} {ok}/{}", results.len());
        for (name, is_ok, message) in &results {
            if !is_ok {
                println!("      FAIL {name}: {message}");
            }
        }
    }
    println!("\n  {passed}/{total} pass");
}

/// The gate. 48/48 today; it may not fall.
///
/// Unlike the URL suite this one is required-complete, because it passes
/// completely: a baseline below 100% here would be recording a divergence that
/// does not exist.
#[test]
fn wpt_headers_all_pass() {
    let files = [
        "headers-basic.any.js",
        "headers-casing.any.js",
        "headers-normalize.any.js",
        "headers-errors.any.js",
    ];
    let mut failures = Vec::new();
    let mut total = 0usize;
    for file in files {
        for (name, ok, message) in run_file(file) {
            total += 1;
            if !ok {
                failures.push(format!("{file}: {name}: {message}"));
            }
        }
    }
    assert_eq!(
        total, 48,
        "the vendored suite changed size; re-baseline deliberately"
    );
    assert!(
        failures.is_empty(),
        "WPT headers regressions:\n  {}",
        failures.join("\n  ")
    );
}

/// The vendored files are what MANIFEST.json says they are.
///
/// Without this, "frozen subset" is a claim rather than a fact — an edited
/// fixture would quietly make the suite pass by changing what it asks.
#[test]
fn vendored_wpt_matches_its_manifest() {
    use std::io::Read;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("third_party/wpt/MANIFEST.json")).unwrap(),
    )
    .unwrap();
    for entry in manifest["files"].as_array().unwrap() {
        let local = root.join(entry["local"].as_str().unwrap());
        let mut bytes = Vec::new();
        std::fs::File::open(&local)
            .unwrap_or_else(|e| panic!("{}: {e}", local.display()))
            .read_to_end(&mut bytes)
            .unwrap();
        let digest = <sha2::Sha256 as sha2::Digest>::digest(&bytes);
        assert_eq!(
            format!("{digest:x}"),
            entry["sha256"].as_str().unwrap(),
            "{} does not match its recorded digest",
            entry["local"].as_str().unwrap()
        );
    }
}
