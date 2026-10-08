//! Publication-local typed canonical values; bounded collision scans fail open.
use super::Resolved;
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    hash::{Hash, Hasher},
    rc::Rc,
};
const SLOTS: usize = 128;
const BUCKET_SLOTS: usize = 8;
#[derive(Default)]
pub(super) struct ResolvedPool {
    buckets: BTreeMap<u64, Vec<Rc<Resolved>>>,
    slots: usize,
}
impl ResolvedPool {
    pub(super) fn intern(&mut self, value: Resolved) -> Rc<Resolved> {
        let mut hash = DefaultHasher::new();
        // The digest is a cheap congruent subset, never a semantic identity.
        // IEEE +0/-0 compare equal for typed cold fields, so normalize here.
        value.other.mask.words.hash(&mut hash);
        (if value.other.font_size == 0. {
            0
        } else {
            value.other.font_size.to_bits()
        })
        .hash(&mut hash);
        value.other.font_weight.hash(&mut hash);
        value.other.font_family.hash(&mut hash);
        value.colors.hash(&mut hash);
        for number in value.transform.iter().flatten() {
            number.to_bits().hash(&mut hash);
        }
        self.intern_at(hash.finish(), value)
    }
    fn intern_at(&mut self, digest: u64, value: Resolved) -> Rc<Resolved> {
        if let Some(previous) = self.buckets.get(&digest).and_then(|bucket| {
            bucket.iter().find(|old| {
                old.other == value.other
                    && old.colors == value.colors
                    && old
                        .transform
                        .iter()
                        .flatten()
                        .zip(value.transform.iter().flatten())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
            })
        }) {
            return previous.clone();
        }
        let shared = Rc::new(value);
        if self.slots < SLOTS && self.buckets.get(&digest).map_or(0, Vec::len) < BUCKET_SLOTS {
            self.slots += 1;
            self.buckets.entry(digest).or_default().push(shared.clone());
        }
        shared
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::{Dimension, StyleProps};
    fn value() -> Resolved {
        Resolved {
            _authored: Rc::new(StyleProps::default()),
            other: StyleProps::default(),
            colors: [[0; 2]; 6],
            transform: [[0.; 2]; 4],
        }
    }
    #[test]
    fn typed_values_share_across_authored_allocations_without_json() {
        let mut pool = ResolvedPool::default();
        let a = pool.intern(value());
        let b = pool.intern(value());
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(pool.slots, 1);
    }
    #[test]
    fn complete_equality_and_transform_bits_guard_digest_collisions() {
        let mut pool = ResolvedPool::default();
        let a = pool.intern_at(0, value());
        for mutation in 0..5 {
            let mut different = value();
            match mutation {
                0 => different.other.width = Dimension::Points(73.),
                1 => different.colors[0][0] = 1,
                2 => different.transform[0][0] = 1.,
                3 => different.other.font_family = 7,
                _ => different.transform[0][0] = -0.,
            }
            assert!(!Rc::ptr_eq(&a, &pool.intern_at(0, different)));
        }
        let mut plus = value();
        plus.other.font_size = 0.;
        let mut minus = value();
        minus.other.font_size = -0.;
        assert!(Rc::ptr_eq(&pool.intern(plus), &pool.intern(minus)));
    }
    #[test]
    fn index_slots_and_collision_scans_are_bounded() {
        let mut pool = ResolvedPool::default();
        for index in 0..1000 {
            let mut v = value();
            v.colors[0][0] = index;
            pool.intern_at(0, v);
        }
        assert_eq!(pool.slots, BUCKET_SLOTS);
        assert_eq!(pool.buckets[&0].len(), BUCKET_SLOTS);
        let mut pool = ResolvedPool::default();
        for index in 0..1000 {
            let mut v = value();
            v.colors[0][0] = index;
            pool.intern_at(index.into(), v);
        }
        assert_eq!(pool.slots, SLOTS);
    }
    #[test]
    fn memo_holds_its_own_authored_guard_when_canonical_origin_differs() {
        let mut pool = ResolvedPool::default();
        let first = pool.intern(value());
        let current = value();
        let current_authored = current._authored.clone();
        let weak = Rc::downgrade(&current_authored);
        let memo = super::super::StyleMemo {
            _authored: current_authored,
            value: pool.intern(current),
            json: None,
        };
        assert!(Rc::ptr_eq(&first, &memo.value));
        assert!(!Rc::ptr_eq(&first._authored, &memo._authored));
        assert!(weak.upgrade().is_some());
        drop(memo);
        assert!(weak.upgrade().is_none());
    }
}
