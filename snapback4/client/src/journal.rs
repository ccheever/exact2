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

/// What a value costs where a host stores it: its JSON text, escaped again
/// as a string inside the host's own change (the web's commit parameter).
pub(crate) fn encoded(value: &Json) -> usize {
    Json::String(value.to_string()).to_string().len()
}

/// Whether `entry` still fits in a segment holding `list`.
pub(crate) fn fits(list: &[Json], entry: &Json) -> bool {
    let bytes: usize = list.iter().map(|item| encoded(item) + 1).sum();
    list.len() < SEGMENT_ENTRIES && bytes + encoded(entry) < SEGMENT_BYTES
}

/// The most one refusal may cost ([`encoded`]) in a segment of its own: half
/// the web's 16 MiB statement bound, so a host's own wrapping still fits.
const ENTRY_BYTES: usize = 8 << 20;

/// A refusal as journaled: whole, or — past [`ENTRY_BYTES`] — without its
/// `args` (`argsOmitted` says so) and, if still too large, with only the
/// refusal's code and family, so one oversized refusal never stops the
/// journal from saving. Its `input` digest still names what it carried.
pub(crate) fn stored(mut refused: Json) -> Json {
    if encoded(&refused) > ENTRY_BYTES {
        refused["args"] = Json::Null;
        refused["argsOmitted"] = json!(true);
    }
    if encoded(&refused) > ENTRY_BYTES {
        let why = &refused["why"];
        refused["why"] = json!({"code": why["code"].as_str().filter(|c| c.len() <= 64).unwrap_or("E_REFUSED"),
            "family": why["family"].as_str().filter(|f| f.len() <= 64).unwrap_or("input"),
            "message": "the server's refusal was too large to keep"});
    }
    refused
}

/// A refused write as the journal keeps it: its id, operation, the input it
/// carried and that input's digest, why it was refused, and when written.
pub(crate) fn entry(write: &Json, why: &Json) -> Json {
    let op = write["op"].as_str().unwrap_or_default();
    stored(json!({"id": write["id"], "op": op, "args": write["args"],
        "input": crate::client::input_key(op, &write["args"]), "why": why, "at": write["now"]}))
}

/// What adding `refused` to `journal` writes, as `(key, value)` metadata in
/// order: the segment it joins, then the index if that segment is new.
/// Nothing when the journal already holds its id.
pub(crate) fn append(journal: &Segments, refused: Json) -> Vec<(String, String)> {
    if journal
        .iter()
        .any(|(_, list)| list.iter().any(|known| known["id"] == refused["id"]))
    {
        return Vec::new();
    }
    let mut segments: Vec<u64> = journal.iter().map(|(n, _)| *n).collect();
    let (n, mut last) = match journal.last() {
        Some((n, list)) if fits(list, &refused) => (*n, list.clone()),
        _ => (segments.iter().max().map_or(0, |n| n + 1), Vec::new()),
    };
    last.push(refused);
    let mut writes = vec![(segment_key(n), Json::Array(last).to_string())];
    if !segments.contains(&n) {
        segments.push(n);
        writes.push((INDEX.into(), index_text(&segments)));
    }
    writes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_too_large_to_store_keeps_all_but_its_args() {
        let small = json!({"id": "a", "args": {"body": "hi"}, "input": "D"});
        assert_eq!(stored(small.clone()), small);
        // Escapes count twice: 5 MiB of backslashes costs 20 once stored.
        let large = stored(
            json!({"id": "b", "args": {"body": "\\".repeat(5 << 20)}, "input": "D", "why": {"code": "TAKEN"}}),
        );
        assert_eq!(large["args"], Json::Null);
        assert_eq!(large["argsOmitted"], true);
        assert_eq!(large["input"], "D");
        assert_eq!(large["why"]["code"], "TAKEN");
        let why = stored(
            json!({"id": "c", "args": {}, "why": {"code": "TAKEN", "message": "\\".repeat(5 << 20)}}),
        );
        assert_eq!(why["why"]["code"], "TAKEN");
        assert!(encoded(&why) < ENTRY_BYTES);
        assert!(!fits(&[], &json!({"body": "x".repeat(SEGMENT_BYTES)})));
    }
}
