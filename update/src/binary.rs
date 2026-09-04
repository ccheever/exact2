//! What the bake wrote into the binary, read back: `compat.json` beside the
//! plan (LLP 1030 D3a; 1030.000 D4, D7), which is where a host finds the
//! app id, the cohort, the channel and its origin, the verification keys,
//! and the activation policy — nothing here is hand-declared.
//!
//! The file is `contract::Compat::to_json`'s: `{"id":…,"inputs":{…,"app":…,
//! "keys":{…}|null,…},"delivery":{"activate":…,"channel":…,"origin":…|null}}`.
//! A field the text does not carry degrades to the dev answer (no keys, no
//! origin, `next-launch`) rather than refusing a boot: the store then admits
//! unsigned heads and checks nowhere until a host names an origin.

use crate::envelope::{base64_decode, sha256_hex};
use crate::store::Embedded;

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
    /// and the check need. Refuses only a file that is not JSON or names no
    /// compatibility id — the two facts a store cannot be opened without.
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
        if let Some(keys) = inputs
            .and_then(|i| i.get("keys"))
            .and_then(|k| k.as_object())
        {
            for (id, encoded) in keys {
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
        let activate = match delivery
            .and_then(|d| d.get("activate"))
            .and_then(|v| v.as_str())
        {
            Some("app-decides") => Activate::AppDecides,
            _ => Activate::NextLaunch,
        };
        Ok(Baked {
            embedded: Embedded {
                app_id,
                compatibility_id,
                seq: 0,
                channel,
                verification_keys,
                embedded_plan_sha256: Some(sha256_hex(plan)),
            },
            origin,
            activate,
            store_linked,
        })
    }

    /// The head this binary checks (`store::head_url`), at `origin` when the
    /// host names one — a dev override — else at the baked origin; `None`
    /// when neither exists.
    pub fn head_url(&self, origin: Option<&str>) -> Option<String> {
        let origin = origin.or(self.origin.as_deref())?;
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
        r#""keys":{"caltrain-2026":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="},"#,
        r#""store":{"L":"A"}},"delivery":{"activate":"app-decides","channel":"beta","#,
        r#""origin":"https://caltrain.example"}}"#,
        "\n"
    );

    #[test]
    fn the_baked_facts_read_back() {
        let b = Baked::from_compat(COMPAT, b"plan").unwrap();
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
    fn a_dev_binary_has_no_keys_no_origin_and_next_launch() {
        let b = Baked::from_compat(
            r#"{"id":"abc","inputs":{"app":"x","keys":null},"delivery":{"channel":"prod","origin":null}}"#,
            b"",
        )
        .unwrap();
        assert!(b.embedded.verification_keys.is_empty());
        assert_eq!(b.origin, None);
        assert_eq!(b.activate, Activate::NextLaunch);
        assert!(b.store_linked, "unsaid is linked");
        assert_eq!(b.head_url(None), None);
        let stripped =
            Baked::from_compat(r#"{"id":"abc","inputs":{"store":{"L":"0"}}}"#, b"").unwrap();
        assert!(!stripped.store_linked);
        assert!(Baked::from_compat("{}", b"").is_err());
        assert!(Baked::from_compat("not json", b"").is_err());
        assert!(
            Baked::from_compat(r#"{"id":"abc","inputs":{"keys":{"k":"AAAA"}}}"#, b"")
                .unwrap_err()
                .contains("32")
        );
    }
}
