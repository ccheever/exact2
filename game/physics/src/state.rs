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
        let fill = |h: ColliderHandle| {
            let e = owners.get(&raw(h))?;
            // A crafted hole over an invalid entry fails the read instead of panicking.
            std::panic::catch_unwind(|| rebuild(e)).ok()?
        };
        let rapier = bincode::DefaultOptions::new()
            .with_limit(payload.len() as u64)
            .deserialize_seed(FillHoles(fill), payload)
            .map_err(|e| DataError::new(format!("physics: invalid snapshot: {e}")))?;
        let entries = entries.into_iter().map(|(k, e)| (k, e.clone())).collect();
        self.live = Some(Live::new(rapier, entries));
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
                self.entries = live.entries.values().cloned().collect();
            }
            self.dirty = false;
        }
        self.bytes.len()
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
        s.write(w);
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
fn rebuild(e: &Entry) -> Option<rapier3d::prelude::Collider> {
    let c = e.collider.as_ref().filter(|_| e.body.is_none())?;
    Some(
        crate::step::collider(c, e.pose, None)
            .position(crate::math::pose(e.pose))
            .build()
            .settled(),
    )
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
        if rebuild(entry).is_some_and(|fresh| bytes(&fresh).ok() == bytes(co).ok()) {
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
