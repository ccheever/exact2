//! One import's work, bounded (LLP 1121 §5 stage 10b's review): a paired
//! client's file reaches the importers, and a small document can multiply
//! (a set referenced eight times a level, a group that extends and refers
//! to the next, an alias named twice in a list). Every step that expands
//! one (a reference followed, a group merged, a value copied, an alias
//! resolved) spends from one budget, charged before it copies, and a
//! document that spends it all is refused, whatever shape multiplies it.

use serde_json::Value;
use std::cell::Cell;

/// The work a fresh import may do: a byte copied or a step taken is one.
/// Calibrated to the isolate an import runs in (`scripts/tamber-isolate.mjs`:
/// 1 GiB): a DTCG import holds about 12 bytes for each unit (measured on
/// designs over 64 contexts, 2026-10-19), so this refuses one with the
/// budget's message by about 600 MB, under three quarters of the isolate's
/// limit, rather than the isolate killing it (LLP 1121 §10).
pub const IMPORT: u64 = 3 << 24;

/// The work a re-import may do. It exports the design to compare (each
/// context's tree charged as it is built) and holds both, about 30 bytes
/// for each unit, so this refuses one by about 750 MB.
pub const REIMPORT: u64 = 3 << 23;

thread_local! {
    static LEFT: Cell<u64> = const { Cell::new(0) };
    static LIMIT: Cell<u64> = const { Cell::new(0) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    /// Whether a spend found the budget too short (not merely spent to 0).
    static OVER: Cell<bool> = const { Cell::new(false) };
}

/// An import's work, from its entry point to its return: the budget full
/// when the outermost one begins, shared by any it calls.
pub struct Work(());

impl Work {
    /// Begin an import of at most `limit` ([`IMPORT`] or [`REIMPORT`]); an
    /// import inside another spends the outer one's.
    pub fn begin(limit: u64) -> Work {
        DEPTH.with(|d| {
            if d.get() == 0 {
                LEFT.with(|l| l.set(limit));
                LIMIT.with(|l| l.set(limit));
                OVER.with(|o| o.set(false));
            }
            d.set(d.get() + 1);
        });
        Work(())
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// Spend `n`, or refuse: the document expands past what one import may.
/// Outside any import's [`Work`], nothing is counted.
pub fn spend(n: u64) -> Result<(), String> {
    if DEPTH.with(Cell::get) == 0 {
        return Ok(());
    }
    LEFT.with(|l| match l.get().checked_sub(n) {
        Some(left) => {
            l.set(left);
            Ok(())
        }
        None => {
            l.set(0);
            OVER.with(|o| o.set(true));
            Err(format!(
                "the source expands past what one import may do ({} steps): its references, groups or aliases multiply, or the design is larger than one import may take",
                LIMIT.with(Cell::get)
            ))
        }
    })
}

/// The refusal's words when this import's budget is spent (a step that
/// stopped at it rather than refusing at once), else `None`.
pub(crate) fn spent() -> Option<String> {
    // (one that spent the budget to the last step exactly is within it:
    // Grok's round 1 on the counts)
    let spent = DEPTH.with(Cell::get) > 0 && OVER.with(Cell::get);
    spent.then(|| spend(1).unwrap_err())
}

/// What copying `v` costs: its bytes, roughly.
pub fn size(v: &Value) -> u64 {
    match v {
        Value::String(s) => 8 + s.len() as u64,
        Value::Array(a) => 8 + a.iter().map(size).sum::<u64>(),
        Value::Object(o) => 8 + o.iter().map(|(k, v)| k.len() as u64 + size(v)).sum::<u64>(),
        _ => 8,
    }
}

/// Spend what copying `v` costs.
pub fn copy(v: &Value) -> Result<(), String> {
    spend(size(v))
}

/// Spend what copying a group's members costs.
pub fn members(m: &serde_json::Map<String, Value>) -> Result<(), String> {
    spend(m.iter().map(|(k, v)| k.len() as u64 + size(v)).sum())
}

/// A context's tree of an export, charged as it is built: inside an
/// import's budget (a re-import exports the design to compare), a design
/// too large for one import is refused here, before its export is whole
/// (LLP 1121 §10). Outside one, nothing is counted.
pub(crate) fn tree(tree: serde_json::Map<String, Value>) -> Result<Value, Vec<crate::Diagnostic>> {
    let tree = Value::Object(tree);
    copy(&tree).map_err(|m| vec![crate::Diagnostic::new("tamber-export", m, 1, 1)])?;
    Ok(tree)
}

#[cfg(test)]
mod spent_tests {
    /// Spending the budget to its last step is within it; a step past it is
    /// what `spent` reports (Grok's round 1 on the counts).
    #[test]
    fn the_last_step_is_within_the_budget_and_one_past_it_is_spent() {
        let _work = super::Work::begin(10);
        assert!(super::spend(10).is_ok());
        assert!(super::spent().is_none());
        assert!(super::spend(1).is_err());
        assert!(super::spent().is_some());
    }
}
