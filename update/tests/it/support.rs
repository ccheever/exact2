//! The store's test harness: an in-memory origin that publishes bundles as
//! `exact deploy` does, and store directories that remove themselves.
//!
//! @ref LLP 1026 D9–D12 / LLP 1030 D3a, D5, D9 / LLP 1030.000 §4 stage 4

use ed25519_dalek::{Signer, SigningKey};
use exact_update::{canonical_bytes, sha256_hex, Check, Embedded, Store};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(crate) const ORIGIN: &str = "https://cdn.example/apps/caltrain";
pub(crate) const APP: &str = "com.exact.caltrain";
pub(crate) const COHORT: &str = "9a1f3c7e5b2d4086";
pub(crate) const EMBEDDED_SEQ: u64 = 3;

// ---------------------------------------------------------------- the origin

/// One published bundle, as `exact deploy` would write it (LLP 1030.000 D3).
pub(crate) struct Bundle {
    pub(crate) app: String,
    pub(crate) cohort: String,
    pub(crate) channel: String,
    pub(crate) seq: u64,
    pub(crate) plan: Vec<u8>,
    pub(crate) assets: Vec<(String, Vec<u8>)>,
    pub(crate) sunset: Option<(String, Option<String>)>,
    pub(crate) signer: Option<(String, SigningKey)>,
    /// Serve a plan whose bytes are not the ones the head names.
    pub(crate) tamper_plan: bool,
}

impl Bundle {
    pub(crate) fn new(seq: u64, plan: &[u8]) -> Bundle {
        Bundle {
            app: APP.into(),
            cohort: COHORT.into(),
            channel: "release".into(),
            seq,
            plan: plan.to_vec(),
            assets: Vec::new(),
            sunset: None,
            signer: None,
            tamper_plan: false,
        }
    }

    pub(crate) fn asset(mut self, name: &str, bytes: &[u8]) -> Bundle {
        self.assets.push((name.into(), bytes.to_vec()));
        self
    }

    /// The head as JSON, and the files the origin serves, by URL.
    pub(crate) fn publish(&self) -> (String, HashMap<String, Vec<u8>>) {
        let mut envelope = serde_json::json!({
            "exact": 1,
            "app": { "id": self.app, "name": "Caltrain" },
            "plan": {
                "url": "./app.plan",
                "sha256": sha256_hex(&self.plan),
                "bytes": self.plan.len(),
                "formatVersion": 4,
                "kernelSchema": "0123456789abcdef",
            },
            "assets": self.assets.iter().map(|(name, bytes)| serde_json::json!({
                "name": name,
                "url": format!("./assets/{name}"),
                "sha256": sha256_hex(bytes),
                "bytes": bytes.len(),
            })).collect::<Vec<_>>(),
            "stream": {
                "app": self.app,
                "channel": self.channel,
                "compatibilityId": self.cohort,
                "seq": self.seq,
            },
            "release": "2026-09-03T18:04:11Z-7f1c",
        });
        if let Some((message, store)) = &self.sunset {
            envelope["sunset"] = match store {
                Some(url) => serde_json::json!({ "message": message, "store": url }),
                None => serde_json::json!({ "message": message }),
            };
        }
        let mut text = serde_json::to_string(&envelope).unwrap();
        if let Some((key_id, key)) = &self.signer {
            let signature = key.sign(&canonical_bytes(&text).unwrap());
            envelope["signature"] = serde_json::json!({
                "keyId": key_id,
                "ed25519": base64(&signature.to_bytes()),
            });
            text = serde_json::to_string(&envelope).unwrap();
        }
        let base = format!("{ORIGIN}/.exact/{}/{}", self.channel, self.cohort);
        let mut files = HashMap::new();
        files.insert(format!("{base}/exact.json"), text.clone().into_bytes());
        // The tampered body is the declared *length* with other bytes, so the
        // digest is what refuses it rather than the byte count.
        let plan = if self.tamper_plan {
            self.plan.iter().map(|b| b ^ 0x20).collect()
        } else {
            self.plan.clone()
        };
        files.insert(format!("{base}/app.plan"), plan);
        for (name, bytes) in &self.assets {
            files.insert(format!("{base}/assets/{name}"), bytes.clone());
        }
        (text, files)
    }
}

/// The origin as a closure: what it serves, and every URL it was asked for.
pub(crate) struct Origin {
    pub(crate) files: HashMap<String, Vec<u8>>,
    pub(crate) asked: Vec<String>,
}

impl Origin {
    pub(crate) fn of(bundle: &Bundle) -> Origin {
        Origin {
            files: bundle.publish().1,
            asked: Vec::new(),
        }
    }

    pub(crate) fn serving(&mut self, bundle: &Bundle) {
        self.files.extend(bundle.publish().1);
    }

    pub(crate) fn check(&mut self, store: &mut Store) -> Result<Check, String> {
        self.check_embedding(store, &[])
    }

    /// The same check from a binary that embeds `assets` (name, bytes).
    pub(crate) fn check_embedding(
        &mut self,
        store: &mut Store,
        assets: &[(&str, &[u8])],
    ) -> Result<Check, String> {
        self.asked.clear();
        let files = &self.files;
        let asked = &mut self.asked;
        let mut fetch = move |url: &str| -> Result<Vec<u8>, String> {
            asked.push(url.to_string());
            files.get(url).cloned().ok_or_else(|| format!("404 {url}"))
        };
        let mut embedded = |name: &str| -> Option<String> {
            assets
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, bytes)| sha256_hex(bytes))
        };
        store.check(
            &exact_update::head_url(ORIGIN, "release", COHORT),
            &mut fetch,
            &mut embedded,
        )
    }
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let triple = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from_be_bytes([0, triple[0], triple[1], triple[2]]);
        let index = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        out.push(ALPHABET[index[0] as usize] as char);
        out.push(ALPHABET[index[1] as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[index[2] as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[index[3] as usize] as char
        } else {
            '='
        });
    }
    out
}

// ----------------------------------------------------------------- the store

/// A store directory that removes itself.
pub(crate) struct Temp(PathBuf);

impl Temp {
    pub(crate) fn new(name: &str) -> Temp {
        let dir = std::env::temp_dir().join(format!("exact-update-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn embedded(keys: &[(&str, [u8; 32])]) -> Embedded {
    Embedded {
        app_id: APP.into(),
        compatibility_id: COHORT.into(),
        seq: EMBEDDED_SEQ,
        channel: "release".into(),
        trust: if keys.is_empty() {
            exact_update::Trust::Development
        } else {
            exact_update::Trust::Production
        },
        verification_keys: keys.iter().map(|(id, k)| ((*id).into(), *k)).collect(),
        embedded_plan_sha256: None,
        embedded_assets: None,
        entry_digest: None,
    }
}

pub(crate) fn open(temp: &Temp) -> Store {
    Store::open(temp.path(), embedded(&[])).unwrap()
}

pub(crate) fn entry_names(temp: &Temp) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(temp.path().join("entries"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A boot that never reaches first pixel: open, count it, drop the store.
pub(crate) fn boot_and_die(temp: &Temp) -> Option<String> {
    let mut store = open(temp);
    let selected = store.select().entry;
    store.boot_started().unwrap();
    selected
}
