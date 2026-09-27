//! Equal styles share one allocation.
//!
//! A node's [`StyleProps`] holds every row, about 1.2 KB, and its engine
//! style (Taffy's) another 550 bytes; most nodes of a list repeat a handful
//! of them — each row's name label is styled like every other row's. So
//! both are held by `Rc`, and a style is looked up here before it is kept:
//! equal styles are one allocation, as a browser shares computed styles. A
//! shared style is never written in place; a change builds the next style
//! and interns it. Nothing here changes a value a reader sees.
//!
//! Two authored styles are equal when their set rows encode the same: a row
//! a style does not set holds its initial value, because the only writers
//! are a patch (which copies set rows) and a clear (which restores initial
//! ones). Two engine styles are equal as the layout already compares them.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;

use crate::generated::StyleProps;
use crate::id::IdHasher;
use crate::wire::codec::Writer;

/// The values under one hash, each with what equality reads.
type Bucket<T, K> = Vec<(K, Rc<T>)>;

/// Values by a hash of their content, each with what equality reads (`K`).
/// A value no one else holds is forgotten at the next sweep.
#[derive(Debug, Clone)]
pub(crate) struct Interner<T, K> {
    table: HashMap<u64, Bucket<T, K>, BuildHasherDefault<IdHasher>>,
    entries: usize,
    /// Entries after the last sweep; the next sweep comes at twice this.
    swept: usize,
}

impl<T, K> Default for Interner<T, K> {
    fn default() -> Self {
        Interner {
            table: HashMap::default(),
            entries: 0,
            swept: 0,
        }
    }
}

impl<T, K> Interner<T, K> {
    /// The held value `same` accepts under `hash`.
    pub(crate) fn get(&self, hash: u64, same: impl Fn(&K, &T) -> bool) -> Option<Rc<T>> {
        let bucket = self.table.get(&hash)?;
        bucket
            .iter()
            .find(|(k, v)| same(k, v))
            .map(|(_, v)| Rc::clone(v))
    }

    /// Hold `value` under `hash`, with what equality reads of it.
    pub(crate) fn insert(&mut self, hash: u64, key: K, value: T) -> Rc<T> {
        let shared = Rc::new(value);
        self.table
            .entry(hash)
            .or_default()
            .push((key, Rc::clone(&shared)));
        self.entries += 1;
        if self.entries > 2 * self.swept + 64 {
            self.sweep();
        }
        shared
    }

    /// Forget the values no one else holds.
    pub(crate) fn sweep(&mut self) {
        self.table.retain(|_, bucket| {
            bucket.retain(|(_, v)| Rc::strong_count(v) > 1);
            !bucket.is_empty()
        });
        self.entries = self.table.values().map(Vec::len).sum();
        self.swept = self.entries;
    }

    /// Distinct values held.
    pub(crate) fn len(&self) -> usize {
        self.entries
    }
}

/// A hash of `bytes`, eight at a time.
pub(crate) fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = IdHasher::default();
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        hasher.write_u64(u64::from_le_bytes(chunk.try_into().expect("eight bytes")));
    }
    hasher.write(chunks.remainder());
    hasher.finish()
}

/// The arena's authored styles.
#[derive(Debug, Clone)]
pub(crate) struct SharedStyles {
    default: Rc<StyleProps>,
    table: Interner<StyleProps, Box<[u8]>>,
    scratch: Writer,
}

impl Default for SharedStyles {
    fn default() -> Self {
        SharedStyles {
            default: Rc::new(StyleProps::default()),
            table: Interner::default(),
            scratch: Writer::new(),
        }
    }
}

impl SharedStyles {
    /// The initial style, shared by every node that sets no row.
    pub(crate) fn default_style(&self) -> Rc<StyleProps> {
        Rc::clone(&self.default)
    }

    /// One allocation for every style whose set rows equal `style`'s.
    pub(crate) fn intern(&mut self, style: StyleProps) -> Rc<StyleProps> {
        if style.mask.is_empty() {
            debug_assert!(
                style == *self.default,
                "an unset row holds its initial value"
            );
            return self.default_style();
        }
        self.scratch.clear();
        style.encode_patch(&mut self.scratch);
        let bytes = self.scratch.as_slice();
        let hash = hash_bytes(bytes);
        if let Some(found) = self.table.get(hash, |b, _| **b == *bytes) {
            debug_assert!(*found == style, "an unset row holds its initial value");
            return found;
        }
        self.table.insert(hash, bytes.into(), style)
    }

    /// Distinct styles held beyond the initial one.
    pub(crate) fn len(&self) -> usize {
        self.table.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Dimension;

    #[test]
    fn equal_styles_share_and_unheld_ones_are_swept() {
        let mut shared = SharedStyles::default();
        let mut a = StyleProps::default();
        a.width = Dimension::Points(10.0);
        a.mask.set(crate::StyleId::Width);
        let x = shared.intern(a.clone());
        let y = shared.intern(a.clone());
        assert!(Rc::ptr_eq(&x, &y));
        let mut b = StyleProps::default();
        b.height = Dimension::Points(10.0);
        b.mask.set(crate::StyleId::Height);
        let z = shared.intern(b);
        assert!(!Rc::ptr_eq(&x, &z));
        // -0 and 0 compare equal but encode apart: never shared.
        let mut c = StyleProps::default();
        c.width = Dimension::Points(-0.0);
        c.mask.set(crate::StyleId::Width);
        let mut d = c.clone();
        d.width = Dimension::Points(0.0);
        let (c, d) = (shared.intern(c), shared.intern(d));
        assert!(!Rc::ptr_eq(&c, &d));
        assert!(Rc::ptr_eq(
            &shared.intern(StyleProps::default()),
            &shared.default
        ));
        assert_eq!(shared.len(), 4);
        drop((x, y, z, c, d));
        shared.table.sweep();
        assert_eq!(shared.len(), 0);
    }
}
