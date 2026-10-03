use crate::{math, step, Body, CapsuleController, Collider, Hit, Shape};
use exact_game::{Entity, Parent, Ref, Transform, Vec3, World};
use rapier3d::parry::{
    partitioning::BvhNode,
    query::{RayCast, RayIntersection, ShapeCastOptions},
    shape::{Capsule, FeatureId, Shape as ParryShape},
};
use rapier3d::{
    parry::{
        partitioning::{Bvh, BvhBuildStrategy},
        query::cast_shapes,
    },
    pipeline::PhysicsWorld,
    prelude::*,
};
use std::cell::RefMut;
use std::collections::{BTreeMap, BTreeSet};

// Write revisions of every column the scene derives from, in this order.
const COLUMNS: usize = 5;
fn revisions(w: &World) -> [u64; COLUMNS] {
    [
        w.revision::<Body>(),
        w.revision::<Collider>(),
        w.revision::<Transform>(),
        w.revision::<Parent>(),
        w.revision::<CapsuleController>(),
    ]
}
pub(crate) struct Cached {
    world: exact_game::WorldId,
    presentation: u64,
    revisions: [u64; COLUMNS],
    scene: Scene,
    #[cfg(test)]
    builds: usize,
}

/// Scoped live queries sharing a world-owned scene. Each operation checks write
/// revisions, including same-tick edits; reads never mutate saved solver state.
/// Drop the view before structural edits or physics::step, then acquire it again.
pub struct Queries<'w> {
    world: &'w World,
    physics: Ref<'w, crate::Physics>,
}
/// Borrow a reusable query view. Call register during world setup first.
/// The derived geometry/BVH survives scopes and ticks; edits update only their rows.
pub fn queries(world: &World) -> Queries<'_> {
    Queries {
        world,
        physics: world.resource::<crate::Physics>(),
    }
}
impl Queries<'_> {
    pub(crate) fn scene(&self) -> RefMut<'_, Scene> {
        // Even an unchanged revision cannot authorize reading a live mutable lease.
        // Hold every row of the relevant columns before consulting the derived cache;
        // a query leases nothing until iterated.
        // @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
        let _leases = (
            self.world.pages::<Body>(),
            self.world.pages::<Collider>(),
            self.world.pages::<Transform>(),
            self.world.pages::<Parent>(),
            self.world.pages::<CapsuleController>(),
        );
        let w = self.world;
        let revisions = revisions(w);
        let mut cache = self.physics.executor.1.borrow_mut();
        let current = cache
            .as_ref()
            .is_some_and(|c| c.world == w.id() && c.presentation == w.presentation_generation());
        if !current || cache.as_ref().unwrap().scene.churned() {
            #[cfg(test)]
            let builds = cache.as_ref().map_or(1, |c| c.builds + 1);
            *cache = Some(Cached {
                world: w.id(),
                presentation: w.presentation_generation(),
                revisions,
                scene: Scene::new(w),
                #[cfg(test)]
                builds,
            });
        } else {
            let c = cache.as_mut().unwrap();
            if c.revisions != revisions {
                c.scene.update(w, &c.revisions, &revisions);
                c.revisions = revisions;
            }
        }
        RefMut::map(cache, |c| &mut c.as_mut().unwrap().scene)
    }
}

// What one entity's collider was derived from, and its handles in the scene.
struct Slot {
    entity: Entity,
    collider: ColliderHandle,
    body: Option<RigidBodyHandle>,
    pose: Transform,
    shape: Collider,
    kind: Option<Body>,
    character: bool,
}

// A live component view: reads see same-tick edits without altering saved solver
// state or consuming collision events. The BVH and all geometry are Rapier/Parry.
// Only rows written since the last operation (and parented colliders, whose pose
// follows their ancestors) are revisited; results never depend on the BVH's shape.
pub(crate) struct Scene {
    pub rapier: PhysicsWorld,
    pub bvh: Bvh,
    slots: BTreeMap<u32, Slot>,
    parented: BTreeSet<u32>,
    // Body insertions, which Rapier's modified-body list retains until a rebuild.
    churn: usize,
    #[cfg(test)]
    pub(crate) updates: usize,
}
impl Scene {
    pub fn new(world: &World) -> Self {
        let mut scene = Self {
            rapier: PhysicsWorld::default(),
            bvh: Bvh::new(),
            slots: BTreeMap::new(),
            parented: BTreeSet::new(),
            churn: 0,
            #[cfg(test)]
            updates: 0,
        };
        let rows: Vec<Entity> = world.query::<&Collider>().iter().map(|(e, _)| e).collect();
        for e in rows {
            scene.refresh(world, e, false);
        }
        scene.churn = 0;
        #[cfg(test)]
        {
            scene.updates = 0;
        }
        let leaves: Vec<_> = scene
            .rapier
            .colliders
            .iter()
            .map(|(h, c)| (h.into_raw_parts().0, c.compute_aabb()))
            .collect();
        scene.bvh = bvh(&leaves);
        scene
    }
    fn churned(&self) -> bool {
        self.churn > 1024 + 2 * self.rapier.bodies.len()
    }
    fn update(&mut self, world: &World, since: &[u64; COLUMNS], now: &[u64; COLUMNS]) {
        // Only rows that hold or held a collider matter; a write to a collider-free
        // mover (a rocket, the camera) is dropped here, before any sorting.
        let slots = &self.slots;
        let relevant = |e: &Entity| slots.contains_key(&e.index()) || world.has::<Collider>(*e);
        let mut rows: Vec<Entity> = Vec::new();
        if since[0] != now[0] {
            rows.extend(world.changed::<Body>(since[0]).filter(relevant));
        }
        if since[1] != now[1] {
            rows.extend(world.changed::<Collider>(since[1]).filter(relevant));
        }
        if since[2] != now[2] {
            rows.extend(world.changed::<Transform>(since[2]).filter(relevant));
        }
        if since[3] != now[3] {
            rows.extend(world.changed::<Parent>(since[3]).filter(relevant));
        }
        if since[4] != now[4] {
            rows.extend(
                world
                    .changed::<CapsuleController>(since[4])
                    .filter(relevant),
            );
        }
        // A child's world pose follows any ancestor's write.
        if since[2] != now[2] || since[3] != now[3] {
            rows.extend(self.parented.iter().map(|i| self.slots[i].entity));
        }
        rows.sort_by_key(|e| e.index());
        rows.dedup_by_key(|e| e.index());
        for e in rows {
            self.refresh(world, e, true);
        }
        self.rapier.colliders.take_modified();
        self.rapier.colliders.take_removed();
    }
    // Make one entity index match the world: unchanged rows cost a comparison.
    fn refresh(&mut self, world: &World, e: Entity, bvh: bool) {
        let index = e.index();
        if !self.slots.contains_key(&index) && !world.has::<Collider>(e) {
            return; // A collider-free mover, such as a camera or a tracer.
        }
        let live = world.contains(e);
        let c = live.then(|| world.get::<Collider>(e)).flatten();
        if live && world.has::<Parent>(e) && c.is_some() {
            self.parented.insert(index);
        } else {
            self.parented.remove(&index);
        }
        let Some(c) = c else {
            if let Some(old) = self.slots.remove(&index) {
                self.detach(&old, bvh);
                if let Some(h) = old.body {
                    self.rapier.remove_body(h);
                }
            }
            return;
        };
        let b = world.get::<Body>(e);
        let t = math::world_pose(world, e);
        let character = world.has::<CapsuleController>(e);
        if self.slots.get(&index).is_some_and(|old| {
            old.entity == e
                && old.pose == t
                && old.shape == *c
                && old.kind.as_ref() == b.as_deref()
                && old.character == character
        }) {
            return;
        }
        let old = self.slots.remove(&index);
        #[cfg(test)]
        {
            self.updates += 1;
        }
        let mut body = None;
        if let Some(old) = &old {
            self.detach(old, bvh);
            body = old.body.filter(|_| old.entity == e);
            if let Some(h) = old.body.filter(|_| b.is_none() || old.entity != e) {
                self.rapier.remove_body(h);
                body = None;
            }
        }
        if let Some(b) = b.as_deref() {
            let dynamic = b.kind == crate::BodyKind::Dynamic;
            let linvel = if dynamic {
                math::vector(b.velocity)
            } else {
                Vector::ZERO
            };
            let angvel = if dynamic {
                math::vector(b.spin)
            } else {
                Vector::ZERO
            };
            match body {
                Some(h) => {
                    let rb = &mut self.rapier.bodies[h];
                    rb.set_body_type(step::body_type(b.kind), false);
                    rb.set_position(math::pose(t), false);
                    rb.set_linvel(linvel, false);
                    rb.set_angvel(angvel, false);
                }
                None => {
                    self.churn += 1;
                    body = Some(
                        self.rapier.insert_body(
                            RigidBodyBuilder::new(step::body_type(b.kind))
                                .pose(math::pose(t))
                                .linvel(linvel)
                                .angvel(angvel),
                        ),
                    );
                }
            }
        }
        // Controllers move with sensors in the solver, but are solid to each other:
        // another character's controller filter excludes sensors.
        let collider = self.rapier.insert_collider(
            step::collider(&c, t, b.as_deref())
                .sensor(c.sensor && !character)
                .position(if b.is_some() {
                    Pose::IDENTITY
                } else {
                    math::pose(t)
                })
                .user_data(index as u128),
            body,
        );
        if let Some(h) = body {
            self.rapier.bodies[h].recompute_mass_properties_from_colliders(&self.rapier.colliders);
        }
        if bvh {
            let aabb = self.rapier.colliders[collider].compute_aabb();
            self.bvh.insert(aabb, collider.into_raw_parts().0);
        }
        self.slots.insert(
            index,
            Slot {
                entity: e,
                collider,
                body,
                pose: t,
                shape: c.clone(),
                kind: b.map(|b| b.clone()),
                character,
            },
        );
    }
    fn detach(&mut self, old: &Slot, bvh: bool) {
        if bvh {
            self.bvh.remove(old.collider.into_raw_parts().0);
        }
        let r = &mut self.rapier;
        r.colliders
            .remove(old.collider, &mut r.islands, &mut r.bodies, false);
    }
    pub fn queries<'a>(&'a self, filter: QueryFilter<'a>) -> QueryPipeline<'a> {
        QueryPipeline {
            dispatcher: self.rapier.narrow_phase.query_dispatcher(),
            bvh: &self.bvh,
            bodies: &self.rapier.bodies,
            colliders: &self.rapier.colliders,
            filter,
        }
    }
    pub fn entity(&self, h: ColliderHandle) -> Entity {
        self.entity_at(self.rapier.colliders[h].user_data as u32)
    }
    pub fn entity_at(&self, index: u32) -> Entity {
        self.slots[&index].entity
    }
    pub fn collider(&self, e: Entity) -> Option<ColliderHandle> {
        self.slots
            .get(&e.index())
            .filter(|s| s.entity == e)
            .map(|s| s.collider)
    }
}
/// A binned BVH over (collider index, AABB) leaves, in the order given.
/// Parry's bulk build assumes leaf ids 0 and 1 below three leaves; insert those.
pub(crate) fn bvh(leaves: &[(u32, Aabb)]) -> Bvh {
    if leaves.len() > 2 {
        return Bvh::from_iter(
            BvhBuildStrategy::Binned,
            leaves.iter().map(|&(i, aabb)| (i as usize, aabb)),
        );
    }
    let mut bvh = Bvh::new();
    for &(i, aabb) in leaves {
        bvh.insert(aabb, i);
    }
    bvh
}
// Parry 0.30 casts capsule rays by support-map GJK, which misses some rays that
// pass straight through (RIVALS: 64 of 16,000 aimed inside); capsules use the
// closed form, including a compound's capsule child (an offset collider).
fn cast_ray(shape: &dyn ParryShape, pos: &Pose, ray: &Ray, max: f32) -> Option<RayIntersection> {
    if let Some(c) = shape.as_capsule() {
        return ray_capsule(c, pos, ray, max);
    }
    if let Some(compound) = shape.as_compound() {
        return compound
            .shapes()
            .iter()
            .filter_map(|(local, child)| cast_ray(&**child, &(*pos * *local), ray, max))
            .min_by(|a, b| a.time_of_impact.total_cmp(&b.time_of_impact));
    }
    shape.cast_ray_and_get_normal(pos, ray, max, true)
}
// Solid: an origin inside returns zero distance and a zero normal, as Parry does.
fn ray_capsule(c: &Capsule, pos: &Pose, ray: &Ray, max: f32) -> Option<RayIntersection> {
    let (o, d) = (
        pos.inverse_transform_point(ray.origin),
        pos.rotation.inverse() * ray.dir,
    );
    let (a, b, r) = (c.segment.a, c.segment.b, c.radius);
    let ba = b - a;
    let baba = ba.dot(ba);
    let closest = |p: Vector| {
        let s = if baba > 0.0 {
            ((p - a).dot(ba) / baba).clamp(0.0, 1.0)
        } else {
            0.0
        };
        a + ba * s
    };
    if (o - closest(o)).length_squared() <= r * r {
        return Some(RayIntersection::new(0.0, Vector::ZERO, FeatureId::Unknown));
    }
    let sphere = |centre: Vector| {
        let oc = o - centre;
        let k = d.dot(oc);
        let h = k * k - (oc.dot(oc) - r * r);
        (h >= 0.0).then(|| -k - h.sqrt())
    };
    let (oa, bard) = (o - a, ba.dot(d));
    let baoa = ba.dot(oa);
    let k2 = baba - bard * bard;
    let t = if k2 > 1e-9 * baba.max(1e-9) {
        let k1 = baba * oa.dot(d) - baoa * bard;
        let k0 = baba * oa.dot(oa) - baoa * baoa - r * r * baba;
        let h = k1 * k1 - k2 * k0;
        if h < 0.0 {
            return None;
        }
        let t = (-k1 - h.sqrt()) / k2;
        let y = baoa + t * bard;
        if y > 0.0 && y < baba {
            Some(t)
        } else {
            sphere(if y <= 0.0 { a } else { b })
        }
    } else {
        match (sphere(a), sphere(b)) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (x, y) => x.or(y),
        }
    }?;
    if !(0.0..=max).contains(&t) {
        return None;
    }
    let p = o + d * t;
    let normal = (p - closest(p)).normalize_or_zero();
    Some(RayIntersection::new(
        t,
        pos.rotation * normal,
        FeatureId::Unknown,
    ))
}
fn nearest(a: &Hit, b: &Hit) -> std::cmp::Ordering {
    a.distance
        .total_cmp(&b.distance)
        .then(a.entity.cmp(&b.entity))
}
impl Queries<'_> {
    /// Nearest collider along a ray. Direction is normalized, distances are metres,
    /// sensors participate, and an inside origin returns zero. Ties use entity order.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32, mask: u32) -> Option<Hit> {
        if !origin.is_finite()
            || !dir.is_finite()
            || dir.length_squared() < 1e-16
            || max.is_nan()
            || max < 0.0
        {
            return None;
        }
        let dir = dir.normalize();
        let scene = self.scene();
        let ray = Ray::new(math::vector(origin), math::vector(dir));
        let colliders = &scene.rapier.colliders;
        scene
            .bvh
            .leaves(|node: &BvhNode| node.aabb().intersects_local_ray(&ray, max))
            .filter_map(|leaf| {
                let (c, h) = colliders.get_unknown_gen(leaf)?;
                (c.collision_groups().memberships.bits() & mask != 0).then_some(())?;
                Some((h, cast_ray(c.shape(), c.position(), &ray, max)?))
            })
            .map(|(h, hit)| Hit {
                entity: scene.entity(h),
                distance: hit.time_of_impact,
                point: origin + dir * hit.time_of_impact,
                normal: math::vec3(hit.normal),
            })
            .min_by(nearest)
    }
    /// All colliders overlapping the supplied shape, sorted in entity order.
    pub fn overlap(&self, shape: &Shape, pose: Transform, mask: u32) -> Vec<Entity> {
        let scene = self.scene();
        let predicate = |_: ColliderHandle, c: &rapier3d::prelude::Collider| {
            c.collision_groups().memberships.bits() & mask != 0
        };
        let q = scene.queries(QueryFilter::default().predicate(&predicate));
        let shape = math::shape(shape, pose.scale);
        let mut result: Vec<_> = q
            .intersect_shape(math::pose(pose), &*shape)
            .map(|(h, _)| scene.entity(h))
            .collect();
        result.sort();
        result
    }
    /// First hit of a translating convex shape. Distance is metres along `motion`;
    /// rotation is held fixed. Sensors participate; equal-distance ties use entity order.
    pub fn sweep(&self, shape: &Shape, pose: Transform, motion: Vec3, mask: u32) -> Option<Hit> {
        if !motion.is_finite() || motion.length_squared() < 1e-16 {
            return None;
        }
        let scene = self.scene();
        let predicate = |_: ColliderHandle, c: &rapier3d::prelude::Collider| {
            c.collision_groups().memberships.bits() & mask != 0
        };
        let q = scene.queries(QueryFilter::default().predicate(&predicate));
        let shape = math::shape(shape, pose.scale);
        assert!(shape.is_convex(), "physics: sweeps need a convex shape");
        let p = math::pose(pose);
        let dir = math::vector(motion.normalize());
        let aabb =
            shape.compute_swept_aabb(&p, &(Pose::from_translation(math::vector(motion)) * p));
        q.intersect_aabb_conservative(aabb)
            .filter_map(|(h, c)| {
                let hit = cast_shapes(
                    c.position(),
                    Vector::ZERO,
                    c.shape(),
                    &p,
                    dir,
                    &*shape,
                    ShapeCastOptions {
                        max_time_of_impact: motion.length(),
                        ..ShapeCastOptions::default()
                    },
                )
                .ok()??;
                Some(Hit {
                    entity: scene.entity(h),
                    distance: hit.time_of_impact,
                    point: math::vec3(c.position() * hit.witness1),
                    normal: math::vec3(c.position().rotation * hit.normal1),
                })
            })
            .min_by(nearest)
    }
}
/// Nearest ray hit using the world's retained query scene. See Queries::raycast.
pub fn raycast(world: &World, origin: Vec3, dir: Vec3, max: f32, mask: u32) -> Option<Hit> {
    queries(world).raycast(origin, dir, max, mask)
}
/// Sorted overlaps using the world's retained query scene. See Queries::overlap.
pub fn overlap(world: &World, shape: &Shape, pose: Transform, mask: u32) -> Vec<Entity> {
    queries(world).overlap(shape, pose, mask)
}
/// First translating-shape hit using the retained scene. See Queries::sweep.
pub fn sweep(
    world: &World,
    shape: &Shape,
    pose: Transform,
    motion: Vec3,
    mask: u32,
) -> Option<Hit> {
    queries(world).sweep(shape, pose, motion, mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_queries_follow_physics_into_an_equal_revision_world() {
        let mut a = World::new(60, 0);
        let mut b = World::new(60, 0);
        crate::register(&mut a);
        crate::register(&mut b);
        a.spawn((Collider::default(), Transform::at(3., 0., 0.)));
        b.spawn((Collider::default(), Transform::at(9., 0., 0.)));
        assert_eq!(revisions(&a), revisions(&b));
        assert!(raycast(&a, Vec3::ZERO, Vec3::X, 4., 1).is_some());
        let physics = std::mem::take(&mut *a.resource_mut::<crate::Physics>());
        drop(a); // The cached identity must keep the original token alive.
        b.insert_resource(physics);
        let hash = b.hash();
        let saved = b.save();
        assert!(raycast(&b, Vec3::ZERO, Vec3::X, 4., 1).is_none());
        assert!(raycast(&b, Vec3::ZERO, Vec3::X, 10., 1).is_some());
        assert_eq!(
            queries(&b)
                .physics
                .executor
                .1
                .borrow()
                .as_ref()
                .unwrap()
                .builds,
            2
        );
        let moved = Box::new(b);
        assert!(raycast(&moved, Vec3::ZERO, Vec3::X, 10., 1).is_some());
        assert_eq!(
            queries(&moved)
                .physics
                .executor
                .1
                .borrow()
                .as_ref()
                .unwrap()
                .builds,
            2
        );
        assert_eq!(moved.hash(), hash);
        assert_eq!(moved.save(), saved);
    }

    #[test]
    fn retained_queries_refresh_after_same_tick_edits_and_restore() {
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        let root = w.spawn(Transform::at(3., 0., 0.));
        let e = w.spawn((Collider::default(), Parent(root)));
        let free = w.spawn(Transform::default());
        let saved = w.save();
        let hash = w.hash();
        let view = queries(&w);
        let cached = |view: &Queries| {
            let c = view.physics.executor.1.borrow();
            let c = c.as_ref().unwrap();
            (c.builds, c.scene.updates)
        };
        for _ in 0..1000 {
            assert_eq!(view.raycast(Vec3::ZERO, Vec3::X, 20., 1).unwrap().entity, e);
        }
        assert_eq!(cached(&view), (1, 0));
        assert_eq!(w.hash(), hash);
        assert_eq!(w.save(), saved);
        // An entity without a collider never revisits the scene's colliders...
        w.get_mut::<Transform>(free).unwrap().position.x = 1.;
        assert!(view.raycast(Vec3::ZERO, Vec3::X, 20., 1).is_some());
        assert_eq!(cached(&view), (1, 0));
        // ...but an ancestor's write moves its parented collider, and only that one.
        w.get_mut::<Transform>(root).unwrap().position.x = 6.;
        assert!(view.raycast(Vec3::ZERO, Vec3::X, 4., 1).is_none());
        assert_eq!(cached(&view), (1, 1));
        let lease = w.get_mut::<Transform>(root).unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| view.raycast(
                Vec3::ZERO,
                Vec3::X,
                20.,
                1
            )))
            .is_err()
        );
        drop(lease);
        w.get_mut::<Collider>(e).unwrap().layer = 2;
        assert!(view
            .overlap(&Shape::default(), Transform::at(6., 0., 0.), 1)
            .is_empty());
        drop(view);
        // Component membership, recycling, and load must invalidate retained views.
        w.remove::<Collider>(e);
        assert!(queries(&w).raycast(Vec3::ZERO, Vec3::X, 20., 2).is_none());
        w.insert(e, Collider::default());
        w.insert(e, Transform::at(2., 0., 0.));
        w.remove::<Parent>(e);
        assert!(queries(&w)
            .sweep(&Shape::default(), Transform::default(), Vec3::X * 10., 1)
            .is_some());
        w.despawn(e);
        assert!(queries(&w).raycast(Vec3::ZERO, Vec3::X, 20., 1).is_none());
        w.load(&saved).unwrap();
        assert_eq!(
            queries(&w)
                .raycast(Vec3::ZERO, Vec3::X, 20., 1)
                .unwrap()
                .entity,
            e
        );
        assert_eq!(w.hash(), hash);
    }
}
