//! The refusal journal's layout in the partition's metadata: an index
//! (`exact:refusals`, `{"segments":[n, …]}`) and its segments
//! (`exact:refusals:<n>`, each a JSON list of refusals). A segment holds at
//! most [`SEGMENT_ENTRIES`] refusals and about [`SEGMENT_BYTES`] of text, so no
//! one stored value grows past what a host can write (Exact's web bound is
//! 16 MiB a statement); a refusal too large for that keeps everything but its
//! `args` ([`stored`]). Nothing is evicted: every refusal stays until the app
//! dismisses it.

use serde_json::{json, Value as Json};

pub(crate) const INDEX: &str = "exact:refusals";
pub(crate) const SEGMENT_ENTRIES: usize = 64;
pub(crate) const SEGMENT_BYTES: usize = 1 << 20;

/// The journal as read: each segment with its refusals, in order.
pub(crate) type Segments = Vec<(u64, Vec<Json>)>;

pub(crate) fn segment_key(n: u64) -> String {
    format!("{INDEX}:{n}")
}

/// The segments an index names, in order (`meta`'s answer: text or null).
pub(crate) fn index(kept: &Json) -> Vec<u64> {
    kept.as_str()
        .and_then(|text| serde_json::from_str::<Json>(text).ok())
        .map(|index| {
            index["segments"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Json::as_u64)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn index_text(segments: &[u64]) -> String {
    json!({"segments": segments}).to_string()
}

/// A segment's refusals, from `meta`'s answer.
pub(crate) fn segment(kept: &Json) -> Vec<Json> {
    kept.as_str()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default()
}

/// Whether `entry` still fits in a segment holding `list`.
pub(crate) fn fits(list: &[Json], entry: &Json) -> bool {
    let bytes: usize = list.iter().map(|item| item.to_string().len() + 1).sum();
    list.len() < SEGMENT_ENTRIES && bytes + entry.to_string().len() < SEGMENT_BYTES
}

/// The most one refusal may take in a segment of its own, under the web's
/// 16 MiB statement bound with room for the segment's own JSON.
const ENTRY_BYTES: usize = 15 << 20;

/// A refusal as journaled: whole, or — past [`ENTRY_BYTES`] — without its
/// `args` (`argsOmitted` says so), so one oversized input never stops the
/// journal from saving. Its `input` digest still names what it carried.
pub(crate) fn stored(mut refused: Json) -> Json {
    if refused.to_string().len() > ENTRY_BYTES {
        refused["args"] = Json::Null;
        refused["argsOmitted"] = json!(true);
    }
    refused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_too_large_to_store_keeps_all_but_its_args() {
        let small = json!({"id": "a", "args": {"body": "hi"}, "input": "D"});
        assert_eq!(stored(small.clone()), small);
        let large =
            stored(json!({"id": "b", "args": {"body": "x".repeat(ENTRY_BYTES)}, "input": "D"}));
        assert_eq!(large["args"], Json::Null);
        assert_eq!(large["argsOmitted"], true);
        assert_eq!(large["input"], "D");
        assert!(!fits(&[], &json!({"body": "x".repeat(SEGMENT_BYTES)})));
    }
}
