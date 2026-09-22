//! WPT `url/resources/urltestdata.json`, run against our `URL`.
//!
//! Data-driven and browser-free, which is why this is the first WPT suite
//! adopted. Reports a breakdown rather than a bare pass/fail so the divergence
//! list stays visible.

use std::path::PathBuf;

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third_party/wpt/urltestdata.json")
}

#[test]
#[ignore]
fn wpt_url_report() {
    let raw = std::fs::read_to_string(data_path()).expect("vendored urltestdata.json");
    let entries: Vec<serde_json::Value> = serde_json::from_str(&raw).expect("valid json");

    let (mut pass, mut fail) = (0usize, 0usize);
    let mut failures: Vec<String> = Vec::new();

    for entry in &entries {
        let Some(case) = entry.as_object() else {
            continue;
        };
        let Some(input) = case.get("input").and_then(|v| v.as_str()) else {
            continue;
        };
        let base = case.get("base").and_then(|v| v.as_str());
        let expects_failure = case
            .get("failure")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let got = ibex2::stdlib::url::parse(input, base);

        if expects_failure {
            match got {
                Err(_) => pass += 1,
                Ok(url) => {
                    fail += 1;
                    failures.push(format!(
                        "should have failed: {input:?} base={base:?} -> {:?}",
                        url.href
                    ));
                }
            }
            continue;
        }

        let want_href = case.get("href").and_then(|v| v.as_str()).unwrap_or("");
        match got {
            Ok(url) if url.href == want_href => pass += 1,
            Ok(url) => {
                fail += 1;
                failures.push(format!(
                    "href: {input:?} base={base:?} -> {:?} want {:?}",
                    url.href, want_href
                ));
            }
            Err(e) => {
                fail += 1;
                failures.push(format!(
                    "error: {input:?} base={base:?} -> {e} want {want_href:?}"
                ));
            }
        }
    }

    let total = pass + fail;
    let mut by_category = std::collections::BTreeMap::new();
    for line in &failures {
        *by_category.entry(categorize(line)).or_insert(0usize) += 1;
    }

    println!("\n=== WPT url/urltestdata.json ===");
    println!(
        "  {pass}/{total} pass ({:.1}%), {fail} divergences",
        100.0 * pass as f64 / total as f64
    );
    for (category, count) in &by_category {
        println!("    {category:24} {count}");
    }
    println!();
    for line in failures.iter().filter(|l| categorize(l) == "other") {
        println!("    other: {line}");
    }
}

/// Group divergences so the list stays a decision rather than a wall of text.
fn categorize(line: &str) -> &'static str {
    if line.contains("file:") {
        "file: scheme"
    } else if line.contains("xn--") || line.contains("international domain") {
        "idna / punycode"
    } else if line.contains("non-special") {
        "non-special scheme"
    } else {
        "other"
    }
}

/// The gate. Pass count may not fall; raising it is deliberate work.
///
/// Baselined rather than required-100%, on the LLP 0027 pattern: a check that
/// is red the day it lands gets switched off within a week.
#[test]
fn wpt_url_baseline_holds() {
    let raw = std::fs::read_to_string(data_path()).expect("vendored urltestdata.json");
    let entries: Vec<serde_json::Value> = serde_json::from_str(&raw).expect("valid json");
    let mut pass = 0usize;
    let mut total = 0usize;

    for entry in &entries {
        let Some(case) = entry.as_object() else {
            continue;
        };
        let Some(input) = case.get("input").and_then(|v| v.as_str()) else {
            continue;
        };
        let base = case.get("base").and_then(|v| v.as_str());
        let expects_failure = case
            .get("failure")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        total += 1;
        let got = ibex2::stdlib::url::parse(input, base);
        let ok = if expects_failure {
            got.is_err()
        } else {
            let want = case.get("href").and_then(|v| v.as_str()).unwrap_or("");
            matches!(got, Ok(url) if url.href == want)
        };
        if ok {
            pass += 1;
        }
    }

    const BASELINE: usize = 828;
    assert_eq!(
        total, 893,
        "the vendored suite changed size; re-baseline deliberately"
    );
    assert!(
        pass >= BASELINE,
        "WPT URL pass count fell to {pass}, below the {BASELINE} baseline"
    );
}
