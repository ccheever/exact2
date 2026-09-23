//! The store under damage and crashes: records that cannot be read, entries
//! that rot, and what a client may still trust afterwards (the 2026-09-22
//! review's probes, as tests).
//!
//! @ref LLP 1026 D11 (anti-rollback, crash recovery) / LLP 1030 D9

use crate::support::*;
use exact_update::Check;

/// A record whose codec is not an integer was written by nobody: it is
/// damaged, and the client carries on updating rather than freezing as it
/// does for a newer binary's (numeric) codec.
#[test]
fn a_record_without_an_integer_codec_is_damaged_not_foreign() {
    for codec in ["\"2\"", "null", "\"newer\"", "2.5", "-1", "{}"] {
        let temp = Temp::new("codec-damaged");
        let record = temp.path().join("record.json");
        std::fs::write(
            &record,
            format!("{{\"codec\":{codec},\"selected\":\"beef\"}}"),
        )
        .unwrap();
        let mut store = open(&temp);
        assert!(!store.frozen(), "codec {codec}");
        assert_eq!(store.select().entry, None, "codec {codec}");
        let mut origin = Origin::of(&Bundle::new(4, b"plan four"));
        assert!(
            matches!(origin.check(&mut store), Ok(Check::Staged { seq: 4, .. })),
            "codec {codec}: the updater still updates"
        );
    }
    let temp = Temp::new("codec-missing");
    std::fs::write(temp.path().join("record.json"), b"{\"selected\":\"beef\"}").unwrap();
    assert!(!open(&temp).frozen());
}

/// Asset names must be distinct files on a case- and normalization-
/// insensitive filesystem (APFS): a head naming two that are one file there,
/// or a file where another needs a directory, is refused before anything is
/// downloaded, and so is a name a filesystem may normalize.
#[test]
fn colliding_or_unportable_asset_names_are_refused_before_download() {
    let refused: [(&[&str], &str); 7] = [
        (&["Logo.png", "logo.png"], "differ only by case"),
        (&["a", "a/b.png"], "is a file where"),
        (&["deck/B/x.html", "deck/b"], "is a file where"),
        (&["caf\u{e9}.png"], "not portable"),
        (&["cafe\u{301}.png"], "not portable"),
        (&["two words.png"], "not portable"),
        (&["mark.png", "mark.png"], "twice"),
    ];
    for (names, why) in refused {
        let temp = Temp::new("names");
        let mut bundle = Bundle::new(4, b"plan four");
        for name in names {
            bundle = bundle.asset(name, name.as_bytes());
        }
        let mut origin = Origin::of(&bundle);
        let mut store = open(&temp);
        let refusal = origin.check(&mut store).unwrap_err();
        assert!(refusal.contains(why), "{names:?}: {refusal}");
        assert_eq!(
            origin.asked.len(),
            1,
            "{names:?}: only the head was fetched"
        );
        assert!(entry_names(&temp).is_empty());
    }
    for names in [
        &["a/b.png", "a/c.png", "A-b_c.1.png"][..],
        &["deck/index.html", "deck/index.html.map"],
    ] {
        exact_update::check_asset_names(names.iter().copied()).unwrap();
    }
}

/// A selected entry corrupted on disk is refused at launch, and entry zero
/// runs; the floor stays at its seq. The head the record accepted must then
/// stage again, or the client is stranded on entry zero until a later seq
/// exists. Another bundle at that seq is still equivocation.
#[test]
fn a_corrupted_selection_is_restaged_from_the_head_it_accepted() {
    let temp = Temp::new("restage");
    let bundle = Bundle::new(4, b"plan four").asset("mark.png", b"a mark");
    let mut origin = Origin::of(&bundle);
    let mut store = open(&temp);
    let Ok(Check::Staged { entry, .. }) = origin.check(&mut store) else {
        panic!("the first head stages");
    };
    drop(store);
    let plan = temp.path().join("entries").join(&entry).join("app.plan");
    std::fs::write(&plan, b"rotten plan").unwrap();
    let mut store = open(&temp);
    assert!(store.prepare_selected().is_err());
    assert_eq!(store.select().entry, None);

    let other = Bundle::new(4, b"another plan four");
    let refusal = Origin::of(&other).check(&mut store).unwrap_err();
    assert!(refusal.contains("equivocate"), "{refusal}");
    assert!(matches!(
        origin.check(&mut store),
        Ok(Check::Staged { seq: 4, .. })
    ));
    drop(store);
    let mut store = open(&temp);
    assert_eq!(store.select().entry.as_deref(), Some(entry.as_str()));
    let prepared = store.prepare_selected().unwrap().unwrap();
    assert_eq!(&*prepared.plan, b"plan four");
}

/// The record is the rollback floor. A crash that left it truncated — an
/// unsynced rename can persist before its bytes — made the store start over
/// at the embedded seq, so an older signed head, replayed by anyone serving
/// the origin, was accepted again. The floor now comes back from the signed
/// entries on disk, and is written back.
#[test]
fn a_crash_truncated_record_keeps_the_rollback_floor() {
    let temp = Temp::new("truncated");
    let four = Bundle::new(4, b"plan four");
    let six = Bundle::new(6, b"plan six");
    let mut store = open(&temp);
    assert!(matches!(
        Origin::of(&four).check(&mut store),
        Ok(Check::Staged { seq: 4, .. })
    ));
    assert!(matches!(
        Origin::of(&six).check(&mut store),
        Ok(Check::Staged { seq: 6, .. })
    ));
    drop(store);
    let record = temp.path().join("record.json");
    let whole = std::fs::read(&record).unwrap();
    let zeros = vec![0u8; whole.len()];
    for damaged in [&whole[..whole.len() / 2], &[][..], &zeros[..]] {
        std::fs::write(&record, damaged).unwrap();
        let mut store = open(&temp);
        let refusal = Origin::of(&four).check(&mut store).unwrap_err();
        assert!(refusal.contains("below the accepted seq 6"), "{refusal}");
        let written: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
        assert_eq!(written["stream"]["seq"], 6);
        assert!(Origin::of(&six).check(&mut store).is_ok());
    }
}

/// Bytes the store directory holds, each file counted once however many
/// names link it.
#[cfg(unix)]
fn stored_bytes(dir: &std::path::Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for item in std::fs::read_dir(&dir).unwrap().flatten() {
            let meta = std::fs::symlink_metadata(item.path()).unwrap();
            if meta.is_dir() {
                stack.push(item.path());
            } else if seen.insert((meta.dev(), meta.ino())) {
                total += meta.len();
            }
        }
    }
    total
}

/// Every entry held a copy of every asset and nothing was ever removed:
/// releases of an unchanged 1 MiB asset took 1 MiB each, and finding a
/// digest parsed every envelope (a check took 2.2 s at 200 entries). Content
/// is now kept once by digest and linked into entries, and only the entries
/// a role keeps survive.
#[test]
#[cfg(unix)]
fn an_unchanged_asset_is_stored_once_and_old_entries_are_collected() {
    let temp = Temp::new("collect");
    let big = vec![7u8; 1 << 20];
    for seq in 4..12 {
        let plan = format!("plan {seq}");
        let bundle = Bundle::new(seq, plan.as_bytes()).asset("big.bin", &big);
        let mut store = open(&temp);
        assert!(matches!(
            Origin::of(&bundle).check(&mut store),
            Ok(Check::Staged { .. })
        ));
        // The next launch boots it to first pixel: it becomes last-good.
        let mut store = open(&temp);
        let generation = store.prepare_selected().unwrap().unwrap().generation;
        store.boot_started().unwrap();
        store.boot_succeeded(&generation).unwrap();
    }
    let store = open(&temp);
    let stored = stored_bytes(temp.path());
    assert!(stored < (1 << 20) + 64 * 1024, "{stored} bytes stored");
    assert!(entry_names(&temp).len() <= 2, "{:?}", entry_names(&temp));
    let prepared = { store }.prepare_selected().unwrap().unwrap();
    assert_eq!(
        prepared.assets.resolve("big.bin").unwrap().unwrap().len(),
        1 << 20
    );
}
