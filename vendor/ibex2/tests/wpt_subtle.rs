//! Unmodified WPT WebCrypto fixtures for the L2 crypto surface.
#![cfg(all(feature = "hermes", feature = "crypto-asymmetric"))]

use ibex2::engine::hermes::{DynamicCode, Hermes};
use std::collections::BTreeMap;

struct Suite {
    name: &'static str,
    scripts: &'static [&'static str],
}

const SUITES: &[Suite] = &[
    Suite {
        name: "digest",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "WebCryptoAPI/digest/digest_test_data.js",
            "WebCryptoAPI/digest/digest.js",
            "WebCryptoAPI/digest/digest.https.any.js",
        ],
    },
    Suite {
        name: "hmac",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "WebCryptoAPI/sign_verify/hmac_vectors.js",
            "WebCryptoAPI/sign_verify/mac.js",
            "WebCryptoAPI/sign_verify/hmac.js",
            "WebCryptoAPI/sign_verify/hmac.https.any.js",
        ],
    },
    Suite {
        name: "aes-gcm",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "WebCryptoAPI/encrypt_decrypt/aes_common_fixtures.js",
            "WebCryptoAPI/encrypt_decrypt/aes_gcm_96_iv_fixtures.js",
            "WebCryptoAPI/encrypt_decrypt/aes_gcm_vectors.js",
            "WebCryptoAPI/encrypt_decrypt/aes.js",
            "WebCryptoAPI/encrypt_decrypt/aes_gcm.https.any.js",
        ],
    },
    Suite {
        name: "hkdf",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "common/subset-tests.js",
            "WebCryptoAPI/derive_bits_keys/hkdf_vectors.js",
            "WebCryptoAPI/derive_bits_keys/kdf.js",
            "WebCryptoAPI/derive_bits_keys/hkdf.js",
            "WebCryptoAPI/derive_bits_keys/hkdf.https.any.js",
        ],
    },
    Suite {
        name: "pbkdf2",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "common/subset-tests.js",
            "WebCryptoAPI/derive_bits_keys/pbkdf2_vectors.js",
            "WebCryptoAPI/derive_bits_keys/kdf.js",
            "WebCryptoAPI/derive_bits_keys/pbkdf2.js",
            "WebCryptoAPI/derive_bits_keys/pbkdf2.https.any.js",
        ],
    },
    Suite {
        name: "import-export",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "WebCryptoAPI/import_export/symmetric_importKey.js",
            "WebCryptoAPI/import_export/symmetric_importKey.https.any.js",
        ],
    },
    Suite {
        name: "generate-hmac",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "common/subset-tests.js",
            "WebCryptoAPI/generateKey/algorithm_registry.js",
            "WebCryptoAPI/generateKey/successes.js",
            "WebCryptoAPI/generateKey/successes_HMAC.https.any.js",
        ],
    },
    Suite {
        name: "generate-aes-gcm",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "common/subset-tests.js",
            "WebCryptoAPI/generateKey/algorithm_registry.js",
            "WebCryptoAPI/generateKey/successes.js",
            "WebCryptoAPI/generateKey/successes_AES-GCM.https.any.js",
        ],
    },
    Suite {
        name: "ecdsa",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "WebCryptoAPI/sign_verify/ecdsa_vectors.js",
            "WebCryptoAPI/sign_verify/signature.js",
            "WebCryptoAPI/sign_verify/ecdsa.js",
            "WebCryptoAPI/sign_verify/ecdsa.https.any.js",
        ],
    },
    Suite {
        name: "ed25519",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "WebCryptoAPI/sign_verify/eddsa_vectors.js",
            "WebCryptoAPI/sign_verify/signature.js",
            "WebCryptoAPI/sign_verify/eddsa.js",
            "WebCryptoAPI/sign_verify/eddsa_curve25519.https.any.js",
        ],
    },
    Suite {
        name: "ec-import-export",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "WebCryptoAPI/util/ec_key_fixtures.js",
            "WebCryptoAPI/import_export/ec_importKey.https.any.js",
        ],
    },
    Suite {
        name: "ed25519-import-export",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "WebCryptoAPI/util/okp_key_fixtures.js",
            "WebCryptoAPI/import_export/okp_importKey_fixtures.js",
            "WebCryptoAPI/import_export/okp_importKey.js",
            "WebCryptoAPI/import_export/okp_importKey_Ed25519.https.any.js",
        ],
    },
    Suite {
        name: "generate-ecdsa",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "common/subset-tests.js",
            "WebCryptoAPI/generateKey/algorithm_registry.js",
            "WebCryptoAPI/generateKey/successes.js",
            "WebCryptoAPI/generateKey/successes_ECDSA.https.any.js",
        ],
    },
    Suite {
        name: "generate-ed25519",
        scripts: &[
            "WebCryptoAPI/util/helpers.js",
            "common/subset-tests.js",
            "WebCryptoAPI/generateKey/algorithm_registry.js",
            "WebCryptoAPI/generateKey/successes.js",
            "WebCryptoAPI/generateKey/successes_Ed25519.https.any.js",
        ],
    },
];

fn run_suite(suite: &Suite) -> Vec<serde_json::Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/wpt");
    let mut runtime = Hermes::new(DynamicCode::Closed).unwrap();
    let context = ibex2::bindings::Context::new(ibex2::grant::GrantSet::none());
    runtime
        .install_runtime(ibex2::bindings::Groups::DEFAULT, &context)
        .unwrap();
    runtime.harden().unwrap();
    runtime.install_test_harness().unwrap();
    runtime
        .eval("globalThis.self = globalThis; globalThis.location = { search: '' };")
        .unwrap();
    for script in suite.scripts {
        let source = std::fs::read_to_string(root.join(script)).unwrap();
        runtime
            .eval(&source)
            .unwrap_or_else(|error| panic!("{}: evaluating {script}: {error}", suite.name));
    }
    runtime.drain_microtasks().unwrap();
    serde_json::from_str(&runtime.eval("__ibex2_test_results()").unwrap()).unwrap()
}

fn excluded(suite: &str, name: &str) -> Option<&'static str> {
    let upper = name.to_ascii_uppercase();
    if name.contains("transferred ") {
        return Some("Hermes does not implement ArrayBuffer.prototype.transfer");
    }
    if upper.contains("SHA-1") {
        return Some("SHA-1 is explicitly outside L2");
    }
    match suite {
        "aes-gcm" if name.contains("192-bit key") => Some("ring does not expose AES-GCM-192"),
        "aes-gcm"
            if [
                "32-bit tag",
                "64-bit tag",
                "96-bit tag",
                "104-bit tag",
                "112-bit tag",
                "120-bit tag",
            ]
            .iter()
            .any(|tag| name.contains(tag)) =>
        {
            Some("ring exposes only the 128-bit AES-GCM tag")
        }
        "aes-gcm" if name.contains("with mismatched key and algorithm") => {
            Some("the fixture's mismatch key uses out-of-scope AES-CBC")
        }
        "hkdf" | "pbkdf2" if name.contains("with wrong (ECDH) key") => {
            Some("the fixture's wrong-key setup requires out-of-scope ECDH")
        }
        "hkdf" | "pbkdf2"
            if name.contains("Derived key of type name: AES-CBC")
                || name.contains("Derived key of type name: AES-CTR")
                || name.contains("Derived key of type name: AES-KW") =>
        {
            Some("the requested derived-key algorithm is outside L2")
        }
        "hkdf" | "pbkdf2" if name.contains("Derived key of type name: AES-GCM length: 192") => {
            Some("ring does not expose AES-GCM-192")
        }
        "import-export"
            if upper.contains("{NAME: AES-CTR")
                || upper.contains("{NAME: AES-CBC")
                || upper.contains("{NAME: AES-KW") =>
        {
            Some("the requested AES algorithm is outside L2")
        }
        "import-export" if name.contains("192 bits") && upper.contains("AES-GCM") => {
            Some("ring does not expose AES-GCM-192")
        }
        "generate-aes-gcm" if upper.contains("LENGTH: 192") => {
            Some("ring does not expose AES-GCM-192")
        }
        "ecdsa" if upper.contains("P-384") || upper.contains("P-521") => {
            Some("P-384 and P-521 are explicitly outside L2")
        }
        "ecdsa"
            if upper.contains("P-256")
                && (upper.contains("SHA-384") || upper.contains("SHA-512")) =>
        {
            Some("unmodified ring exposes fixed-length P-256 only with SHA-256")
        }
        "ecdsa" if name.contains("wrong algorithm name") => {
            Some("the fixture's wrong-key setup requires out-of-scope SHA-1")
        }
        "ecdsa" if name.contains("verification failure due to wrong hash") => {
            Some("the fixture's alternate hash is out-of-scope SHA-1")
        }
        "ed25519" if name.contains("wrong algorithm name") => {
            Some("the fixture's wrong-key setup requires out-of-scope SHA-1")
        }
        "ec-import-export" if upper.contains("{NAME: ECDH") => {
            Some("ECDH is explicitly outside L2")
        }
        "ec-import-export"
            if upper.contains("P-384")
                || upper.contains("P-521")
                || upper.contains("384 BITS")
                || upper.contains("521 BITS") =>
        {
            Some("P-384 and P-521 are explicitly outside L2")
        }
        "ec-import-export" if name.contains("compressed") => {
            Some("ring does not decompress SEC1 compressed points")
        }
        "ec-import-export" if name.contains("PKCS8 private-only: P-256") => {
            Some("ring's public API cannot derive a missing P-256 public point")
        }
        "generate-ecdsa" if upper.contains("P-384") || upper.contains("P-521") => {
            Some("P-384 and P-521 are explicitly outside L2")
        }
        _ => None,
    }
}

#[test]
fn l2_plan_uses_the_final_crypto_snapshot() {
    let plan = include_str!("../../../llp/0057.000-wintertc-and-the-platform-families.plan.md");
    for stale in [
        "14,137 subtests",
        "6,025 pass",
        "8,112 are explicit",
        "4,301,792 bytes",
        "4,386,416 bytes",
        "84,624-byte",
    ] {
        assert!(!plan.contains(stale), "stale L2 snapshot remains: {stale}");
    }
    for final_value in [
        "14,933 WPT subtests",
        "6,239 passes",
        "8,694 named",
        "4,399,824 bytes",
        "4,504,344 with `crypto`",
        "4,773,448 with",
        "104,520 bytes",
        "269,104 bytes",
    ] {
        assert!(
            plan.contains(final_value),
            "final L2 snapshot is missing: {final_value}"
        );
    }
}

#[test]
fn webcrypto_wpt() {
    let mut failures = Vec::new();
    let mut exclusions = Vec::new();
    let mut exclusion_reasons = BTreeMap::new();
    let mut total = 0;
    for suite in SUITES {
        let results = run_suite(suite);
        let suite_total = results.len();
        let mut suite_excluded = 0;
        let mut suite_failed = 0;
        total += results.len();
        for result in results {
            if result["ok"] != true {
                let name = result["name"].as_str().unwrap();
                if let Some(reason) = excluded(suite.name, name) {
                    suite_excluded += 1;
                    *exclusion_reasons.entry(reason).or_insert(0) += 1;
                    exclusions.push(format!("{}: {name} — {reason}", suite.name));
                } else {
                    suite_failed += 1;
                    failures.push(format!(
                        "{}: {}: {}",
                        suite.name, result["name"], result["message"]
                    ));
                }
            }
        }
        println!(
            "{}: {} passed; {suite_excluded} excluded; {suite_failed} failed",
            suite.name,
            suite_total - suite_excluded - suite_failed
        );
    }
    println!(
        "{total} total; {} passed; {} excluded; {} failed",
        total - exclusions.len() - failures.len(),
        exclusions.len(),
        failures.len()
    );
    for (reason, count) in exclusion_reasons {
        println!("excluded {count}: {reason}");
    }
    assert_eq!(total, 14_933, "the pinned upstream test set changed");
    assert_eq!(
        exclusions.len(),
        8_694,
        "an exclusion changed; audit it before updating the count"
    );
    assert!(
        failures.is_empty(),
        "{}",
        failures
            .into_iter()
            .take(250)
            .collect::<Vec<_>>()
            .join("\n")
    );
}
