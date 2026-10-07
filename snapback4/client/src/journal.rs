//! The refusal journal's layout in the partition's metadata: an index
//! (`exact:refusals`, `{"segments":[n, …]}`) and its segments
//! (`exact:refusals:<n>`, each a JSON list of refusals). A segment holds at
//! most [`SEGMENT_ENTRIES`] refusals and about [`SEGMENT_BYTES`] of text, so no
//! one stored value grows past what a host can write (Exact's web bound is
//! 16 MiB a statement). Nothing is evicted: every refusal stays until the app
//! dismisses it. An index that is still a plain list (the first layout) is
//! read as one segment.

use serde_json::{json, Value as Json};

pub(crate) const INDEX: &str = "exact:refusals";
pub(crate) const SEGMENT_ENTRIES: usize = 64;
pub(crate) const SEGMENT_BYTES: usize = 1 << 20;

/// The journal as read: each segment (`None` for the first layout's inline
/// list) with its refusals, in order.
pub(crate) type Segments = Vec<(Option<u64>, Vec<Json>)>;

pub(crate) fn segment_key(n: u64) -> String {
    format!("{INDEX}:{n}")
}

/// What the index names: its segments in order, or — for the first layout —
/// the refusals it holds itself.
pub(crate) enum Index {
    Segments(Vec<u64>),
    Inline(Vec<Json>),
}

/// Read an index from `meta`'s answer (text or null).
pub(crate) fn index(kept: &Json) -> Index {
    match kept
        .as_str()
        .and_then(|text| serde_json::from_str::<Json>(text).ok())
    {
        Some(Json::Array(list)) => Index::Inline(list),
        Some(index) => Index::Segments(
            index["segments"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Json::as_u64)
                .collect(),
        ),
        None => Index::Segments(Vec::new()),
    }
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
