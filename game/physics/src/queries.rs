use crate::{math, step, Body, Collider, Hit, Shape};
use exact_game::{Entity, Parent, Ref, Transform, Vec3, World};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::{
    parry::{
        partitioning::{Bvh, BvhBuildStrategy},
        query::cast_shapes,
    },
    pipeline::PhysicsWorld,
    prelude::*,
};
use std::cell::RefMut;
use std::collections::BTreeMap;

#[derive(PartialEq, Eq)]
struct Revisions([u64; 5]);
impl Revisions {
    fn of(w: &World) -> Self {
        Self([
            w.revision::<Body>(),
            w.revision::<Collider>(),
            w.revision::<Transform>(),
            w.revision::<Parent>(),
            w.presentation_generation(),
        ])
    }
}
pub(crate) struct Cached {
    world: exact_game::WorldId,
    revisions: Revisions,
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
/// The derived geometry/BVH survives scopes and ticks until relevant data changes.
pub fn queries(world: &World) -> Queries<'_> {
    Queries {
        world,
        physics: world.resource::<crate::Physics>(),
    }
}
impl Queries<'_> {
    pub(crate) fn scene(&self) -> RefMut<'_, Scene> {
        // Even an unchanged revision cannot authorize reading a live mutable lease.
        // Acquire the relevant shared leases before consulting the derived cache.
        let _leases = self.world.query::<(
            Option<&Body>,
            Option<&Collider>,
            Option<&Transform>,
            Option<&Parent>,
        )>();
        let revisions = Revisions::of(self.world);
        let mut cache = self.physics.executor.1.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|c| c.world != self.world.id() || c.revisions != revisions)
        {
            #[cfg(test)]
            let builds = cache.as_ref().map_or(1, |c| c.builds + 1);
            *cache = Some(Cached {
                world: self.world.id(),
                revisions,
                scene: Scene::new(self.world),
                #[cfg(test)]
                builds,
            });
        }
        RefMut::map(cache, |c| &mut c.as_mut().unwrap().scene)
    }
}

// A live component view: reads see same-tick edits without altering saved solver
// state or consuming collision events. The BVH and all geometry are Rapier/Parry.
pub(crate) struct Scene {
    pub rapier: PhysicsWorld,
    pub bvh: Bvh,
    pub entities: BTreeMap<[u32; 2], Entity>,
}
impl Scene {
    pub fn new(world: &World) -> Self {
        let mut rapier = PhysicsWorld::default();
        let mut entities = BTreeMap::new();
        for (e, (c, b)) in world.query::<(&Collider, Option<&Body>)>().iter() {
            let t = math::world_pose(world, e);
            let body = b.map(|b| {
                rapier.insert_body(
                    RigidBodyBuilder::new(step::body_type(b.kind))
                        .pose(math::pose(t))
                        .linvel(if b.kind == crate::BodyKind::Dynamic {
                            math::vector(b.velocity)
                        } else {
                            Vector::ZERO
                        })
                        .angvel(if b.kind == crate::BodyKind::Dynamic {
                            math::vector(b.spin)
                        } else {
                            Vector::ZERO
                        }),
                )
            });
            let h = rapier.insert_collider(
                step::collider(c, t, b).position(if b.is_some() {
                    Pose::IDENTITY
                } else {
                    math::pose(t)
                }),
                body,
            );
            if let Some(b) = body {
                rapier.bodies[b].recompute_mass_properties_from_colliders(&rapier.colliders);
            }
            entities.insert(crate::state::raw(h), e);
        }
        let bvh = Bvh::from_iter(
            BvhBuildStrategy::Binned,
            rapier
                .colliders
                .iter()
                .map(|(h, c)| (h.into_raw_parts().0 as usize, c.compute_aabb())),
        );
        Self {
            rapier,
            bvh,
            entities,
        }
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
        self.entities[&crate::state::raw(h)]
    }
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
        let predicate = |_: ColliderHandle, c: &rapier3d::prelude::Collider| {
            c.collision_groups().memberships.bits() & mask != 0
        };
        let q = scene.queries(QueryFilter::default().predicate(&predicate));
        q.intersect_ray(Ray::new(math::vector(origin), math::vector(dir)), max, true)
            .map(|(h, _, hit)| Hit {
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
        assert!(Revisions::of(&a) == Revisions::of(&b));
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
        let saved = w.save();
        let hash = w.hash();
        let view = queries(&w);
        for _ in 0..1000 {
            assert_eq!(view.raycast(Vec3::ZERO, Vec3::X, 20., 1).unwrap().entity, e);
        }
        assert_eq!(view.physics.executor.1.borrow().as_ref().unwrap().builds, 1);
        assert_eq!(w.hash(), hash);
        assert_eq!(w.save(), saved);
        w.get_mut::<Transform>(root).unwrap().position.x = 6.;
        assert!(view.raycast(Vec3::ZERO, Vec3::X, 4., 1).is_none());
        assert_eq!(view.physics.executor.1.borrow().as_ref().unwrap().builds, 2);
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
