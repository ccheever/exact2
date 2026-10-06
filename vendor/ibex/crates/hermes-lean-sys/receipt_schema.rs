use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SCHEMA: &str = "ibex/hermes-upstream-pinned-receipt/2";
pub(crate) const SOURCE_COMMIT: &str = "d412d3bd851278712c20cca25d094e32641a0465";
pub(crate) const ICU_SOURCE_COMMIT: &str = "2d029329c82c7792b985024b2bdab5fc7278fbc8";
pub(crate) const ICU_TRIMMED_FILTER_DIGEST: &str =
    "sha256-c5d1b182d6e92212ff4952d7a5c956f3d54611f300cb6fa1fdca39a6510f9702";
pub(crate) const EMPTY_PATCH_SET: &str =
    "sha256-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Translate Rust target aliases to the exact target identifier carried by
/// the selected published bundle. Both simulator architectures consume the
/// same universal release artifact and therefore the same receipt identity.
pub(crate) fn bundle_target_for_rust_target(target: &str) -> &str {
    match target {
        "aarch64-apple-ios-sim" | "x86_64-apple-ios" => "universal-apple-ios-simulator",
        target => target,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CanonicalReceipt {
    pub(crate) target: String,
    pub(crate) engine_binary: String,
    pub(crate) engine_digest: String,
    pub(crate) compiler_digest: String,
    pub(crate) bytecode_version: u64,
    #[allow(dead_code)] // The runtime's shared parser consumes only engine claims.
    pub(crate) icu: Option<CanonicalIcuReceipt>,
    archive_digests: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct CanonicalIcuReceipt {
    pub(crate) code_archives: Vec<String>,
    pub(crate) trimmed_data_archive: String,
    pub(crate) full_data_archive: String,
    pub(crate) trimmed_filter_path: String,
    pub(crate) trimmed_filter_digest: String,
}

impl CanonicalReceipt {
    pub(crate) fn archive_digests(&self) -> &BTreeMap<String, String> {
        &self.archive_digests
    }
}

pub(crate) fn validate(
    document: &Value,
    expected_target: Option<&str>,
) -> Result<CanonicalReceipt, String> {
    let root = document
        .as_object()
        .ok_or("receipt root is not an object")?;
    if string(root.get("schema"), "receipt has no schema")? != SCHEMA {
        return Err(format!("receipt is not canonical schema {SCHEMA}"));
    }
    if root.contains_key("producedOn") {
        return Err("v2 receipt contains volatile producedOn".into());
    }

    let upstream = object(root.get("upstream"), "v2 receipt has no upstream object")?;
    let source_commit = string(
        upstream.get("sourceCommit"),
        "v2 receipt has no upstream sourceCommit",
    )?;
    if source_commit != SOURCE_COMMIT {
        return Err(format!(
            "v2 receipt sourceCommit {source_commit} is not pinned Hermes commit {SOURCE_COMMIT}"
        ));
    }

    let patch_set = object(root.get("patchSet"), "v2 receipt has no patchSet object")?;
    if string(
        patch_set.get("digest"),
        "v2 receipt has no patch-set digest",
    )? != EMPTY_PATCH_SET
    {
        return Err("v2 receipt patch-set digest is not the canonical empty set".into());
    }
    let applied = patch_set
        .get("applied")
        .and_then(Value::as_array)
        .ok_or("v2 receipt patchSet.applied is not an array")?;
    if !applied.is_empty() {
        return Err("v2 receipt patchSet.applied is not empty".into());
    }

    let target = string(root.get("target"), "v2 receipt has no target")?;
    if let Some(expected) = expected_target {
        if target != expected {
            return Err(format!(
                "v2 receipt target {target} does not match selected target {expected}"
            ));
        }
    }
    string(root.get("profile"), "v2 receipt has no profile")?;
    let build = object(root.get("build"), "v2 receipt has no build object")?;
    let flags = build
        .get("flags")
        .and_then(Value::as_array)
        .ok_or("v2 receipt build.flags is not an array")?;
    if flags
        .iter()
        .any(|value| value.as_str().filter(|flag| !flag.is_empty()).is_none())
    {
        return Err("v2 receipt build.flags contains an empty or non-string value".into());
    }

    let bytecode = object(root.get("bytecode"), "v2 receipt has no bytecode object")?;
    let bytecode_version = bytecode
        .get("version")
        .and_then(Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or("v2 receipt has no positive HBC bytecode version")?;

    let compiler = object(root.get("compiler"), "v2 receipt has no compiler object")?;
    if let Some(binary) = compiler.get("binary") {
        validate_path(&string(
            Some(binary),
            "v2 receipt compiler has no binary path",
        )?)?;
    }
    let compiler_digest = digest(compiler.get("digest"), "v2 receipt has no compiler digest")?;

    let engine = object(root.get("engine"), "v2 receipt has no engine object")?;
    let engine_binary = string(engine.get("binary"), "v2 receipt has no engine binary path")?;
    validate_path(&engine_binary)?;
    let expected_engine_name = if target.ends_with("-pc-windows-msvc") {
        "hermesvm_a.lib"
    } else {
        "libhermesvm_a.a"
    };
    if engine_binary.rsplit('/').next() != Some(expected_engine_name) {
        return Err(format!(
            "v2 receipt engine binary is not the target's full VM archive {expected_engine_name}"
        ));
    }
    let engine_digest = digest(
        engine.get("binaryDigest"),
        "v2 receipt engine has no binaryDigest",
    )?;
    string(engine.get("variant"), "v2 receipt engine has no variant")?;

    let archives = manifest(root.get("archives"), "archives")?;
    if !archives
        .iter()
        .any(|(path, digest)| path == &engine_binary && digest == &engine_digest)
    {
        return Err("v2 receipt engine is not bound by its archive manifest".into());
    }
    let expected_lean_engine_name = if target.ends_with("-pc-windows-msvc") {
        "hermesvmlean_a.lib"
    } else {
        "libhermesvmlean_a.a"
    };
    if archives
        .iter()
        .filter(|(path, _)| path.rsplit('/').next() == Some(expected_lean_engine_name))
        .count()
        > 1
    {
        return Err(format!(
            "v2 receipt archive manifest names more than one target lean VM archive {expected_lean_engine_name}"
        ));
    }
    manifest(root.get("headers"), "headers")?;
    let icu = match root.get("icu") {
        Some(value) => Some(validate_icu(value, &target, &archives)?),
        None => None,
    };

    let links = root
        .get("linkDirectives")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or("v2 receipt has no ordered link directives")?;
    if links
        .iter()
        .any(|value| value.as_str().filter(|item| !item.is_empty()).is_none())
    {
        return Err("v2 receipt linkDirectives contains an empty or non-string value".into());
    }

    Ok(CanonicalReceipt {
        target,
        engine_binary,
        engine_digest,
        compiler_digest,
        bytecode_version,
        icu,
        archive_digests: archives.into_iter().collect(),
    })
}

fn validate_icu(
    value: &Value,
    target: &str,
    archives: &[(String, String)],
) -> Result<CanonicalIcuReceipt, String> {
    if !target.ends_with("-unknown-linux-gnu") {
        return Err("v2 receipt carries Linux ICU metadata for a non-Linux target".into());
    }
    let icu = object(Some(value), "v2 receipt ICU metadata is not an object")?;
    let upstream = object(
        icu.get("upstream"),
        "v2 receipt ICU metadata has no upstream",
    )?;
    if string(
        upstream.get("artifact"),
        "v2 receipt ICU upstream has no artifact",
    )? != "unicode-org/icu"
    {
        return Err("v2 receipt ICU upstream is not unicode-org/icu".into());
    }
    if string(
        upstream.get("sourceCommit"),
        "v2 receipt ICU upstream has no sourceCommit",
    )? != ICU_SOURCE_COMMIT
    {
        return Err(format!(
            "v2 receipt ICU sourceCommit is not pinned commit {ICU_SOURCE_COMMIT}"
        ));
    }
    if string(
        upstream.get("sourceRef"),
        "v2 receipt ICU upstream has no sourceRef",
    )? != "release-74-2"
        || string(
            upstream.get("sourceVersion"),
            "v2 receipt ICU upstream has no sourceVersion",
        )? != "74.2"
    {
        return Err("v2 receipt ICU upstream is not release-74-2 / 74.2".into());
    }

    let code_archives = icu
        .get("codeArchives")
        .and_then(Value::as_array)
        .ok_or("v2 receipt ICU metadata has no codeArchives")?;
    let code_archives = code_archives
        .iter()
        .map(|value| string(Some(value), "v2 receipt ICU code archive is not a string"))
        .collect::<Result<Vec<_>, _>>()?;
    if code_archives.len() != 2
        || code_archives[0].rsplit('/').next() != Some("libicui18n.a")
        || code_archives[1].rsplit('/').next() != Some("libicuuc.a")
    {
        return Err(
            "v2 receipt ICU codeArchives must name libicui18n.a and libicuuc.a once".into(),
        );
    }

    let data = object(
        icu.get("data"),
        "v2 receipt ICU metadata has no data object",
    )?;
    let trimmed = object(
        data.get("trimmed"),
        "v2 receipt ICU metadata has no trimmed data object",
    )?;
    let full = object(
        data.get("full"),
        "v2 receipt ICU metadata has no full data object",
    )?;
    let trimmed_data_archive = string(
        trimmed.get("archive"),
        "v2 receipt ICU trimmed data has no archive",
    )?;
    let full_data_archive = string(
        full.get("archive"),
        "v2 receipt ICU full data has no archive",
    )?;
    if trimmed_data_archive.rsplit('/').next() != Some("libicudata.a")
        || full_data_archive.rsplit('/').next() != Some("libicudata-full.a")
    {
        return Err(
            "v2 receipt ICU data variants must name libicudata.a and libicudata-full.a".into(),
        );
    }
    for archive in code_archives
        .iter()
        .chain([&trimmed_data_archive, &full_data_archive])
    {
        validate_path(archive)?;
        if !archives.iter().any(|(path, _)| path == archive) {
            return Err(format!(
                "v2 receipt ICU archive {archive} is not bound by the archive manifest"
            ));
        }
    }

    let filter = object(
        trimmed.get("filter"),
        "v2 receipt ICU trimmed data has no filter object",
    )?;
    let trimmed_filter_path = string(
        filter.get("path"),
        "v2 receipt ICU trimmed data filter has no path",
    )?;
    validate_path(&trimmed_filter_path)?;
    let trimmed_filter_digest = digest(
        filter.get("digest"),
        "v2 receipt ICU trimmed data filter has no digest",
    )?;
    if trimmed_filter_digest != ICU_TRIMMED_FILTER_DIGEST {
        return Err(format!(
            "v2 receipt ICU trimmed filter digest is not pinned digest {ICU_TRIMMED_FILTER_DIGEST}"
        ));
    }

    Ok(CanonicalIcuReceipt {
        code_archives,
        trimmed_data_archive,
        full_data_archive,
        trimmed_filter_path,
        trimmed_filter_digest,
    })
}

fn manifest(value: Option<&Value>, name: &str) -> Result<Vec<(String, String)>, String> {
    let entries = value
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| format!("v2 receipt has no {name} manifest"))?;
    let mut parsed = Vec::with_capacity(entries.len());
    let mut seen = BTreeSet::new();
    let mut previous: Option<String> = None;
    for entry in entries {
        let item = entry
            .as_object()
            .ok_or_else(|| format!("v2 receipt {name} manifest contains a non-object"))?;
        let path = string(
            item.get("path"),
            &format!("v2 receipt {name} entry has no path"),
        )?;
        validate_path(&path)?;
        if previous
            .as_deref()
            .is_some_and(|value| value >= path.as_str())
        {
            return Err(format!(
                "v2 receipt {name} manifest is not strictly sorted by path"
            ));
        }
        if !seen.insert(path.clone()) {
            return Err(format!("v2 receipt {name} manifest repeats {path}"));
        }
        previous = Some(path.clone());
        let digest = digest(
            item.get("digest"),
            &format!("v2 receipt {name} entry has no digest"),
        )?;
        parsed.push((path, digest));
    }
    Ok(parsed)
}

fn validate_path(path: &str) -> Result<(), String> {
    let first = path.as_bytes().first().copied();
    if path.is_empty()
        || path.contains('\\')
        || matches!(first, Some(b'/'))
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
        || path.as_bytes().get(1) == Some(&b':')
    {
        return Err(format!(
            "v2 receipt path {path:?} is not a safe bundle-relative path"
        ));
    }
    Ok(())
}

fn digest(value: Option<&Value>, missing: &str) -> Result<String, String> {
    let value = string(value, missing)?;
    let Some(hex) = value.strip_prefix("sha256-") else {
        return Err(format!("{missing}; expected sha256-<64 lowercase hex>"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("invalid SHA-256 digest {value:?}"));
    }
    Ok(value)
}

fn string(value: Option<&Value>, missing: &str) -> Result<String, String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| missing.to_owned())
}

fn object<'a>(value: Option<&'a Value>, missing: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .and_then(Value::as_object)
        .ok_or_else(|| missing.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = include_str!("testdata/receipt-v2-valid.json");

    fn document() -> Value {
        serde_json::from_str(VALID).expect("shared receipt fixture")
    }

    #[test]
    fn shared_canonical_fixture_is_accepted_for_its_exact_target() {
        let receipt = validate(&document(), Some("aarch64-apple-darwin")).expect("canonical v2");
        assert_eq!(receipt.target, "aarch64-apple-darwin");
        assert_eq!(receipt.bytecode_version, 99);
    }

    #[test]
    fn simulator_aliases_select_the_universal_bundle_receipt_target() {
        assert_eq!(
            bundle_target_for_rust_target("aarch64-apple-ios-sim"),
            "universal-apple-ios-simulator"
        );
        assert_eq!(
            bundle_target_for_rust_target("x86_64-apple-ios"),
            "universal-apple-ios-simulator"
        );
        assert_eq!(
            bundle_target_for_rust_target("aarch64-apple-ios"),
            "aarch64-apple-ios"
        );
    }

    #[test]
    fn shared_fixture_rejects_wrong_target_commit_patches_and_missing_closure() {
        assert!(validate(&document(), Some("aarch64-apple-ios")).is_err());

        let mut wrong_commit = document();
        wrong_commit["upstream"]["sourceCommit"] = Value::String("0".repeat(40));
        assert!(validate(&wrong_commit, None)
            .unwrap_err()
            .contains("pinned"));

        let mut patched = document();
        patched["patchSet"]["applied"] = serde_json::json!(["0001.patch"]);
        assert!(validate(&patched, None).unwrap_err().contains("not empty"));

        for field in ["archives", "headers", "linkDirectives"] {
            let mut missing = document();
            missing.as_object_mut().expect("object").remove(field);
            assert!(
                validate(&missing, None).is_err(),
                "accepted missing {field}"
            );
        }
    }

    #[test]
    fn shared_fixture_requires_compiler_identity_and_positive_hbc() {
        let mut no_compiler = document();
        no_compiler
            .as_object_mut()
            .expect("object")
            .remove("compiler");
        assert!(validate(&no_compiler, None)
            .unwrap_err()
            .contains("compiler object"));

        let mut zero_hbc = document();
        zero_hbc["bytecode"]["version"] = Value::from(0);
        assert!(validate(&zero_hbc, None)
            .unwrap_err()
            .contains("positive HBC"));
    }

    #[test]
    fn shared_fixture_binds_one_target_lean_archive() {
        validate(&document(), None).expect("canonical v2 with full and lean VMs");

        let mut duplicate = document();
        let archives = duplicate["archives"].as_array_mut().expect("archives");
        archives.push(serde_json::json!({
            "path": "other/libhermesvmlean_a.a",
            "digest": "sha256-6666666666666666666666666666666666666666666666666666666666666666"
        }));
        archives.sort_by(|left, right| {
            left["path"]
                .as_str()
                .expect("path")
                .cmp(right["path"].as_str().expect("path"))
        });
        assert!(validate(&duplicate, None)
            .unwrap_err()
            .contains("more than one target lean VM archive"));
    }

    #[test]
    fn shared_fixture_exposes_the_lean_archive_identity() {
        let receipt = validate(&document(), None).expect("canonical v2");
        assert_eq!(
            receipt
                .archive_digests()
                .get("lib/libhermesvmlean_a.a")
                .map(String::as_str),
            Some("sha256-5555555555555555555555555555555555555555555555555555555555555555")
        );
    }

    #[test]
    fn linux_icu_metadata_binds_both_data_variants_and_the_filter() {
        let mut document = document();
        document["target"] = Value::String("aarch64-unknown-linux-gnu".into());
        let archives = document["archives"].as_array_mut().expect("archives");
        for (path, byte) in [
            ("lib/libicudata-full.a", '6'),
            ("lib/libicudata.a", '7'),
            ("lib/libicui18n.a", '8'),
            ("lib/libicuuc.a", '9'),
        ] {
            archives.push(serde_json::json!({
                "path": path,
                "digest": format!("sha256-{}", byte.to_string().repeat(64)),
            }));
        }
        archives.sort_by(|left, right| {
            left["path"]
                .as_str()
                .expect("path")
                .cmp(right["path"].as_str().expect("path"))
        });
        document["icu"] = serde_json::json!({
            "upstream": {
                "artifact": "unicode-org/icu",
                "sourceCommit": ICU_SOURCE_COMMIT,
                "sourceRef": "release-74-2",
                "sourceVersion": "74.2"
            },
            "codeArchives": ["lib/libicui18n.a", "lib/libicuuc.a"],
            "data": {
                "trimmed": {
                    "archive": "lib/libicudata.a",
                    "filter": {
                        "path": "share/icu/filters-root-en.json",
                        "digest": ICU_TRIMMED_FILTER_DIGEST
                    }
                },
                "full": { "archive": "lib/libicudata-full.a" }
            }
        });

        let receipt = validate(&document, Some("aarch64-unknown-linux-gnu"))
            .expect("canonical Linux ICU metadata");
        assert_eq!(
            receipt.icu.expect("ICU metadata").full_data_archive,
            "lib/libicudata-full.a"
        );

        document["icu"]["data"]["trimmed"]["filter"]["digest"] =
            Value::String(format!("sha256-{}", "0".repeat(64)));
        assert!(validate(&document, None)
            .unwrap_err()
            .contains("trimmed filter digest"));
    }
}
