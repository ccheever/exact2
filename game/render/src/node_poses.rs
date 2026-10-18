//! Retained presentation endpoints shared by palette packing and socket chains.
use exact_game::{Entity, Mesh, NodeOffsets, Transform, World};
use glam::{Quat, Vec3};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct Row {
    entity: Entity,
    tick: u64,
    pair: [NodeOffsets; 2],
}
#[derive(Default)]
pub(crate) struct Histories {
    rows: Vec<Row>,
}
impl Histories {
    pub fn update(&mut self, w: &World, initial: bool) {
        if initial {
            self.rows.clear();
        }
        for row in &mut self.rows {
            let curr = w
                .get::<NodeOffsets>(row.entity)
                .map(|o| *o)
                .unwrap_or_default();
            if row.tick != w.tick() {
                row.pair[0] = row.pair[1];
            }
            row.pair[1] = curr;
            if w.is_fresh(row.entity) {
                row.pair[0] = curr;
            }
            row.tick = w.tick();
        }
        self.rows.retain(|r| {
            w.contains(r.entity)
                && (!r.pair[0].rows().is_empty()
                    || !r.pair[1].rows().is_empty()
                    || r.pair[0].overflowed()
                    || r.pair[1].overflowed())
        });
        for (entity, offsets) in w.query::<&NodeOffsets>().iter() {
            if offsets.rows().is_empty() && !offsets.overflowed() {
                continue;
            }
            if let Err(at) = self
                .rows
                .binary_search_by_key(&entity.index(), |r| r.entity.index())
            {
                self.rows.insert(
                    at,
                    Row {
                        entity,
                        tick: w.tick(),
                        pair: [
                            if initial || w.is_fresh(entity) {
                                *offsets
                            } else {
                                NodeOffsets::default()
                            },
                            *offsets,
                        ],
                    },
                );
            }
        }
    }
    pub fn pair(&self, e: Entity, snap: bool) -> [NodeOffsets; 2] {
        let pair = self
            .rows
            .binary_search_by_key(&e.index(), |r| r.entity.index())
            .ok()
            .map(|i| self.rows[i])
            .filter(|r| r.entity == e)
            .map(|r| r.pair)
            .unwrap_or_default();
        if snap {
            [pair[1]; 2]
        } else {
            pair
        }
    }
}
pub(crate) fn rotation(q: Quat) -> Quat {
    glam::Vec4::from(q)
        .try_normalize()
        .filter(|q| q.is_finite())
        .map(Quat::from_vec4)
        .unwrap_or(Quat::IDENTITY)
}
/// Node-frame TRS product. With nonuniform base scale this discards affine shear.
pub(crate) fn compose(base: Transform, delta: Transform) -> Option<Transform> {
    if !delta.position.is_finite()
        || !delta.rotation.is_finite()
        || !delta.scale.is_finite()
        || delta.scale.cmple(Vec3::ZERO).any()
    {
        return None;
    }
    let product = base.rotation * rotation(delta.rotation);
    if !product.is_finite() {
        return None;
    }
    let t = Transform {
        position: base.position + base.rotation * (base.scale * delta.position),
        rotation: rotation(product),
        scale: base.scale * delta.scale,
    };
    (t.position.is_finite() && t.rotation.is_finite() && t.scale.is_finite()).then_some(t)
}
pub(crate) fn read(p: &[f32]) -> Transform {
    Transform {
        position: Vec3::from_slice(p),
        rotation: Quat::from_slice(&p[3..]),
        scale: Vec3::from_slice(&p[7..]),
    }
}
pub(crate) fn put(p: &mut [f32], t: Transform) {
    p[..3].copy_from_slice(&t.position.to_array());
    p[3..7].copy_from_slice(&t.rotation.to_array());
    p[7..10].copy_from_slice(&t.scale.to_array());
}
pub(crate) fn node_pair(
    base: [Transform; 2],
    rows: [NodeOffsets; 2],
    node: &exact_game::asset::Node,
) -> [Transform; 2] {
    if !node.posable {
        return base;
    }
    resolve_pair(base, rows, &node.name).unwrap_or(base)
}
/// Validate the whole row before either endpoint is committed.
pub(crate) fn resolve_pair(
    base: [Transform; 2],
    rows: [NodeOffsets; 2],
    name: &str,
) -> Option<[Transform; 2]> {
    let hash = exact_game::NodeName::hash_str(name);
    let mut pair = base;
    for i in 0..2 {
        if let Some(row) = rows[i]
            .rows()
            .iter()
            .find(|r| r.node.hash() == hash && r.node.as_str() == name)
        {
            pair[i] = compose(base[i], row.transform)?;
        }
    }
    Some(pair)
}

/// Observe assignments even while an instance has no visible draw records.
#[derive(Default)]
pub(crate) struct Assignments {
    rows: BTreeMap<Entity, (Mesh, u64)>,
    revision: u64,
}
impl Assignments {
    pub fn update(&mut self, w: &World) {
        self.rows.retain(|e, _| w.contains(*e) && w.has::<Mesh>(*e));
        for (e, mesh) in w.query::<&Mesh>().iter() {
            if self.rows.get(&e).is_none_or(|(old, _)| old != mesh) {
                self.revision += 1;
                self.rows.insert(e, (mesh.clone(), self.revision));
            }
        }
    }
    pub fn revision(&self, e: Entity) -> u64 {
        self.rows.get(&e).map_or(0, |(_, revision)| *revision)
    }
}

/// Surface primes survive until drawn; re-feeds at that tick keep the captured endpoint.
#[derive(Default)]
pub(crate) struct Priming {
    tick: Option<u64>,
    assignment: (u64, u64),
    posed: bool,
    forced_tick: Option<u64>,
    primed_tick: Option<u64>,
    capture: bool,
    pending_draw: bool,
    undrawn: bool,
    previous: Vec<Transform>,
    offsets: NodeOffsets,
}
impl Priming {
    pub fn begin(
        &mut self,
        tick: u64,
        assignment: (u64, u64),
        posed: bool,
        force: bool,
        rows: [NodeOffsets; 2],
    ) -> [NodeOffsets; 2] {
        self.capture = self.tick.is_none()
            || self.assignment != assignment
            || (!self.posed && posed)
            || (force && self.forced_tick != Some(tick))
            || (self.pending_draw && self.tick != Some(tick));
        if force {
            self.forced_tick = Some(tick);
        }
        if self.capture {
            self.primed_tick = Some(tick);
            self.undrawn = true;
            self.offsets = rows[1];
        }
        self.tick = Some(tick);
        self.assignment = assignment;
        self.posed = posed;
        self.rows(rows)
    }
    /// Surface feeds may be overwritten by later ticks before encoding a frame.
    pub fn retain_until_draw(&mut self, retain: bool) {
        self.pending_draw = retain && self.undrawn && self.primed_tick == self.tick;
        if !retain {
            self.undrawn = false;
        }
    }
    pub fn rows(&self, rows: [NodeOffsets; 2]) -> [NodeOffsets; 2] {
        if self.primed_tick == self.tick {
            [self.offsets, rows[1]]
        } else {
            rows
        }
    }
    pub fn capturing(&self) -> bool {
        self.capture
    }
    pub fn captured(&mut self) {
        self.capture = false;
    }
    pub fn pair(&mut self, index: usize, mut base: [Transform; 2]) -> [Transform; 2] {
        if self.capture {
            if index >= self.previous.len() {
                self.previous.resize(index + 1, Transform::default());
            }
            self.previous[index] = base[1];
        }
        if self.primed_tick == self.tick {
            base[0] = self.previous[index];
        }
        base
    }
}
