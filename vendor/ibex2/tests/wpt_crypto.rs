//! Unmodified WPT WebCrypto randomness tests, pinned in the existing manifest.
#![cfg(feature = "hermes")]

use ibex2::engine::hermes::{DynamicCode, Hermes};

#[test]
fn webcrypto_randomness_wpt() {
    let mut failures = Vec::new();
    let mut total = 0;
    let mut unavailable = 0;
    for file in ["getRandomValues.any.js", "randomUUID.https.any.js"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../third_party/wpt/WebCryptoAPI")
            .join(file);
        let source = std::fs::read_to_string(path).unwrap();
        let mut rt = Hermes::new(DynamicCode::Closed).unwrap();
        assert!(rt.install_stdlib());
        rt.install_bindings().unwrap();
        rt.harden().unwrap();
        rt.install_test_harness().unwrap();
        // WPT's worker-global alias, test-only (no worker is being installed).
        rt.eval("globalThis.self = globalThis").unwrap();
        let float16 = rt.eval("typeof Float16Array === 'function'").unwrap() == "true";
        rt.eval(&source).unwrap();
        let results: Vec<serde_json::Value> =
            serde_json::from_str(&rt.eval("__ibex2_test_results()").unwrap()).unwrap();
        for result in results {
            total += 1;
            if result["ok"] != true {
                // Stock Hermes has no Float16Array. Run and report that case
                // honestly as an engine gap; do not alias it to another type
                // or silently remove it from the upstream fixture. As soon
                // as the engine supplies it this case must pass too.
                if !float16 && result["name"] == "Float16 arrays" {
                    assert!(result["message"]
                        .as_str()
                        .unwrap()
                        .contains("Float16Array' doesn't exist"));
                    unavailable += 1;
                    continue;
                }
                failures.push(format!("{file}: {}: {}", result["name"], result["message"]));
            }
        }
    }
    assert_eq!(total, 42, "the pinned upstream test set changed");
    println!(
        "{} passed; {unavailable} unavailable (Hermes Float16Array); {} failed",
        total - unavailable - failures.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
