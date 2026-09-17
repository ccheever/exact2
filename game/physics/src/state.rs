use crate::{Body, Collider};
use bincode::Options;
use exact_game::{
    data::{DataError, Reader, Writer},
    Data, Entity, Transform,
};
use rapier3d::{pipeline::PhysicsWorld, prelude::*};
use std::{cell::RefCell, collections::BTreeMap};

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
        self.live.get_or_insert_with(|| {
            if self.bytes.is_empty() {
                return Live::default();
            }
            let rapier = bincode::DefaultOptions::new()
                .deserialize(&self.bytes)
                .expect("physics: invalid Rapier snapshot");
            let entries: BTreeMap<_, _> = self
                .entries
                .iter()
                .cloned()
                .map(|e| (e.entity, e))
                .collect();
            let reverse = self
                .entries
                .iter()
                .filter_map(|e| e.collider_handle.map(|h| (h, e.entity)))
                .collect();
            Live {
                rapier,
                entries,
                reverse,
            }
        })
    }
    fn refresh(&mut self) -> usize {
        if self.dirty {
            if let Some(live) = &self.live {
                self.bytes.clear();
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
pub(crate) struct Executor(pub RefCell<Saved>);
impl Executor {
    pub fn refresh(&self) -> usize {
        self.0.borrow_mut().refresh()
    }
}
impl Clone for Executor {
    fn clone(&self) -> Self {
        let mut s = self.0.borrow_mut();
        s.refresh();
        Self(RefCell::new(Saved {
            bytes: s.bytes.clone(),
            entries: s.entries.clone(),
            ..Saved::default()
        }))
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
        *self.0.get_mut() = next;
        Ok(())
    }
}
pub(crate) fn raw(handle: ColliderHandle) -> [u32; 2] {
    let (i, g) = handle.into_raw_parts();
    [i, g]
}
