use crate::{Body, Collider};
use bincode::Options;
use exact_game::{
    bin,
    data::{Bulk, DataError, Reader, Writer},
    hash, Data, Entity, Transform,
};
use rapier3d::{
    pipeline::{FillHoles, PhysicsWorld},
    prelude::*,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

// Bump when Rapier, its serde representation, bincode options or the saved form
// change. v3 writes each static collider that its entry rebuilds bit-exactly as a
// hole; v4 saves the entries as pages.
const SNAPSHOT: &[u8] = b"EXPHYS\0\x04";

// Entries are ordered by entity (index, then generation), as `Entity` orders.
// A key, unlike an Entity, can bound a range: a page is a run of indices.
pub(crate) type Key = (u32, u32);
pub(crate) fn key(e: Entity) -> Key {
    (e.index(), e.generation())
}
// A page holds the entries of 256 consecutive entity indices.
const PAGE_BITS: u32 = 8;
fn page_of(index: u32) -> usize {
    (index >> PAGE_BITS) as usize
}
fn page_range(page: usize) -> std::ops::Range<Key> {
    let lo = (page as u64) << PAGE_BITS;
    let hi = (lo + (1 << PAGE_BITS)).min(u32::MAX as u64);
    (lo as u32, 0)..(hi as u32, 0)
}

#[derive(Clone, Debug, Default, Data)]
pub(crate) struct Entry {
    pub entity: Entity,
    pub body_handle: Option<[u32; 2]>,
    pub collider_handle: Option<[u32; 2]>,
    pub body: Option<Body>,
    pub collider: Option<Collider>,
    pub pose: Transform,
    // Derived from the saved fields; sync and writeback clear it before editing.
    #[data(skip)]
    pub digest: Option<u64>,
}
impl Entry {
    pub fn bh(&self) -> Option<RigidBodyHandle> {
        self.body_handle
            .map(|h| RigidBodyHandle::from_raw_parts(h[0], h[1]))
    }
    pub fn ch(&self) -> Option<ColliderHandle> {
        self.collider_handle
            .map(|h| ColliderHandle::from_raw_parts(h[0], h[1]))
    }
}
// The world and write revisions the last sync observed; derived, never saved.
#[derive(Clone)]
pub(crate) struct Synced {
    pub world: exact_game::WorldId,
    pub presentation: u64,
    pub revisions: [u64; 4],
}
// Which colliders a capture may write as holes. Collider handles already index a
// dense Rapier arena: the verified generation sits at its slot, so a reused slot
// cannot inherit the previous collider's verification. `pending` holds every
// collider not verified, so a capture compares only those: an unchanged static
// world costs nothing here. A collider whose comparison fails stays pending and
// is compared again at every capture, so whether a collider is a hole depends on
// the live state alone, never on when it was last compared.
#[derive(Default)]
pub(crate) struct Elidable {
    verified: Vec<Option<u32>>,
    pending: BTreeSet<[u32; 2]>,
}
impl Elidable {
    fn contains(&self, h: &[u32; 2]) -> bool {
        self.verified.get(h[0] as usize).copied().flatten() == Some(h[1])
    }
    fn insert(&mut self, h: [u32; 2]) {
        let slot = h[0] as usize;
        if self.verified.len() <= slot {
            self.verified.resize(slot + 1, None);
        }
        self.verified[slot] = Some(h[1]);
        self.pending.remove(&h);
    }
    // A new or edited collider: compare it at the next capture.
    pub fn forget(&mut self, h: [u32; 2]) {
        if self.contains(&h) {
            self.verified[h[0] as usize] = None;
        }
        self.pending.insert(h);
    }
    // A removed collider.
    pub fn remove(&mut self, h: &[u32; 2]) {
        if self.contains(h) {
            self.verified[h[0] as usize] = None;
        }
        self.pending.remove(h);
    }
}
// What capture derived from each page of entries: the page's row count and
// digest, and its saved bytes. Sync and writeback forget a page they touch.
#[derive(Clone, Default)]
struct Page {
    digest: Option<(u64, u64)>,
    bytes: Option<Vec<u8>>,
}
#[derive(Clone, Default)]
pub(crate) struct Pages(Vec<Page>);
impl Pages {
    pub fn touch(&mut self, e: Entity) {
        if let Some(p) = self.0.get_mut(page_of(e.index())) {
            *p = Page::default();
        }
    }
    fn page(&mut self, page: usize) -> &mut Page {
        if self.0.len() <= page {
            self.0.resize(page + 1, Page::default());
        }
        &mut self.0[page]
    }
}
pub(crate) struct Live {
    pub rapier: PhysicsWorld,
    pub entries: BTreeMap<Key, Entry>,
    pub reverse: BTreeMap<[u32; 2], Entity>,
    // Derived from entries: entities holding a Rapier body, entities that were
    // parented at the last sync, and each index's entry.
    pub bodies: BTreeSet<Entity>,
    pub parented: BTreeSet<Entity>,
    pub slots: BTreeMap<u32, Entity>,
    // Collider handles removed this step, unmapped once its events are named.
    pub removed: Vec<[u32; 2]>,
    pub synced: Option<Synced>,
    // Static colliders verified equal to their entry's rebuild since last edited;
    // sync forgets each row it changes.
    pub elidable: Elidable,
    pub pages: Pages,
}
impl Live {
    fn new(rapier: PhysicsWorld, entries: BTreeMap<Key, Entry>) -> Self {
        let reverse: BTreeMap<_, _> = entries
            .values()
            .filter_map(|e| e.collider_handle.map(|h| (h, e.entity)))
            .collect();
        Self {
            elidable: Elidable {
                verified: Vec::new(),
                pending: reverse.keys().copied().collect(),
            },
            reverse,
            bodies: entries
                .values()
                .filter(|e| e.body_handle.is_some())
                .map(|e| e.entity)
                .collect(),
            parented: BTreeSet::new(),
            slots: entries
                .values()
                .map(|e| (e.entity.index(), e.entity))
                .collect(),
            removed: Vec::new(),
            synced: None,
            pages: Pages::default(),
            rapier,
            entries,
        }
    }
}
impl Default for Live {
    fn default() -> Self {
        let mut rapier = PhysicsWorld::default();
        rapier
            .integration_parameters
            .normalized_allowed_linear_error = 0.0001;
        Self::new(rapier, BTreeMap::new())
    }
}
// The saved form: the snapshot, then each nonempty page of entries in index
// order, every page a `Vec<Entry>` in `bin`. Exactly one form exists for each
// state, so equal states save equal bytes. `Executor::write` writes it in place.
#[derive(Default, Data)]
struct Form {
    bytes: Vec<u8>,
    pages: Vec<Vec<u8>>,
}
#[derive(Default)]
pub(crate) struct Saved {
    bytes: Vec<u8>,
    pub live: Option<Live>,
    pub dirty: bool,
    // The snapshot bytes' digest.
    bytes_hash: Option<u64>,
}
impl Saved {
    pub fn live(&mut self) -> &mut Live {
        self.live.get_or_insert_with(Live::default)
    }
    // Read the saved form: entries strictly ascending by index, grouped by page.
    fn read_form(form: Form) -> Result<Self, DataError> {
        let mut entries = BTreeMap::new();
        let mut last: Option<(usize, u32)> = None;
        for bytes in &form.pages {
            let page: Vec<Entry> = bin::from_slice(bytes)
                .map_err(|e| DataError::new(format!("physics: invalid entries: {e}")))?;
            let first = page
                .first()
                .ok_or_else(|| DataError::new("physics: an entries page is empty"))?;
            let number = page_of(first.entity.index());
            if last.is_some_and(|(p, _)| p >= number) {
                return Err(DataError::new("physics: entries pages out of order"));
            }
            for e in page {
                let index = e.entity.index();
                if page_of(index) != number || last.is_some_and(|(_, i)| i >= index) {
                    return Err(DataError::new("physics: entries out of order"));
                }
                last = Some((number, index));
                entries.insert(key(e.entity), e);
            }
        }
        if form.bytes.is_empty() && !entries.is_empty() {
            return Err(DataError::new("physics: entries without a snapshot"));
        }
        Self::decode(form.bytes, entries)
    }
    fn decode(bytes: Vec<u8>, entries: BTreeMap<Key, Entry>) -> Result<Self, DataError> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let payload = bytes.strip_prefix(SNAPSHOT).ok_or_else(|| {
            DataError::new(format!(
                "physics: expected EXPHYS v4 (v1–v3 snapshots predate paged entries; start a new world); saw {:02x?}",
                &bytes[..bytes.len().min(8)]
            ))
        })?;
        let owners: BTreeMap<_, _> = entries
            .values()
            .filter_map(|e| Some((e.collider_handle?, e)))
            .collect();
        // A hole over a missing or invalid entry fails the read by name; nothing
        // here may panic, since release builds abort.
        let mut refused = None;
        // A filled hole is its entry's rebuild by construction: elidable without
        // re-verifying, so a restore does not re-serialize every static collider.
        let mut filled = Vec::new();
        let fill = |h: ColliderHandle| {
            let rebuilt = owners
                .get(&raw(h))
                .ok_or("physics: a collider hole has no entry")
                .and_then(|e| rebuild(e));
            filled.push(raw(h));
            rebuilt.map_err(|e| refused = Some(e)).ok()
        };
        let rapier = bincode::DefaultOptions::new()
            .with_limit(payload.len() as u64)
            .deserialize_seed(FillHoles(fill), payload)
            .map_err(|e| match refused {
                Some(why) => DataError::new(format!("physics: invalid snapshot: {why}")),
                None => DataError::new(format!("physics: invalid snapshot: {e}")),
            })?;
        let mut live = Live::new(rapier, entries);
        for h in filled {
            live.elidable.insert(h);
        }
        Ok(Self {
            bytes,
            live: Some(live),
            ..Self::default()
        })
    }
    fn refresh(&mut self) -> usize {
        if self.dirty {
            if let Some(live) = &mut self.live {
                verify(live);
                self.bytes.clear();
                self.bytes.extend_from_slice(SNAPSHOT);
                let elidable = &live.elidable;
                bincode::DefaultOptions::new()
                    .serialize_into(
                        &mut self.bytes,
                        &live.rapier.with_holes(|h, _| elidable.contains(&raw(h))),
                    )
                    .expect("physics: snapshot serialization");
                self.bytes_hash = None;
            }
            self.dirty = false;
        }
        self.bytes.len()
    }
    // Each nonempty page in index order, for `each` to read from the cache.
    fn pages(&mut self, mut each: impl FnMut(usize, &mut Page, &mut BTreeMap<Key, Entry>)) {
        let Some(live) = &mut self.live else {
            return;
        };
        let Some(&(last, _)) = live.entries.keys().next_back() else {
            return;
        };
        for number in 0..=page_of(last) {
            each(number, live.pages.page(number), &mut live.entries);
        }
    }
    fn write_form(&mut self, w: &mut dyn Writer) {
        // Encode the pages no save has encoded since they were touched; an empty
        // page encodes as nothing and is left out.
        self.pages(|number, page, entries| {
            page.bytes.get_or_insert_with(|| {
                let rows = entries.range(page_range(number));
                let count = rows.clone().count();
                if count == 0 {
                    return Vec::new();
                }
                let mut e = bin::Encoder::default();
                e.begin_seq(count);
                for (_, entry) in rows {
                    e.item();
                    entry.write(&mut e);
                }
                e.end_seq();
                e.finish()
            });
        });
        let pages: Vec<&[u8]> = match &self.live {
            Some(live) => live
                .entries
                .keys()
                .next_back()
                .map_or(&[][..], |&(last, _)| &live.pages.0[..=page_of(last)])
                .iter()
                .filter_map(|p| p.bytes.as_deref().filter(|b| !b.is_empty()))
                .collect(),
            None => Vec::new(),
        };
        // The derived `Form::write`, over the cached bytes instead of copies.
        w.begin_struct();
        w.field("bytes");
        w.bytes(Bulk::U8(&self.bytes));
        w.field("pages");
        w.begin_seq(pages.len());
        for page in pages {
            w.item();
            w.bytes(Bulk::U8(page));
        }
        w.end_seq();
        w.end_struct();
    }
    // What a hash reads in place of the saved content: the snapshot bytes' digest,
    // then each nonempty page's number and digest, a page's digest covering its
    // row count and each entry's hash in entity order. All are functions of the
    // saved content alone, recomputed identically after a load; only touched
    // entries are rehashed, and only touched pages recombined.
    fn digest(&mut self) -> u64 {
        let bytes = *self
            .bytes_hash
            .get_or_insert_with(|| digest_bytes(&self.bytes));
        let mut h = hash::Hasher::default();
        bytes.write(&mut h);
        self.pages(|number, page, entries| {
            let (rows, digest) = *page.digest.get_or_insert_with(|| {
                let mut h = hash::Hasher::default();
                let mut rows = 0u64;
                for (_, entry) in entries.range_mut(page_range(number)) {
                    rows += 1;
                    let d = match entry.digest {
                        Some(d) => d,
                        None => *entry.digest.insert(hash::of(entry)),
                    };
                    d.write(&mut h);
                }
                rows.write(&mut h);
                (rows, h.finish())
            });
            if rows != 0 {
                (number as u64).write(&mut h);
                digest.write(&mut h);
            }
        });
        h.finish()
    }
}
// The snapshot bytes' digest: four SplitMix64 lanes over interleaved little-endian
// words, so megabytes are not one serial multiply chain, then the engine's hash
// over the lanes, the length and the tail.
fn digest_bytes(bytes: &[u8]) -> u64 {
    fn mix(mut n: u64) -> u64 {
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        n ^ (n >> 31)
    }
    let mut lanes: [u64; 4] = [1, 2, 3, 4].map(|k: u64| mix(k.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
    let mut blocks = bytes.chunks_exact(32);
    for block in &mut blocks {
        for (lane, word) in lanes.iter_mut().zip(block.chunks_exact(8)) {
            *lane = mix(*lane ^ u64::from_le_bytes(word.try_into().unwrap())).rotate_left(27);
        }
    }
    let mut h = hash::Hasher::default();
    for lane in lanes {
        lane.write(&mut h);
    }
    (bytes.len() as u64).write(&mut h);
    h.bytes(Bulk::U8(blocks.remainder()));
    h.finish()
}
#[derive(Default)]
pub(crate) struct Executor(
    pub RefCell<Saved>,
    pub RefCell<Option<crate::queries::Cached>>,
);
impl Executor {
    pub fn refresh(&self) -> usize {
        self.0.borrow_mut().refresh()
    }
}
impl Clone for Executor {
    fn clone(&self) -> Self {
        let mut s = self.0.borrow_mut();
        s.refresh();
        let entries = s
            .live
            .as_ref()
            .map(|l| l.entries.clone())
            .unwrap_or_default();
        let mut saved =
            Saved::decode(s.bytes.clone(), entries).expect("physics: own snapshot must decode");
        // The same entries: their digests (cloned with them) and pages hold.
        if let (Some(to), Some(from)) = (&mut saved.live, &s.live) {
            to.pages = from.pages.clone();
        }
        saved.bytes_hash = s.bytes_hash;
        Self(RefCell::new(saved), RefCell::new(None))
    }
}
impl std::fmt::Debug for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Executor")
            .field("snapshot_bytes", &self.0.borrow().bytes.len())
            .finish_non_exhaustive()
    }
}
impl Data for Executor {
    fn write(&self, w: &mut dyn Writer) {
        let mut s = self.0.borrow_mut();
        s.refresh();
        if w.digests() {
            s.digest().write(w);
        } else {
            s.write_form(w);
        }
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        let mut form = Form::default();
        form.read(r)?;
        *self.0.get_mut() = Saved::read_form(form)?;
        *self.1.get_mut() = None;
        Ok(())
    }
}
// A static collider exactly as sync builds it, settled as after a step. Only a
// collider without a Body is rebuilt; its entry is the component it was built from.
fn rebuild(e: &Entry) -> Result<rapier3d::prelude::Collider, &'static str> {
    let c = e
        .collider
        .as_ref()
        .filter(|_| e.body.is_none())
        .ok_or("physics: a collider hole's entry is not a static collider")?;
    Ok(crate::step::try_collider(c, e.pose, None)?
        .position(crate::math::try_pose(e.pose)?)
        .build()
        .settled())
}
// Mark each static collider whose rebuild is byte-identical to the live one, so
// the snapshot carries its entry (its components) but not Rapier's copy.
fn verify(live: &mut Live) {
    // Reuse comparison buffers across colliders. `serialize` allocates a vector
    // and walks each collider twice (size, then bytes); only the bytes are needed.
    let (mut actual, mut rebuilt) = (Vec::new(), Vec::new());
    let options = bincode::DefaultOptions::new();
    let mut verified = Vec::new();
    for h in &live.elidable.pending {
        let Some(entry) = live.reverse.get(h).and_then(|e| live.entries.get(&key(*e))) else {
            continue;
        };
        let co = &live.rapier.colliders[ColliderHandle::from_raw_parts(h[0], h[1])];
        let same = |fresh: rapier3d::prelude::Collider| {
            actual.clear();
            rebuilt.clear();
            options.serialize_into(&mut actual, co).is_ok()
                && options.serialize_into(&mut rebuilt, &fresh).is_ok()
                && actual == rebuilt
        };
        if rebuild(entry).is_ok_and(same) {
            verified.push(*h);
        }
    }
    for h in verified {
        live.elidable.insert(h);
    }
}
pub(crate) fn raw(handle: ColliderHandle) -> [u32; 2] {
    let (i, g) = handle.into_raw_parts();
    [i, g]
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_game::bin;

    #[test]
    fn snapshot_refusal_names_expected_format_and_seen_bytes() {
        let bytes = b"random!!".to_vec();
        let error = Saved::decode(bytes.clone(), BTreeMap::new())
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("EXPHYS v4") && error.contains("start a new world"),
            "{error}"
        );
        assert!(error.contains(&format!("{bytes:02x?}")), "{error}");
    }

    // A save is input: a hole over an entry that cannot be rebuilt is refused by
    // name during the read (release builds abort on panic, so nothing may panic).
    #[test]
    fn holes_over_invalid_entries_are_refused_by_name() {
        use crate::{Collider, Shape};
        use exact_game::{Quat, Vec3, World};
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        w.spawn((Transform::default(), Collider::default()));
        crate::step(&mut w);
        let physics = w.resource::<crate::Physics>();
        let mut good = physics.executor.0.borrow_mut();
        good.refresh();
        let (bytes, entries) = (
            good.bytes.clone(),
            good.live.as_ref().unwrap().entries.clone(),
        );
        let shape = |shape| Collider {
            shape,
            ..Collider::default()
        };
        let cases: Vec<(Option<Collider>, Transform, &str)> = vec![
            (
                Some(Collider {
                    bounce: 2.0,
                    ..Collider::default()
                }),
                Transform::default(),
                "invalid material",
            ),
            (
                Some(Collider::default()),
                Transform {
                    rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 2.0),
                    ..Transform::default()
                },
                "invalid pose",
            ),
            (
                Some(shape(Shape::Capsule {
                    radius: 1.0,
                    height: 1.0,
                })),
                Transform::default(),
                "twice its radius",
            ),
            (
                Some(shape(Shape::Heightfield {
                    rows: 3,
                    cols: 3,
                    heights: vec![0.0; 4],
                    scale: Vec3::ONE,
                })),
                Transform::default(),
                "invalid heightfield",
            ),
            (
                Some(shape(Shape::Mesh {
                    vertices: vec![Vec3::ZERO; 3],
                    indices: vec![[0, 1, 7]],
                })),
                Transform::default(),
                "invalid mesh",
            ),
            (
                Some(shape(Shape::Box {
                    half: Vec3::splat(1e30),
                })),
                Transform {
                    scale: Vec3::splat(1e30),
                    ..Transform::default()
                },
                "overflows",
            ),
            (None, Transform::default(), "not a static collider"),
        ];
        for (collider, pose, why) in cases {
            let mut entries = entries.clone();
            let first = entries.values_mut().next().unwrap();
            first.collider = collider;
            first.pose = pose;
            let error = Saved::decode(bytes.clone(), entries)
                .err()
                .unwrap()
                .to_string();
            assert!(error.contains(why), "{why}: {error}");
        }
        Saved::decode(bytes, entries).unwrap();
    }

    #[test]
    fn warmed_hash_matches_saved_state_after_physics_edits() {
        use exact_game::{hash, Parent, Vec3, World};
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        let parent = w.spawn(Transform::default());
        let mut e = w.spawn((Transform::default(), Collider::default()));
        // One verification pass compares different byte lengths, including a
        // mesh followed by a sphere. Reused buffers must not keep trailing bytes.
        for (x, shape) in [
            crate::Shape::Mesh {
                vertices: vec![Vec3::ZERO, Vec3::X, Vec3::Z],
                indices: vec![[0, 2, 1]],
            },
            crate::Shape::Sphere { radius: 0.5 },
            crate::Shape::Capsule {
                radius: 0.25,
                height: 2.0,
            },
        ]
        .into_iter()
        .enumerate()
        {
            w.spawn((
                Transform::at(10.0 + x as f32 * 3.0, 0.0, 0.0),
                Collider {
                    shape,
                    ..Collider::default()
                },
            ));
        }
        let mut previous_collider = None;
        for stage in 0..12 {
            match stage {
                1 => w.get_mut::<Transform>(e).unwrap().position.x = 3.0,
                2 => w.get_mut::<Collider>(e).unwrap().friction = 0.2,
                3 => {
                    w.insert(e, Body::default());
                }
                4 => w.get_mut::<Body>(e).unwrap().velocity = Vec3::Y,
                5 => {
                    w.remove::<Body>(e);
                    w.insert(e, Parent(parent));
                }
                6 => w.get_mut::<Transform>(parent).unwrap().position.z = 2.0,
                7 => {
                    w.despawn(e);
                }
                8 => {
                    let previous = e;
                    e = w.spawn((Transform::at(5.0, 0.0, 0.0), Collider::default()));
                    assert_eq!(e.index(), previous.index(), "exercise slot reuse");
                    assert_ne!(e, previous);
                }
                9 => {
                    w.remove::<Collider>(e);
                }
                10 => {
                    w.insert(
                        e,
                        Collider {
                            sensor: true,
                            ..Collider::default()
                        },
                    );
                }
                11 => w.get_mut::<Collider>(e).unwrap().sensor = false,
                _ => {}
            }
            crate::step(&mut w);
            let physics = w.resource::<crate::Physics>();
            let executor = &physics.executor;
            let before = bin::to_vec(executor);
            // Compare to capture that proves every hole afresh, ignoring the
            // cache. Reused slots, changed materials and body transitions must
            // preserve the exact snapshot, not only a successfully decoded one.
            {
                let saved = executor.0.borrow();
                let live = saved.live.as_ref().unwrap();
                if let Some(handle) = live.entries.get(&key(e)).and_then(|e| e.collider_handle) {
                    if matches!(stage, 8 | 10) {
                        let old: [u32; 2] = previous_collider.unwrap();
                        assert_eq!(handle[0], old[0], "exercise Rapier slot reuse");
                        assert_ne!(handle[1], old[1], "exercise a new generation");
                    }
                    previous_collider = Some(handle);
                }
                let options = bincode::DefaultOptions::new();
                let mut expected = SNAPSHOT.to_vec();
                options
                    .serialize_into(
                        &mut expected,
                        &live.rapier.with_holes(|h, co| {
                            let entry = &live.entries[&key(live.reverse[&raw(h)])];
                            rebuild(entry).is_ok_and(|fresh| {
                                options.serialize(co).unwrap() == options.serialize(&fresh).unwrap()
                            })
                        }),
                    )
                    .unwrap();
                assert_eq!(saved.bytes, expected, "uncached snapshot at stage {stage}");
            }
            let warm = hash::of(executor);
            assert_eq!(warm, hash::of(executor), "repeat at stage {stage}");
            assert_eq!(
                before,
                bin::to_vec(executor),
                "hash changed save at stage {stage}"
            );
            let cold = bin::from_slice::<Executor>(&before).unwrap();
            assert_eq!(warm, hash::of(&cold), "saved state at stage {stage}");
            let cloned = executor.clone();
            assert_eq!(warm, hash::of(&cloned), "clone at stage {stage}");
        }
    }

    #[test]
    fn stale_physics_is_refused_during_read() {
        for bytes in [
            b"old rapier snapshot".to_vec(),
            b"EXPHYS\0\x01broken".to_vec(),
            b"EXPHYS\0\x02broken".to_vec(),
            b"EXPHYS\0\x03broken".to_vec(),
            b"EXPHYS\0\x04broken".to_vec(),
        ] {
            let form = Form {
                bytes,
                pages: Vec::new(),
            };
            let result = bin::from_slice::<Executor>(&bin::to_vec(&form));
            assert!(result.is_err(), "stale physics accepted by Data::read");
            assert!(result.unwrap_err().to_string().contains("physics"));
        }
    }

    // 600 static colliders over three pages, a falling box on the last.
    fn paged_world() -> exact_game::World {
        use exact_game::World;
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        for i in 0..600 {
            w.spawn((
                Transform::at((i % 30) as f32 * 2.0, 0.0, (i / 30) as f32 * 2.0),
                Collider::default(),
            ));
        }
        w.spawn((
            Transform::at(1.0, 3.0, 1.0),
            Body::default(),
            Collider::default(),
        ));
        crate::step(&mut w);
        w
    }

    // One saved form per state: pages ascend, each holds only its own indices in
    // order, and none is empty. Any other arrangement is refused by name.
    #[test]
    fn saved_entry_pages_are_canonical() {
        let w = paged_world();
        let saved = bin::to_vec(&w.resource::<crate::Physics>().executor);
        let form: Form = bin::from_slice(&saved).unwrap();
        assert_eq!(form.pages.len(), 3);
        let again = bin::from_slice::<Executor>(&saved).unwrap();
        assert_eq!(
            bin::to_vec(&again),
            saved,
            "a read form saves the same bytes"
        );
        let refused = |pages: Vec<Vec<u8>>, bytes: Vec<u8>| {
            let form = Form { bytes, pages };
            bin::from_slice::<Executor>(&bin::to_vec(&form))
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
        };
        let p = &form.pages;
        let b = &form.bytes;
        let swapped = vec![p[1].clone(), p[0].clone(), p[2].clone()];
        assert!(refused(swapped, b.clone()).contains("out of order"));
        let twice = vec![p[0].clone(), p[0].clone(), p[1].clone(), p[2].clone()];
        assert!(refused(twice, b.clone()).contains("out of order"));
        let mut merged: Vec<Entry> = bin::from_slice(&p[0]).unwrap();
        merged.extend(bin::from_slice::<Vec<Entry>>(&p[1]).unwrap());
        let merged = vec![bin::to_vec(&merged), p[2].clone()];
        assert!(refused(merged, b.clone()).contains("out of order"));
        let empty = vec![bin::to_vec(&Vec::<Entry>::new()), p[0].clone()];
        assert!(refused(empty, b.clone()).contains("page is empty"));
        assert!(refused(p.clone(), Vec::new()).contains("without a snapshot"));
        assert!(refused(vec![b"junk".to_vec()], b.clone()).contains("invalid entries"));
    }

    // A restore rebuilds every hole, and its first step's full sync finds every
    // static row unchanged: the next capture compares no static collider, and
    // the restored world continues exactly as the original does.
    #[test]
    fn a_restored_static_world_stays_warm() {
        let mut w = paged_world();
        let mut r = exact_game::World::new(60, 0);
        crate::register(&mut r);
        r.load(&w.save()).unwrap();
        for _ in 0..3 {
            crate::step(&mut w);
            crate::step(&mut r);
            for world in [&w, &r] {
                world.resource::<crate::Physics>().refresh_snapshot();
            }
            {
                let physics = r.resource::<crate::Physics>();
                let saved = physics.executor.0.borrow();
                let pending = saved.live.as_ref().unwrap().elidable.pending.len();
                assert_eq!(pending, 1, "only the body's collider");
            }
            assert_eq!(w.hash(), r.hash());
            assert_eq!(w.save(), r.save());
        }
    }
}

#[cfg(test)]
#[path = "../tests/continuation/mod.rs"]
mod continuation;
