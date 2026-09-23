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
