//! Decoded pixels kept between replays, bounded by their storage cost.
use std::collections::{BTreeMap, HashMap};
use vello::peniko::ImageData;

pub(crate) type Key = (u64, u32, u32);
const BUDGET: usize = 32 * 1024 * 1024;
// Charge the two indexes and their allocation overhead as well as pixels,
// so a stream of tiny images cannot retain millions of cache entries.
const ENTRY_COST: usize = 128;

#[derive(Default)]
pub(crate) struct Images {
    entries: HashMap<Key, (ImageData, u64)>,
    order: BTreeMap<u64, Key>,
    serial: u64,
    cost: usize,
    pub(crate) bytes: usize,
}

impl Images {
    fn stamp(&mut self) -> u64 {
        if self.serial == u64::MAX {
            self.clear();
        }
        self.serial += 1;
        self.serial
    }

    pub(crate) fn get(&mut self, key: &Key) -> Option<ImageData> {
        let stamp = self.stamp();
        let (image, previous) = self.entries.get_mut(key)?;
        self.order.remove(previous);
        self.order.insert(stamp, *key);
        *previous = stamp;
        Some(image.clone())
    }

    pub(crate) fn insert(&mut self, key: Key, image: ImageData) {
        let bytes = image.data.len();
        let Some(cost) = bytes.checked_add(ENTRY_COST).filter(|cost| *cost <= BUDGET) else {
            return;
        };
        let stamp = self.stamp();
        if let Some((old, previous)) = self.entries.remove(&key) {
            self.order.remove(&previous);
            self.bytes -= old.data.len();
            self.cost -= old.data.len() + ENTRY_COST;
        }
        while self.cost + cost > BUDGET {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some((old, _)) = self.entries.remove(&oldest) {
                self.bytes -= old.data.len();
                self.cost -= old.data.len() + ENTRY_COST;
            }
        }
        self.order.insert(stamp, key);
        self.entries.insert(key, (image, stamp));
        self.bytes += bytes;
        self.cost += cost;
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use vello::peniko::{Blob, ImageAlphaType, ImageFormat};

    fn image(bytes: usize) -> ImageData {
        ImageData {
            data: Blob::new(Arc::new(vec![0; bytes])),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::AlphaPremultiplied,
            width: (bytes / 4) as u32,
            height: 1,
        }
    }

    #[test]
    fn recency_replacement_oversize_and_trim_respect_the_cost_bound() {
        let mut cache = Images::default();
        let size = (BUDGET / 3) & !3;
        let a = image(size);
        cache.insert((1, 1, 1), a.clone());
        cache.insert((2, 1, 1), image(size));
        assert_eq!(cache.get(&(1, 1, 1)).unwrap().data.id(), a.data.id());
        cache.insert((3, 1, 1), image(size));
        assert!(cache.get(&(1, 1, 1)).is_some());
        assert!(
            cache.get(&(2, 1, 1)).is_none(),
            "a hit kept the older image hot"
        );
        cache.insert((1, 1, 1), image(4));
        assert_eq!(cache.bytes, size + 4);
        let before = cache.cost;
        cache.insert((4, 1, 1), image(BUDGET));
        assert!(
            cache.get(&(4, 1, 1)).is_none(),
            "oversized images are used without caching"
        );
        assert_eq!(cache.cost, before);
        cache.clear();
        assert_eq!(
            (
                cache.bytes,
                cache.cost,
                cache.entries.len(),
                cache.order.len()
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(a.data.len(), size, "live scenes retain their own pixels");
    }
}
