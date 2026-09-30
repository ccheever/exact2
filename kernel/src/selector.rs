//! Selector index: `testId` as a kernel citizen.
//!
//! Agents address nodes by `testId`, which is deliberately one-to-many —
//! repeated literals serve `count` clauses. The index is an exact-value
//! multimap; uniqueness is enforced at operation time by the caller, and
//! results are returned in structural tree order by the kernel, never in hash
//! order.

use crate::id::IdSet;
use std::collections::BTreeMap;

/// `testId` → slot indexes, maintained on set/clear/destroy/reset. A tree:
/// a list's rows add ids out of order (`row-10` sorts before `row-2`), which
/// a sorted vector pays for by moving its tail each time.
#[derive(Debug, Default, Clone)]
pub struct SelectorIndex {
    by_test_id: BTreeMap<String, Vec<u32>>,
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
            match self.by_test_id.get_mut(new) {
                Some(slots) => slots.push(slot),
                None => {
                    self.by_test_id.insert(new.to_string(), vec![slot]);
                }
            }
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

    /// Record that none of `slots` carries `old`: one pass over its entries
    /// however many leave, as a run of destroys does.
    pub fn remove_all(&mut self, old: &str, slots: &IdSet<u32>) {
        if let Some(entries) = self.by_test_id.get_mut(old) {
            entries.retain(|s| !slots.contains(s));
            if entries.is_empty() {
                self.by_test_id.remove(old);
            }
        }
    }

    /// Index a live node's `testId` and `id` props (a rebuilt index).
    pub fn index(&mut self, slot: u32, props: &crate::props::PropList) {
        use crate::generated::PropId;
        if let Some(test_id) = props.str(PropId::TestId) {
            self.update(slot, None, Some(test_id));
        }
        if let Some(id) = props.str(PropId::Id) {
            self.update_id(slot, None, Some(id));
        }
    }

    /// The index's key for an `id` prop: ids share the map, apart from
    /// test ids by a leading control character no test id carries.
    pub fn id_key(id: &str) -> String {
        format!("\u{1}{id}")
    }

    /// Record that `slot`'s `id` prop is now `new` (LLP 1055.000 D3).
    pub fn update_id(&mut self, slot: u32, old: Option<&str>, new: Option<&str>) {
        let (old, new) = (old.map(Self::id_key), new.map(Self::id_key));
        self.update(slot, old.as_deref(), new.as_deref());
    }

    /// Every slot whose `id` prop is `id`, in insertion order.
    pub fn lookup_id(&self, id: &str) -> &[u32] {
        self.lookup(&Self::id_key(id))
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
