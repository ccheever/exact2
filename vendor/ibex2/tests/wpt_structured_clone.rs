//! Unmodified WPT structured-clone battery, pinned in `third_party/wpt`.
#![cfg(feature = "hermes")]

use ibex2::engine::hermes::{DynamicCode, Hermes};

#[derive(Debug)]
struct Report {
    total_upstream: usize,
    selected: usize,
    passed: usize,
    failures: Vec<String>,
    exclusions: std::collections::BTreeMap<String, usize>,
}

fn run_wpt() -> Report {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/wpt/html/webappapis/structured-clone");
    let battery = std::fs::read_to_string(root.join("structured-clone-battery-of-tests.js"))
        .expect("pinned battery");
    let harness =
        std::fs::read_to_string(root.join("structured-clone-battery-of-tests-harness.js"))
            .expect("pinned battery harness");
    let entry =
        std::fs::read_to_string(root.join("structured-clone.any.js")).expect("pinned entry");

    let mut runtime = Hermes::new(DynamicCode::Closed).expect("runtime");
    let context = ibex2::bindings::Context::new(ibex2::grant::GrantSet::none());
    runtime
        .install_runtime(ibex2::bindings::Groups::DEFAULT, &context)
        .expect("bindings");
    runtime.harden().expect("harden");
    runtime.install_test_harness().expect("test harness");
    runtime
        .eval(
            r#"
            globalThis.self = globalThis;
            // The upstream battery eagerly constructs Blob/File values while
            // registering tests. Ibex deliberately does not expose those APIs,
            // so inert test-only shells let the unmodified file register; every
            // test involving them is excluded before the battery runs.
            globalThis.Blob = function Blob() {};
            globalThis.File = function File() { Blob.call(this); };
            File.prototype = Object.create(Blob.prototype);
            File.prototype.constructor = File;
            globalThis.Response = function Response() {};
            "#,
        )
        .expect("test-only unavailable API shells");
    runtime
        .eval(&battery)
        .expect("unmodified battery evaluates");

    let catalogue: Vec<serde_json::Value> = serde_json::from_str(
        &runtime
            .eval(
                r#"JSON.stringify(structuredCloneBatteryOfTests.map(function (test) {
                    return { description: test.description, requiresDocument: !!test.requiresDocument };
                }))"#,
            )
            .expect("catalogue"),
    )
    .expect("catalogue json");

    fn exclusion(description: &str, requires_document: bool) -> Option<&'static str> {
        if requires_document {
            return Some("document-only FileList/ImageData/ImageBitmap");
        }
        if description.contains("Blob") || description.contains("File") {
            return Some("Blob/File are outside v1");
        }
        if description.contains("Resizable")
            || description.contains("Growable")
            || description.contains("Length-tracking")
            || description.contains("OOB")
        {
            return Some("Hermes lacks resizable/growable buffers");
        }
        if description == "Serializing a non-serializable platform object fails" {
            return Some("WPT requires a browser Response constructor");
        }
        if description.contains("interface is deleted from the global")
            || description.contains("closest serializable superclass")
        {
            return Some("requires Blob/File platform serialization");
        }
        None
    }

    let mut exclusions = std::collections::BTreeMap::new();
    let mut selected = Vec::new();
    for test in &catalogue {
        let description = test["description"].as_str().unwrap_or("");
        if let Some(reason) = exclusion(
            description,
            test["requiresDocument"].as_bool().unwrap_or(false),
        ) {
            *exclusions.entry(reason.to_string()).or_insert(0) += 1;
        } else {
            selected.push(description.to_string());
        }
    }
    exclusions.insert("transferables unsupported in v1".to_string(), 14);
    runtime
        .eval(&format!(
            "const __ibex2_wpt_selected = new Set({}); structuredCloneBatteryOfTests = structuredCloneBatteryOfTests.filter(function (test) {{ return __ibex2_wpt_selected.has(test.description); }});",
            serde_json::to_string(&selected).unwrap()
        ))
        .expect("filter unsupported WPT cases");
    runtime
        .eval(&harness)
        .expect("unmodified WPT harness evaluates");
    runtime
        .eval(&entry)
        .expect("unmodified WPT entry evaluates");
    // Every selected clone is synchronous, but the upstream runner wraps it in
    // promises and async functions. Drain that finite chain explicitly.
    runtime.drain_microtasks().expect("WPT microtasks");

    let results: Vec<serde_json::Value> =
        serde_json::from_str(&runtime.eval("__ibex2_test_results()").expect("WPT results"))
            .expect("WPT results json");
    let mut failures = Vec::new();
    for result in &results {
        if result["ok"] != true {
            failures.push(format!(
                "{}: {}",
                result["name"].as_str().unwrap_or("<unnamed>"),
                result["message"].as_str().unwrap_or("<no message>")
            ));
        }
    }
    Report {
        total_upstream: catalogue.len() + 14, // the separately pinned transfer battery
        selected: selected.len(),
        passed: results.len() - failures.len(),
        failures,
        exclusions,
    }
}

#[test]
fn structured_clone_wpt_baseline_holds() {
    let report = run_wpt();
    println!("{report:#?}");
    assert_eq!(
        report.total_upstream, 152,
        "the pinned upstream set changed"
    );
    assert_eq!(report.exclusions.values().sum::<usize>(), 58);
    assert_eq!(report.selected, report.passed + report.failures.len());
    assert!(
        report.failures.is_empty(),
        "WPT structured-clone regressions:\n  {}",
        report.failures.join("\n  ")
    );
}
