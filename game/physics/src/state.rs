use crate::{Body, Collider};
use bincode::Options;
use exact_game::{
    data::{DataError, Reader, Writer},
    Data, Entity, Transform,
};
use rapier3d::{pipeline::PhysicsWorld, prelude::*};
use std::{cell::RefCell, collections::BTreeMap};

// Bump when Rapier, its serde representation, or bincode options change.
const SNAPSHOT: &[u8] = b"EXPHYS\0\x01";

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
pub(crate) struct Live {
    pub changes: crate::changes::Changes,
    pub rapier: PhysicsWorld,
    pub entries: BTreeMap<Entity, Entry>,
    pub reverse: BTreeMap<[u32; 2], Entity>,
}
impl Default for Live {
    fn default() -> Self {
        let mut rapier = PhysicsWorld::default();
        rapier
            .integration_parameters
            .normalized_allowed_linear_error = 0.0001;
        Self {
            rapier,
            entries: BTreeMap::new(),
            reverse: BTreeMap::new(),
            changes: Default::default(),
        }
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
                "physics: expected EXPHYS v1 (unversioned snapshots obsolete); saw {:02x?}",
                &self.bytes[..self.bytes.len().min(8)]
            ))
        })?;
        let rapier = bincode::DefaultOptions::new()
            .with_limit(payload.len() as u64)
            .deserialize(payload)
            .map_err(|e| DataError::new(format!("physics: invalid snapshot: {e}")))?;
        self.live = Some(Live {
            changes: Default::default(),
            rapier,
            entries: self
                .entries
                .iter()
                .cloned()
                .map(|e| (e.entity, e))
                .collect(),
            reverse: self
                .entries
                .iter()
                .filter_map(|e| e.collider_handle.map(|h| (h, e.entity)))
                .collect(),
        });
        Ok(())
    }
    fn refresh(&mut self) -> usize {
        if self.dirty {
            if let Some(live) = &self.live {
                self.bytes.clear();
                self.bytes.extend_from_slice(SNAPSHOT);
                bincode::DefaultOptions::new()
                    .serialize_into(&mut self.bytes, &live.rapier)
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
        Self(
            RefCell::new(Saved {
                bytes: s.bytes.clone(),
                entries: s.entries.clone(),
                live: s.live.as_ref().map(|live| Live {
                    changes: Default::default(),
                    rapier: PhysicsWorld {
                        gravity: live.rapier.gravity,
                        integration_parameters: live.rapier.integration_parameters,
                        islands: live.rapier.islands.clone(),
                        broad_phase: live.rapier.broad_phase.clone(),
                        narrow_phase: live.rapier.narrow_phase.clone(),
                        bodies: live.rapier.bodies.clone(),
                        colliders: live.rapier.colliders.clone(),
                        impulse_joints: live.rapier.impulse_joints.clone(),
                        multibody_joints: live.rapier.multibody_joints.clone(),
                        ..PhysicsWorld::default()
                    },
                    entries: live.entries.clone(),
                    reverse: live.reverse.clone(),
                }),
                dirty: false,
            }),
            RefCell::new(None),
        )
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
            error.contains("EXPHYS v1") && error.contains("unversioned snapshots obsolete"),
            "{error}"
        );
        assert!(error.contains(&format!("{:02x?}", saved.bytes)), "{error}");
    }

    #[test]
    fn stale_physics_is_refused_during_read() {
        for bytes in [
            b"old rapier snapshot".to_vec(),
            b"EXPHYS\0\x01broken".to_vec(),
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
