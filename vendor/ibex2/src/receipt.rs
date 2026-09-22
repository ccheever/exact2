//! Receipts: what an artifact is allowed to claim about itself.
//!
//! LLP 0058.000.001 §5 (tombstoned) defined a four-artifact chain; LLP 0067 §5
//! keeps the first link. This implements the first
//! link, `HermesInputReceipt`, and binds module artifacts to it. The
//! `GraduationManifest` and the post-link `GreenfieldFinalArtifactReceipt` are
//! **not** implemented: the first needs a tier-definition process and the second
//! a linker-closure scanner, neither of which exists. They are absent rather
//! than stubbed, so nothing here can be mistaken for the full chain.
//!
//! The property this buys: "vanilla" stops being a build flag someone
//! remembered to pass and becomes a checkable claim about an artifact —
//! produced by a separate tool, verified here, and refused if the engine
//! carries the patch series' exports.
//!
//! @ref LLP 0067#5-the-engine-and-the-artifacts — the receipt, and what is verified where
//! @ref LLP 0067#5-the-engine-and-the-artifacts — vanilla means zero patches: the claim being checked

use std::path::Path;

/// The schema this understands. A different one is refused rather than guessed
/// at: a receipt reader that tolerates unknown versions is not a check.
pub const HERMES_INPUT_SCHEMA: &str = "ibex/hermes-upstream-pinned-receipt/1";

/// SHA-256 of no input at all — what an empty patch set must hash to.
pub const CANONICAL_EMPTY_PATCH_SET: &str =
    "sha256-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// What a `HermesInputReceipt` asserts about an installed engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesInput {
    pub binary_digest: String,
    pub variant: String,
    pub patch_set_digest: String,
    pub patches_applied: usize,
    pub compiler_digest: Option<String>,
}

impl HermesInput {
    pub fn path(engine_dir: &Path) -> std::path::PathBuf {
        engine_dir.join("hermes-input-receipt.json")
    }

    /// Read and check a receipt beside an engine.
    ///
    /// Deliberately hand-parsed rather than pulled through a JSON crate: this
    /// runs before anything is trusted, and the fields are few and fixed.
    pub fn read(engine_dir: &Path) -> Result<Self, String> {
        let path = Self::path(engine_dir);
        let text = std::fs::read_to_string(&path)
            .map_err(|_| format!("no HermesInputReceipt at {}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let field = |name: &str| -> Option<String> {
            let needle = format!("\"{name}\"");
            let at = text.find(&needle)?;
            let rest = &text[at + needle.len()..];
            let colon = rest.find(':')?;
            let after = rest[colon + 1..].trim_start();
            if let Some(stripped) = after.strip_prefix('"') {
                stripped.find('"').map(|end| stripped[..end].to_string())
            } else if after.starts_with('[') {
                // Only used for `applied`, whose emptiness is the claim.
                let end = after.find(']')?;
                Some(after[..=end].to_string())
            } else {
                None
            }
        };

        let schema = field("schema").ok_or("receipt has no schema")?;
        if schema != HERMES_INPUT_SCHEMA {
            return Err(format!(
                "unknown receipt schema {schema:?}; expected {HERMES_INPUT_SCHEMA:?}"
            ));
        }

        let applied = field("applied").unwrap_or_default();
        let patches_applied = applied.matches('"').count() / 2;

        Ok(Self {
            binary_digest: field("binaryDigest").ok_or("receipt has no binaryDigest")?,
            variant: field("variant").ok_or("receipt has no variant")?,
            patch_set_digest: field("digest").ok_or("receipt has no patch-set digest")?,
            patches_applied,
            compiler_digest: field("digest")
                .filter(|_| text.contains("\"compiler\""))
                .and_then(|_| {
                    // The compiler's digest is the one after the "compiler" key.
                    let at = text.find("\"compiler\"")?;
                    let rest = &text[at..];
                    let d = rest.find("\"digest\"")?;
                    let after = rest[d + "\"digest\"".len()..].trim_start();
                    let after = after.strip_prefix(':')?.trim_start().strip_prefix('"')?;
                    after.find('"').map(|end| after[..end].to_string())
                }),
        })
    }

    /// Does this receipt claim an unpatched engine, and is the claim coherent?
    pub fn is_vanilla(&self) -> bool {
        self.patch_set_digest == CANONICAL_EMPTY_PATCH_SET && self.patches_applied == 0
    }

    /// Check the receipt against the engine sitting beside it.
    ///
    /// A receipt that does not describe the bytes actually present is worse
    /// than no receipt: it is a claim someone may rely on.
    pub fn verify_binary(&self, engine_dir: &Path) -> Result<(), String> {
        let binary = engine_binary(engine_dir)?;
        let bytes =
            std::fs::read(&binary).map_err(|e| format!("cannot read {}: {e}", binary.display()))?;
        let actual = format!(
            "sha256-{}",
            hex(&<sha2::Sha256 as sha2::Digest>::digest(&bytes))
        );
        if actual != self.binary_digest {
            return Err(format!(
                "receipt describes a different engine than the one present\n  \
                 receipt: {}\n  actual:  {actual}",
                self.binary_digest
            ));
        }
        Ok(())
    }
}

fn engine_binary(engine_dir: &Path) -> Result<std::path::PathBuf, String> {
    for relative in [
        "hermesvm.framework/Versions/1/hermesvm",
        "linux-static/libhermesvm_a.a",
    ] {
        let candidate = engine_dir.join(relative);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "no Hermes engine binary under {}",
        engine_dir.display()
    ))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VANILLA: &str = r#"{
      "schema": "ibex/hermes-upstream-pinned-receipt/1",
      "engine": { "binaryDigest": "sha256-abc", "variant": "release" },
      "patchSet": { "digest": "sha256-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "applied": [] },
      "compiler": { "digest": "sha256-def" }
    }"#;

    #[test]
    fn a_vanilla_receipt_parses_and_reads_as_vanilla() {
        let receipt = HermesInput::parse(VANILLA).expect("parse");
        assert_eq!(receipt.binary_digest, "sha256-abc");
        assert_eq!(receipt.variant, "release");
        assert_eq!(receipt.patches_applied, 0);
        assert!(receipt.is_vanilla());
        assert_eq!(receipt.compiler_digest.as_deref(), Some("sha256-def"));
    }

    #[test]
    fn a_receipt_claiming_applied_patches_is_not_vanilla() {
        let patched = VANILLA.replace(r#""applied": []"#, r#""applied": ["0001-x.patch"]"#);
        let receipt = HermesInput::parse(&patched).expect("parse");
        assert_eq!(receipt.patches_applied, 1);
        assert!(!receipt.is_vanilla());
    }

    /// The digest is the claim, so a receipt whose digest is not the canonical
    /// empty one is not vanilla even if it says nothing was applied.
    #[test]
    fn an_empty_list_with_a_non_empty_digest_is_not_vanilla() {
        let inconsistent = VANILLA.replace(CANONICAL_EMPTY_PATCH_SET, "sha256-something-else");
        assert!(!HermesInput::parse(&inconsistent).unwrap().is_vanilla());
    }

    #[test]
    fn an_unknown_schema_is_refused_rather_than_guessed_at() {
        let future = VANILLA.replace(
            "ibex/hermes-upstream-pinned-receipt/1",
            "ibex/hermes-upstream-pinned-receipt/2",
        );
        let err = HermesInput::parse(&future).unwrap_err();
        assert!(err.contains("unknown receipt schema"), "{err}");
    }

    #[test]
    fn a_receipt_missing_required_fields_is_refused() {
        assert!(HermesInput::parse("{}").is_err());
        assert!(
            HermesInput::parse(r#"{"schema":"ibex/hermes-upstream-pinned-receipt/1"}"#).is_err()
        );
    }

    /// The receipt the build actually produced, checked against the engine it
    /// describes. Skipped where no vanilla engine is installed.
    #[test]
    fn the_installed_receipt_describes_the_installed_engine() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for dir in [
            "ios/Frameworks-vanilla-nodebug",
            "ios/Frameworks-vanilla",
            "linux/Frameworks-vanilla",
        ] {
            let engine = root.join(dir);
            if !HermesInput::path(&engine).exists() {
                continue;
            }
            let receipt = HermesInput::read(&engine).expect("read");
            assert!(receipt.is_vanilla(), "{dir} receipt is not vanilla");
            receipt.verify_binary(&engine).expect("binary matches");
        }
    }
}
