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
