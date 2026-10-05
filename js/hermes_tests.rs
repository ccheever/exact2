//! File/receipt admission tests, not evidence that these synthetic bytes link.
use super::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf, Value);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "exact lean # café {}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let mut names = ARCHIVES.to_vec();
        names.extend([
            "hermesc.exe",
            "hermes-headers/hermes/hermes.h",
            "hermes-headers/jsi/jsi.h",
            "icu-headers/unicode/utypes.h",
            "icu-headers/unicode/dtptngen.h",
            "icu-headers/unicode/timezone.h",
            "icu-headers/ICU-LICENSE",
            ICU_DATA,
        ]);
        let mut files = Vec::new();
        for name in names {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, name.as_bytes()).unwrap();
            let (bytes, sha256) = hash(&path).unwrap();
            files.push(json!({"path": name, "bytes": bytes, "sha256": sha256}));
        }
        let fixture = Self(
            root,
            json!({
                "schema":"exact/hermes-windows-lean/3", "sourceCommit":HERMES_PIN,
                "target":"x86_64-pc-windows-msvc", "role":"lean", "crt":"MD",
                "iteratorDebugLevel":0, "debugger":false, "jit":false, "intl":true,
                "systemLibraries":SYSTEM_LIBRARIES, "bytecodeVersion":99, "files":files,
                "icu": {
                    "version":"76.1", "sourceCommit":ICU_PIN, "sourceSha512":ICU_SOURCE_SHA512,
                    "linkage":"static", "dataPath":ICU_DATA,
                    "extras":false, "toolsEnabled":true, "dataEnabled":true,
                    "dataIdentity":files.iter().find(|entry| entry["path"] == ICU_DATA).unwrap(),
                    "tools":{"msys2Sha256":MSYS2_SHA256, "makeSha256":MAKE_SHA256},
                },
                "build":{"intlPatch":intl_patch().unwrap(), "intlProbe":{
                    "assertions":424,
                    "result":{"assertions":424},
                    "compiler":files.iter().find(|entry| entry["path"] == "hermesc.exe").unwrap(),
                    "source":{"sha256":intl_patch().unwrap()["probe"]["sourceSha256"]},
                    "runner":{"sha256":intl_patch().unwrap()["probe"]["runnerSha256"]},
                }, "sourcePatch":{
                    "path":"CMakeLists.txt", "beforeSha256":PATCH_BEFORE,
                    "afterSha256":PATCH_AFTER, "old":PATCH_OLD, "new":PATCH_NEW,
                    "sha256":format!("{:x}", Sha256::digest(format!("{PATCH_OLD}\n{PATCH_NEW}\n"))),
                }},
            }),
        );
        fixture.write(&fixture.1);
        fixture
    }
    fn write(&self, value: &Value) {
        std::fs::write(self.0.join("hermes-input-receipt.json"), value.to_string()).unwrap();
    }
    fn validate(&self) -> Result<Install, String> {
        validate(&self.0, "x86_64-pc-windows-msvc", None)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn complete_install_and_identical_compiler_alias_preserve_identity() {
    let f = Fixture::new();
    let first = f.validate().unwrap();
    let second = f.validate().unwrap();
    assert_eq!(first.receipt_sha256, second.receipt_sha256);
    assert_eq!(first.inputs, second.inputs);
    assert_eq!(first.archives.len(), 6);
    assert_eq!(first.bytecode_version, 99);
    let other = Fixture::new();
    let alias = other.0.join("compiler alias.exe");
    std::fs::copy(&first.compiler, &alias).unwrap();
    let selected = validate(&f.0, "x86_64-pc-windows-msvc", Some(&alias)).unwrap();
    assert_eq!(selected.receipt_sha256, first.receipt_sha256);
    assert!(selected.inputs.contains(&alias));
    std::fs::write(&alias, b"another compiler").unwrap();
    assert!(validate(&f.0, "x86_64-pc-windows-msvc", Some(&alias))
        .err()
        .unwrap()
        .contains("does not match"));
}

#[test]
fn altered_or_missing_archives_headers_and_compiler_refuse_without_cached_acceptance() {
    let f = Fixture::new();
    let accepted = f.validate().unwrap().receipt_sha256;
    for name in [
        ARCHIVES[0],
        ARCHIVES[1],
        ARCHIVES[2],
        ARCHIVES[3],
        ARCHIVES[4],
        ARCHIVES[5],
        "hermesc.exe",
        "hermes-headers/hermes/hermes.h",
        "icu-headers/unicode/dtptngen.h",
        ICU_DATA,
    ] {
        let path = f.0.join(name);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(
            f.validate().err().unwrap().contains("input mismatch"),
            "{name}"
        );
        std::fs::remove_file(&path).unwrap();
        assert!(f.validate().is_err(), "missing {name}");
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(f.validate().unwrap().receipt_sha256, accepted);
    }
    std::fs::write(f.0.join("hermes-headers/unrecorded.h"), b"new header").unwrap();
    assert!(f.validate().err().unwrap().contains("incomplete"));
}

#[test]
fn incompatible_policy_and_bad_inventory_are_named_refusals() {
    let f = Fixture::new();
    for (key, value) in [
        ("schema", json!("ibex/hermes-upstream-pinned-receipt/2")),
        ("schema", json!("exact/hermes-windows-lean/1")),
        ("schema", json!("exact/hermes-windows-lean/2")),
        (
            "sourceCommit",
            json!("d412d3bd851278712c20cca25d094e32641a0465"),
        ),
        ("target", json!("aarch64-pc-windows-msvc")),
        ("role", json!("full")),
        ("crt", json!("MT")),
        ("iteratorDebugLevel", json!(2)),
        ("bytecodeVersion", json!(98)),
        ("debugger", json!(true)),
        ("jit", json!(true)),
        ("intl", json!(false)),
        ("systemLibraries", json!([])),
    ] {
        let mut bad = f.1.clone();
        bad[key] = value;
        f.write(&bad);
        assert!(f.validate().is_err(), "accepted invalid {key}");
    }
    for name in [
        "../outside.lib",
        "windows-static/hermesvm_a.lib",
        "/absolute.lib",
    ] {
        let mut bad = f.1.clone();
        bad["files"][0]["path"] = json!(name);
        f.write(&bad);
        assert!(f.validate().err().unwrap().contains("unexpected"));
    }
    let mut duplicate = f.1.clone();
    duplicate["files"]
        .as_array_mut()
        .unwrap()
        .push(f.1["files"][0].clone());
    f.write(&duplicate);
    assert!(f.validate().err().unwrap().contains("repeated"));
    std::fs::write(f.0.join("hermes-input-receipt.json"), b"{invalid").unwrap();
    assert!(f
        .validate()
        .err()
        .unwrap()
        .contains("hermes-input-receipt.json"));
    f.write(&f.1);
    assert!(validate(&f.0, "x86_64-pc-windows-gnu", None)
        .err()
        .unwrap()
        .contains("does not support"));
}

#[test]
fn static_icu_policy_patch_and_unrecorded_payload_are_required() {
    let f = Fixture::new();
    for (key, value) in [
        ("version", json!("75.1")),
        ("sourceCommit", json!("other")),
        ("sourceSha512", json!("other")),
        ("linkage", json!("system")),
        ("extras", json!(true)),
        ("toolsEnabled", json!(false)),
        ("dataEnabled", json!(false)),
        ("dataPath", json!("ambient.dat")),
        ("tools", json!({})),
    ] {
        let mut receipt = f.1.clone();
        receipt["icu"][key] = value;
        f.write(&receipt);
        assert!(f.validate().err().unwrap().contains("ICU receipt"), "{key}");
    }
    for key in [
        "path",
        "beforeSha256",
        "afterSha256",
        "old",
        "new",
        "sha256",
    ] {
        let mut receipt = f.1.clone();
        receipt["build"]["sourcePatch"][key] = json!("other");
        f.write(&receipt);
        assert!(
            f.validate().err().unwrap().contains("patch identity"),
            "{key}"
        );
    }
    for key in ["intlPatch", "intlProbe"] {
        let mut receipt = f.1.clone();
        receipt["build"][key] = json!({});
        f.write(&receipt);
        assert!(
            f.validate().err().unwrap().contains("Intl semantic"),
            "{key}"
        );
    }
    let mut receipt = f.1.clone();
    receipt.as_object_mut().unwrap().remove("icu");
    f.write(&receipt);
    assert!(f.validate().is_err());
    let mut receipt = f.1.clone();
    receipt["icu"]["dataIdentity"]["sha256"] = json!("other");
    f.write(&receipt);
    assert!(f.validate().err().unwrap().contains("data identity"));
    f.write(&f.1);
    std::fs::write(f.0.join("icuuc.dll"), b"unrecorded dependency").unwrap();
    assert!(f.validate().err().unwrap().contains("unexpected"));
}

#[test]
fn a_valid_new_receipt_has_a_new_identity_for_producer_invalidation() {
    let f = Fixture::new();
    let accepted = f.validate().unwrap().receipt_sha256;
    let path = f.0.join(ARCHIVES[0]);
    std::fs::write(&path, b"new accepted archive").unwrap();
    assert!(f.validate().is_err());
    let mut receipt = f.1.clone();
    let (bytes, sha256) = hash(&path).unwrap();
    receipt["files"][0]["bytes"] = json!(bytes);
    receipt["files"][0]["sha256"] = json!(sha256);
    f.write(&receipt);
    assert_ne!(f.validate().unwrap().receipt_sha256, accepted);
}
