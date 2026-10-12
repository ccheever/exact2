//! Equal styles share one allocation.
//!
//! A node's [`StyleProps`] holds its rows, with effect groups shared on demand,
//! and its engine style holds Taffy's rows; most nodes of a list repeat a handful
//! of them — each row's name label is styled like every other row's. So
//! both are held by `Rc`, and a style is looked up here before it is kept:
//! equal styles are one allocation, as a browser shares computed styles. A
//! shared style is never written in place; a patch looks up the next style
//! before constructing it. Nothing here changes a value a reader sees.
//!
//! Two authored styles are equal when their set rows encode the same: a row
//! a style does not set holds its initial value, because the only writers
//! are a patch (which copies set rows) and a clear (which restores initial
//! ones). Two engine styles are equal as the layout already compares them.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;

use crate::generated::{StyleMask, StyleProps};
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

    /// Sweep, then return the table's spare capacity when it holds more
    /// than twice what it needs.
    pub(crate) fn trim(&mut self) {
        self.sweep();
        if self.table.capacity() > 2 * self.table.len().max(64) {
            self.table.shrink_to_fit();
        }
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
        // What was written in `rem`/`em` tells two styles apart even when
        // their pixels agree (LLP 1069.000 D3).
        let mut keyed;
        let bytes = if style.relative.is_empty() {
            self.scratch.as_slice()
        } else {
            keyed = self.scratch.as_slice().to_vec();
            style.relative.encode(&mut keyed);
            &keyed[..]
        };
        let hash = hash_bytes(bytes);
        if let Some(found) = self.table.get(hash, |b, _| **b == *bytes) {
            debug_assert!(*found == style, "an unset row holds its initial value");
            return found;
        }
        self.table.insert(hash, bytes.into(), style)
    }

    /// Look up a patch's result before cloning the full style. Repeated
    /// updates of equally styled nodes normally find the first node's result.
    pub(crate) fn patch(&mut self, base: &StyleProps, patch: &StyleProps) -> Rc<StyleProps> {
        let mask = base.mask.union(patch.mask);
        if mask.is_empty() {
            return self.default_style();
        }
        self.scratch.clear();
        base.encode_overlaid(mask, patch, &mut self.scratch);
        let mut keyed;
        let bytes = if base.relative.is_empty() && patch.relative.is_empty() {
            self.scratch.as_slice()
        } else {
            let mut relative = base.relative.clone();
            relative.copy(&patch.relative, patch.mask);
            keyed = self.scratch.as_slice().to_vec();
            relative.encode(&mut keyed);
            &keyed[..]
        };
        let hash = hash_bytes(bytes);
        if let Some(found) = self.table.get(hash, |b, _| **b == *bytes) {
            return found;
        }
        let mut next = base.clone();
        next.apply_patch(patch);
        self.table.insert(hash, bytes.into(), next)
    }

    /// Look up the remaining rows before materializing a cleared style.
    pub(crate) fn clear(&mut self, base: &StyleProps, cleared: StyleMask) -> Rc<StyleProps> {
        let mask = base.mask.minus(cleared);
        if mask.is_empty() {
            return self.default_style();
        }
        self.scratch.clear();
        base.encode_masked(mask, &mut self.scratch);
        let mut keyed;
        let bytes = if base.relative.is_empty() {
            self.scratch.as_slice()
        } else {
            let mut relative = base.relative.clone();
            relative.copy(&self.default.relative, cleared);
            keyed = self.scratch.as_slice().to_vec();
            relative.encode(&mut keyed);
            &keyed[..]
        };
        let hash = hash_bytes(bytes);
        if let Some(found) = self.table.get(hash, |b, _| **b == *bytes) {
            return found;
        }
        let mut next = base.clone();
        next.clear(cleared);
        self.table.insert(hash, bytes.into(), next)
    }

    /// Sweep, then return spare capacity.
    pub(crate) fn trim(&mut self) {
        self.table.trim();
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
    use crate::{StyleId, StyleMask, StyleValue};

    #[test]
    fn overlay_sharing_preserves_explicit_authorship() {
        let mut base = StyleProps::default();
        base.set_dynamic(StyleId::Width, &StyleValue::Number(10.0))
            .unwrap();
        base.set_dynamic(StyleId::Height, &StyleValue::Number(20.0))
            .unwrap();
        base.written = base.mask;
        let mut shared = SharedStyles::default();
        let authored = shared.intern(base.clone());
        let mut patch = StyleProps::default();
        patch
            .set_dynamic(StyleId::Width, &StyleValue::Number(10.0))
            .unwrap();
        let implied = shared.patch(&base, &patch);
        assert!(!Rc::ptr_eq(&authored, &implied));
        assert!(!implied.written.has(StyleId::Width));
        assert!(implied.written.has(StyleId::Height));
        patch.written = patch.mask;
        assert!(Rc::ptr_eq(&authored, &shared.patch(&implied, &patch)));
        let cleared = shared.clear(&authored, StyleMask::of(StyleId::Width));
        assert!(!cleared.written.has(StyleId::Width));
        assert!(cleared.written.has(StyleId::Height));
    }

    #[test]
    fn style_edits_share_exactly_the_materialized_result() {
        let values = [
            StyleValue::Number(0.0),
            StyleValue::Number(-0.0),
            StyleValue::Number(2.0),
            StyleValue::Percent(25.0),
            StyleValue::Vec2(2.0, -3.0),
        ]
        .into_iter()
        .chain(
            [
                "auto",
                "none",
                "inherit",
                "#123456",
                "bold",
                "hidden",
                "sticky",
                "2rem",
                "3em",
                "12px",
                "1fr 2fr",
                "1 / span 2",
                "dense",
                "center",
                "translate(2px, 3px)",
                "2px 3px #123456",
            ]
            .map(|text| StyleValue::Text(text.into())),
        )
        .collect::<Vec<_>>();
        let mut base = StyleProps::default();
        // Every row participates in the key, including defaults and rare rows.
        for id in StyleId::ALL {
            for value in values.iter().rev() {
                if base.set_dynamic(id, value).is_ok() {
                    break;
                }
            }
        }
        base.mask = StyleMask::ALL;
        let encoded = |style: &StyleProps| {
            let mut writer = Writer::new();
            style.encode_patch(&mut writer);
            writer.into_vec()
        };
        let original = encoded(&base);
        for id in StyleId::ALL {
            let mut patches = vec![StyleProps {
                mask: StyleMask::of(id),
                ..StyleProps::default()
            }];
            for value in &values {
                let mut patch = StyleProps::default();
                if patch.set_dynamic(id, value).is_ok() {
                    patches.push(patch);
                }
            }
            for patch in patches {
                let mut expected = base.clone();
                expected.apply_patch(&patch);
                let mut shared = SharedStyles::default();
                // First a miss; then an equal authored style and another patch hit.
                let first = shared.patch(&base, &patch);
                assert_eq!(*first, expected, "{id:?}");
                let authored = shared.intern(expected);
                assert!(Rc::ptr_eq(&first, &authored), "{id:?}");
                let again = shared.patch(&base, &patch);
                assert!(Rc::ptr_eq(&first, &again), "{id:?}");
                assert_eq!(encoded(&base), original, "patch changed its source: {id:?}");
            }
        }
        for mask in StyleId::ALL
            .into_iter()
            .map(StyleMask::of)
            .chain([StyleMask::EMPTY, StyleMask::ALL])
        {
            let mut expected = base.clone();
            expected.clear(mask);
            let mut shared = SharedStyles::default();
            let first = shared.clear(&base, mask);
            assert_eq!(*first, expected, "{mask:?}");
            let authored = shared.intern(expected);
            assert!(Rc::ptr_eq(&first, &authored), "{mask:?}");
            let again = shared.clear(&base, mask);
            assert!(Rc::ptr_eq(&first, &again), "{mask:?}");
            assert_eq!(
                encoded(&base),
                original,
                "clear changed its source: {mask:?}"
            );
        }
    }

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
        // The same row its author wrote is another style (LLP 1115.000 D2):
        // one node's bit is never another's.
        let mut written = a.clone();
        written.written.set(crate::StyleId::Width);
        let w = shared.intern(written.clone());
        assert!(!Rc::ptr_eq(&x, &w));
        assert!(w.written.has(crate::StyleId::Width) && !x.written.has(crate::StyleId::Width));
        assert!(Rc::ptr_eq(&w, &shared.intern(written)));
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
        assert_eq!(shared.len(), 5);
        drop((x, y, z, w, c, d));
        shared.trim();
        assert_eq!(shared.len(), 0);
    }
}
