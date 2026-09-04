//! The update store, driven: a temporary directory and a fetch closure over an
//! in-memory origin. No network, no host, no sockets.
//!
//! @ref LLP 1026 D9–D12 / LLP 1030 D3a, D5, D9 / LLP 1030.000 §4 stage 4

use ed25519_dalek::{Signer, SigningKey};
use exact_update::{canonical_bytes, sha256_hex, Check, Embedded, Store};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const ORIGIN: &str = "https://cdn.example/apps/caltrain";
const APP: &str = "com.exact.caltrain";
const COHORT: &str = "9a1f3c7e5b2d4086";
const EMBEDDED_SEQ: u64 = 3;

// ---------------------------------------------------------------- the origin

/// One published bundle, as `exact deploy` would write it (LLP 1030.000 D3).
struct Bundle {
    app: String,
    cohort: String,
    channel: String,
    seq: u64,
    plan: Vec<u8>,
    assets: Vec<(String, Vec<u8>)>,
    sunset: Option<(String, Option<String>)>,
    signer: Option<(String, SigningKey)>,
    /// Serve a plan whose bytes are not the ones the head names.
    tamper_plan: bool,
}

impl Bundle {
    fn new(seq: u64, plan: &[u8]) -> Bundle {
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

    fn asset(mut self, name: &str, bytes: &[u8]) -> Bundle {
        self.assets.push((name.into(), bytes.to_vec()));
        self
    }

    /// The head as JSON, and the files the origin serves, by URL.
    fn publish(&self) -> (String, HashMap<String, Vec<u8>>) {
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
        let base = format!("{ORIGIN}/.exact/{}", self.cohort);
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
struct Origin {
    files: HashMap<String, Vec<u8>>,
    asked: Vec<String>,
}

impl Origin {
    fn of(bundle: &Bundle) -> Origin {
        Origin {
            files: bundle.publish().1,
            asked: Vec::new(),
        }
    }

    fn serving(&mut self, bundle: &Bundle) {
        self.files.extend(bundle.publish().1);
    }

    fn check(&mut self, store: &mut Store) -> Result<Check, String> {
        self.asked.clear();
        let files = &self.files;
        let asked = &mut self.asked;
        let mut fetch = move |url: &str| -> Result<Vec<u8>, String> {
            asked.push(url.to_string());
            files.get(url).cloned().ok_or_else(|| format!("404 {url}"))
        };
        store.check(ORIGIN, &mut fetch)
    }
}

fn base64(bytes: &[u8]) -> String {
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
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Temp {
        let dir = std::env::temp_dir().join(format!("exact-update-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn embedded(keys: &[(&str, [u8; 32])]) -> Embedded {
    Embedded {
        app_id: APP.into(),
        compatibility_id: COHORT.into(),
        seq: EMBEDDED_SEQ,
        verification_keys: keys.iter().map(|(id, k)| ((*id).into(), *k)).collect(),
        embedded_digest: None,
    }
}

fn open(temp: &Temp) -> Store {
    Store::open(temp.path(), embedded(&[])).unwrap()
}

fn entry_names(temp: &Temp) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(temp.path().join("entries"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A boot that never reaches first pixel: open, count it, drop the store.
fn boot_and_die(temp: &Temp) -> Option<String> {
    let mut store = open(temp);
    let selected = store.select().entry;
    store.boot_started().unwrap();
    selected
}

// ------------------------------------------------------------------- selection

#[test]
fn a_fresh_store_selects_entry_zero() {
    let temp = Temp::new("fresh");
    let store = open(&temp);
    let selection = store.select();
    assert_eq!(selection.plan, None);
    assert_eq!(selection.assets_dir, None);
    assert_eq!(selection.entry, None);
    assert_eq!(selection.seq, EMBEDDED_SEQ);
    let status = store.status();
    assert_eq!(status.stream, "embedded");
    assert_eq!(status.selected_seq, EMBEDDED_SEQ);
    assert_eq!(status.embedded_seq, EMBEDDED_SEQ);
    assert!(!status.staged);
    assert_eq!(status.entry, None);
    assert!(temp.path().join("entries").is_dir());
}

#[test]
fn a_valid_head_is_staged_and_selected_at_the_next_launch() {
    let temp = Temp::new("staged");
    let bundle = Bundle::new(4, b"plan four").asset("mark.png", b"a mark");
    let mut origin = Origin::of(&bundle);
    let mut store = open(&temp);
    let Ok(Check::Staged { entry, seq, .. }) = origin.check(&mut store) else {
        panic!("the head should have staged");
    };
    assert_eq!(seq, 4);
    let dir = temp.path().join("entries").join(&entry);
    assert_eq!(std::fs::read(dir.join("app.plan")).unwrap(), b"plan four");
    assert_eq!(
        std::fs::read(dir.join("assets/mark.png")).unwrap(),
        b"a mark"
    );
    assert!(dir.join("exact.json").is_file());
    // Staged, not running: this launch booted entry zero.
    assert_eq!(store.staged().map(|s| s.entry), Some(entry.clone()));
    assert_eq!(store.status().entry, None);

    let next = open(&temp);
    let selection = next.select();
    assert_eq!(selection.entry, Some(entry.clone()));
    assert_eq!(selection.plan, Some(dir.join("app.plan")));
    assert_eq!(selection.assets_dir, Some(dir.join("assets")));
    assert_eq!(selection.seq, 4);
    assert!(next.staged().is_none());
    assert_eq!(next.status().stream, format!("release/{COHORT}"));
    assert_eq!(next.status().entry, Some(entry));
}

#[test]
fn the_same_head_again_is_current() {
    let temp = Temp::new("current");
    let bundle = Bundle::new(4, b"plan four");
    let mut origin = Origin::of(&bundle);
    let mut store = open(&temp);
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));

    let mut next = open(&temp);
    assert!(matches!(
        origin.check(&mut next),
        Ok(Check::Current { sunset: None })
    ));
    assert_eq!(origin.asked.len(), 1, "only the head is fetched");
    assert_eq!(entry_names(&temp).len(), 1);
    assert!(next.staged().is_none());
}

// ------------------------------------------------------------------- refusals

#[test]
fn a_lower_seq_is_refused() {
    let temp = Temp::new("rollback");
    let mut origin = Origin::of(&Bundle::new(5, b"plan five"));
    let mut store = open(&temp);
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));

    let older = Bundle::new(4, b"plan four");
    origin.serving(&older);
    let mut next = open(&temp);
    let refusal = origin.check(&mut next).unwrap_err();
    assert!(
        refusal.contains("below the selected seq 5"),
        "unexpected refusal: {refusal}"
    );
    assert_eq!(origin.asked.len(), 1, "nothing was downloaded");
    assert_eq!(next.select().seq, 5);
}

#[test]
fn another_compatibility_id_is_refused_before_any_download() {
    let temp = Temp::new("cohort");
    let mut other = Bundle::new(4, b"plan four");
    other.cohort = "0000000000000000".into();
    // Served at *this* cohort's path: a misrouted or replayed head.
    let (text, _) = other.publish();
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    origin.files.insert(
        format!("{ORIGIN}/.exact/{COHORT}/exact.json"),
        text.into_bytes(),
    );
    let mut store = open(&temp);
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("cohort 0000000000000000"),
        "unexpected refusal: {refusal}"
    );
    assert_eq!(origin.asked.len(), 1, "nothing was downloaded");
    assert!(entry_names(&temp).is_empty());
}

#[test]
fn another_app_id_is_refused() {
    let temp = Temp::new("app");
    let mut other = Bundle::new(4, b"plan four");
    other.app = "com.exact.weird-castle".into();
    let mut origin = Origin::of(&other);
    let mut store = open(&temp);
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("com.exact.weird-castle"),
        "unexpected refusal: {refusal}"
    );
    assert!(entry_names(&temp).is_empty());
}

#[test]
fn a_tampered_plan_is_refused_and_nothing_is_written() {
    let temp = Temp::new("tamper");
    let mut bundle = Bundle::new(4, b"plan four");
    bundle.tamper_plan = true;
    let mut origin = Origin::of(&bundle);
    let mut store = open(&temp);
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("app.plan hashes to"),
        "unexpected refusal: {refusal}"
    );
    assert!(
        entry_names(&temp).is_empty(),
        "no entry, and no half of one: {:?}",
        entry_names(&temp)
    );
    assert_eq!(store.select().entry, None);
    assert_eq!(open(&temp).select().entry, None);

    // A body of another length is refused before it is even hashed.
    origin.files.insert(
        format!("{ORIGIN}/.exact/{COHORT}/app.plan"),
        b"a much longer plan than the head declared".to_vec(),
    );
    let mut store = open(&temp);
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("the head declared 9"),
        "unexpected refusal: {refusal}"
    );
    assert!(entry_names(&temp).is_empty());
}

#[test]
fn a_head_over_64_kb_is_refused() {
    let temp = Temp::new("huge");
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    origin.files.insert(
        format!("{ORIGIN}/.exact/{COHORT}/exact.json"),
        vec![b' '; 64 * 1024 + 1],
    );
    let mut store = open(&temp);
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(refusal.contains("the most is"), "unexpected: {refusal}");
}

// ------------------------------------------------------------------ signatures

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

#[test]
fn a_signed_head_verifies_with_the_right_key_and_is_refused_with_the_wrong_one() {
    let signing = key(7);
    let mut bundle = Bundle::new(4, b"plan four");
    bundle.signer = Some(("k1".into(), signing.clone()));
    let mut origin = Origin::of(&bundle);

    let good = Temp::new("signed-good");
    let mut store = Store::open(
        good.path(),
        embedded(&[("k1", signing.verifying_key().to_bytes())]),
    )
    .unwrap();
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));

    // The same key id, another key: the signature does not verify.
    let wrong = Temp::new("signed-wrong");
    let mut store = Store::open(
        wrong.path(),
        embedded(&[("k1", key(9).verifying_key().to_bytes())]),
    )
    .unwrap();
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(refusal.contains("does not verify"), "unexpected: {refusal}");
    assert!(entry_names(&wrong).is_empty());

    // A key id this binary does not carry at all.
    let unknown = Temp::new("signed-unknown");
    let mut store = Store::open(
        unknown.path(),
        embedded(&[("k2", signing.verifying_key().to_bytes())]),
    )
    .unwrap();
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("signed by k1, which this binary does not carry"),
        "unexpected: {refusal}"
    );
}

#[test]
fn an_unsigned_head_is_refused_with_keys_and_admitted_without_them() {
    let bundle = Bundle::new(4, b"plan four");
    let mut origin = Origin::of(&bundle);

    let release = Temp::new("unsigned-release");
    let mut store = Store::open(
        release.path(),
        embedded(&[("k1", key(7).verifying_key().to_bytes())]),
    )
    .unwrap();
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("unsigned and this binary carries keys"),
        "unexpected: {refusal}"
    );
    assert!(entry_names(&release).is_empty());

    let dev = Temp::new("unsigned-dev");
    let mut store = open(&dev);
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));
}

// -------------------------------------------------------------- crash recovery

#[test]
fn two_failed_boots_fall_back_to_the_last_good_entry_then_to_entry_zero() {
    let temp = Temp::new("crash");
    let good = Bundle::new(4, b"plan four");
    let mut origin = Origin::of(&good);
    let mut store = open(&temp);
    let Ok(Check::Staged { entry: first, .. }) = origin.check(&mut store) else {
        panic!("the first head should have staged");
    };

    // A launch that reaches first pixel: this entry is the last good one.
    let mut store = open(&temp);
    assert_eq!(store.select().entry.as_deref(), Some(first.as_str()));
    store.boot_started().unwrap();
    store.boot_succeeded().unwrap();

    let bad = Bundle::new(5, b"plan five");
    origin.serving(&bad);
    let Ok(Check::Staged { entry: second, .. }) = origin.check(&mut store) else {
        panic!("the second head should have staged");
    };

    // Two launches that never reach first pixel.
    assert_eq!(boot_and_die(&temp).as_deref(), Some(second.as_str()));
    assert_eq!(boot_and_die(&temp).as_deref(), Some(second.as_str()));
    let store = open(&temp);
    assert_eq!(
        store.select().entry.as_deref(),
        Some(first.as_str()),
        "the last good entry is selected"
    );

    // The same again, with nothing good left: entry zero.
    assert_eq!(boot_and_die(&temp).as_deref(), Some(first.as_str()));
    assert_eq!(boot_and_die(&temp).as_deref(), Some(first.as_str()));
    let store = open(&temp);
    assert_eq!(store.select().entry, None);
    assert_eq!(store.select().seq, EMBEDDED_SEQ);
    assert_eq!(store.status().stream, "embedded");

    // And a bundle demoted for failing twice is not staged again.
    let mut store = open(&temp);
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("failed to reach first pixel twice"),
        "unexpected: {refusal}"
    );
}

// ---------------------------------------------------------- assets and sunset

#[test]
fn an_asset_already_in_the_store_is_reused_by_digest() {
    let temp = Temp::new("assets");
    let first = Bundle::new(4, b"plan four").asset("mark.png", b"a mark");
    let mut origin = Origin::of(&first);
    let mut store = open(&temp);
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));
    assert_eq!(origin.asked.len(), 3, "head, plan, asset");

    let second = Bundle::new(5, b"plan five").asset("mark.png", b"a mark");
    origin.serving(&second);
    let mut store = open(&temp);
    let Ok(Check::Staged { entry, .. }) = origin.check(&mut store) else {
        panic!("the second head should have staged");
    };
    assert_eq!(
        origin.asked,
        vec![
            format!("{ORIGIN}/.exact/{COHORT}/exact.json"),
            format!("{ORIGIN}/.exact/{COHORT}/app.plan"),
        ],
        "the asset was reused, not fetched"
    );
    assert_eq!(
        std::fs::read(
            temp.path()
                .join("entries")
                .join(&entry)
                .join("assets/mark.png")
        )
        .unwrap(),
        b"a mark"
    );
}

#[test]
fn the_sunset_card_passes_through() {
    let temp = Temp::new("sunset");
    let mut bundle = Bundle::new(4, b"plan four");
    bundle.sunset = Some((
        "This version is retiring. Please update.".into(),
        Some("https://apps.example/caltrain".into()),
    ));
    let mut origin = Origin::of(&bundle);
    let mut store = open(&temp);
    let Ok(Check::Staged { sunset, .. }) = origin.check(&mut store) else {
        panic!("the head should have staged");
    };
    let card = sunset.expect("the card rides the head");
    assert_eq!(card.message, "This version is retiring. Please update.");
    assert_eq!(card.store.as_deref(), Some("https://apps.example/caltrain"));
    assert_eq!(store.status().sunset.as_ref(), Some(&card));

    // And again from the entry on disk, at the next launch and on a Current.
    let mut next = open(&temp);
    assert_eq!(next.status().sunset.as_ref(), Some(&card));
    let Ok(Check::Current { sunset }) = origin.check(&mut next) else {
        panic!("the same head should be current");
    };
    assert_eq!(sunset.as_ref(), Some(&card));
}

// ------------------------------------------------------------ codec and record

#[test]
fn an_unknown_record_codec_selects_entry_zero_and_leaves_the_record_alone() {
    let temp = Temp::new("codec");
    let record = temp.path().join("record.json");
    let foreign = b"{\"codec\":2,\"selected\":\"beef\",\"lastGood\":null,\"failures\":0}";
    std::fs::write(&record, foreign).unwrap();

    let mut store = Store::open(temp.path(), embedded(&[])).unwrap();
    assert!(store.frozen());
    assert_eq!(store.select().entry, None);
    assert_eq!(store.select().seq, EMBEDDED_SEQ);
    store.boot_started().unwrap();
    store.boot_succeeded().unwrap();
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(refusal.contains("newer binary"), "unexpected: {refusal}");
    assert_eq!(std::fs::read(&record).unwrap(), foreign);
}

#[test]
fn a_record_from_another_cohort_starts_this_one_at_entry_zero() {
    let temp = Temp::new("othercohort");
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    let mut store = open(&temp);
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));

    let mut moved = embedded(&[]);
    moved.compatibility_id = "1111111111111111".into();
    let store = Store::open(temp.path(), moved).unwrap();
    assert_eq!(
        store.select().entry,
        None,
        "another cohort's entry is not selectable"
    );
    assert_eq!(store.status().stream, "embedded");
}

#[test]
fn activate_hands_over_the_staged_plan_and_status_follows_each_step() {
    let temp = Temp::new("activate");
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    let mut store = open(&temp);
    store.boot_started().unwrap();
    store.boot_succeeded().unwrap();
    assert!(store.activate().is_none(), "nothing is staged yet");

    let Ok(Check::Staged { entry, seq, .. }) = origin.check(&mut store) else {
        panic!("the head should have staged");
    };
    let status = store.status();
    assert!(status.staged);
    assert_eq!(status.entry, None, "entry zero is still running");
    assert_eq!(status.selected_seq, 4);
    assert_eq!(status.embedded_seq, EMBEDDED_SEQ);
    assert_eq!(status.stream, format!("release/{COHORT}"));
    let staged = store.staged().expect("staged");
    assert_eq!(staged.entry, entry);
    assert_eq!(staged.seq, seq);

    assert_eq!(store.activate(), Some(b"plan four".to_vec()));
    let status = store.status();
    assert!(!status.staged);
    assert_eq!(status.entry, Some(entry));
    assert!(store.activate().is_none(), "activated once");
}

// -------------------------------------------------------------- canonical bytes

#[test]
fn canonical_bytes_sorts_keys_drops_the_signature_and_refuses_a_float() {
    let bytes = canonical_bytes(
        r#"{ "b": [3, {"z": 1, "a": 2}], "a": "x", "signature": {"keyId": "k1"} }"#,
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        r#"{"a":"x","b":[3,{"a":2,"z":1}]}"#
    );
    let float = canonical_bytes(r#"{"seq": 1.5}"#).unwrap_err();
    assert!(float.contains("non-integer"), "unexpected: {float}");
    assert!(canonical_bytes("not json").is_err());
    assert!(
        canonical_bytes("[1,2]").is_err(),
        "the envelope is an object"
    );
}

#[test]
fn the_canonical_bytes_are_json_stringify_over_recursively_sorted_keys() {
    // `expected` is what node printed for `JSON.stringify(sortKeysDeep(head))`
    // with `signature` deleted — the publisher is a Node script, and this is
    // the agreement, byte for byte: sorted keys, no whitespace, `"` `\` and the
    // C0 controls escaped and nothing else, non-ASCII literal.
    let head = r#"{"exact":1,"signature":{"keyId":"k1","ed25519":"AA=="},"app":{"name":"Weird Castle é—ü","id":"com.exact.weird-castle"},"plan":{"url":"./app.plan","bytes":12580,"sha256":"aa","formatVersion":4},"assets":[{"name":"a/b \"q\" \\ \u0001.png","bytes":0}],"stream":{"seq":41,"channel":"release","compatibilityId":"9a1f","app":"com.exact.weird-castle"},"sunset":{"message":"Retiring — update.","store":null}}"#;
    let expected = r#"{"app":{"id":"com.exact.weird-castle","name":"Weird Castle é—ü"},"assets":[{"bytes":0,"name":"a/b \"q\" \\ \u0001.png"}],"exact":1,"plan":{"bytes":12580,"formatVersion":4,"sha256":"aa","url":"./app.plan"},"stream":{"app":"com.exact.weird-castle","channel":"release","compatibilityId":"9a1f","seq":41},"sunset":{"message":"Retiring — update.","store":null}}"#;
    assert_eq!(
        String::from_utf8(canonical_bytes(head).unwrap()).unwrap(),
        expected
    );
}

#[test]
fn a_head_the_binary_already_embeds_is_current_and_downloads_nothing() {
    let temp = Temp::new("embedded-digest");
    let bundle = Bundle::new(EMBEDDED_SEQ, b"plan three");
    let (text, _) = bundle.publish();
    let mut origin = Origin::of(&bundle);
    let mut carried = embedded(&[]);
    carried.embedded_digest = Some(sha256_hex(text.as_bytes()));
    let mut store = Store::open(temp.path(), carried).unwrap();
    assert!(matches!(
        origin.check(&mut store),
        Ok(Check::Current { sunset: None })
    ));
    assert_eq!(origin.asked.len(), 1, "only the head is fetched");
    assert!(
        entry_names(&temp).is_empty(),
        "entry zero is not in the store"
    );
}

#[test]
fn the_canonical_bytes_are_what_a_head_is_signed_over() {
    // The publisher signs canonical bytes; a head that differs only in
    // whitespace and key order still verifies, and one whose seq was edited
    // does not.
    let signing = key(11);
    let mut bundle = Bundle::new(4, b"plan four");
    bundle.signer = Some(("k1".into(), signing.clone()));
    let (text, _) = bundle.publish();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let shuffled = serde_json::to_string_pretty(&value).unwrap();

    let temp = Temp::new("canonical-head");
    let mut origin = Origin::of(&bundle);
    origin.files.insert(
        format!("{ORIGIN}/.exact/{COHORT}/exact.json"),
        shuffled.into_bytes(),
    );
    let mut store = Store::open(
        temp.path(),
        embedded(&[("k1", signing.verifying_key().to_bytes())]),
    )
    .unwrap();
    assert!(
        matches!(origin.check(&mut store), Ok(Check::Staged { .. })),
        "whitespace and key order are not what is signed"
    );

    let edited = text.replace("\"seq\":4", "\"seq\":9");
    assert_ne!(edited, text);
    let temp = Temp::new("canonical-edited");
    origin.files.insert(
        format!("{ORIGIN}/.exact/{COHORT}/exact.json"),
        edited.into_bytes(),
    );
    let mut store = Store::open(
        temp.path(),
        embedded(&[("k1", signing.verifying_key().to_bytes())]),
    )
    .unwrap();
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(refusal.contains("does not verify"), "unexpected: {refusal}");
}
