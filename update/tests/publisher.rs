//! The cross-language proof `exact deploy` rests on: a head that
//! `scripts/deploy.mjs` produced — Caltrain's ios stream at `seq` 1, signed in
//! Node with `crypto.sign(null, canonicalBytes, key)` over the direct
//! recursively key-sorted serialization — is read and verified here, byte for
//! byte, by the client's own reader, and the whole published stream is staged
//! by the store.
//!
//! @ref LLP 1026 D11 (the signature over the canonical bytes) / LLP 1030 D3a
//! (the head binds the stream) / LLP 1030.000 D3 item 5, §4 stage 4 (the
//! publisher)
//!
//! `fixtures/publisher/` is the stream directory as the publisher wrote it
//! (`node scripts/deploy.mjs caltrain --yes --origin <dir>` with a throwaway
//! key, whose public half is `caltrain-2026.pub` in the manifest's base64
//! form): `exact.json`, `app.plan`, `assets/<name>`; plus `canonical.bin`,
//! the bytes Node signed, written by the same `canonicalBytes`. The private
//! key was discarded; the manifest's real key is Charlie's, and the release
//! record was left out (it is the publisher's, not the client's). The head's
//! cards point at the origin-wide immutable blob tree, as a live publisher's
//! stream does.

use exact_update::{canonical_bytes, Check, Embedded, Envelope, Store};
use std::path::{Path, PathBuf};

const HEAD: &[u8] = include_bytes!("fixtures/publisher/exact.json");
const CANONICAL: &str = include_str!("fixtures/publisher/canonical.bin");
const PUBLIC_KEY: &str = include_str!("fixtures/publisher/caltrain-2026.pub");
const KEY_ID: &str = "caltrain-2026";
const APP: &str = "com.exact.caltrain";

/// The manifest's base64 of the 32 raw key bytes, decoded.
fn key() -> [u8; 32] {
    fn sextet(b: u8) -> u8 {
        match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("not base64: {b}"),
        }
    }
    let text = PUBLIC_KEY.trim().as_bytes();
    let mut out = Vec::new();
    for chunk in text.chunks(4) {
        let padding = chunk.iter().filter(|b| **b == b'=').count();
        let mut acc: u32 = 0;
        for b in chunk {
            acc = (acc << 6) | u32::from(if *b == b'=' { 0 } else { sextet(*b) });
        }
        out.extend_from_slice(&acc.to_be_bytes()[1..4 - padding]);
    }
    out.as_slice().try_into().expect("32 raw Ed25519 bytes")
}

fn keys() -> Vec<(String, [u8; 32])> {
    vec![(KEY_ID.into(), key())]
}

fn text() -> &'static str {
    std::str::from_utf8(HEAD).unwrap()
}

/// A store directory that removes itself.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Temp {
        let dir =
            std::env::temp_dir().join(format!("exact-publisher-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_canonical_bytes_agree_with_node_byte_for_byte() {
    assert_eq!(
        canonical_bytes(text()).unwrap(),
        CANONICAL.trim_end().as_bytes()
    );
    // And they are what the rule says: no whitespace, sorted keys, the
    // signature gone.
    let canonical = CANONICAL.trim_end();
    assert!(canonical.starts_with("{\"app\":{\"id\":\"com.exact.caltrain\""));
    assert!(!canonical.contains("signature"));
    assert!(!canonical.contains(": "));
}

#[test]
fn a_head_the_node_publisher_signed_verifies_with_the_manifests_key() {
    let head = Envelope::parse(HEAD).unwrap();
    assert_eq!(head.app_id, APP);
    assert_eq!(head.app_name.as_deref(), Some("Caltrain"));
    assert_eq!(head.stream.app.as_deref(), Some(APP));
    assert_eq!(head.stream.channel, "prod");
    assert_eq!(head.stream.seq, 1);
    assert_eq!(head.stream.compatibility_id.len(), 32);
    assert_eq!(head.plan.url, format!("../../blobs/{}", head.plan.sha256));
    assert_eq!(head.format_version, Some(4));
    assert_eq!(head.kernel_schema.as_deref().map(str::len), Some(16));
    assert_eq!(head.assets.len(), 6);
    let png = head
        .assets
        .iter()
        .find(|a| a.name == "assets/caltrain.png")
        .expect("the icon is an asset");
    assert_eq!(png.url, format!("../../blobs/{}", png.sha256));
    assert_eq!(png.bytes, 699);
    assert!(head.release.as_deref().unwrap().starts_with("r-"));
    assert_eq!(head.sunset, None);
    assert_eq!(head.signature.as_ref().unwrap().key_id, KEY_ID);

    head.verify(&keys()).unwrap();
    // A missing trust root never authenticates a signed publisher head.
    assert!(head.verify(&[]).is_err());
    // The same key under another id is an unknown signer.
    let err = head.verify(&[("other".into(), key())]).unwrap_err();
    assert!(err.contains("does not carry"), "{err}");
    // Another key under the right id does not verify.
    let mut other = key();
    other[0] ^= 0x01;
    assert!(head.verify(&[(KEY_ID.into(), other)]).is_err());
}

#[test]
fn one_byte_changed_anywhere_is_refused() {
    // The seq: same length, still parses, no longer signed.
    let bumped = text().replacen("\"seq\":1", "\"seq\":2", 1);
    assert_ne!(bumped, text());
    let head = Envelope::parse(bumped.as_bytes()).unwrap();
    assert_eq!(head.stream.seq, 2);
    let err = head.verify(&keys()).unwrap_err();
    assert!(err.contains("does not verify"), "{err}");

    // A digest: one hex digit of the plan's.
    let plan = Envelope::parse(HEAD).unwrap().plan.sha256;
    let mut swapped = plan.clone().into_bytes();
    swapped[0] = if swapped[0] == b'0' { b'1' } else { b'0' };
    let forged = text().replacen(&plan, std::str::from_utf8(&swapped).unwrap(), 1);
    let err = Envelope::parse(forged.as_bytes())
        .unwrap()
        .verify(&keys())
        .unwrap_err();
    assert!(err.contains("does not verify"), "{err}");

    // The channel: a head replayed onto another stream (LLP 1030 D3a).
    let replayed = text().replacen("\"channel\":\"prod\"", "\"channel\":\"beta\"", 1);
    let err = Envelope::parse(replayed.as_bytes())
        .unwrap()
        .verify(&keys())
        .unwrap_err();
    assert!(err.contains("does not verify"), "{err}");

    // A byte of the signature itself.
    let at = text().find("\"ed25519\":\"").unwrap() + "\"ed25519\":\"".len();
    let mut raw = HEAD.to_vec();
    raw[at] = if raw[at] == b'A' { b'B' } else { b'A' };
    let err = Envelope::parse(&raw).unwrap().verify(&keys()).unwrap_err();
    assert!(err.contains("does not verify"), "{err}");
}

#[test]
fn the_published_stream_is_staged_whole_by_a_client() {
    let temp = Temp::new("stream");
    let head = Envelope::parse(HEAD).unwrap();
    let mut store = Store::open(
        &temp.0,
        Embedded {
            app_id: APP.into(),
            compatibility_id: head.stream.compatibility_id.clone(),
            seq: 0,
            channel: head.stream.channel.clone(),
            verification_keys: keys(),
            trust: exact_update::Trust::Production,
            embedded_plan_sha256: None,
        },
    )
    .unwrap();
    // The store asks for `<origin>/.exact/<channel>/<compatibilityId>/exact.json`
    // (`head_url`) and resolves each card's url against it — the layout the
    // publisher writes (LLP 1030.000 D7: a channel is a prefix). Payload cards
    // resolve out to `.exact/blobs/<sha256>` and this closure maps those
    // content-addressed URLs back to the fixture bytes.
    let origin = "https://caltrain.exact.invalid";
    let head_url =
        exact_update::head_url(origin, &head.stream.channel, &head.stream.compatibility_id);
    let prefix = format!(
        "{origin}/.exact/{}/{}/",
        head.stream.channel, head.stream.compatibility_id
    );
    let blob_prefix = format!("{origin}/.exact/blobs/");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/publisher");
    let asked = std::cell::RefCell::new(Vec::new());
    let mut fetch = |url: &str| -> Result<Vec<u8>, String> {
        asked.borrow_mut().push(url.to_string());
        if url == head_url {
            return Ok(HEAD.to_vec());
        }
        let digest = url
            .strip_prefix(&blob_prefix)
            .ok_or_else(|| format!("{url} is not under the blob tree"))?;
        let path = if digest == head.plan.sha256 {
            fixture.join("app.plan")
        } else {
            let card = head
                .assets
                .iter()
                .find(|card| card.sha256 == digest)
                .ok_or_else(|| format!("{url} names no card"))?;
            fixture.join("assets").join(&card.name)
        };
        std::fs::read(path).map_err(|e| format!("{url}: {e}"))
    };
    let Ok(Check::Staged { entry, seq, sunset }) =
        store.check(&head_url, &mut fetch, &mut |_| None)
    else {
        panic!("the published stream should have staged");
    };
    assert_eq!(seq, 1);
    assert_eq!(sunset, None);
    assert_eq!(
        entry, head.digest,
        "the entry is named by the head's digest"
    );
    // The head, the plan, and every asset were fetched by the head's urls.
    let urls = asked.borrow().clone();
    assert_eq!(urls.len(), 8, "{urls:?}");
    assert_eq!(urls[0], format!("{prefix}exact.json"));
    assert!(urls.contains(&format!("{blob_prefix}{}", head.plan.sha256)));
    assert!(urls.contains(&format!(
        "{blob_prefix}{}",
        head.assets
            .iter()
            .find(|card| card.name == "assets/caltrain.png")
            .unwrap()
            .sha256
    )));
    assert!(urls.iter().any(|url| url
        == &format!(
            "{blob_prefix}{}",
            head.assets
                .iter()
                .find(|card| card.name == "shaders/aurora.wgsl")
                .unwrap()
                .sha256
        )));
    let dir = temp.0.join("entries").join(&entry);
    assert_eq!(std::fs::read(dir.join("exact.json")).unwrap(), HEAD);
    assert_eq!(
        std::fs::read(dir.join("app.plan")).unwrap(),
        std::fs::read(fixture.join("app.plan")).unwrap()
    );
    assert!(dir.join("assets/assets/caltrain.png").is_file());
    assert!(dir.join("assets/deck/index.html").is_file());
    assert_eq!(store.staged().map(|s| s.seq), Some(1));

    // The next launch selects it; the same head is then Current.
    let mut next = Store::open(
        &temp.0,
        Embedded {
            app_id: APP.into(),
            compatibility_id: head.stream.compatibility_id.clone(),
            seq: 0,
            channel: head.stream.channel.clone(),
            verification_keys: keys(),
            trust: exact_update::Trust::Production,
            embedded_plan_sha256: None,
        },
    )
    .unwrap();
    assert_eq!(next.select().seq, 1);
    assert!(matches!(
        next.check(&head_url, &mut fetch, &mut |_| None),
        Ok(Check::Current { sunset: None })
    ));
}
