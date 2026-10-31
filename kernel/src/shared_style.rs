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
//! ones). A held style's encoding is made again to compare, rather than
//! kept: it was a few hundred bytes a style. Two engine styles are equal as
//! the layout already compares them.
//!
//! Styles that differ elsewhere often hold equal rare or cold rows (a row of
//! lights, each with its own `animation` delay and the same
//! `animation-timeline`), so those groups are shared too ([`Groups`]).

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;
use std::sync::{Arc, Weak};

use crate::generated::{Cold, ColdRows, Rare, RareRows, StyleMask, StyleProps};
use crate::id::IdHasher;
use crate::wire::codec::Writer;

/// Values by a hash of their content. A value no one else holds is
/// forgotten at the next sweep.
#[derive(Debug, Clone)]
pub(crate) struct Interner<T> {
    table: HashMap<u64, Vec<Rc<T>>, BuildHasherDefault<IdHasher>>,
    entries: usize,
    /// Entries after the last sweep; the next sweep comes at twice this.
    swept: usize,
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Interner {
            table: HashMap::default(),
            entries: 0,
            swept: 0,
        }
    }
}

impl<T> Interner<T> {
    /// The held value `same` accepts under `hash`.
    pub(crate) fn get(&self, hash: u64, mut same: impl FnMut(&T) -> bool) -> Option<Rc<T>> {
        let bucket = self.table.get(&hash)?;
        bucket.iter().find(|v| same(v)).map(Rc::clone)
    }

    /// Hold `value` under `hash`.
    pub(crate) fn insert(&mut self, hash: u64, value: T) -> Rc<T> {
        let shared = Rc::new(value);
        self.table.entry(hash).or_default().push(Rc::clone(&shared));
        self.entries += 1;
        if self.entries > 2 * self.swept + 64 {
            self.sweep();
        }
        shared
    }

    /// Forget the values no one else holds.
    pub(crate) fn sweep(&mut self) {
        self.table.retain(|_, bucket| {
            bucket.retain(|v| Rc::strong_count(v) > 1);
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
    table: Interner<StyleProps>,
    /// The key being looked up.
    scratch: Writer,
    /// A held style's key, made again to compare with `scratch`.
    probe: Writer,
    groups: Groups,
    /// Each held style's wide colour slots, by its allocation, visited once:
    /// a style is never changed in place, and a dead one's entry no longer
    /// upgrades (the wide table's sweep, LLP 1121 stage 4).
    wide: HashMap<usize, (std::rc::Weak<StyleProps>, Box<[u16]>)>,
    /// Every held style's wide colours, a superset: added to as styles come,
    /// rebuilt when styles have gone (the table's own sweeps, amortized).
    wide_all: crate::style::wide::WideMark,
    /// Styles have gone since `wide_all` was built.
    wide_stale: bool,
}

impl Default for SharedStyles {
    fn default() -> Self {
        SharedStyles {
            default: Rc::new(StyleProps::default()),
            table: Interner::default(),
            scratch: Writer::new(),
            probe: Writer::new(),
            groups: Groups::default(),
            wide: HashMap::new(),
            wide_all: Default::default(),
            wide_stale: false,
        }
    }
}

/// A style's key: its set rows encoded, then what was written in `rem`/`em`,
/// which tells two styles apart even when their pixels agree (LLP 1069.000
/// D3).
fn key(style: &StyleProps, w: &mut Writer) {
    w.clear();
    style.encode_patch(w);
    relative_key(&style.relative, w);
}

fn relative_key(relative: &crate::style::relative::Relative, w: &mut Writer) {
    if !relative.is_empty() {
        let mut tail = Vec::new();
        relative.encode(&mut tail);
        w.bytes(&tail);
    }
}

impl SharedStyles {
    /// The initial style, shared by every node that sets no row.
    pub(crate) fn default_style(&self) -> Rc<StyleProps> {
        Rc::clone(&self.default)
    }

    /// The held style whose key is `scratch`'s.
    fn find(&mut self, hash: u64) -> Option<Rc<StyleProps>> {
        let (bytes, probe) = (self.scratch.as_slice(), &mut self.probe);
        self.table.get(hash, |held| {
            key(held, probe);
            probe.as_slice() == bytes
        })
    }

    /// Hold `style` under `hash`, its groups shared.
    fn hold(&mut self, hash: u64, mut style: StyleProps) -> Rc<StyleProps> {
        self.groups
            .share(&mut style, &mut self.scratch, &mut self.probe);
        let held = self.table.len();
        let style = self.table.insert(hash, style);
        self.noted(&style, held);
        style
    }

    /// One allocation for every style whose set rows equal `style`'s.
    pub(crate) fn intern(&mut self, style: StyleProps) -> Rc<StyleProps> {
        if style.mask.is_empty() && style.written.is_empty() {
            debug_assert!(
                style == *self.default,
                "an unset row holds its initial value"
            );
            return self.default_style();
        }
        key(&style, &mut self.scratch);
        let hash = hash_bytes(self.scratch.as_slice());
        if let Some(found) = self.find(hash) {
            debug_assert!(*found == style, "an unset row holds its initial value");
            return found;
        }
        self.hold(hash, style)
    }

    /// Look up a patch's result before cloning the full style. Repeated
    /// updates of equally styled nodes normally find the first node's result.
    pub(crate) fn patch(&mut self, base: &StyleProps, patch: &StyleProps) -> Rc<StyleProps> {
        let affected = patch.mask.union(patch.written);
        let mask = base.mask.minus(affected).union(patch.mask);
        let written = base.written.minus(affected).union(patch.written);
        if mask.is_empty() && written.is_empty() {
            return self.default_style();
        }
        self.scratch.clear();
        base.encode_overlaid(mask, written, patch, &mut self.scratch);
        if !base.relative.is_empty() || !patch.relative.is_empty() {
            let mut relative = base.relative.clone();
            relative.copy(&patch.relative, affected);
            relative_key(&relative, &mut self.scratch);
        }
        let hash = hash_bytes(self.scratch.as_slice());
        if let Some(found) = self.find(hash) {
            return found;
        }
        let mut next = base.clone();
        next.apply_patch(patch);
        self.hold(hash, next)
    }

    /// Look up the remaining rows before materializing a cleared style.
    pub(crate) fn clear(
        &mut self,
        base: &StyleProps,
        cleared: StyleMask,
        authored: bool,
    ) -> Rc<StyleProps> {
        let mask = base.mask.minus(cleared);
        let written =
            base.written
                .minus(cleared)
                .union(if authored { cleared } else { StyleMask::EMPTY });
        if mask.is_empty() && written.is_empty() {
            return self.default_style();
        }
        self.scratch.clear();
        base.encode_overlaid(mask, written, base, &mut self.scratch);
        if !base.relative.is_empty() {
            let mut relative = base.relative.clone();
            relative.copy(&self.default.relative, cleared);
            relative_key(&relative, &mut self.scratch);
        }
        let hash = hash_bytes(self.scratch.as_slice());
        if let Some(found) = self.find(hash) {
            return found;
        }
        let mut next = base.clone();
        next.clear(cleared);
        next.written = written;
        self.hold(hash, next)
    }

    /// Sweep, then return spare capacity.
    pub(crate) fn trim(&mut self) {
        self.wide_stale = true;
        self.table.trim();
        self.groups.sweep();
        self.wide.retain(|_, (weak, _)| weak.strong_count() > 0);
    }

    /// Distinct styles held beyond the initial one.
    pub(crate) fn len(&self) -> usize {
        self.table.len()
    }
}

/// The rare and cold rows of held styles, each distinct group once, by a
/// hash of every row in it encoded (so `-0` and `0` stay apart, as styles
/// do). One under a hash another live group already has stays the style's
/// own. Held weakly: a group no style holds is in no wide-colour mark (LLP
/// 1121 stage 4), so its colours' ids may be reclaimed and must not be read.
#[derive(Debug, Clone, Default)]
struct Groups {
    rare: HashMap<u64, Weak<RareRows>, BuildHasherDefault<IdHasher>>,
    cold: HashMap<u64, Weak<ColdRows>, BuildHasherDefault<IdHasher>>,
    /// Groups after the last sweep; the next comes at twice this.
    swept: usize,
}

impl Groups {
    /// Replace `style`'s groups with equal ones already held.
    fn share(&mut self, style: &mut StyleProps, ours: &mut Writer, theirs: &mut Writer) {
        if let Some(own) = style.rare.rows() {
            let rows = StyleMask::rare();
            ours.clear();
            style.encode_overlaid(rows, StyleMask::EMPTY, style, ours);
            let hash = hash_bytes(ours.as_slice());
            match self.rare.get(&hash).and_then(Weak::upgrade) {
                Some(held) if Arc::ptr_eq(&held, own) => {}
                Some(held) => {
                    let probe = StyleProps {
                        rare: Rare::share(held),
                        ..StyleProps::default()
                    };
                    theirs.clear();
                    probe.encode_overlaid(rows, StyleMask::EMPTY, &probe, theirs);
                    if theirs.as_slice() == ours.as_slice() {
                        style.rare = probe.rare;
                    }
                }
                None => {
                    self.rare.insert(hash, Arc::downgrade(own));
                }
            }
        }
        if let Some(own) = style.cold.rows() {
            let rows = StyleMask::cold();
            ours.clear();
            style.encode_overlaid(rows, StyleMask::EMPTY, style, ours);
            let hash = hash_bytes(ours.as_slice());
            match self.cold.get(&hash).and_then(Weak::upgrade) {
                Some(held) if Arc::ptr_eq(&held, own) => {}
                Some(held) => {
                    let probe = StyleProps {
                        cold: Cold::share(held),
                        ..StyleProps::default()
                    };
                    theirs.clear();
                    probe.encode_overlaid(rows, StyleMask::EMPTY, &probe, theirs);
                    if theirs.as_slice() == ours.as_slice() {
                        style.cold = probe.cold;
                    }
                }
                None => {
                    self.cold.insert(hash, Arc::downgrade(own));
                }
            }
        }
        if self.rare.len() + self.cold.len() > 2 * self.swept + 64 {
            self.sweep();
        }
    }

    /// Forget the groups no style holds.
    fn sweep(&mut self) {
        self.rare.retain(|_, rows| rows.strong_count() > 0);
        self.cold.retain(|_, rows| rows.strong_count() > 0);
        self.swept = self.rare.len() + self.cold.len();
    }
}

impl<T> Interner<T> {
    /// Every value held, whoever else holds it.
    pub(crate) fn rcs(&self) -> impl Iterator<Item = &Rc<T>> {
        self.table.values().flatten()
    }
}

impl SharedStyles {
    /// A style just held (the table held `before` until then): its wide
    /// colours join `wide_all`; one the insert's own sweep let go makes it
    /// stale.
    fn noted(&mut self, style: &Rc<StyleProps>, before: usize) {
        if self.table.len() <= before {
            // The insert swept: forget the dead styles' cached slots too.
            self.wide_stale = true;
            self.wide.retain(|_, (weak, _)| weak.strong_count() > 0);
        }
        if crate::style::wide::Holder::needed() {
            let slots = self.slots(style).to_vec();
            for slot in slots {
                self.wide_all.slot(slot);
            }
        }
    }

    /// One held style's wide colour slots, visited once.
    fn slots(&mut self, v: &Rc<StyleProps>) -> &[u16] {
        use crate::style::wide::Colors;
        let key = Rc::as_ptr(v) as usize;
        let fresh = self
            .wide
            .get(&key)
            .is_some_and(|(weak, _)| weak.upgrade().is_some_and(|w| Rc::ptr_eq(&w, v)));
        if !fresh {
            let mut own = crate::style::wide::WideMark::default();
            v.colors(&mut own);
            self.wide
                .insert(key, (Rc::downgrade(v), own.slots().into_boxed_slice()));
        }
        &self.wide[&key].1
    }

    /// Every style's colours, each visited (a clone's keep, once).
    pub(crate) fn visit(&self, m: &mut crate::style::wide::WideMark) {
        use crate::style::wide::Colors;
        self.default.colors(m);
        for v in self.table.rcs() {
            v.colors(m);
        }
    }

    /// Mark the colours the styles hold, the default's and every one in the
    /// table, whoever else holds it (one handed out again later still holds
    /// its ids). Every node's style is one of these: production writes go
    /// through `intern`, `patch` and `clear`. A superset between rebuilds,
    /// rebuilt once styles have gone.
    pub(crate) fn wide_colors(&mut self, m: &mut crate::style::wide::WideMark) {
        use crate::style::wide::Colors;
        self.default.colors(m);
        if self.wide_stale {
            self.table.sweep();
            self.wide.retain(|_, (weak, _)| weak.strong_count() > 0);
            let all: Vec<Rc<StyleProps>> = self.table.rcs().cloned().collect();
            let mut rebuilt = crate::style::wide::WideMark::default();
            for v in &all {
                for &slot in self.slots(v) {
                    rebuilt.slot(slot);
                }
            }
            drop(all);
            self.wide_all = rebuilt;
            self.wide_stale = false;
        }
        m.union(&self.wide_all);
    }
}

/// A hash of the rows most styles differ in; equality decides the rest.
pub(crate) fn engine_hash(s: &taffy::Style) -> u64 {
    use std::hash::Hasher;
    let mut h = crate::id::IdHasher::default();
    let mut len = |l: taffy::CompactLength| {
        h.write_u64(((l.tag() as u64) << 32) | u64::from(l.value().to_bits()));
    };
    len(s.size.width.into_raw());
    len(s.size.height.into_raw());
    len(s.min_size.width.into_raw());
    len(s.min_size.height.into_raw());
    len(s.flex_basis.into_raw());
    len(s.margin.left.into_raw());
    len(s.margin.top.into_raw());
    len(s.padding.left.into_raw());
    len(s.padding.top.into_raw());
    len(s.gap.width.into_raw());
    h.write_u64(u64::from(s.flex_grow.to_bits()) << 32 | u64::from(s.flex_shrink.to_bits()));
    h.write_u64(
        (s.display as u64)
            | (s.position as u64) << 8
            | (s.flex_direction as u64) << 16
            | (s.box_sizing as u64) << 24,
    );
    h.finish()
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
        let cleared = shared.clear(&authored, StyleMask::of(StyleId::Width), false);
        assert!(!cleared.written.has(StyleId::Width));
        assert!(cleared.written.has(StyleId::Height));
    }

    #[test]
    fn explicit_clear_intent_is_part_of_the_shared_style_identity() {
        let color = StyleMask::of(StyleId::TextColor);
        let mut shared = SharedStyles::default();
        let empty = shared.default_style();
        let mut clear = StyleProps::default();
        clear.written = color;
        let first = shared.intern(clear.clone());
        assert!(!Rc::ptr_eq(&empty, &first));
        assert!(Rc::ptr_eq(&first, &shared.intern(clear.clone())));
        assert!(Rc::ptr_eq(&first, &shared.patch(&empty, &clear)));
        let mut value = StyleProps::default();
        value
            .set_dynamic(StyleId::TextColor, &StyleValue::Text("#123456".into()))
            .unwrap();
        value.written = color;
        let authored = shared.patch(&first, &value);
        assert!(authored.mask.has(StyleId::TextColor));
        assert!(Rc::ptr_eq(&first, &shared.patch(&authored, &clear)));
        assert!(Rc::ptr_eq(&empty, &shared.clear(&first, color, false)));
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
            let first = shared.clear(&base, mask, false);
            assert_eq!(*first, expected, "{mask:?}");
            let authored = shared.intern(expected);
            assert!(Rc::ptr_eq(&first, &authored), "{mask:?}");
            let again = shared.clear(&base, mask, false);
            assert!(Rc::ptr_eq(&first, &again), "{mask:?}");
            assert_eq!(
                encoded(&base),
                original,
                "clear changed its source: {mask:?}"
            );
        }
    }

    #[test]
    fn styles_that_differ_share_their_equal_rare_and_cold_rows() {
        let style = |width: f64, z: f64| {
            let mut s = StyleProps::default();
            s.set_dynamic(StyleId::Width, &StyleValue::Number(width))
                .unwrap();
            s.set_dynamic(StyleId::ScrollPaddingTop, &StyleValue::Number(8.0))
                .unwrap();
            s.set_dynamic(StyleId::ZIndex, &StyleValue::Number(z))
                .unwrap();
            s
        };
        let rare = |s: &StyleProps| Arc::clone(s.rare.rows().unwrap());
        let cold = |s: &StyleProps| Arc::clone(s.cold.rows().unwrap());
        let mut shared = SharedStyles::default();
        let a = shared.intern(style(10.0, 1.0));
        let b = shared.intern(style(20.0, 1.0));
        assert!(!Rc::ptr_eq(&a, &b));
        assert!(Arc::ptr_eq(&rare(&a), &rare(&b)));
        assert!(Arc::ptr_eq(&cold(&a), &cold(&b)));
        let c = shared.patch(&b, &style(30.0, 2.0));
        assert!(Arc::ptr_eq(&rare(&a), &rare(&c)));
        assert!(!Arc::ptr_eq(&cold(&a), &cold(&c)));
        assert_eq!(*c, style(30.0, 2.0));
        assert_eq!(*b, style(20.0, 1.0));
        drop((a, b, c));
        shared.trim();
        assert!(shared.groups.rare.is_empty() && shared.groups.cold.is_empty());
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
