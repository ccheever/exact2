//! Every observed property of every node, most of them held as a bit.
//!
//! A host restates four properties of every node it creates (receipt sync),
//! and almost every one is settled at its identity: an unmoved, unscaled,
//! unrotated, opaque box. Such a slot is only a bit in its node's word; an
//! entry exists for a slot that holds anything else (another target, a
//! presentation, a curve, an owner). Readers see a settled identity slot
//! either way, and a property the engine never observed stays apart from one
//! observed at its identity (CSS Transitions §3: only the latter has a
//! before-change style).

use super::{same, Slot, SlotHasher};
use crate::property::{Property, Value};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

type Key = (u64, Property);
type Map<K, V> = HashMap<K, V, BuildHasherDefault<SlotHasher>>;

#[derive(Debug)]
pub(super) struct Slots {
    /// The slots a bit cannot stand for.
    entries: Map<Key, Slot>,
    /// Which properties each node has a slot for, a bit a property:
    /// removing a node removes those slots, not a probe for every property
    /// there is.
    held: Map<u64, u32>,
    /// The two identities a bit stands for, for readers that borrow a slot.
    zero: Slot,
    one: Slot,
}

impl Default for Slots {
    fn default() -> Self {
        Slots {
            entries: Map::default(),
            held: Map::default(),
            zero: Slot::settled(Value::ZERO),
            one: Slot::settled(Value::scalar(1.0)),
        }
    }
}

impl Slots {
    /// The settled slot a bit stands for, for `property`.
    fn identity(&self, property: Property) -> Option<&Slot> {
        let identity = property.identity()?;
        [&self.zero, &self.one]
            .into_iter()
            .find(|slot| same(slot.target, identity))
    }

    /// The properties `node` has slots for, a bit a property.
    pub(super) fn held(&self, node: u64) -> u32 {
        self.held.get(&node).copied().unwrap_or(0)
    }

    pub(super) fn contains_key(&self, key: &Key) -> bool {
        self.held(key.0) & (1 << key.1 as u32) != 0
    }

    pub(super) fn get(&self, key: &Key) -> Option<&Slot> {
        match self.entries.get(key) {
            Some(slot) => Some(slot),
            None if self.contains_key(key) => self.identity(key.1),
            None => None,
        }
    }

    /// The slot, made an entry if a bit stood for it.
    pub(super) fn get_mut(&mut self, key: &Key) -> Option<&mut Slot> {
        if !self.entries.contains_key(key) {
            let implied = self.identity(key.1).filter(|_| self.contains_key(key))?;
            let slot = Slot::settled(implied.target);
            self.entries.insert(*key, slot);
        }
        self.entries.get_mut(key)
    }

    pub(super) fn insert(&mut self, key: Key, slot: Slot) {
        *self.held.entry(key.0).or_insert(0) |= 1 << key.1 as u32;
        if implied(key.1, &slot) {
            self.entries.remove(&key);
        } else {
            self.entries.insert(key, slot);
        }
    }

    pub(super) fn remove(&mut self, key: &Key) -> Option<Slot> {
        let bit = 1 << key.1 as u32;
        let held = self.held.get_mut(&key.0).filter(|held| **held & bit != 0)?;
        *held &= !bit;
        if *held == 0 {
            self.held.remove(&key.0);
        }
        self.entries
            .remove(key)
            .or_else(|| self.identity(key.1).cloned())
    }

    /// Make `key`'s entry a bit again if a bit can stand for it.
    pub(super) fn compact(&mut self, key: &Key) {
        if self
            .entries
            .get(key)
            .is_some_and(|slot| implied(key.1, slot))
        {
            self.entries.remove(key);
        }
    }

    /// Forget every slot of `node`.
    pub(super) fn remove_node(&mut self, node: u64) {
        let mut held = self.held.remove(&node).unwrap_or(0);
        while held != 0 {
            let property = Property::from_slot(held.trailing_zeros());
            held &= held - 1;
            self.entries.remove(&(node, property));
        }
    }

    /// The slots that are entries: every slot that has a presentation, a
    /// curve or an owner is one.
    pub(super) fn entries(&self) -> impl Iterator<Item = &Slot> {
        self.entries.values()
    }

    /// Give back the entries a bit can stand for again, and the tables'
    /// spare room once they hold more than twice what they need.
    pub(super) fn trim(&mut self) {
        self.entries.retain(|key, slot| !implied(key.1, slot));
        if self.entries.capacity() > 2 * self.entries.len().max(256) {
            self.entries.shrink_to_fit();
        }
        if self.held.capacity() > 2 * self.held.len().max(256) {
            self.held.shrink_to_fit();
        }
    }

    /// Observed properties, entries or bits.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.held
            .values()
            .map(|held| held.count_ones() as usize)
            .sum()
    }

    /// How many slots are entries.
    #[cfg(test)]
    pub(super) fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Whether a bit stands for `slot`: settled at `property`'s identity.
fn implied(property: Property, slot: &Slot) -> bool {
    let identity = |i: Value| same(i, slot.target) && i.oklab == slot.target.oklab;
    slot.live.is_none() && property.identity().is_some_and(identity)
}

impl std::ops::Index<&Key> for Slots {
    type Output = Slot;
    fn index(&self, key: &Key) -> &Slot {
        self.get(key).expect("an observed property")
    }
}
