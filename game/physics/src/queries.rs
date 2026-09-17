use crate::{math, step, Body, Collider, Hit, Shape};
use exact_game::{Entity, Transform, Vec3, World};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::{
    parry::{
        partitioning::{Bvh, BvhBuildStrategy},
        query::cast_shapes,
    },
    pipeline::PhysicsWorld,
    prelude::*,
};
use std::collections::BTreeMap;

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
/// Nearest collider along a ray. Direction is normalized, distances are metres,
/// sensors participate, and an inside origin returns zero. Ties use entity order.
pub fn raycast(world: &World, origin: Vec3, dir: Vec3, max: f32, mask: u32) -> Option<Hit> {
    if !origin.is_finite()
        || !dir.is_finite()
        || dir.length_squared() < 1e-16
        || max.is_nan()
        || max < 0.0
    {
        return None;
    }
    let dir = dir.normalize();
    let scene = Scene::new(world);
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
pub fn overlap(world: &World, shape: &Shape, pose: Transform, mask: u32) -> Vec<Entity> {
    let scene = Scene::new(world);
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
pub fn sweep(
    world: &World,
    shape: &Shape,
    pose: Transform,
    motion: Vec3,
    mask: u32,
) -> Option<Hit> {
    if !motion.is_finite() || motion.length_squared() < 1e-16 {
        return None;
    }
    let scene = Scene::new(world);
    let predicate = |_: ColliderHandle, c: &rapier3d::prelude::Collider| {
        c.collision_groups().memberships.bits() & mask != 0
    };
    let q = scene.queries(QueryFilter::default().predicate(&predicate));
    let shape = math::shape(shape, pose.scale);
    assert!(shape.is_convex(), "physics: sweeps need a convex shape");
    let p = math::pose(pose);
    let dir = math::vector(motion.normalize());
    let aabb = shape.compute_swept_aabb(&p, &(Pose::from_translation(math::vector(motion)) * p));
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
