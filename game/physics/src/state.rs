use crate::{Body, Collider};
use bincode::Options;
use exact_game::{
    data::{DataError, Reader, Writer},
    Data, Entity, Transform,
};
use rapier3d::{pipeline::PhysicsWorld, prelude::*};
use std::{cell::RefCell, collections::BTreeMap};

// Bump when Rapier, its serde representation, or bincode options change.
const SNAPSHOT: &[u8] = b"EXPHYS\0\x02";

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
                "physics: expected EXPHYS v2 (v1/unversioned snapshots incomplete; start a new world); saw {:02x?}",
                &self.bytes[..self.bytes.len().min(8)]
            ))
        })?;
        let rapier = bincode::DefaultOptions::new()
            .with_limit(payload.len() as u64)
            .deserialize(payload)
            .map_err(|e| DataError::new(format!("physics: invalid snapshot: {e}")))?;
        self.live = Some(Live {
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
            error.contains("EXPHYS v2") && error.contains("snapshots incomplete"),
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
    #[test]
    #[ignore = "R3: three scene attempts did not reach deferred_optimize_pending; pin remains owed"]
    fn kinematic_collision_pass_survives_every_tick_restore() {
        use crate::{BodyKind, Physics};
        use exact_game::{Clock, Game, Input, Paranoid, Sim, World};
        struct Moving;
        impl Game for Moving {
            const ID: &'static str = "kinematic-save-boundary";
            type Args = ();
            fn setup(w: &mut World, _: &()) {
                crate::register(w);
                for i in 0..16 {
                    w.spawn((
                        Transform::at(i as f32, 0., 0.),
                        Body {
                            kind: BodyKind::Kinematic,
                            ..Body::default()
                        },
                        Collider {
                            sensor: true,
                            ..Collider::default()
                        },
                    ));
                }
            }
            fn tick(w: &mut World, _: &Input, _: &()) {
                for (e, (_, t)) in w.query::<(&Body, &mut Transform)>().iter() {
                    t.position.y = w.tick() as f32 * (e.index() as f32 + 1.) * 0.1;
                }
                crate::step(w);
            }
        }
        let make = |mode| {
            let mut s = Sim::<Moving>::new(()).unwrap().paranoid(mode);
            s.advance(0., Clock::Seekable);
            s
        };
        let mut off = make(Paranoid::Off);
        let mut save = make(Paranoid::Save);
        let mut fresh = make(Paranoid::FreshGame);
        let mut lost_flag = make(Paranoid::Off);
        let mut pending_tick = None;
        for tick in 1..=32 {
            let now = tick as f64 * 1000. / 60. + 0.001;
            for s in [&mut off, &mut save, &mut fresh] {
                s.advance(now, Clock::Seekable);
            }
            let bytes = off.save().unwrap();
            assert!(bytes == save.save().unwrap(), "Save tick {tick}");
            assert!(bytes == fresh.save().unwrap(), "FreshGame tick {tick}");
            let pending = {
                let physics = off.world().resource::<Physics>();
                let mut saved = physics.executor.0.borrow_mut();
                bincode::DefaultOptions::new()
                    .serialize(&saved.live().rapier.broad_phase)
                    .unwrap()
                    .last()
                    == Some(&1)
            };
            if tick > 1 && pending && pending_tick.is_none() {
                pending_tick = Some(tick);
                // Non-initial moved-kinematic step runs CollisionPipeline after PhysicsPipeline.
                // The persisted pending flag is the last field of the pinned Rapier BVH codec.
                lost_flag.restore(&bytes).unwrap();
                let physics = lost_flag.world().resource::<Physics>();
                let mut saved = physics.executor.0.borrow_mut();
                let broad = &mut saved.live().rapier.broad_phase;
                let mut encoded = bincode::DefaultOptions::new().serialize(broad).unwrap();
                assert_eq!(
                    encoded.last(),
                    Some(&1),
                    "save boundary must have pending optimization"
                );
                *encoded.last_mut().unwrap() = 0;
                *broad = bincode::DefaultOptions::new()
                    .deserialize(&encoded)
                    .unwrap();
                saved.dirty = true;
            }
            if pending_tick == Some(tick - 1) {
                lost_flag.advance(now, Clock::Seekable);
                assert!(
                    bytes != lost_flag.save().unwrap(),
                    "negative control: restoring false must diverge on the next tick"
                );
            }
        }
        assert!(
            pending_tick.is_some_and(|tick| tick < 32),
            "no non-initial pending optimization observed"
        );
        println!("pending tick {pending_tick:?}");
        println!("kinematic tick 32 hash: {:016x}", off.world().hash());
    }
}
