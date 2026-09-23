//! The update store, driven: a temporary directory and a fetch closure over an
//! in-memory origin. No network, no host, no sockets.
//!
//! @ref LLP 1026 D9–D12 / LLP 1030 D3a, D5, D9 / LLP 1030.000 §4 stage 4

use crate::support::*;
use ed25519_dalek::SigningKey;
use exact_update::{canonical_bytes, sha256_hex, Check, Envelope, Store};

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
fn a_selected_plan_changed_after_staging_is_refused_before_it_counts_or_boots() {
    let temp = Temp::new("selected-plan-corrupt");
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    let mut store = open(&temp);
    let Ok(Check::Staged { entry, .. }) = origin.check(&mut store) else {
        panic!("the head should have staged");
    };
    std::fs::write(
        temp.path().join("entries").join(&entry).join("app.plan"),
        b"PLAN FOUR",
    )
    .unwrap();

    let mut launch = open(&temp);
    let refusal = launch.prepare_selected().unwrap_err();
    assert_eq!(refusal.entry, entry);
    assert!(
        refusal.reason.contains("hashes to"),
        "unexpected refusal: {}",
        refusal.reason
    );
    assert_eq!(refusal.status.entry, None);
    assert_eq!(refusal.status.stream, "embedded");
    assert_eq!(refusal.status.running_seq, EMBEDDED_SEQ);
    assert_eq!(refusal.status.selected_seq, EMBEDDED_SEQ);
    launch.boot_started().unwrap();
    launch.boot_succeeded(&launch.generation()).unwrap();

    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(temp.path().join("record.json")).unwrap())
            .unwrap();
    assert_eq!(record["selected"], serde_json::Value::Null);
    assert_eq!(record["lastGood"], serde_json::Value::Null);
    assert_eq!(record["failures"], 0);
    assert_eq!(record["stream"]["seq"], 4, "the rollback floor remains");
}

#[test]
fn selected_assets_are_a_complete_lazy_verified_generation() {
    let temp = Temp::new("selected-assets");
    let bundle = Bundle::new(4, b"plan four")
        .asset("icons/mark.png", b"a mark")
        .asset("unused.png", b"not read yet");
    let mut origin = Origin::of(&bundle);
    let mut store = open(&temp);
    let Ok(Check::Staged { entry, .. }) = origin.check(&mut store) else {
        panic!("the head should have staged");
    };

    let mut launch = open(&temp);
    let entry_dir = temp.path().join("entries").join(&entry);
    std::fs::remove_file(entry_dir.join("exact.json")).unwrap();
    let prepared = launch
        .prepare_selected()
        .unwrap()
        .expect("the update generation");
    assert_eq!(prepared.generation.entry.as_deref(), Some(entry.as_str()));
    assert_eq!(prepared.generation.seq, 4);
    assert_eq!(&*prepared.plan, b"plan four");
    assert_eq!(
        prepared.assets.names(),
        vec!["icons/mark.png".to_string(), "unused.png".to_string()]
    );
    assert_eq!(prepared.assets.generation(), &prepared.generation);
    std::fs::remove_file(entry_dir.join("app.plan")).unwrap();
    assert_eq!(
        &*prepared.plan, b"plan four",
        "the authenticated envelope and verified plan are not read twice"
    );
    assert!(
        prepared.assets.resolve("removed.png").unwrap().is_none(),
        "absence from the complete roster is a tombstone"
    );

    let first = prepared
        .assets
        .resolve("icons/mark.png")
        .unwrap()
        .expect("declared asset");
    assert_eq!(&*first, b"a mark");
    let asset_path = entry_dir.join("assets/icons/mark.png");
    std::fs::write(&asset_path, b"changed").unwrap();
    let again = prepared
        .assets
        .clone()
        .resolve("icons/mark.png")
        .unwrap()
        .expect("the verified bytes stay pinned");
    assert!(
        std::sync::Arc::ptr_eq(&first, &again),
        "a clean asset is read and verified only once across pinned clones"
    );
    assert_eq!(&*again, b"a mark");

    let unused = entry_dir.join("assets/unused.png");
    std::fs::remove_file(&unused).unwrap();
    let reason = prepared.assets.resolve("unused.png").unwrap_err();
    assert!(
        reason.contains("not a regular file"),
        "unexpected: {reason}"
    );
    std::fs::write(&unused, b"not read yet").unwrap();
    assert_eq!(
        prepared.assets.resolve("unused.png").unwrap_err(),
        reason,
        "a terminal integrity refusal is cached across clones too"
    );

    let refusal = launch.refuse_prepared(&prepared.generation, reason);
    assert_eq!(refusal.status.entry, None);
    assert_eq!(refusal.status.stream, "embedded");
    assert_eq!(launch.select().entry, None);
    launch.boot_started().unwrap();
    launch.boot_succeeded(&launch.generation()).unwrap();
    let record = std::fs::read_to_string(temp.path().join("record.json")).unwrap();
    assert!(record.contains("\"failures\":0"));
}

#[test]
fn a_selected_asset_changed_before_first_resolution_is_refused_by_its_card() {
    let temp = Temp::new("selected-asset-corrupt");
    let bundle = Bundle::new(4, b"plan four").asset("mark.png", b"a mark");
    let mut origin = Origin::of(&bundle);
    let mut store = open(&temp);
    let Ok(Check::Staged { entry, .. }) = origin.check(&mut store) else {
        panic!("the head should have staged");
    };
    let asset = temp
        .path()
        .join("entries")
        .join(entry)
        .join("assets/mark.png");
    std::fs::write(asset, b"b mark").unwrap();

    let mut launch = open(&temp);
    let prepared = launch.prepare_selected().unwrap().unwrap();
    let refusal = prepared.assets.resolve("mark.png").unwrap_err();
    assert!(refusal.contains("hashes to"), "unexpected: {refusal}");
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

#[test]
fn a_refused_selection_reports_entry_zero_as_the_running_generation() {
    let temp = Temp::new("refused-running");
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    let mut store = open(&temp);
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));

    let mut launch = open(&temp);
    launch.boot_started().unwrap();
    launch.entry_refused();
    let status = launch.status();
    assert_eq!(status.entry, None);
    assert_eq!(status.stream, "embedded");
    assert_eq!(status.running_seq, EMBEDDED_SEQ);
    assert_eq!(status.selected_seq, 4, "the durable selection still failed");

    let record = std::fs::read_to_string(temp.path().join("record.json")).unwrap();
    assert!(
        record.contains("\"failures\":1"),
        "the refusal is still counted"
    );
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
        refusal.contains("below the accepted seq 5"),
        "unexpected refusal: {refusal}"
    );
    assert_eq!(origin.asked.len(), 1, "nothing was downloaded");
    assert_eq!(next.select().seq, 5);
}

#[test]
fn the_embedded_and_observed_sequences_remain_rollback_floors() {
    let fresh = Temp::new("embedded-floor");
    let mut origin = Origin::of(&Bundle::new(2, b"plan two"));
    let mut store = open(&fresh);
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("below the accepted seq 3"),
        "the embedded generation is the first floor: {refusal}"
    );

    let temp = Temp::new("observed-floor");
    origin.serving(&Bundle::new(5, b"plan five"));
    let mut store = open(&temp);
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));
    boot_and_die(&temp);
    boot_and_die(&temp);
    assert_eq!(open(&temp).select().entry, None, "the bad update demoted");

    origin.serving(&Bundle::new(4, b"plan four"));
    let mut fallback = open(&temp);
    let refusal = origin.check(&mut fallback).unwrap_err();
    assert!(
        refusal.contains("below the accepted seq 5"),
        "fallback must not lower the observed floor: {refusal}"
    );
}

#[test]
fn a_signed_sequence_cannot_name_two_bundles() {
    let signing = key(13);
    let keys = [("k1", signing.verifying_key().to_bytes())];
    let temp = Temp::new("seq-equivocation");
    let mut first = Bundle::new(4, b"plan four");
    first.signer = Some(("k1".into(), signing.clone()));
    let mut origin = Origin::of(&first);
    let mut store = Store::open(temp.path(), embedded(&keys)).unwrap();
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));

    let mut equivocation = Bundle::new(4, b"another plan");
    equivocation.signer = Some(("k1".into(), signing.clone()));
    origin.serving(&equivocation);
    let mut next = Store::open(temp.path(), embedded(&keys)).unwrap();
    let refusal = origin.check(&mut next).unwrap_err();
    assert!(
        refusal.contains("used sequence cannot equivocate"),
        "a valid signature cannot reuse an observed sequence: {refusal}"
    );
    assert_eq!(origin.asked.len(), 1, "no equivocating payload was fetched");

    equivocation.seq = 5;
    origin.serving(&equivocation);
    assert!(
        matches!(origin.check(&mut next), Ok(Check::Staged { seq: 5, .. })),
        "the same valid content at a genuinely higher sequence is admissible"
    );
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
        format!("{ORIGIN}/.exact/release/{COHORT}/exact.json"),
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
        format!("{ORIGIN}/.exact/release/{COHORT}/app.plan"),
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
        format!("{ORIGIN}/.exact/release/{COHORT}/exact.json"),
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
fn an_unsigned_head_requires_explicit_development_policy() {
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

#[test]
fn a_production_store_with_no_keys_refuses_unsigned_and_signed_heads() {
    for signed in [false, true] {
        let temp = Temp::new(if signed {
            "no-keys-signed"
        } else {
            "no-keys-unsigned"
        });
        let mut bundle = Bundle::new(4, b"plan four");
        if signed {
            bundle.signer = Some(("k1".into(), key(7)));
        }
        let mut origin = Origin::of(&bundle);
        let mut facts = embedded(&[]);
        facts.trust = exact_update::Trust::Production;
        let mut store = Store::open(temp.path(), facts).unwrap();
        let refusal = origin.check(&mut store).unwrap_err();
        assert!(refusal.contains("no verification keys"), "{refusal}");
        assert_eq!(origin.asked.len(), 1, "no payloads fetched");
        assert!(entry_names(&temp).is_empty());
        assert!(store.select().entry.is_none());
    }
}

#[test]
fn development_does_not_ignore_a_supplied_unverifiable_signature() {
    let temp = Temp::new("dev-bad-signature");
    let mut bundle = Bundle::new(4, b"plan four");
    bundle.signer = Some(("k1".into(), key(7)));
    let mut origin = Origin::of(&bundle);
    let mut store = open(&temp);
    assert!(origin.check(&mut store).is_err());
    assert!(entry_names(&temp).is_empty());
    let mut facts = embedded(&[("k1", key(7).verifying_key().to_bytes())]);
    facts.trust = exact_update::Trust::Development;
    let mut store = Store::open(temp.path(), facts).unwrap();
    origin.serving(&Bundle::new(4, b"unsigned local plan"));
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
    store.boot_succeeded(&store.generation()).unwrap();

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
            format!("{ORIGIN}/.exact/release/{COHORT}/exact.json"),
            format!("{ORIGIN}/.exact/release/{COHORT}/app.plan"),
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
    let foreign = b"{\"codec\":999,\"selected\":\"beef\",\"lastGood\":null,\"failures\":0}";
    std::fs::write(&record, foreign).unwrap();

    let mut store = Store::open(temp.path(), embedded(&[])).unwrap();
    assert!(store.frozen());
    assert_eq!(store.select().entry, None);
    assert_eq!(store.select().seq, EMBEDDED_SEQ);
    store.boot_started().unwrap();
    store.boot_succeeded(&store.generation()).unwrap();
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(refusal.contains("newer binary"), "unexpected: {refusal}");
    assert_eq!(std::fs::read(&record).unwrap(), foreign);
}

#[test]
fn the_pre_one_point_zero_record_is_replaced_without_booting_its_selection() {
    let temp = Temp::new("old-codec");
    let record = temp.path().join("record.json");
    std::fs::write(
        &record,
        b"{\"codec\":1,\"selected\":\"beef\",\"stream\":{\"compatibilityId\":\"9a1f3c7e5b2d4086\",\"seq\":99}}",
    )
    .unwrap();

    let store = open(&temp);
    assert!(!store.frozen());
    assert_eq!(store.select().entry, None);
    assert_eq!(store.select().seq, EMBEDDED_SEQ);
    let replaced: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(record).unwrap()).unwrap();
    assert_eq!(replaced["codec"], exact_update::STORE_CODEC);
    assert_eq!(replaced["selected"], serde_json::Value::Null);
    assert_eq!(replaced["stream"]["channel"], "release");
    assert_eq!(replaced["stream"]["seq"], EMBEDDED_SEQ);
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
fn a_record_from_another_channel_starts_at_entry_zero() {
    let temp = Temp::new("other-channel");
    let mut origin = Origin::of(&Bundle::new(4, b"release plan"));
    let mut store = open(&temp);
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));

    let mut beta = embedded(&[]);
    beta.channel = "beta".into();
    let store = Store::open(temp.path(), beta).unwrap();
    assert_eq!(store.select().entry, None);
    assert_eq!(store.select().seq, EMBEDDED_SEQ);
    assert_eq!(store.status().stream, "embedded");
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(temp.path().join("record.json")).unwrap())
            .unwrap();
    assert_eq!(record["stream"]["channel"], "beta");
}

#[test]
fn activate_hands_over_the_staged_plan_and_status_follows_each_step() {
    let temp = Temp::new("activate");
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    let mut store = open(&temp);
    store.boot_started().unwrap();
    store.boot_succeeded(&store.generation()).unwrap();
    assert!(activate(&mut store).is_none(), "nothing is staged yet");

    let Ok(Check::Staged { entry, seq, .. }) = origin.check(&mut store) else {
        panic!("the head should have staged");
    };
    let status = store.status();
    assert!(status.staged);
    assert_eq!(status.entry, None, "entry zero is still running");
    assert_eq!(status.selected_seq, 4);
    assert_eq!(
        status.running_seq, EMBEDDED_SEQ,
        "entry zero is still running"
    );
    assert_eq!(status.embedded_seq, EMBEDDED_SEQ);
    assert_eq!(status.stream, "embedded", "entry zero is still running");
    let staged = store.staged().expect("staged");
    assert_eq!(staged.entry, entry);
    assert_eq!(staged.seq, seq);

    assert_eq!(activate(&mut store), Some(b"plan four".to_vec()));
    let status = store.status();
    assert!(!status.staged);
    assert_eq!(status.entry, Some(entry));
    assert_eq!(status.running_seq, 4);
    assert!(activate(&mut store).is_none(), "activated once");
}

#[test]
fn app_decides_holds_a_checked_bundle_until_the_app_activates_it() {
    let temp = Temp::new("app-decides");
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    let mut store = open(&temp);
    store.hold_staged(true);
    store.boot_started().unwrap();
    store.boot_succeeded(&store.generation()).unwrap();
    let Ok(Check::Staged { entry, seq, .. }) = origin.check(&mut store) else {
        panic!("the head should have staged");
    };
    assert_eq!(seq, 4);
    let status = store.status();
    assert!(status.staged, "held, and the app is told");
    assert_eq!(status.selected_seq, EMBEDDED_SEQ, "but nothing is selected");
    assert_eq!(status.stream, "embedded");
    assert!(
        matches!(origin.check(&mut store), Ok(Check::Current { .. })),
        "the held bundle is what the head names: current, not staged twice"
    );

    // The next launch still boots entry zero, and still holds it.
    let mut next = open(&temp);
    next.hold_staged(true);
    assert_eq!(next.select().entry, None);
    assert_eq!(next.staged().map(|s| s.entry), Some(entry.clone()));

    // Until the app activates: then it runs, and the launches after boot it.
    assert_eq!(activate(&mut next), Some(b"plan four".to_vec()));
    assert_eq!(next.status().entry, Some(entry.clone()));
    assert_eq!(next.status().running_seq, 4);
    assert!(!next.status().staged);
    let after = open(&temp);
    assert_eq!(after.select().entry, Some(entry));
    assert_eq!(after.select().seq, 4);
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
    for spelling in ["7.0", "7e0", "-0"] {
        let raw = format!(r#"{{"seq":{spelling}}}"#);
        assert!(
            canonical_bytes(&raw).is_err(),
            "{spelling} is not a shortest integer"
        );
    }
    assert!(canonical_bytes("not json").is_err());
    assert!(
        canonical_bytes("[1,2]").is_err(),
        "the envelope is an object"
    );
}

#[test]
fn canonical_bytes_match_the_direct_node_serializer() {
    // `expected` is also asserted by the Bun publisher fixture. The numeric
    // unknown keys are deliberately written in JavaScript's enumeration order
    // and must emerge in UTF-8 lexical order, byte for byte.
    let head = r#"{"exact":1,"signature":{"keyId":"k1","ed25519":"AA=="},"app":{"name":"Weird Castle é—ü","id":"com.exact.weird-castle"},"plan":{"url":"./app.plan","bytes":12580,"sha256":"aa","formatVersion":4},"assets":[{"name":"a/b \"q\" \\ \u0001.png","bytes":0}],"stream":{"seq":41,"channel":"release","compatibilityId":"9a1f","app":"com.exact.weird-castle"},"sunset":{"message":"Retiring — update.","store":null},"unknownKeys":{"2":"two","10":"ten"}}"#;
    let expected = r#"{"app":{"id":"com.exact.weird-castle","name":"Weird Castle é—ü"},"assets":[{"bytes":0,"name":"a/b \"q\" \\ \u0001.png"}],"exact":1,"plan":{"bytes":12580,"formatVersion":4,"sha256":"aa","url":"./app.plan"},"stream":{"app":"com.exact.weird-castle","channel":"release","compatibilityId":"9a1f","seq":41},"sunset":{"message":"Retiring — update.","store":null},"unknownKeys":{"10":"ten","2":"two"}}"#;
    assert_eq!(
        String::from_utf8(canonical_bytes(head).unwrap()).unwrap(),
        expected
    );
}

#[test]
fn envelope_parse_refuses_node_normalizations_before_admission() {
    let (valid, _) = Bundle::new(7, b"plan").publish();
    for raw in [
        valid.replace("\"seq\":7", "\"seq\":7.0"),
        valid.replace("\"seq\":7", "\"seq\":7e0"),
    ] {
        assert!(Envelope::parse(raw.as_bytes()).is_err(), "admitted {raw}");
        assert!(canonical_bytes(&raw).is_err(), "canonicalized {raw}");
    }
    let mut value: serde_json::Value = serde_json::from_str(&valid).unwrap();
    value["unknownNumber"] = serde_json::json!(0);
    let negative_zero = serde_json::to_string(&value)
        .unwrap()
        .replace("\"unknownNumber\":0", "\"unknownNumber\":-0");
    assert!(Envelope::parse(negative_zero.as_bytes()).is_err());
    value["unknownText"] = serde_json::json!("scalar");
    let lone_surrogate = serde_json::to_string(&value)
        .unwrap()
        .replace("\"scalar\"", "\"\\ud800\"");
    assert!(Envelope::parse(lone_surrogate.as_bytes()).is_err());
    assert!(canonical_bytes(&lone_surrogate).is_err());
}

#[test]
fn a_head_naming_the_embedded_plan_and_assets_is_current_and_downloads_nothing() {
    let temp = Temp::new("embedded-plan");
    let bundle = Bundle::new(EMBEDDED_SEQ, b"plan three").asset("mark.png", b"a mark");
    let mut origin = Origin::of(&bundle);
    let mut carried = embedded(&[]);
    carried.embedded_plan_sha256 = Some(sha256_hex(b"plan three"));
    carried.embedded_assets = Some(std::collections::BTreeMap::from([(
        "mark.png".into(),
        (sha256_hex(b"a mark"), 6),
    )]));
    let mut store = Store::open(temp.path(), carried.clone()).unwrap();
    assert!(matches!(
        origin.check_embedding(&mut store, &[("mark.png", b"a mark")]),
        Ok(Check::Current { sunset: None })
    ));
    assert_eq!(origin.asked.len(), 1, "only the head is fetched");
    assert!(
        entry_names(&temp).is_empty(),
        "entry zero is not in the store"
    );
    assert_eq!(store.status().stream, "embedded");
    assert_eq!(store.status().running_seq, EMBEDDED_SEQ);

    // The same plan at a higher sequence with a changed asset is an
    // asset-only update: staged.
    let update = Bundle::new(4, b"plan three").asset("mark.png", b"a mark");
    origin.serving(&update);
    let mut store = Store::open(temp.path(), carried.clone()).unwrap();
    let Ok(Check::Staged { entry, .. }) =
        origin.check_embedding(&mut store, &[("mark.png", b"an older mark")])
    else {
        panic!("a changed asset should have staged");
    };
    assert_eq!(origin.asked.len(), 3, "head, plan, asset");
    assert_eq!(entry_names(&temp), vec![entry]);

    // And a binary that does not embed the asset at all stages it too.
    let temp = Temp::new("embedded-plan-no-asset");
    let mut store = Store::open(temp.path(), carried).unwrap();
    assert!(matches!(origin.check(&mut store), Ok(Check::Staged { .. })));
}

#[test]
fn every_asset_url_is_admitted_before_embedded_current_advances_the_floor() {
    let temp = Temp::new("embedded-current-card-origin");
    let bundle = Bundle::new(4, b"plan three")
        .asset("first.png", b"first")
        .asset("second.png", b"second");
    let mut origin = Origin::of(&bundle);
    let head_url = exact_update::head_url(ORIGIN, "release", COHORT);
    let mut head: serde_json::Value =
        serde_json::from_slice(origin.files.get(&head_url).unwrap()).unwrap();
    head["assets"][1]["url"] =
        serde_json::Value::String("https://attacker.example/second.png".into());
    origin.files.insert(
        head_url,
        serde_json::to_vec(&head).expect("the edited head is JSON"),
    );

    let mut carried = embedded(&[]);
    carried.embedded_plan_sha256 = Some(sha256_hex(b"plan three"));
    let mut store = Store::open(temp.path(), carried).unwrap();
    store.boot_succeeded(&store.generation()).unwrap();
    let record = temp.path().join("record.json");
    let before = std::fs::read(&record).unwrap();
    let refusal = origin
        .check_embedding(
            &mut store,
            &[("first.png", b"first"), ("second.png", b"second")],
        )
        .unwrap_err();
    assert!(refusal.contains("cross-origin"), "unexpected: {refusal}");
    assert_eq!(origin.asked.len(), 1, "no card URL was fetched");
    assert_eq!(
        std::fs::read(record).unwrap(),
        before,
        "a bad card cannot advance the embedded generation's observed floor"
    );
    assert_eq!(store.status().selected_seq, EMBEDDED_SEQ);
    assert!(entry_names(&temp).is_empty());
}

#[test]
fn a_plan_url_is_admitted_before_a_whole_old_entry_is_reused() {
    let temp = Temp::new("whole-entry-card-origin");
    let bundle = Bundle::new(4, b"plan four").asset("mark.png", b"a mark");
    let mut origin = Origin::of(&bundle);
    let head_url = exact_update::head_url(ORIGIN, "release", COHORT);
    let mut head: serde_json::Value =
        serde_json::from_slice(origin.files.get(&head_url).unwrap()).unwrap();
    head["plan"]["url"] = serde_json::Value::String("https://attacker.example/app.plan".into());
    let raw = serde_json::to_vec(&head).expect("the edited head is JSON");
    let envelope = exact_update::Envelope::parse(&raw).unwrap();
    origin.files.insert(head_url, raw.clone());

    let mut store = open(&temp);
    store.boot_succeeded(&store.generation()).unwrap();
    let record = temp.path().join("record.json");
    let before = std::fs::read(&record).unwrap();
    let entry = temp.path().join("entries").join(&envelope.digest);
    std::fs::create_dir_all(entry.join("assets")).unwrap();
    std::fs::write(entry.join("exact.json"), raw).unwrap();
    std::fs::write(entry.join("app.plan"), b"plan four").unwrap();
    std::fs::write(entry.join("assets/mark.png"), b"a mark").unwrap();

    let refusal = origin.check(&mut store).unwrap_err();
    assert!(refusal.contains("cross-origin"), "unexpected: {refusal}");
    assert_eq!(origin.asked.len(), 1, "the whole entry was not reused");
    assert_eq!(store.select().entry, None);
    assert_eq!(
        std::fs::read(record).unwrap(),
        before,
        "a bad card cannot mutate selection or its accepted floor"
    );
}

#[test]
fn another_channel_is_refused_before_any_download() {
    let temp = Temp::new("channel");
    let mut beta = Bundle::new(4, b"plan four");
    beta.channel = "beta".into();
    // Served at the release channel's path: a head replayed across channels.
    let (text, _) = beta.publish();
    let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
    origin.files.insert(
        format!("{ORIGIN}/.exact/release/{COHORT}/exact.json"),
        text.into_bytes(),
    );
    let mut store = open(&temp);
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("channel beta; this binary is release"),
        "unexpected refusal: {refusal}"
    );
    assert_eq!(origin.asked.len(), 1, "nothing was downloaded");
    assert!(entry_names(&temp).is_empty());
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
    let object = value.as_object().unwrap();
    let mut keys: Vec<&String> = object.keys().collect();
    keys.sort_by(|a, b| b.cmp(a));
    let shuffled = format!(
        "{{{}}}",
        keys.iter()
            .map(|key| format!(
                "{}:{}",
                serde_json::to_string(key).unwrap(),
                serde_json::to_string(&object[*key]).unwrap()
            ))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_ne!(
        shuffled, text,
        "the test explicitly reverses top-level keys"
    );
    assert_eq!(
        canonical_bytes(&shuffled).unwrap(),
        canonical_bytes(&text).unwrap(),
        "key order is not part of the authenticated payload"
    );
    assert_eq!(
        exact_update::Envelope::parse(text.as_bytes())
            .unwrap()
            .digest,
        exact_update::Envelope::parse(shuffled.as_bytes())
            .unwrap()
            .digest,
        "the authenticated payload, not its transport serialization, names the bundle"
    );

    let temp = Temp::new("canonical-head");
    let mut origin = Origin::of(&bundle);
    origin.files.insert(
        format!("{ORIGIN}/.exact/release/{COHORT}/exact.json"),
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
        format!("{ORIGIN}/.exact/release/{COHORT}/exact.json"),
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

#[test]
fn reserializing_a_bad_signed_bundle_does_not_evade_quarantine() {
    let signing = key(12);
    let mut bundle = Bundle::new(4, b"plan four");
    bundle.signer = Some(("k1".into(), signing.clone()));
    let mut origin = Origin::of(&bundle);
    let temp = Temp::new("canonical-quarantine");
    let carried = embedded(&[("k1", signing.verifying_key().to_bytes())]);
    let mut store = Store::open(temp.path(), carried.clone()).unwrap();
    let Ok(Check::Staged { entry, .. }) = origin.check(&mut store) else {
        panic!("the signed bundle should stage");
    };

    for _ in 0..2 {
        let mut launch = Store::open(temp.path(), carried.clone()).unwrap();
        assert_eq!(launch.select().entry.as_deref(), Some(entry.as_str()));
        launch.boot_started().unwrap();
    }
    assert_eq!(
        Store::open(temp.path(), carried.clone())
            .unwrap()
            .select()
            .entry,
        None
    );

    let head_url = exact_update::head_url(ORIGIN, "release", COHORT);
    let raw = origin.files.get(&head_url).unwrap();
    let value: serde_json::Value = serde_json::from_slice(raw).unwrap();
    let pretty = serde_json::to_string_pretty(&value).unwrap().into_bytes();
    assert_ne!(&pretty, raw);
    origin.files.insert(head_url, pretty);

    let mut store = Store::open(temp.path(), carried).unwrap();
    let refusal = origin.check(&mut store).unwrap_err();
    assert!(
        refusal.contains("failed to reach first pixel twice"),
        "reformatting must retain the bad bundle's identity: {refusal}"
    );
    assert_eq!(entry_names(&temp), vec![entry]);
}

fn activate(store: &mut Store) -> Option<Vec<u8>> {
    let candidate = store.prepare_activation().ok()??;
    store.commit_activation(&candidate.generation).ok()?;
    Some(candidate.plan.to_vec())
}

#[test]
fn preparation_refusal_and_stale_commit_leave_the_running_generation_and_record() {
    let temp = Temp::new("activation-atomic");
    let mut store = open(&temp);
    store.hold_staged(true);
    let mut origin = Origin::of(&Bundle::new(4, b"candidate"));
    origin.check(&mut store).unwrap();
    let record = temp.path().join("record.json");
    let before = std::fs::read(&record).unwrap();
    let running = store.generation();
    let candidate = store.prepare_activation().unwrap().unwrap();
    assert_eq!(&*candidate.plan, b"candidate");
    assert_eq!(store.generation(), running);
    assert_eq!(std::fs::read(&record).unwrap(), before);
    drop(candidate); // a host refusal: nothing was committed
    assert_eq!(store.generation(), running);
    let candidate = store.prepare_activation().unwrap().unwrap();
    origin = Origin::of(&Bundle::new(5, b"successor"));
    origin.check(&mut store).unwrap();
    let before = std::fs::read(&record).unwrap();
    assert!(store.commit_activation(&candidate.generation).is_err());
    assert_eq!(store.generation(), running);
    assert_eq!(std::fs::read(&record).unwrap(), before);
    let candidate = store.prepare_activation().unwrap().unwrap();
    // A record write refusal must not move the in-memory running identity.
    let saved = record.with_extension("saved");
    std::fs::rename(&record, &saved).unwrap();
    std::fs::create_dir(&record).unwrap();
    assert!(store.commit_activation(&candidate.generation).is_err());
    assert_eq!(store.generation(), running);
    std::fs::remove_dir(&record).unwrap();
    std::fs::rename(saved, &record).unwrap();
    store.commit_activation(&candidate.generation).unwrap();
    assert_eq!(store.generation(), candidate.generation);
    store.boot_started().unwrap();
    let before = std::fs::read(&record).unwrap();
    store.boot_succeeded(&running).unwrap(); // delayed draw of entry zero
    assert_eq!(std::fs::read(&record).unwrap(), before);
    store.boot_succeeded(&candidate.generation).unwrap();
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(record).unwrap()).unwrap();
    assert_eq!(record["failures"], 0);
    assert_eq!(record["lastGood"], candidate.generation.entry.unwrap());
}

#[test]
fn complete_rosters_represent_embedded_and_stored_removal_then_readdition() {
    let temp = Temp::new("complete-rosters");
    let mut binary = embedded(&[]);
    binary.embedded_plan_sha256 = Some(sha256_hex(b"same"));
    binary.embedded_assets = Some(std::collections::BTreeMap::from([(
        "mark.png".into(),
        (sha256_hex(b"old"), 3),
    )]));
    let mut store = Store::open(temp.path(), binary).unwrap();
    for (seq, asset) in [
        (4, None),
        (5, Some(b"new".as_slice())),
        (6, None),
        (7, Some(b"again".as_slice())),
    ] {
        let mut bundle = Bundle::new(seq, b"same");
        if let Some(asset) = asset {
            bundle = bundle.asset("mark.png", asset);
        }
        let mut origin = Origin::of(&bundle);
        assert!(matches!(
            origin
                .check_embedding(&mut store, &[("mark.png", b"old")])
                .unwrap(),
            Check::Staged { .. }
        ));
        let candidate = store.prepare_activation().unwrap().unwrap();
        assert_eq!(
            candidate.assets.resolve("mark.png").unwrap().as_deref(),
            asset
        );
        store.commit_activation(&candidate.generation).unwrap();
        let pinned = store.prepare_selected().unwrap().unwrap();
        assert_eq!(pinned.assets.resolve("mark.png").unwrap().as_deref(), asset);
    }
}

#[test]
fn embedded_current_and_fallback_keep_the_accepted_canonical_digest() {
    let temp = Temp::new("current-digest");
    let first = Bundle::new(4, b"same");
    let (json, _) = first.publish();
    let mut binary = embedded(&[]);
    binary.embedded_plan_sha256 = Some(sha256_hex(b"same"));
    binary.embedded_assets = Some(Default::default());
    let mut store = Store::open(temp.path(), binary.clone()).unwrap();
    assert!(matches!(
        Origin::of(&first).check(&mut store).unwrap(),
        Check::Current { .. }
    ));
    drop(store);
    let mut store = Store::open(temp.path(), binary.clone()).unwrap();
    let mut other = Bundle::new(4, b"same");
    other.sunset = Some(("changed metadata at the same seq".into(), None));
    assert!(Origin::of(&other)
        .check(&mut store)
        .unwrap_err()
        .contains("equivocate"));
    let baked = Temp::new("baked-digest");
    binary.seq = 4;
    binary.entry_digest = Some(Envelope::parse(json.as_bytes()).unwrap().digest);
    let mut store = Store::open(baked.path(), binary).unwrap();
    assert!(Origin::of(&other)
        .check(&mut store)
        .unwrap_err()
        .contains("equivocate"));
    assert!(matches!(
        Origin::of(&first).check(&mut store).unwrap(),
        Check::Current { .. }
    ));
}
