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
struct Revisions([u64; 6]);
impl Revisions {
    fn of(w: &World) -> Self {
        Self([
            w.revision::<Body>(),
            w.revision::<Collider>(),
            w.revision::<Transform>(),
            w.revision::<Parent>(),
            w.presentation_generation(),
            w.entities_revision(),
        ])
    }
}
pub(crate) struct Cached {
    world: exact_game::WorldId,
    revisions: Revisions,
    scene: Scene,
    changes: crate::changes::Changes,
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
            .is_none_or(|c| c.world != self.world.id() || c.revisions.0[4] != revisions.0[4])
        {
            #[cfg(test)]
            let builds = cache.as_ref().map_or(0, |c| c.builds);
            *cache = Some(Cached {
                world: self.world.id(),
                revisions: Revisions([u64::MAX; 6]),
                scene: Scene::default(),
                changes: Default::default(),
                #[cfg(test)]
                builds,
            });
        }
        let cached = cache.as_mut().unwrap();
        if cached.revisions != revisions {
            let changed = cached.changes.refresh(self.world);
            cached.scene.refresh(self.world, &cached.changes, &changed);
            cached.revisions = revisions;
            #[cfg(test)]
            {
                cached.builds += 1;
            }
        }
        RefMut::map(cache, |c| &mut c.as_mut().unwrap().scene)
    }
}

// Only component values and derived geometry live here; neither partition enters
// EXPHYS. A dirty page is a candidate, not permission to rebuild static geometry.
struct Geometry {
    collider: Collider,
    body: Option<Body>,
    pose: Transform,
    handle: ColliderHandle,
}
#[derive(Default)]
pub(crate) struct Scene {
    parts: [Part; 2],
    controller: Option<Part>,
    #[cfg(test)]
    builds: [usize; 2],
}
impl Scene {
    fn refresh(
        &mut self,
        world: &World,
        changes: &crate::changes::Changes,
        changed: &crate::changes::Changed,
    ) {
        // The controller applies transient impulses to its private bodies. Even
        // if game code restores the previous component value, a write must discard
        // those impulses, just as the former revision-based scene rebuild did.
        self.controller = None;
        let mut dirty = [false; 2];
        if changed.membership {
            for (i, part) in self.parts.iter().enumerate() {
                dirty[i] = part
                    .rows
                    .keys()
                    .any(|&e| !world.has::<Collider>(e) || usize::from(world.has::<Body>(e)) != i);
            }
        }
        for &e in &changed.rows {
            let Some(c) = world.get::<Collider>(e) else {
                continue;
            };
            let b = world.get::<Body>(e);
            let i = usize::from(b.is_some());
            if !dirty[i] {
                dirty[i] = self.parts[i].rows.get(&e).is_none_or(|old| {
                    old.collider != *c
                        || old.body.as_ref() != b.as_deref()
                        || old.pose != math::world_pose(world, e)
                });
            }
        }
        for (i, members) in [&changes.statics, &changes.bodies].into_iter().enumerate() {
            if dirty[i] {
                self.parts[i] = Part::new(world, members);
                #[cfg(test)]
                {
                    self.builds[i] += 1;
                }
            }
        }
    }

    // Rapier's character controller requires one concrete QueryPipeline. Assemble
    // its view lazily from retained shapes, in the original entity/handle order,
    // with the original binned BVH. Its traversal ties affect pinned crate pushes.
    // Ray/overlap/sweep queries never pay for this combined controller view.
    pub(crate) fn controller(&mut self) -> &mut Part {
        self.controller.get_or_insert_with(|| {
            let mut ordered = BTreeMap::new();
            for part in &self.parts {
                for (&e, row) in &part.rows {
                    ordered.insert(e, (part, row));
                }
            }
            let mut result = Part::default();
            for (e, (part, row)) in ordered {
                let co = &part.rapier.colliders[row.handle];
                let body = co
                    .parent()
                    .map(|h| result.rapier.insert_body(part.rapier.bodies[h].clone()));
                let h = result.rapier.insert_collider(co.clone(), body);
                if let Some(b) = body {
                    result.rapier.bodies[b]
                        .recompute_mass_properties_from_colliders(&result.rapier.colliders);
                }
                result.entities.insert(crate::state::raw(h), e);
            }
            result.bvh = Bvh::from_iter(
                BvhBuildStrategy::Binned,
                result
                    .rapier
                    .colliders
                    .iter()
                    .map(|(h, c)| (h.into_raw_parts().0 as usize, c.compute_aabb())),
            );
            result
        })
    }
}
// A live component view: reads see same-tick edits without altering saved solver
// state or consuming collision events. The BVH and all geometry are Rapier/Parry.
#[derive(Default)]
pub(crate) struct Part {
    rows: BTreeMap<Entity, Geometry>,
    pub rapier: PhysicsWorld,
    pub bvh: Bvh,
    pub entities: BTreeMap<[u32; 2], Entity>,
}
impl Part {
    fn new(world: &World, members: &[Entity]) -> Self {
        let mut rows = BTreeMap::new();
        let mut rapier = PhysicsWorld::default();
        let mut entities = BTreeMap::new();
        for &e in members {
            let Some(collider) = world.get::<Collider>(e) else {
                continue;
            };
            let body = world.get::<Body>(e);
            let (c, b) = (&*collider, body.as_deref());
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
            rows.insert(
                e,
                Geometry {
                    collider: c.clone(),
                    body: b.cloned(),
                    pose: t,
                    handle: h,
                },
            );
        }
        let bvh = Bvh::from_iter(
            BvhBuildStrategy::Binned,
            rapier
                .colliders
                .iter()
                .map(|(h, c)| (h.into_raw_parts().0 as usize, c.compute_aabb())),
        );
        Self {
            rows,
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
        scene
            .parts
            .iter()
            .flat_map(|part| {
                let q = part.queries(QueryFilter::default().predicate(&predicate));
                q.intersect_ray(Ray::new(math::vector(origin), math::vector(dir)), max, true)
                    .map(|(h, _, hit)| Hit {
                        entity: part.entity(h),
                        distance: hit.time_of_impact,
                        point: origin + dir * hit.time_of_impact,
                        normal: math::vec3(hit.normal),
                    })
                    .min_by(nearest)
            })
            .min_by(nearest)
    }
    /// All colliders overlapping the supplied shape, sorted in entity order.
    pub fn overlap(&self, shape: &Shape, pose: Transform, mask: u32) -> Vec<Entity> {
        let scene = self.scene();
        let predicate = |_: ColliderHandle, c: &rapier3d::prelude::Collider| {
            c.collision_groups().memberships.bits() & mask != 0
        };
        let shape = math::shape(shape, pose.scale);
        let mut result = Vec::new();
        for part in &scene.parts {
            let q = part.queries(QueryFilter::default().predicate(&predicate));
            result.extend(
                q.intersect_shape(math::pose(pose), &*shape)
                    .map(|(h, _)| part.entity(h)),
            );
        }
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
        let shape = math::shape(shape, pose.scale);
        assert!(shape.is_convex(), "physics: sweeps need a convex shape");
        let p = math::pose(pose);
        let dir = math::vector(motion.normalize());
        let aabb =
            shape.compute_swept_aabb(&p, &(Pose::from_translation(math::vector(motion)) * p));
        scene
            .parts
            .iter()
            .flat_map(|part| {
                let q = part.queries(QueryFilter::default().predicate(&predicate));
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
                            entity: part.entity(h),
                            distance: hit.time_of_impact,
                            point: math::vec3(c.position() * hit.witness1),
                            normal: math::vec3(c.position().rotation * hit.normal1),
                        })
                    })
                    .min_by(nearest)
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

#[cfg(test)]
mod partition_tests {
    use super::*;
    use exact_game::PAGE;

    fn builds(w: &World) -> [usize; 2] {
        queries(w).scene().builds
    }

    #[test]
    fn controller_impulses_are_discarded_when_game_restores_the_cached_velocity() {
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        let e = w.spawn((Transform::default(), Collider::default(), Body::default()));
        let view = queries(&w);
        {
            let mut scene = view.scene();
            let controller = scene.controller();
            let handle = controller.rapier.bodies.iter().next().unwrap().0;
            controller.rapier.bodies[handle].set_linvel(Vector::X, true);
        }
        w.get_mut::<Body>(e).unwrap().velocity = Vec3::ZERO;
        let mut scene = view.scene();
        assert_eq!(scene.builds, [0, 1]);
        let controller = scene.controller();
        assert_eq!(
            controller.rapier.bodies.iter().next().unwrap().1.linvel(),
            Vector::ZERO
        );
    }
    fn check_against_single_scene(w: &World) {
        let members = w
            .query::<&Collider>()
            .iter()
            .map(|(e, _)| e)
            .collect::<Vec<_>>();
        let all = Part::new(w, &members);
        let view = queries(w);
        for mask in [1, 2, u32::MAX] {
            let predicate = |_: ColliderHandle, c: &rapier3d::prelude::Collider| {
                c.collision_groups().memberships.bits() & mask != 0
            };
            let q = all.queries(QueryFilter::default().predicate(&predicate));
            for x in [-2., 0., 3., 6., 9., 20.] {
                let origin = Vec3::new(x, 0., 0.);
                let expected = q
                    .intersect_ray(Ray::new(math::vector(origin), Vector::X), 100., true)
                    .map(|(h, _, hit)| Hit {
                        entity: all.entity(h),
                        distance: hit.time_of_impact,
                        point: origin + Vec3::X * hit.time_of_impact,
                        normal: math::vec3(hit.normal),
                    })
                    .min_by(nearest);
                let fields = |h: Hit| (h.entity, h.distance, h.point, h.normal);
                assert_eq!(
                    view.raycast(origin, Vec3::X, 100., mask).map(fields),
                    expected.map(fields)
                );
                let pose = Transform::at(x, 0., 0.);
                let shape = math::shape(&Shape::default(), Vec3::ONE);
                let mut expected = q
                    .intersect_shape(math::pose(pose), &*shape)
                    .map(|(h, _)| all.entity(h))
                    .collect::<Vec<_>>();
                expected.sort();
                assert_eq!(view.overlap(&Shape::default(), pose, mask), expected);
                // The previous single-BVH sweep narrow phase, including hit witnesses.
                let expected = all
                    .rapier
                    .colliders
                    .iter()
                    .filter(|(h, c)| predicate(*h, c))
                    .filter_map(|(h, c)| {
                        let hit = cast_shapes(
                            c.position(),
                            Vector::ZERO,
                            c.shape(),
                            &math::pose(pose),
                            Vector::X,
                            &*shape,
                            ShapeCastOptions {
                                max_time_of_impact: 100.,
                                ..Default::default()
                            },
                        )
                        .ok()??;
                        Some(Hit {
                            entity: all.entity(h),
                            distance: hit.time_of_impact,
                            point: math::vec3(c.position() * hit.witness1),
                            normal: math::vec3(c.position().rotation * hit.normal1),
                        })
                    })
                    .min_by(nearest);
                assert_eq!(
                    view.sweep(&Shape::default(), pose, Vec3::X * 100., mask)
                        .map(fields),
                    expected.map(fields)
                );
            }
        }
        // Character traversal keeps the former handles, shapes, body masses and BVH.
        let mut scene = view.scene();
        let combined = scene.controller();
        assert_eq!(combined.entities, all.entities);
        assert_eq!(
            bincode::serialize(&combined.bvh).unwrap(),
            bincode::serialize(&all.bvh).unwrap()
        );
        for (h, c) in all.rapier.colliders.iter() {
            let actual = &combined.rapier.colliders[h];
            assert_eq!(actual.position(), c.position());
            if let Some(b) = c.parent() {
                assert_eq!(
                    combined.rapier.bodies[b].mass(),
                    all.rapier.bodies[b].mass()
                );
                assert_eq!(
                    combined.rapier.bodies[b].linvel(),
                    all.rapier.bodies[b].linvel()
                );
            }
        }
    }

    #[test]
    fn static_bvh_survives_motion_and_tracks_cross_page_ancestors() {
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        w.resource_mut::<crate::Physics>().gravity = Vec3::ZERO;
        let grand = w.spawn(Transform::default());
        let parent = w.spawn(Parent(grand));
        while w.len() < PAGE {
            w.spawn(());
        }
        let e = w.spawn((
            Collider::default(),
            Transform::at(3., 0., 0.),
            Parent(parent),
        ));
        let unrelated = w.spawn(Transform::default());
        while w.len() < PAGE * 2 {
            w.spawn(());
        }
        w.spawn((
            Collider::default(),
            Transform::at(0., 10., 0.),
            Body {
                velocity: Vec3::X,
                ..Default::default()
            },
        ));
        assert_eq!(builds(&w), [1, 1]);
        for _ in 0..20 {
            crate::step(&mut w);
            assert_eq!(builds(&w)[0], 1);
        }
        // A conservative dirty page need not rebuild any unchanged static shapes.
        w.get_mut::<Transform>(unrelated).unwrap().position.x = 99.;
        assert_eq!(builds(&w)[0], 1);
        w.get_mut::<Transform>(grand).unwrap().position.x = 3.;
        assert_eq!(builds(&w)[0], 2);
        assert!(raycast(&w, Vec3::ZERO, Vec3::X, 4., 1).is_none());
        check_against_single_scene(&w);
        w.insert(parent, Transform::at(3., 0., 0.));
        assert_eq!(builds(&w)[0], 3);
        w.remove::<Transform>(grand);
        assert_eq!(builds(&w)[0], 4);
        w.get_mut::<Transform>(e).unwrap().position.x = 0.;
        assert_eq!(builds(&w)[0], 5);
        w.get_mut::<Collider>(e).unwrap().offset.x = 1.;
        assert_eq!(builds(&w)[0], 6);
        w.get_mut::<Collider>(e).unwrap().layer = 2;
        assert_eq!(builds(&w)[0], 7);
        check_against_single_scene(&w);
        let other = w.spawn(Transform::at(20., 0., 0.));
        w.insert(parent, Parent(other));
        check_against_single_scene(&w);
        w.despawn(other);
        check_against_single_scene(&w);
        let saved = w.save();
        w.get_mut::<Transform>(e).unwrap().position.x = 100.;
        check_against_single_scene(&w);
        w.load(&saved).unwrap();
        check_against_single_scene(&w);
    }

    #[test]
    fn ties_masks_membership_and_recycling_match_single_bvh() {
        for dynamic_first in [false, true] {
            let mut w = World::new(60, 0);
            crate::register(&mut w);
            let a = w.spawn((Transform::at(3., 0., 0.), Collider::default()));
            let b = w.spawn((Transform::at(3., 0., 0.), Collider::default()));
            w.insert(if dynamic_first { a } else { b }, Body::default());
            check_against_single_scene(&w);
            assert_eq!(raycast(&w, Vec3::ZERO, Vec3::X, 10., 1).unwrap().entity, a);
            assert_eq!(
                sweep(
                    &w,
                    &Shape::default(),
                    Transform::default(),
                    Vec3::X * 10.,
                    1
                )
                .unwrap()
                .entity,
                a
            );
            w.remove::<Body>(a);
            w.remove::<Body>(b);
            check_against_single_scene(&w);
            w.insert(
                a,
                Body {
                    kind: crate::BodyKind::Static,
                    ..Default::default()
                },
            );
            w.get_mut::<Collider>(b).unwrap().sensor = true;
            check_against_single_scene(&w);
            w.remove::<Collider>(a);
            check_against_single_scene(&w);
            w.despawn(b);
            let recycled = w.spawn((Transform::at(6., 0., 0.), Collider::default()));
            assert_ne!(b, recycled);
            check_against_single_scene(&w);
        }
    }
}
