//! Selector index: `testId` as a kernel citizen.
//!
//! Agents address nodes by `testId`, which is deliberately one-to-many —
//! repeated literals serve `count` clauses. The index is an exact-value
//! multimap; uniqueness is enforced at operation time by the caller, and
//! results are returned in structural tree order by the kernel, never in hash
//! order.

use std::collections::HashMap;

/// `testId` → slot indexes, maintained on set/clear/destroy/reset.
#[derive(Debug, Default, Clone)]
pub struct SelectorIndex {
    by_test_id: HashMap<String, Vec<u32>>,
}

impl SelectorIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `slot` now carries `new` (and no longer carries `old`).
    pub fn update(&mut self, slot: u32, old: Option<&str>, new: Option<&str>) {
        if old == new {
            return;
        }
        if let Some(old) = old {
            self.remove(slot, old);
        }
        if let Some(new) = new {
            self.by_test_id
                .entry(new.to_string())
                .or_default()
                .push(slot);
        }
    }

    /// Record that `slot` no longer carries `old`.
    pub fn remove(&mut self, slot: u32, old: &str) {
        if let Some(slots) = self.by_test_id.get_mut(old) {
            slots.retain(|s| *s != slot);
            if slots.is_empty() {
                self.by_test_id.remove(old);
            }
        }
    }

    /// Every slot carrying `test_id`, in insertion order (callers sort by tree order).
    pub fn lookup(&self, test_id: &str) -> &[u32] {
        self.by_test_id
            .get(test_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Number of distinct ids indexed.
    pub fn len(&self) -> usize {
        self.by_test_id.len()
    }

    /// Whether nothing is indexed.
    pub fn is_empty(&self) -> bool {
        self.by_test_id.is_empty()
    }

    /// Forget everything.
    pub fn clear(&mut self) {
        self.by_test_id.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multimap_semantics() {
        let mut idx = SelectorIndex::new();
        idx.update(1, None, Some("row"));
        idx.update(2, None, Some("row"));
        idx.update(3, None, Some("header"));
        assert_eq!(idx.lookup("row"), &[1, 2]);
        assert_eq!(idx.lookup("header"), &[3]);
        idx.update(2, Some("row"), Some("footer"));
        assert_eq!(idx.lookup("row"), &[1]);
        assert_eq!(idx.lookup("footer"), &[2]);
        idx.update(1, Some("row"), None);
        assert!(idx.lookup("row").is_empty());
        assert_eq!(idx.len(), 2);
        idx.clear();
        assert!(idx.is_empty());
    }
}
