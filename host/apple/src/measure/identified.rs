//! Payload-free, per-measurer scalar memo. Catalog lifetime is this measurer's
//! lifetime: each boot/candidate constructs a new one before installing fonts.
//! Misses still execute the complete synchronous native measurement path.
use exact_kernel::{AxisOffer, NodeKey, ParagraphStamp, TextMetrics};
use std::collections::{HashMap, VecDeque};

const OWNERS: usize = 256;
const OFFERS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Definite(u32),
    MinContent,
    MaxContent,
}
impl From<AxisOffer> for Axis {
    fn from(offer: AxisOffer) -> Self {
        match offer {
            AxisOffer::Definite(value) => Self::Definite(value.to_bits()),
            AxisOffer::MinContent => Self::MinContent,
            AxisOffer::MaxContent => Self::MaxContent,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Offers(Axis, Axis);
struct Owner {
    stamp: ParagraphStamp,
    values: [Option<(Offers, TextMetrics)>; OFFERS],
    next: usize,
}
impl Owner {
    fn new(stamp: &ParagraphStamp) -> Self {
        Self {
            stamp: stamp.clone(),
            values: [None; OFFERS],
            next: 0,
        }
    }
    fn refresh(&mut self, stamp: &ParagraphStamp) {
        if !self.stamp.same_metrics(stamp) {
            self.values = [None; OFFERS];
            self.next = 0;
        }
        // Retain only the current proof, including after paint-only changes.
        self.stamp = stamp.clone();
    }
}

#[derive(Default)]
pub(super) struct Memo {
    owners: HashMap<NodeKey, Owner>,
    // FIFO contains each retained owner exactly once. Hits/revisions never
    // append history or walk/shift the owner table.
    order: VecDeque<NodeKey>,
}
impl Memo {
    pub(super) fn get(
        &mut self,
        stamp: &ParagraphStamp,
        width: AxisOffer,
        height: AxisOffer,
    ) -> Option<TextMetrics> {
        let owner = self.owners.get_mut(&stamp.owner())?;
        owner.refresh(stamp);
        let key = Offers(width.into(), height.into());
        owner
            .values
            .iter()
            .flatten()
            .find_map(|&(offers, metrics)| (offers == key).then_some(metrics))
    }
    pub(super) fn put(
        &mut self,
        stamp: &ParagraphStamp,
        width: AxisOffer,
        height: AxisOffer,
        metrics: TextMetrics,
    ) {
        let key = stamp.owner();
        if !self.owners.contains_key(&key) {
            if self.owners.len() == OWNERS {
                if let Some(old) = self.order.pop_front() {
                    self.owners.remove(&old);
                }
            }
            self.order.push_back(key);
            self.owners.insert(key, Owner::new(stamp));
        }
        let owner = self.owners.get_mut(&key).expect("inserted owner");
        owner.refresh(stamp);
        owner.values[owner.next] = Some((Offers(width.into(), height.into()), metrics));
        owner.next = (owner.next + 1) % OFFERS;
    }
    #[cfg(test)]
    pub(super) fn counts(&self) -> (usize, usize, usize) {
        (
            self.owners.len(),
            self.order.len(),
            self.owners
                .values()
                .map(|o| o.values.iter().flatten().count())
                .sum(),
        )
    }
}

#[cfg(test)]
#[path = "offers_tests.rs"]
mod offers_tests;
