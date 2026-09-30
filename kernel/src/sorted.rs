//! Ordered maps and sets as one sorted vector, and a set of arena slots.
//!
//! @ref LLP 1047 §6 (the core diet: maps)
//!
//! `BTreeMap` and `BTreeSet` compile a node tree's worth of code for every key
//! and value type: about 4–10 KB of wasm each, and the web wasm carried fifty.
//! The engine's maps are small, or grow at the end (view ids are never
//! reused), or are keyed by dense arena slots. For those, one sorted vector
//! or a bitset is as fast and a few hundred bytes. Iteration order, `Debug`,
//! equality, `insert`'s old value and `FromIterator`'s last-equal-key-wins
//! are `BTreeMap`'s and `BTreeSet`'s, so a swap changes no output.

use std::borrow::Borrow;
use std::fmt;

/// An ordered map: `BTreeMap`'s order and the part of its API the engine
/// uses, over one vector sorted by key. Insertion and removal move the tail,
/// so it is for maps that stay small or grow at the end.
#[derive(Clone, PartialEq, Eq)]
pub struct SortedMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> Default for SortedMap<K, V> {
    fn default() -> Self {
        SortedMap {
            entries: Vec::new(),
        }
    }
}

impl<K: Ord, V> SortedMap<K, V> {
    /// Empty.
    pub const fn new() -> Self {
        SortedMap {
            entries: Vec::new(),
        }
    }

    /// Where `key` is, or would go. A key past the last appends unsearched.
    fn find<Q: Ord + ?Sized>(&self, key: &Q) -> Result<usize, usize>
    where
        K: Borrow<Q>,
    {
        match self.entries.last() {
            Some((last, _)) if last.borrow() < key => Err(self.entries.len()),
            _ => self.entries.binary_search_by(|(k, _)| k.borrow().cmp(key)),
        }
    }

    /// How many entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The value at `key`.
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.find(key).ok().map(|i| &self.entries[i].1)
    }

    /// The value at `key`, mutably.
    pub fn get_mut<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        self.find(key).ok().map(|i| &mut self.entries[i].1)
    }

    /// Whether `key` has a value.
    pub fn contains_key<Q: Ord + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.find(key).is_ok()
    }

    /// Set `key` to `value`; the value it replaced, if any.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.find(&key) {
            Ok(i) => Some(std::mem::replace(&mut self.entries[i].1, value)),
            Err(i) => {
                self.entries.insert(i, (key, value));
                None
            }
        }
    }

    /// Remove `key`; its value, if it had one.
    pub fn remove<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        self.find(key).ok().map(|i| self.entries.remove(i).1)
    }

    /// The value at `key`, inserted from `make` when absent.
    pub fn get_or_insert_with(&mut self, key: K, make: impl FnOnce() -> V) -> &mut V {
        let i = match self.find(&key) {
            Ok(i) => i,
            Err(i) => {
                self.entries.insert(i, (key, make()));
                i
            }
        };
        &mut self.entries[i].1
    }

    /// Entries in key order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    /// Keys in order.
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.entries.iter().map(|(k, _)| k)
    }

    /// Values in key order.
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.entries.iter().map(|(_, v)| v)
    }

    /// Keep the entries `keep` accepts, in order.
    pub fn retain(&mut self, mut keep: impl FnMut(&K, &mut V) -> bool) {
        self.entries.retain_mut(|(k, v)| keep(k, v));
    }

    /// Remove everything.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for SortedMap<K, V> {
    /// As `BTreeMap`: of equal keys, the last value stays. Sorted once, not
    /// inserted one by one: entries out of key order would each move the
    /// tail (a thousand-row list's views, quadratic).
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut entries: Vec<(K, V)> = iter.into_iter().collect();
        // Stable: of equal keys the last collected is last.
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries.dedup_by(|later, kept| {
            let same = later.0 == kept.0;
            if same {
                std::mem::swap(&mut later.1, &mut kept.1);
            }
            same
        });
        SortedMap { entries }
    }
}

impl<K: Ord, V> Extend<(K, V)> for SortedMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        if self.entries.is_empty() {
            *self = iter.into_iter().collect();
            return;
        }
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<'a, K, V> IntoIterator for &'a SortedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = std::iter::Map<std::slice::Iter<'a, (K, V)>, fn(&'a (K, V)) -> (&'a K, &'a V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

impl<K, V> IntoIterator for SortedMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<K: Ord + Borrow<Q>, Q: Ord + ?Sized, V> std::ops::Index<&Q> for SortedMap<K, V> {
    type Output = V;
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for SortedMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

/// An ordered set: `BTreeSet`'s order over one sorted vector.
#[derive(Clone, PartialEq, Eq)]
pub struct SortedSet<T> {
    items: Vec<T>,
}

impl<T> Default for SortedSet<T> {
    fn default() -> Self {
        SortedSet { items: Vec::new() }
    }
}

impl<T: Ord> SortedSet<T> {
    /// Empty.
    pub const fn new() -> Self {
        SortedSet { items: Vec::new() }
    }

    fn find<Q: Ord + ?Sized>(&self, item: &Q) -> Result<usize, usize>
    where
        T: Borrow<Q>,
    {
        match self.items.last() {
            Some(last) if last.borrow() < item => Err(self.items.len()),
            _ => self.items.binary_search_by(|x| x.borrow().cmp(item)),
        }
    }

    /// How many items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether `item` is in the set.
    pub fn contains<Q: Ord + ?Sized>(&self, item: &Q) -> bool
    where
        T: Borrow<Q>,
    {
        self.find(item).is_ok()
    }

    /// Add `item`; whether it was new.
    pub fn insert(&mut self, item: T) -> bool {
        match self.find(&item) {
            Ok(_) => false,
            Err(i) => {
                self.items.insert(i, item);
                true
            }
        }
    }

    /// Remove `item`; whether it was there.
    pub fn remove<Q: Ord + ?Sized>(&mut self, item: &Q) -> bool
    where
        T: Borrow<Q>,
    {
        self.find(item).map(|i| self.items.remove(i)).is_ok()
    }

    /// Items in order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    /// Remove everything.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

impl<T: Ord> FromIterator<T> for SortedSet<T> {
    /// As `BTreeSet`: of equal items, the first stays. Sorted once, as
    /// [`SortedMap`]'s.
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut items: Vec<T> = iter.into_iter().collect();
        items.sort();
        items.dedup();
        SortedSet { items }
    }
}

impl<'a, T> IntoIterator for &'a SortedSet<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<T> IntoIterator for SortedSet<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<T: fmt::Debug> fmt::Debug for SortedSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.items.iter()).finish()
    }
}

/// A set of arena slots, a bit each: constant-time insert and remove, and
/// ascending iteration as `BTreeSet<u32>` iterates. Slots are dense.
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct SlotSet {
    words: Vec<u64>,
    len: usize,
}

impl SlotSet {
    /// Add `slot`; whether it was new.
    pub(crate) fn insert(&mut self, slot: u32) -> bool {
        let (word, bit) = ((slot / 64) as usize, 1u64 << (slot % 64));
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        let new = self.words[word] & bit == 0;
        self.words[word] |= bit;
        self.len += usize::from(new);
        new
    }

    /// Remove `slot`; whether it was there.
    pub(crate) fn remove(&mut self, slot: u32) -> bool {
        let (word, bit) = ((slot / 64) as usize, 1u64 << (slot % 64));
        match self.words.get_mut(word) {
            Some(w) if *w & bit != 0 => {
                *w &= !bit;
                self.len -= 1;
                true
            }
            _ => false,
        }
    }

    /// Whether there are none.
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether `slot` is in the set.
    pub(crate) fn contains(&self, slot: u32) -> bool {
        self.words
            .get((slot / 64) as usize)
            .is_some_and(|w| w & (1u64 << (slot % 64)) != 0)
    }

    /// Remove everything.
    pub(crate) fn clear(&mut self) {
        self.words.clear();
        self.len = 0;
    }

    /// The slots in ascending order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.words.iter().enumerate().flat_map(|(i, &word)| {
            let mut rest = word;
            std::iter::from_fn(move || {
                (rest != 0).then(|| {
                    let bit = rest.trailing_zeros();
                    rest &= rest - 1;
                    i as u32 * 64 + bit
                })
            })
        })
    }
}

impl fmt::Debug for SlotSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{SlotSet, SortedMap, SortedSet};
    use std::collections::{BTreeMap, BTreeSet};

    /// xorshift64*, deterministic.
    fn rng(seed: &mut u64) -> u64 {
        *seed ^= *seed >> 12;
        *seed ^= *seed << 25;
        *seed ^= *seed >> 27;
        seed.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    #[test]
    fn maps_and_sets_answer_as_btree_does() {
        let mut seed = 7;
        let (mut map, mut tree) = (SortedMap::new(), BTreeMap::new());
        let (mut set, mut bset) = (SortedSet::new(), BTreeSet::new());
        let (mut slots, mut sset) = (SlotSet::default(), BTreeSet::new());
        for step in 0..20_000u32 {
            let key = (rng(&mut seed) % 300) as u32;
            match rng(&mut seed) % 4 {
                0 | 1 => {
                    assert_eq!(map.insert(key, step), tree.insert(key, step));
                    assert_eq!(set.insert(key), bset.insert(key));
                    assert_eq!(slots.insert(key), sset.insert(key));
                }
                2 => {
                    assert_eq!(map.remove(&key), tree.remove(&key));
                    assert_eq!(set.remove(&key), bset.remove(&key));
                    assert_eq!(slots.remove(key), sset.remove(&key));
                }
                _ => {
                    *map.get_or_insert_with(key, || 0) += 1;
                    *tree.entry(key).or_insert(0) += 1;
                }
            }
            assert_eq!(map.get(&key), tree.get(&key));
            assert_eq!(set.contains(&key), bset.contains(&key));
        }
        assert!(map.iter().eq(tree.iter()));
        assert!(set.iter().eq(bset.iter()));
        assert!(slots.iter().eq(sset.iter().copied()));
        assert_eq!(format!("{map:?}"), format!("{tree:?}"));
        assert_eq!(format!("{set:?}"), format!("{bset:?}"));
        assert_eq!(format!("{slots:?}"), format!("{sset:?}"));
        let pairs = [(3, 'a'), (1, 'b'), (3, 'c'), (2, 'd')];
        let collected: SortedMap<_, _> = pairs.into_iter().collect();
        let expected: BTreeMap<_, _> = pairs.into_iter().collect();
        assert!(
            collected.iter().eq(expected.iter()),
            "the last of equal keys wins"
        );
        let mut extended = SortedMap::new();
        extended.extend(pairs);
        assert!(extended.iter().eq(expected.iter()));
        extended.extend([(0, 'e'), (3, 'f')]);
        let mut both = expected.clone();
        both.extend([(0, 'e'), (3, 'f')]);
        assert!(extended.iter().eq(both.iter()));
        let items = [3, 1, 3, 2, 1];
        let set: SortedSet<_> = items.into_iter().collect();
        assert!(set
            .iter()
            .eq(items.into_iter().collect::<BTreeSet<_>>().iter()));
    }
}
