//! Each exact identity owns one immutable source; width snapshots share it.
//! Accounting separates canonical key K, shared source S, width layout L and ink.
//! Sources of pinned widths are live storage, not multiplied cold-policy costs.
//! Width snapshots are weakly indexed; cold entries and bounded handoffs are owned.
//! A painter pins each accepted generational node independently of text identity.
//! A new width retires the previous unpinned widths of that exact identity before
//! allocation. At most 64 identities keep their latest definite measurement until
//! the next paint attempt ends. This transient category has a count cap, not a byte
//! cap. Intrinsic misses retire their identity's handoff before allocating scratch;
//! cached scalar hits do not. Other widths/offers are not promised retention.
//! The cold target is enforced on maintenance, not on last-caller drop or hits:
//! an oversized raw lookup result can remain cold and over target while idle.
//! Handoffs and externally pinned snapshots are additional live storage; neither
//! their bytes nor paragraph size are bounded by the cold target.
//! Paragraph costs include current lazy CPU ink capacity in O(1); diagnostics and
//! maintenance run outside paint/build while its exclusive ink borrow is released.
//! At most 256 payload-free stamp shortcuts refer to current canonical identities.
//! They own neither text nor paragraph; revision replacement and cold eviction
//! retire them. Their vector capacity is counted in key/owned diagnostics, not
//! added to the existing cold-paragraph budget. Catalog replacement drops all.
use super::{Paragraph, Run, ShapedSource, Spec};
use exact_kernel::{ParagraphStamp, TextMetrics};
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::mem::size_of;
use std::rc::{Rc, Weak};

pub(super) const COLD_BYTES: usize = 64 * 1024 * 1024;
pub(super) const COLD_IDENTITIES: usize = 256;
pub(super) const HANDOFF_IDENTITIES: usize = 64;

/// Transient measured storage awaiting a paint attempt. Unique backings are
/// counted once here, but can also belong to accepted owners/residency: do not
/// add these categories to claim a total. Keys/font scratch/allocator are excluded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HandoffResidency {
    /// Distinct exact content/metric identities with pending measurements.
    pub identities: usize,
    /// Count limit, not a byte limit or a bound on all live paragraph owners.
    pub identity_limit: usize,
    /// Unique backing allocations in this transient set.
    pub paragraphs: usize,
    /// Exact accessible vector capacities of those unique backings.
    pub owned_capacity_bytes: usize,
    /// Zero: source Strings now expose capacity, counted in owned bytes.
    pub private_text_bytes_estimate: usize,
    /// Accessible capacities plus private text-length estimates, excluding keys.
    pub policy_bytes: usize,
    /// Comparison only: the cold target does not limit live handoff storage.
    pub cold_target_reference_bytes: usize,
    /// Positive excess over that reference, not a violated handoff byte limit.
    pub above_cold_target_bytes: usize,
}

/// Catalog-local paragraph storage. Vector/String capacities below are exact
/// accessible storage, not allocator/RSS accounting. Cosmic private caches,
/// font-system scratch, font data and glyph caches are outside this count.
#[derive(Clone, Copy, Debug, Default)]
pub struct Residency {
    /// Exact visible vector capacities of indexed paragraphs and canonical keys.
    pub owned_capacity_bytes: usize,
    /// Zero for moved source Strings; private cosmic internals remain excluded.
    pub private_text_bytes_estimate: usize,
    /// Unique live width snapshots, including cache and painter owners.
    pub paragraphs: usize,
    /// Snapshots retained by a caller, frame, or measured handoff. These overlap
    /// HandoffResidency; they are not an accepted-frame-only count.
    pub pinned_paragraphs: usize,
    /// Snapshots owned only by the cache.
    pub cold_paragraphs: usize,
    /// Cold source and layout capacities, source counted once; excludes keys.
    pub cold_owned_capacity_bytes: usize,
    /// Maintenance policy cost: cold accessible capacities + private text length
    /// estimates + canonical keys with no pinned width. Not exact resident bytes.
    pub cold_policy_bytes: usize,
    /// Soft maintenance-time cold target, excluding externally pinned snapshots.
    pub cold_target_bytes: usize,
    /// Policy cost above the target now. Last-caller drop/hits do not trim;
    /// measurement-to-paint handoff may leave this nonzero until maintenance.
    pub cold_overage_bytes: usize,
    /// Canonical exact-text/metric identities, not visited widths.
    pub identities: usize,
    /// Cached intrinsic scalar answers. At most two per identity.
    pub intrinsic_metrics: usize,
    /// Exact capacity of canonical UTF8/run vectors and bounded stamp shortcuts.
    pub key_capacity_bytes: usize,
}

/// Accepted painter storage outside the current catalog's paragraph index.
/// Shared Rc backings are counted once; this excludes keys, fonts, private cosmic
/// caches, allocator overhead and other engines/callers. This is not total RSS.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetiringResidency {
    /// Accepted generational node owners outside the current catalog.
    pub owners: usize,
    /// Unique paragraph backings shared by those owners.
    pub paragraphs: usize,
    /// Exact accessible vector capacities of those unique paragraph backings.
    pub owned_capacity_bytes: usize,
    /// Zero for moved source Strings; private cosmic internals remain excluded.
    pub private_text_bytes_estimate: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(super) struct Width(Option<u32>);
impl From<Option<f32>> for Width {
    fn from(width: Option<f32>) -> Self {
        Self(width.map(f32::to_bits))
    }
}

struct Snapshot {
    weak: Weak<Paragraph>,
    cold: Option<Rc<Paragraph>>,
    used: u64,
}
impl Snapshot {
    fn pinned(&self) -> bool {
        self.weak.strong_count() > usize::from(self.cold.is_some())
    }
}
struct Identity {
    id: u64,
    spec: Rc<Spec>,
    source: Option<Rc<ShapedSource>>,
    widths: HashMap<Width, Snapshot>,
    intrinsic: [Option<TextMetrics>; 2],
    used: u64,
}

struct Handoff {
    identity: u64,
    width: Width,
    paragraph: Rc<Paragraph>,
}
impl Identity {
    fn pinned(&self) -> bool {
        self.widths.values().any(Snapshot::pinned)
    }
    fn source_bytes(&self) -> usize {
        self.source
            .as_ref()
            .map_or(0, |s| s.accessible_capacity_bytes)
    }
    fn key_bytes(&self) -> usize {
        self.spec.runs.capacity() * size_of::<Run>()
            + self.spec.strut.text.capacity()
            + self
                .spec
                .runs
                .iter()
                .map(|r| r.text.capacity())
                .sum::<usize>()
    }
}

pub(super) struct Cache {
    identities: HashMap<u64, Vec<Identity>>,
    serial: u64,
    clock: u64,
    target: usize,
    // Oldest measurement first; refreshed deterministically, never width history.
    handoffs: Vec<Handoff>,
    // Non-owning shortcuts, one latest metric identity per owner; no revisions.
    bindings: Vec<(ParagraphStamp, (u64, u64))>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            identities: HashMap::new(),
            serial: 0,
            clock: 0,
            target: COLD_BYTES,
            handoffs: Vec::new(),
            bindings: Vec::new(),
        }
    }
}
impl Cache {
    /// The catalog owns these specs; shortcut handles never retain them.
    pub fn spec(&self, key: (u64, u64)) -> Option<Rc<Spec>> {
        self.identities
            .get(&key.0)?
            .iter()
            .find(|e| e.id == key.1)
            .map(|e| e.spec.clone())
    }
    pub fn identified(&mut self, stamp: &ParagraphStamp) -> Option<(u64, u64)> {
        let i = self
            .bindings
            .iter()
            .position(|(old, _)| old.same_metrics(stamp))?;
        let (_, key) = self.bindings.remove(i);
        self.spec(key)?; // Evicted identities are misses, never unchecked handles.
        self.clock += 1;
        self.entry(key).used = self.clock;
        self.bindings.push((stamp.clone(), key));
        Some(key)
    }
    pub fn bind(&mut self, stamp: &ParagraphStamp, key: (u64, u64)) {
        // Equal NodeKeys in different domains can replace a shortcut, never
        // alias: lookup above compares the complete metric proof. Correctness
        // falls back to exact content; this table is only a bounded accelerator.
        self.bindings
            .retain(|(old, _)| old.owner() != stamp.owner());
        if self.bindings.len() == COLD_IDENTITIES {
            self.bindings.remove(0);
        }
        self.bindings.push((stamp.clone(), key));
    }
    fn prune_bindings(&mut self) {
        let identities = &self.identities;
        self.bindings.retain(|(_, key)| {
            identities
                .get(&key.0)
                .is_some_and(|bucket| bucket.iter().any(|e| e.id == key.1))
        });
    }

    pub fn prepare_handoff(&mut self, identity: u64, width: Width) {
        if let Some(index) = self.handoffs.iter().position(|h| h.identity == identity) {
            if self.handoffs[index].width == width {
                return;
            }
            self.handoffs.remove(index);
        }
        if self.handoffs.len() == HANDOFF_IDENTITIES {
            self.handoffs.remove(0);
        }
    }

    pub fn hold_measured(&mut self, identity: u64, width: Width, paragraph: &Rc<Paragraph>) {
        self.release_handoff(identity);
        if self.handoffs.len() == HANDOFF_IDENTITIES {
            self.handoffs.remove(0);
        }
        self.handoffs.push(Handoff {
            identity,
            width,
            paragraph: paragraph.clone(),
        });
    }

    pub fn release_handoff(&mut self, identity: u64) {
        if let Some(index) = self.handoffs.iter().position(|h| h.identity == identity) {
            self.handoffs.remove(index);
        }
    }

    pub fn finish_handoff(&mut self) {
        self.handoffs.clear();
    }

    pub fn handoff_residency(&self) -> HandoffResidency {
        let mut result = HandoffResidency {
            identities: self.handoffs.len(),
            identity_limit: HANDOFF_IDENTITIES,
            cold_target_reference_bytes: self.target,
            ..HandoffResidency::default()
        };
        let mut unique = HashSet::new();
        let mut sources = HashSet::new();
        for h in &self.handoffs {
            if unique.insert(Rc::as_ptr(&h.paragraph)) {
                result.paragraphs += 1;
                result.owned_capacity_bytes += h.paragraph.layout_capacity_bytes();
                if sources.insert(Rc::as_ptr(&h.paragraph.source)) {
                    result.owned_capacity_bytes += h.paragraph.source.accessible_capacity_bytes;
                }
                result.private_text_bytes_estimate += h.paragraph.private_text_bytes_estimate;
            }
        }
        result.policy_bytes = result.owned_capacity_bytes + result.private_text_bytes_estimate;
        result.above_cold_target_bytes = result.policy_bytes.saturating_sub(self.target);
        result
    }

    pub fn identity(&mut self, spec: &Spec) -> (u64, u64) {
        let hash = fingerprint(spec);
        self.clock += 1;
        if let Some(entry) = self
            .identities
            .get_mut(&hash)
            .and_then(|bucket| bucket.iter_mut().find(|e| equal(&e.spec, spec)))
        {
            entry.used = self.clock;
            return (hash, entry.id);
        }
        self.trim(None);
        self.serial += 1;
        self.identities.entry(hash).or_default().push(Identity {
            id: self.serial,
            spec: Rc::new(spec.clone()),
            source: None,
            widths: HashMap::new(),
            intrinsic: [None; 2],
            used: self.clock,
        });
        (hash, self.serial)
    }
    fn entry(&mut self, key: (u64, u64)) -> &mut Identity {
        self.identities
            .get_mut(&key.0)
            .unwrap()
            .iter_mut()
            .find(|e| e.id == key.1)
            .unwrap()
    }
    pub fn source(&mut self, key: (u64, u64)) -> Option<Rc<ShapedSource>> {
        self.entry(key).source.clone()
    }
    pub fn set_source(&mut self, key: (u64, u64), source: Rc<ShapedSource>) {
        let entry = self.entry(key);
        debug_assert!(entry.source.is_none());
        entry.source = Some(source);
    }
    pub fn get(&mut self, key: (u64, u64), width: Width) -> Option<Rc<Paragraph>> {
        self.clock += 1;
        let clock = self.clock;
        let snapshot = self.entry(key).widths.get_mut(&width)?;
        snapshot.used = clock;
        snapshot.weak.upgrade()
    }
    /// BEFORE allocating a new width layout: every unpinned old width of this exact
    /// identity dies; its immutable shape remains. Pinned widths stay indexed.
    pub fn before_shape(&mut self, key: (u64, u64)) {
        let entry = self.entry(key);
        for value in entry.widths.values_mut() {
            value.cold = None;
        }
        entry
            .widths
            .retain(|_, value| value.weak.strong_count() != 0);
        self.trim(Some(key.1));
    }
    pub fn insert(&mut self, key: (u64, u64), width: Width, p: &Rc<Paragraph>) {
        self.clock += 1;
        let used = self.clock;
        self.entry(key).widths.insert(
            width,
            Snapshot {
                weak: Rc::downgrade(p),
                cold: Some(p.clone()),
                used,
            },
        );
        // The caller owns the current working snapshot. It may exceed the soft
        // cold target; no text/geometry is refused or shortened to meet it.
        self.trim(Some(key.1));
    }
    pub fn intrinsic(&mut self, key: (u64, u64), minimum: bool) -> Option<TextMetrics> {
        self.entry(key).intrinsic[usize::from(minimum)]
    }
    pub fn set_intrinsic(&mut self, key: (u64, u64), minimum: bool, metrics: TextMetrics) {
        self.entry(key).intrinsic[usize::from(minimum)] = Some(metrics);
        self.trim(Some(key.1));
    }
    pub fn residency(&self) -> Residency {
        let mut result = Residency {
            cold_target_bytes: self.target,
            ..Residency::default()
        };
        for entry in self.identities.values().flatten() {
            result.identities += 1;
            result.key_capacity_bytes += entry.key_bytes();
            let source_bytes = entry.source_bytes();
            result.owned_capacity_bytes += source_bytes;
            if !entry.pinned() {
                result.cold_owned_capacity_bytes += source_bytes;
                result.cold_policy_bytes += entry.key_bytes() + source_bytes;
            }
            result.intrinsic_metrics += entry.intrinsic.iter().flatten().count();
            for slot in entry.widths.values() {
                let pinned = slot.pinned();
                let Some(paragraph) = slot.weak.upgrade() else {
                    continue;
                };
                let owned = paragraph.layout_capacity_bytes();
                result.paragraphs += 1;
                result.owned_capacity_bytes += owned;
                result.private_text_bytes_estimate += paragraph.private_text_bytes_estimate;
                if pinned {
                    result.pinned_paragraphs += 1;
                } else {
                    result.cold_paragraphs += 1;
                    result.cold_owned_capacity_bytes += owned;
                    result.cold_policy_bytes += owned + paragraph.private_text_bytes_estimate;
                }
            }
        }
        result.owned_capacity_bytes += result.key_capacity_bytes;
        // Bounded shortcut metadata owns no source or paragraph. Count its
        // allocated vector capacity separately from the cold paragraph policy.
        let bindings = self.bindings.capacity() * size_of::<(ParagraphStamp, (u64, u64))>();
        result.key_capacity_bytes += bindings;
        result.owned_capacity_bytes += bindings;
        result.cold_overage_bytes = result.cold_policy_bytes.saturating_sub(self.target);
        result
    }
    pub fn retiring<'a>(
        &self,
        accepted: impl Iterator<Item = &'a Rc<Paragraph>>,
    ) -> RetiringResidency {
        // Diagnostic-only scan. Pointer sets neither retain a catalog nor add a
        // persistent owner/history index, and no Rc upgrade changes pin counts.
        let current: HashSet<_> = self
            .identities
            .values()
            .flatten()
            .flat_map(|e| e.widths.values().map(|s| s.weak.as_ptr()))
            .collect();
        let mut seen = HashSet::new();
        let mut sources = HashSet::new();
        let mut result = RetiringResidency::default();
        for paragraph in accepted {
            let pointer = Rc::as_ptr(paragraph);
            if current.contains(&pointer) {
                continue;
            }
            result.owners += 1;
            if seen.insert(pointer) {
                result.paragraphs += 1;
                result.owned_capacity_bytes += paragraph.layout_capacity_bytes();
                if sources.insert(Rc::as_ptr(&paragraph.source)) {
                    result.owned_capacity_bytes += paragraph.source.accessible_capacity_bytes;
                }
                result.private_text_bytes_estimate += paragraph.private_text_bytes_estimate;
            }
        }
        result
    }
    /// Drop cold/transient ownership, preserving weak dedup for frame-owned ones.
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.handoffs.clear();
        for entry in self.identities.values_mut().flatten() {
            for snapshot in entry.widths.values_mut() {
                snapshot.cold = None;
            }
        }
        self.identities.retain(|_, bucket| {
            bucket.retain(Identity::pinned);
            !bucket.is_empty()
        });
        self.prune_bindings();
    }
    pub fn trim(&mut self, keep: Option<u64>) {
        let mut cold = Vec::new();
        let mut bytes = 0;
        let mut cold_keys = Vec::new();
        for (hash, bucket) in &mut self.identities {
            for entry in bucket {
                entry
                    .widths
                    .retain(|_, value| value.weak.strong_count() != 0);
                if !entry.pinned() {
                    bytes += entry.key_bytes() + entry.source_bytes();
                    if Some(entry.id) != keep {
                        cold_keys.push((entry.used, *hash, entry.id));
                    }
                }
                for (width, slot) in &entry.widths {
                    if !slot.pinned() {
                        if let Some(p) = &slot.cold {
                            let cost = p.layout_capacity_bytes() + p.private_text_bytes_estimate;
                            bytes += cost;
                            cold.push((slot.used, *hash, entry.id, *width, cost));
                        }
                    }
                }
            }
        }
        cold.sort_unstable_by_key(|v| v.0);
        for (_, hash, id, width, cost) in cold {
            if bytes <= self.target {
                break;
            }
            self.entry((hash, id)).widths.remove(&width);
            bytes -= cost;
        }
        cold_keys.sort_unstable();
        let mut count = cold_keys.len() + usize::from(keep.is_some());
        for (_, hash, id) in cold_keys {
            if count <= COLD_IDENTITIES && bytes <= self.target {
                break;
            }
            let bucket = self.identities.get_mut(&hash).unwrap();
            let pos = bucket.iter().position(|e| e.id == id).unwrap();
            let old = bucket.remove(pos);
            bytes = bytes.saturating_sub(old.key_bytes() + old.source_bytes());
            count -= 1;
            if bucket.is_empty() {
                self.identities.remove(&hash);
            }
        }
        self.prune_bindings();
    }
    #[cfg(test)]
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
    #[cfg(test)]
    pub fn forget_bindings(&mut self) {
        self.bindings.clear();
    }
    #[cfg(test)]
    pub fn indexed_widths(&self) -> usize {
        self.identities
            .values()
            .flatten()
            .map(|e| e.widths.len())
            .sum()
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.residency().paragraphs
    }
    #[cfg(test)]
    pub fn set_target(&mut self, bytes: usize) {
        self.target = bytes;
        self.trim(None);
    }
}

fn fingerprint(spec: &Spec) -> u64 {
    let mut h = DefaultHasher::new();
    std::mem::discriminant(&spec.align).hash(&mut h);
    std::mem::discriminant(&spec.overflow_wrap).hash(&mut h);
    spec.line_clamp.hash(&mut h);
    spec.runs.len().hash(&mut h);
    for run in std::iter::once(&spec.strut).chain(&spec.runs) {
        #[cfg(test)]
        super::identified_tests::hashed(run.text.len());
        run.text.hash(&mut h);
        run.size.to_bits().hash(&mut h);
        run.weight.hash(&mut h);
        run.family.hash(&mut h);
        run.italic.hash(&mut h);
        run.line_height.map(f32::to_bits).hash(&mut h);
        run.letter_spacing.to_bits().hash(&mut h);
    }
    h.finish()
}
fn equal(a: &Spec, b: &Spec) -> bool {
    a.align == b.align
        && a.overflow_wrap == b.overflow_wrap
        && a.line_clamp == b.line_clamp
        && a.runs.len() == b.runs.len()
        && std::iter::once(&a.strut)
            .chain(&a.runs)
            .zip(std::iter::once(&b.strut).chain(&b.runs))
            .all(|(a, b)| {
                a.text == b.text
                    && a.size.to_bits() == b.size.to_bits()
                    && a.weight == b.weight
                    && a.family == b.family
                    && a.italic == b.italic
                    && a.line_height.map(f32::to_bits) == b.line_height.map(f32::to_bits)
                    && a.letter_spacing.to_bits() == b.letter_spacing.to_bits()
            })
}

pub(super) fn capacities(paragraph: &Paragraph) -> usize {
    fn vector<T>(v: &Vec<T>) -> usize {
        v.capacity() * size_of::<T>()
    }
    let mut bytes = paragraph.source.accessible_capacity_bytes
        + vector(&paragraph.baselines)
        + vector(&paragraph.layouts);
    for layouts in &paragraph.layouts {
        bytes += vector(layouts);
        for line in layouts {
            bytes += vector(&line.glyphs) + vector(&line.decorations);
        }
    }
    bytes
}
