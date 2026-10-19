//! One import's work, bounded (LLP 1121 §5 stage 10b's review): a paired
//! client's file reaches the importers, and a small document can multiply
//! (a set referenced eight times a level, a group that extends and refers
//! to the next, an alias named twice in a list). Every step that expands
//! one (a reference followed, a group merged, a value copied, an alias
//! resolved) spends from one budget, charged before it copies, and a
//! document that spends it all is refused, whatever shape multiplies it.

use serde_json::Value;
use std::cell::Cell;

/// The work one import may do: a byte copied or a step taken is one. Far
/// past a real design's (a 1 MiB file read a few times over), short of
/// what exhausts the server.
pub const LIMIT: u64 = 1 << 26;

thread_local! {
    static LEFT: Cell<u64> = const { Cell::new(LIMIT) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// An import's work, from its entry point to its return: the budget full
/// when the outermost one begins, shared by any it calls.
pub struct Work(());

impl Work {
    pub fn begin() -> Work {
        DEPTH.with(|d| {
            if d.get() == 0 {
                LEFT.with(|l| l.set(LIMIT));
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
            Err(format!(
                "the source expands past what one import may do ({LIMIT} steps): its references, groups or aliases multiply"
            ))
        }
    })
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
