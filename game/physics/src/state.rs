use crate::{Body, Collider};
use bincode::Options;
use exact_game::{
    data::{DataError, Reader, Writer},
    Data, Entity, Transform,
};
use rapier3d::{
    pipeline::{FillHoles, PhysicsWorld},
    prelude::*,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

// Bump when Rapier, its serde representation, or bincode options change.
// v3 writes each static collider that its entry rebuilds bit-exactly as a hole.
const SNAPSHOT: &[u8] = b"EXPHYS\0\x03";

#[derive(Clone, Debug, Default, Data)]
pub(crate) struct Entry {
    pub entity: Entity,
    pub body_handle: Option<[u32; 2]>,
    pub collider_handle: Option<[u32; 2]>,
    pub body: Option<Body>,
    pub collider: Option<Collider>,
    pub pose: Transform,
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
pub(crate) struct Live {
    pub rapier: PhysicsWorld,
    pub entries: BTreeMap<Entity, Entry>,
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
    // sync forgets every row it visits.
    pub elidable: BTreeSet<[u32; 2]>,
    // Each entry's hash, kept until sync or writeback next touches the entry.
    pub digests: BTreeMap<Entity, u64>,
}
impl Live {
    fn new(rapier: PhysicsWorld, entries: BTreeMap<Entity, Entry>) -> Self {
        Self {
            reverse: entries
                .values()
                .filter_map(|e| e.collider_handle.map(|h| (h, e.entity)))
                .collect(),
            bodies: entries
                .values()
                .filter(|e| e.body_handle.is_some())
                .map(|e| e.entity)
                .collect(),
            parented: BTreeSet::new(),
            slots: entries.keys().map(|e| (e.index(), *e)).collect(),
            removed: Vec::new(),
            synced: None,
            elidable: BTreeSet::new(),
            digests: BTreeMap::new(),
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
#[derive(Default, Data)]
pub(crate) struct Saved {
    bytes: Vec<u8>,
    entries: Vec<Entry>,
    #[data(skip)]
    pub live: Option<Live>,
    #[data(skip)]
    pub dirty: bool,
    // The snapshot bytes' hash, and whether `entries` lags the live entries.
    #[data(skip)]
    bytes_hash: Option<u64>,
    #[data(skip)]
    entries_stale: bool,
}
impl Saved {
    pub fn live(&mut self) -> &mut Live {
        self.live.get_or_insert_with(Live::default)
    }
    fn decode(&mut self) -> Result<(), DataError> {
        if self.bytes.is_empty() {
            return Ok(());
        }
        let payload = self.bytes.strip_prefix(SNAPSHOT).ok_or_else(|| {
            DataError::new(format!(
                "physics: expected EXPHYS v3 (v1/v2 snapshots predate static-collider rebuilds; start a new world); saw {:02x?}",
                &self.bytes[..self.bytes.len().min(8)]
            ))
        })?;
        let entries: BTreeMap<_, _> = self.entries.iter().map(|e| (e.entity, e)).collect();
        let owners: BTreeMap<_, _> = self
            .entries
            .iter()
            .filter_map(|e| Some((e.collider_handle?, e)))
            .collect();
        // A hole over a missing or invalid entry fails the read by name; nothing
        // here may panic, since release builds abort.
        let mut refused = None;
        // A filled hole is its entry's rebuild by construction: elidable without
        // re-verifying, so a restore does not re-serialize every static collider.
        let mut filled = BTreeSet::new();
        let fill = |h: ColliderHandle| {
            let rebuilt = owners
                .get(&raw(h))
                .ok_or("physics: a collider hole has no entry")
                .and_then(|e| rebuild(e));
            filled.insert(raw(h));
            rebuilt.map_err(|e| refused = Some(e)).ok()
        };
        let rapier = bincode::DefaultOptions::new()
            .with_limit(payload.len() as u64)
            .deserialize_seed(FillHoles(fill), payload)
            .map_err(|e| match refused {
                Some(why) => DataError::new(format!("physics: invalid snapshot: {why}")),
                None => DataError::new(format!("physics: invalid snapshot: {e}")),
            })?;
        let entries = entries.into_iter().map(|(k, e)| (k, e.clone())).collect();
        let mut live = Live::new(rapier, entries);
        live.elidable = filled;
        self.live = Some(live);
        Ok(())
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
                self.entries_stale = true;
            }
            self.dirty = false;
        }
        self.bytes.len()
    }
    // A save (or a clone) needs the entries themselves; a hash needs only digests.
    fn entries(&mut self) {
        if std::mem::take(&mut self.entries_stale) {
            if let Some(live) = &self.live {
                self.entries = live.entries.values().cloned().collect();
            }
        }
    }
    // What a hash reads in place of the saved content: the snapshot bytes' hash and
    // each entry's hash in entity order. Both are functions of the saved content
    // alone, recomputed identically after a load; only touched entries are rehashed.
    fn digest(&mut self) -> u64 {
        let bytes = *self
            .bytes_hash
            .get_or_insert_with(|| exact_game::hash::of(&self.bytes));
        let mut h = exact_game::hash::Hasher::default();
        bytes.write(&mut h);
        match &mut self.live {
            Some(live) => {
                (live.entries.len() as u64).write(&mut h);
                for (e, entry) in &live.entries {
                    let d = live
                        .digests
                        .entry(*e)
                        .or_insert_with(|| exact_game::hash::of(entry));
                    d.write(&mut h);
                }
            }
            None => {
                (self.entries.len() as u64).write(&mut h);
                for entry in &self.entries {
                    exact_game::hash::of(entry).write(&mut h);
                }
            }
        }
        h.finish()
    }
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
        s.entries();
        let mut saved = Saved {
            bytes: s.bytes.clone(),
            entries: s.entries.clone(),
            ..Saved::default()
        };
        saved.decode().expect("physics: own snapshot must decode");
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
            s.entries();
            s.write(w);
        }
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        let mut next = Saved::default();
        next.read(r)?;
        next.decode()?;
        *self.0.get_mut() = next;
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
    let bytes = |c: &rapier3d::prelude::Collider| bincode::DefaultOptions::new().serialize(c);
    for (h, e) in &live.reverse {
        if live.elidable.contains(h) {
            continue;
        }
        let entry = &live.entries[e];
        let co = &live.rapier.colliders[ColliderHandle::from_raw_parts(h[0], h[1])];
        let same = |fresh: rapier3d::prelude::Collider| {
            bytes(co).is_ok_and(|live| bytes(&fresh).is_ok_and(|fresh| fresh == live))
        };
        if rebuild(entry).is_ok_and(same) {
            live.elidable.insert(*h);
        }
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
        let mut saved = Saved {
            bytes: b"random!!".to_vec(),
            ..Saved::default()
        };
        let error = saved.decode().unwrap_err().to_string();
        assert!(
            error.contains("EXPHYS v3") && error.contains("start a new world"),
            "{error}"
        );
        assert!(error.contains(&format!("{:02x?}", saved.bytes)), "{error}");
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
        good.entries();
        let (bytes, entries) = (good.bytes.clone(), good.entries.clone());
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
            entries[0].collider = collider;
            entries[0].pose = pose;
            let mut saved = Saved {
                bytes: bytes.clone(),
                entries,
                ..Saved::default()
            };
            let error = saved.decode().unwrap_err().to_string();
            assert!(error.contains(why), "{why}: {error}");
        }
        let mut saved = Saved {
            bytes,
            entries,
            ..Saved::default()
        };
        saved.decode().unwrap();
    }

    #[test]
    fn stale_physics_is_refused_during_read() {
        for bytes in [
            b"old rapier snapshot".to_vec(),
            b"EXPHYS\0\x01broken".to_vec(),
            b"EXPHYS\0\x02broken".to_vec(),
            b"EXPHYS\0\x03broken".to_vec(),
        ] {
            let saved = Saved {
                bytes,
                ..Saved::default()
            };
            let result = bin::from_slice::<Executor>(&bin::to_vec(&saved));
            assert!(result.is_err(), "stale physics accepted by Data::read");
            assert!(result.unwrap_err().to_string().contains("physics"));
        }
    }
}

#[cfg(test)]
#[path = "../tests/continuation/mod.rs"]
mod continuation;
