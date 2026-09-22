//! What the bake wrote into the binary, read back: `compat.json` beside the
//! plan (LLP 1030 D3a; 1030.000 D4, D7), which is where a host finds the
//! app id, the cohort, the channel and its origin, the verification keys,
//! and the activation policy — nothing here is hand-declared.
//!
//! Trust is an explicit compatibility input. Missing or malformed trust
//! policy refuses the updater; it never becomes development by omission.

use crate::envelope::{base64_decode, sha256_hex};
use crate::store::{Embedded, Trust};

/// When a staged bundle applies (LLP 1030.000 D4, `deploy.activate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activate {
    /// The next launch boots it; `deliveryActivate` applies it now.
    NextLaunch,
    /// Nothing until the app calls `deliveryActivate` — a launch keeps
    /// booting what it booted last time.
    AppDecides,
}

/// The binary's delivery facts as the bake wrote them: entry zero's
/// identity for the store, and where and how this binary takes updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baked {
    /// What [`crate::Store::open`] takes.
    pub embedded: Embedded,
    /// The channel's origin (`deploy.channels.<channel>`, else `app.origin`),
    /// when the manifest names one.
    pub origin: Option<String>,
    /// The activation policy.
    pub activate: Activate,
    /// Whether this binary links an update store at all (`inputs.store.L`
    /// of `A`; LLP 1030 D4). A stripped binary (`0`) opens none.
    pub store_linked: bool,
}

impl Baked {
    /// Read `compat.json` and the embedded plan's bytes into what the store
    /// and the check need. Missing/malformed trust policy and production
    /// artifacts without trust roots fail closed before a store opens.
    pub fn from_compat(compat_json: &str, plan: &[u8]) -> Result<Baked, String> {
        let value: serde_json::Value = serde_json::from_str(compat_json)
            .map_err(|e| format!("compat.json is not JSON: {e}"))?;
        let compatibility_id = value
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "compat.json names no compatibility id".to_string())?
            .to_string();
        let inputs = value.get("inputs");
        let trust = match inputs.and_then(|i| i.get("trust")).and_then(|v| v.as_str()) {
            Some("production") => Trust::Production,
            Some("development") => Trust::Development,
            _ => {
                return Err(
                    "compat.json needs an explicit production or development trust policy".into(),
                )
            }
        };
        let store_linked = inputs
            .and_then(|i| i.get("store"))
            .and_then(|s| s.get("L"))
            .and_then(|v| v.as_str())
            != Some("0");
        let app_id = inputs
            .and_then(|i| i.get("app"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let mut verification_keys = Vec::new();
        let keys = match inputs.and_then(|i| i.get("keys")) {
            Some(serde_json::Value::Object(keys)) => Some(keys),
            None | Some(serde_json::Value::Null) => None,
            _ => return Err("compat.json verification keys must be an object".into()),
        };
        if let Some(keys) = keys {
            for (id, encoded) in keys {
                if id.is_empty() {
                    return Err("compat.json verification key id is empty".into());
                }
                let encoded = encoded
                    .as_str()
                    .ok_or_else(|| format!("the verification key {id} is not a string"))?;
                let bytes = base64_decode(encoded)
                    .map_err(|e| format!("the verification key {id} is not base64: {e}"))?;
                let key: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
                    format!(
                        "the verification key {id} is {} bytes; an Ed25519 public key is 32",
                        bytes.len()
                    )
                })?;
                verification_keys.push((id.clone(), key));
            }
        }
        verification_keys.sort_by(|a, b| a.0.cmp(&b.0));
        let delivery = value.get("delivery");
        let channel = delivery
            .and_then(|d| d.get("channel"))
            .and_then(|v| v.as_str())
            .unwrap_or("prod")
            .to_string();
        let origin = delivery
            .and_then(|d| d.get("origin"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        if trust == Trust::Production
            && verification_keys.is_empty()
            && (store_linked || origin.is_some())
        {
            return Err("production updater requires at least one verification key".into());
        }
        let activate = match delivery
            .and_then(|d| d.get("activate"))
            .and_then(|v| v.as_str())
        {
            Some("app-decides") => Activate::AppDecides,
            _ => Activate::NextLaunch,
        };
        let metadata = value.get("embedded");
        let (seq, embedded_assets, entry_digest) = if let Some(metadata) = metadata {
            let seq = metadata
                .get("seq")
                .and_then(|v| v.as_u64())
                .ok_or("compat.json embedded.seq must be a nonnegative integer")?;
            let expected = metadata
                .get("plan")
                .ok_or("compat.json embedded.plan is missing")?;
            if expected.get("sha256").and_then(|v| v.as_str()) != Some(sha256_hex(plan).as_str())
                || expected.get("bytes").and_then(|v| v.as_u64()) != Some(plan.len() as u64)
            {
                return Err("compat.json embedded.plan does not match the baked plan".into());
            }
            let cards = metadata
                .get("assets")
                .and_then(|v| v.as_array())
                .ok_or("compat.json embedded.assets must be a complete array")?;
            let mut assets = std::collections::BTreeMap::new();
            for card in cards {
                let name = card
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or("embedded asset has no name")?;
                crate::envelope::safe_name(name)?;
                let sha = card
                    .get("sha256")
                    .and_then(|v| v.as_str())
                    .ok_or("embedded asset has no digest")?;
                if sha.len() != 64
                    || !sha
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                {
                    return Err("embedded asset digest is not lowercase SHA-256".into());
                }
                let bytes = card
                    .get("bytes")
                    .and_then(|v| v.as_u64())
                    .ok_or("embedded asset has no byte count")?;
                if assets
                    .insert(name.to_string(), (sha.to_string(), bytes))
                    .is_some()
                {
                    return Err(format!("embedded asset {name} is duplicated"));
                }
            }
            let digest = metadata
                .get("entryDigest")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if let Some(digest) = &digest {
                if digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                {
                    return Err("embedded entryDigest is not lowercase SHA-256".into());
                }
            } else if trust == Trust::Production
                && !(seq == 0 && metadata.get("genesis").and_then(|v| v.as_bool()) == Some(true))
            {
                return Err("production embedding needs an authenticated entryDigest or explicit seq-zero genesis".into());
            }
            (seq, Some(assets), digest)
        } else if trust == Trust::Development || (!store_linked && origin.is_none()) {
            (0, None, None)
        } else {
            return Err("production compat.json needs complete embedded release metadata".into());
        };
        Ok(Baked {
            embedded: Embedded {
                app_id,
                compatibility_id,
                seq,
                channel,
                verification_keys,
                trust,
                embedded_plan_sha256: Some(sha256_hex(plan)),
                embedded_assets,
                entry_digest,
            },
            origin,
            activate,
            store_linked,
        })
    }

    /// The head this binary checks (`store::head_url`), at `origin` when the
    /// host names one — a dev override — else at the baked origin; `None`
    /// when neither exists. A development binary admits unsigned heads, so
    /// it never checks the baked origin: only one the developer names.
    pub fn head_url(&self, origin: Option<&str>) -> Option<String> {
        let baked = match self.embedded.trust {
            Trust::Production => self.origin.as_deref(),
            Trust::Development => None,
        };
        let origin = origin.or(baked)?;
        Some(crate::store::head_url(
            origin,
            &self.embedded.channel,
            &self.embedded.compatibility_id,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real `compat.json`, shortened, with one key (32 zero bytes).
    const COMPAT: &str = concat!(
        r#"{"id":"9f1c0a2b3d4e5f60718293a4b5c6d7e8","inputs":{"app":"com.exact.caltrain","#,
        r#""trust":"production","keys":{"caltrain-2026":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="},"#,
        r#""store":{"L":"A"}},"delivery":{"activate":"app-decides","channel":"beta","#,
        r#""origin":"https://caltrain.example"}}"#,
        "\n"
    );

    #[test]
    fn the_baked_facts_read_back() {
        let mut compat: serde_json::Value = serde_json::from_str(COMPAT).unwrap();
        compat["embedded"] = serde_json::json!({"seq":0,"genesis":true,"entryDigest":null,"assets":[],"plan":{"sha256":sha256_hex(b"plan"),"bytes":4}});
        let b = Baked::from_compat(&compat.to_string(), b"plan").unwrap();
        assert_eq!(b.embedded.app_id, "com.exact.caltrain");
        assert_eq!(
            b.embedded.compatibility_id,
            "9f1c0a2b3d4e5f60718293a4b5c6d7e8"
        );
        assert_eq!(b.embedded.channel, "beta");
        assert_eq!(b.embedded.seq, 0);
        assert_eq!(
            b.embedded.verification_keys,
            vec![("caltrain-2026".to_string(), [0u8; 32])]
        );
        assert_eq!(
            b.embedded.embedded_plan_sha256.as_deref(),
            Some(sha256_hex(b"plan").as_str())
        );
        assert_eq!(b.origin.as_deref(), Some("https://caltrain.example"));
        assert_eq!(b.activate, Activate::AppDecides);
        assert!(b.store_linked);
        assert_eq!(
            b.head_url(None).as_deref(),
            Some(
                "https://caltrain.example/.exact/beta/9f1c0a2b3d4e5f60718293a4b5c6d7e8/exact.json"
            )
        );
        assert_eq!(
            b.head_url(Some("http://127.0.0.1:8000/")).as_deref(),
            Some("http://127.0.0.1:8000/.exact/beta/9f1c0a2b3d4e5f60718293a4b5c6d7e8/exact.json")
        );
    }

    #[test]
    fn a_development_binary_checks_only_a_named_origin() {
        let b = Baked::from_compat(
            r#"{"id":"abc","inputs":{"app":"x","keys":null,"trust":"development"},"delivery":{"channel":"prod","origin":"https://caltrain.example"}}"#,
            b"",
        )
        .unwrap();
        assert_eq!(b.origin.as_deref(), Some("https://caltrain.example"));
        assert_eq!(
            b.head_url(None),
            None,
            "unsigned heads never come from the baked origin"
        );
        assert_eq!(
            b.head_url(Some("http://127.0.0.1:8000/")).as_deref(),
            Some("http://127.0.0.1:8000/.exact/prod/abc/exact.json")
        );
    }

    #[test]
    fn a_dev_binary_has_no_keys_no_origin_and_next_launch() {
        let b = Baked::from_compat(
            r#"{"id":"abc","inputs":{"app":"x","keys":null,"trust":"development"},"delivery":{"channel":"prod","origin":null}}"#,
            b"",
        )
        .unwrap();
        assert!(b.embedded.verification_keys.is_empty());
        assert_eq!(b.origin, None);
        assert_eq!(b.activate, Activate::NextLaunch);
        assert!(b.store_linked, "unsaid is linked");
        assert_eq!(b.head_url(None), None);
        let stripped = Baked::from_compat(
            r#"{"id":"abc","inputs":{"store":{"L":"0"},"trust":"production"}}"#,
            b"",
        )
        .unwrap();
        assert!(!stripped.store_linked);
        assert!(Baked::from_compat("{}", b"").is_err());
        assert!(Baked::from_compat("not json", b"").is_err());
        assert!(Baked::from_compat(
            r#"{"id":"abc","inputs":{"keys":{"k":"AAAA"},"trust":"production"}}"#,
            b""
        )
        .unwrap_err()
        .contains("32"));
    }
    #[test]
    fn missing_or_malformed_trust_never_infers_development() {
        for trust in [
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!(false)),
            Some(serde_json::json!("dev")),
        ] {
            let mut value: serde_json::Value = serde_json::from_str(COMPAT).unwrap();
            let inputs = value["inputs"].as_object_mut().unwrap();
            inputs.remove("trust");
            if let Some(trust) = trust {
                inputs.insert("trust".into(), trust);
            }
            assert!(Baked::from_compat(&value.to_string(), b"plan")
                .unwrap_err()
                .contains("trust policy"));
        }
    }

    #[test]
    fn production_with_missing_null_empty_or_malformed_keys_is_refused() {
        for keys in [
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!({})),
            Some(serde_json::json!([])),
            Some(serde_json::json!({"key": false})),
        ] {
            let mut value: serde_json::Value = serde_json::from_str(COMPAT).unwrap();
            let inputs = value["inputs"].as_object_mut().unwrap();
            inputs.remove("keys");
            if let Some(keys) = keys {
                inputs.insert("keys".into(), keys);
            }
            assert!(
                Baked::from_compat(&value.to_string(), b"plan").is_err(),
                "{value}"
            );
        }
        // Declaring no store does not excuse a production update origin.
        let value = serde_json::json!({"id":"abc","inputs":{"trust":"production","store":{"L":"0"}},"delivery":{"origin":"https://updates.example"}});
        assert!(Baked::from_compat(&value.to_string(), b"").is_err());
    }
    #[test]
    fn production_embedded_metadata_carries_floor_digest_and_complete_roster() {
        let mut value: serde_json::Value = serde_json::from_str(COMPAT).unwrap();
        assert!(Baked::from_compat(&value.to_string(), b"plan").is_err());
        value["embedded"] = serde_json::json!({
            "seq":7,"entryDigest":"1".repeat(64),
            "plan":{"sha256":sha256_hex(b"plan"),"bytes":4},
            "assets":[{"name":"assets/a","sha256":"2".repeat(64),"bytes":3}]
        });
        let baked = Baked::from_compat(&value.to_string(), b"plan").unwrap();
        assert_eq!(baked.embedded.seq, 7);
        assert_eq!(baked.embedded.entry_digest, Some("1".repeat(64)));
        assert_eq!(baked.embedded.embedded_assets.unwrap().len(), 1);
        for (path, invalid) in [
            ("seq", serde_json::json!(-1)),
            ("entryDigest", serde_json::Value::Null),
            ("assets", serde_json::Value::Null),
            (
                "plan",
                serde_json::json!({"sha256":sha256_hex(b"other"),"bytes":4}),
            ),
        ] {
            let mut wrong = value.clone();
            wrong["embedded"][path] = invalid;
            assert!(
                Baked::from_compat(&wrong.to_string(), b"plan").is_err(),
                "{wrong}"
            );
        }
    }
}
