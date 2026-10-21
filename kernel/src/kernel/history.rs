//! The complete receipt history, with ordered generation-checked key ranges.
//! Live commits still return ordinary receipts. Only explicit history reads
//! reconstruct their vectors; the rendering path does not read this ring.
use crate::{CommitReceipt, Exit, Handoff, NodeKey};

struct KeyRange {
    first: NodeKey,
    end: u32,
}

enum Keys {
    Raw(Box<[NodeKey]>),
    Ranges { len: usize, ranges: Box<[KeyRange]> },
}

fn follows(before: NodeKey, next: NodeKey) -> bool {
    before.generation == next.generation && before.index.checked_add(1) == Some(next.index)
}

impl Keys {
    fn new(keys: &[NodeKey]) -> Self {
        // Tiny lists do not earn a second pass or save an allocator size class.
        if keys.len() < 8 {
            return Self::Raw(keys.into());
        }
        let count = 1 + keys.windows(2).filter(|w| !follows(w[0], w[1])).count();
        if count * std::mem::size_of::<KeyRange>() >= std::mem::size_of_val(keys) {
            return Self::Raw(keys.into());
        }
        let mut ranges: Vec<KeyRange> = Vec::with_capacity(count);
        for &key in keys {
            if let Some(last) = ranges.last_mut().filter(|last| {
                last.first.generation == key.generation
                    && last.end.checked_add(1) == Some(key.index)
            }) {
                last.end = key.index;
            } else {
                ranges.push(KeyRange {
                    first: key,
                    end: key.index,
                });
            }
        }
        Self::Ranges {
            len: keys.len(),
            ranges: ranges.into_boxed_slice(),
        }
    }

    fn keys(&self) -> Vec<NodeKey> {
        match self {
            Self::Raw(keys) => keys.to_vec(),
            Self::Ranges { len, ranges } => {
                let mut keys = Vec::with_capacity(*len);
                for range in ranges {
                    keys.extend((range.first.index..=range.end).map(|index| NodeKey {
                        index,
                        generation: range.first.generation,
                    }));
                }
                keys
            }
        }
    }
}

pub(super) struct RetainedReceipt {
    batch: u64,
    root_id: u32,
    epoch: u64,
    created: Keys,
    destroyed: Keys,
    touched: Keys,
    layout_invalidated: bool,
    exits: Vec<Exit>,
    handoffs: Vec<Handoff>,
    display_changed: Keys,
    timelines: Keys,
    renewed: Keys,
    revealed: Keys,
}

impl RetainedReceipt {
    pub(super) fn new(r: &CommitReceipt) -> Self {
        Self {
            batch: r.batch,
            root_id: r.root_id,
            epoch: r.epoch,
            created: Keys::new(&r.created),
            destroyed: Keys::new(&r.destroyed),
            touched: Keys::new(&r.touched),
            layout_invalidated: r.layout_invalidated,
            exits: r.exits.clone(),
            handoffs: r.handoffs.clone(),
            display_changed: Keys::new(&r.display_changed),
            timelines: Keys::new(&r.timelines),
            renewed: Keys::new(&r.renewed),
            revealed: Keys::new(&r.revealed),
        }
    }

    pub(super) fn receipt(&self) -> CommitReceipt {
        CommitReceipt {
            batch: self.batch,
            root_id: self.root_id,
            epoch: self.epoch,
            created: self.created.keys(),
            destroyed: self.destroyed.keys(),
            touched: self.touched.keys(),
            layout_invalidated: self.layout_invalidated,
            exits: self.exits.clone(),
            handoffs: self.handoffs.clone(),
            display_changed: self.display_changed.keys(),
            timelines: self.timelines.keys(),
            renewed: self.renewed.keys(),
            revealed: self.revealed.keys(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Kernel, NodeType, Op};

    fn key(index: u32, generation: u32) -> NodeKey {
        NodeKey { index, generation }
    }

    #[test]
    fn ranges_preserve_order_duplicates_generations_and_index_boundaries() {
        let contiguous: Vec<_> = (0..128).map(|i| key(i, 7)).collect();
        assert!(matches!(Keys::new(&contiguous), Keys::Ranges { .. }));
        let sparse: Vec<_> = (0..128).map(|i| key(2 * i, 7)).collect();
        assert!(matches!(Keys::new(&sparse), Keys::Raw(_)));
        let mut mixed = contiguous.clone();
        mixed.extend([
            key(127, 7),
            key(128, 8),
            key(0, 8),
            key(u32::MAX, 8),
            key(0, 8),
        ]);
        mixed.extend((u32::MAX - 8..=u32::MAX).map(|i| key(i, u32::MAX)));
        for keys in [
            vec![],
            vec![key(u32::MAX, u32::MAX)],
            contiguous,
            sparse,
            mixed,
        ] {
            assert_eq!(Keys::new(&keys).keys(), keys);
        }
        let mut seed = 0xabcddcbau32;
        for len in 0..512 {
            let mut keys = Vec::new();
            let mut current = key(u32::MAX, u32::MAX);
            for _ in 0..len {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                match seed >> 29 {
                    0 => {}
                    1 => current.generation = current.generation.wrapping_add(1),
                    2 => current.index = u32::MAX,
                    3 => current.index = seed % 64,
                    _ => current.index = current.index.wrapping_add(1),
                }
                keys.push(current);
            }
            assert_eq!(Keys::new(&keys).keys(), keys);
        }
    }

    #[test]
    fn history_keeps_every_receipt_field() {
        let keys: Vec<_> = (0..128).map(|i| key(i, 99)).collect();
        let mut reverse = keys.clone();
        reverse.reverse();
        let receipt = CommitReceipt {
            batch: u64::MAX,
            root_id: 17,
            epoch: u64::MAX - 1,
            created: keys.clone(),
            destroyed: reverse,
            touched: keys[4..].to_vec(),
            layout_invalidated: true,
            exits: vec![Exit {
                key: key(11, 3),
                parent: key(2, 4),
                animations: Default::default(),
            }],
            handoffs: vec![Handoff {
                name: "history handoff".into(),
                from: key(11, 3),
                to: key(11, 4),
                transition: crate::Transitions::parse("300ms ease").unwrap().0.remove(0),
            }],
            display_changed: keys[..3].to_vec(),
            timelines: keys[5..100].to_vec(),
            renewed: keys.clone(),
            revealed: keys,
        };
        assert_eq!(RetainedReceipt::new(&receipt).receipt(), receipt);
    }

    #[test]
    fn the_ring_keeps_all_64_batches_with_destroyed_and_reused_generations() {
        let mut kernel = Kernel::with_monospace();
        let mut expected = Vec::new();
        for batch in 0..100 {
            let mut ops = Vec::new();
            if batch > 0 {
                ops.push(Op::DestroyView { id: 1 });
            }
            ops.push(Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            });
            expected.push(kernel.apply(3, batch, &ops).unwrap());
        }
        let actual: Vec<_> = kernel.receipts().collect();
        assert_eq!(actual.len(), super::super::RECEIPT_RING);
        assert_eq!(actual, expected[36..]);
        assert_ne!(actual[0].created[0], actual[0].destroyed[0]);
        kernel.reset();
        assert_eq!(kernel.receipts().count(), 0);
    }
}
