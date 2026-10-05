// @ref LLP 1046.008#a-effective-visibility — one current-row presentation predicate.
use crate::{Affine3A, Entity, Offset, Opacity, Parent, Transform, Visible, World};

/// An entity as drawn: its displayed pose and effective opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    /// drawn(parent)·local·offset down the Parent chain.
    pub pose: Affine3A,
    /// The product of `Opacity` down the Parent chain.
    pub opacity: f32,
}

impl World {
    /// Current effective visibility through living Parent ancestors.
    /// Missing rows are true; false ancestors cannot be overridden by children.
    /// Dangling chains and unrepaired cycles are hidden. This reads current rows
    /// without propagation, caching, or changing saved state. SocketFollow alone
    /// is independent; visibility affects presentation and picking, not simulation.
    #[track_caller]
    pub fn is_visible(&self, mut entity: Entity) -> bool {
        // A living acyclic chain cannot visit more entities than the world owns.
        for _ in 0..self.len() {
            if !self.contains(entity) || self.get::<Visible>(entity).is_some_and(|v| !v.0) {
                return false;
            }
            let parent = self.get::<Parent>(entity).map(|p| p.0);
            match parent {
                Some(parent) => entity = parent,
                None => return true,
            }
        }
        false
    }
    /// The entity as drawn, from current rows: pose drawn(parent)·local·offset
    /// and opacity multiplied down the Parent chain, in the same bounded walk as
    /// [`World::is_visible`]; `None` wherever that is false (hidden, dead,
    /// dangling or cyclic) or the entity has no Transform. Presentation reads:
    /// the renderer and hooks call it, ticks cannot (it reads Offset/Opacity).
    #[track_caller]
    pub fn drawn(&self, mut entity: Entity) -> Option<Drawn> {
        self.get::<Transform>(entity)?;
        let mut chain = Vec::new();
        for _ in 0..self.len() {
            if !self.contains(entity) || self.get::<Visible>(entity).is_some_and(|v| !v.0) {
                return None;
            }
            chain.push(entity);
            match self.get::<Parent>(entity).map(|p| p.0) {
                Some(parent) => entity = parent,
                None => {
                    let opacity = chain
                        .iter()
                        .map(|&e| self.get::<Opacity>(e).map_or(1.0, |o| o.0))
                        .product();
                    // Above the root-most offset the pose is the propagated
                    // global exactly (no recomposition, so no rounding drift);
                    // without any offset it is that global.
                    let Some(top) = chain.iter().rposition(|&e| self.has::<Offset>(e)) else {
                        return Some(Drawn {
                            pose: self.global(chain[0])?,
                            opacity,
                        });
                    };
                    let mut pose = chain
                        .get(top + 1)
                        .map_or(Some(Affine3A::IDENTITY), |&parent| self.global(parent))?;
                    for &e in chain[..=top].iter().rev() {
                        let local = self
                            .get::<Transform>(e)
                            .map_or_else(Transform::default, |t| *t);
                        let offset = self
                            .get::<Offset>(e)
                            .map_or_else(Transform::default, |o| o.0);
                        pose = pose * affine(local) * affine(offset);
                    }
                    return Some(Drawn { pose, opacity });
                }
            }
        }
        None
    }
    /// The entity's own presentation offset (identity when absent), for a
    /// renderer that interpolates each link of the chain itself.
    pub fn drawn_local_offset(&self, entity: Entity) -> Transform {
        self.get::<Offset>(entity)
            .map_or_else(Transform::default, |o| o.0)
    }
}

fn affine(t: Transform) -> Affine3A {
    Affine3A::from_scale_rotation_translation(t.scale, t.rotation, t.position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transform;

    #[test]
    fn current_ancestor_rows_and_reparenting_control_visibility() {
        let mut w = World::new(60, 0);
        let root = w.spawn(Transform::default());
        let hidden = w.spawn(Visible(false));
        let child = w.spawn((Parent(root), Visible(true)));
        let leaf = w.spawn(Parent(child));
        let local_false = w.spawn((Parent(root), Visible(false)));
        assert!(w.is_visible(leaf));
        w.insert(root, Visible(false));
        assert!(!w.is_visible(child));
        assert!(!w.is_visible(leaf));
        w.remove::<Visible>(root);
        assert!(w.is_visible(leaf));
        assert!(!w.is_visible(local_false));
        w.insert(child, Parent(hidden));
        assert!(!w.is_visible(leaf));
        w.remove::<Parent>(child);
        assert!(w.is_visible(leaf));
        // None of the transitions requires a tick or a propagate.
        assert_eq!(w.tick(), 0);
    }

    #[test]
    fn stale_generations_and_cycles_never_reveal_a_child() {
        let mut w = World::new(60, 0);
        let root = w.spawn(Transform::default());
        let child = w.spawn((Transform::default(), Parent(root)));
        w.despawn(root);
        let replacement = w.spawn(Transform::default());
        assert!(!w.is_visible(root));
        assert!(!w.is_visible(child));
        assert!(w.is_visible(replacement));
        w.insert(child, Parent(replacement));
        assert!(w.is_visible(child));
        w.insert(replacement, Parent(child));
        assert!(!w.is_visible(child));
        assert!(!w.is_visible(replacement));
        w.propagate();
        assert!(w.is_visible(child));
        assert!(w.is_visible(replacement));
    }

    #[test]
    fn picks_follow_visibility_without_mutating_saves_or_spatial_queries() {
        use crate::{spatial, Camera, Mesh, Vec2};
        let mut w = World::new(60, 0);
        let root = w.spawn((Transform::default(), Visible(false)));
        let child = w.spawn((Transform::at(0., 0., -3.), Parent(root), Mesh::cube(1.)));
        // Camera choice deliberately ignores visibility.
        w.spawn((Transform::default(), Camera::default(), Visible(false)));
        w.propagate();
        let saved = w.save();
        let hash = w.hash();
        let view = spatial::View::new(&w, Vec2::splat(200.)).unwrap();
        assert!(spatial::pick(&w, &view, Vec2::splat(100.)).is_none());
        assert_eq!(w.nearest_xz::<Mesh>(root, 10.), Some(child));
        assert!(!w.is_visible(child));
        assert_eq!(w.save(), saved);
        assert_eq!(w.hash(), hash);
        w.remove::<Visible>(root);
        assert_eq!(
            spatial::pick(&w, &view, Vec2::splat(100.)).unwrap().0,
            child
        );
    }

    #[test]
    fn multipart_and_deep_visibility_query_cost() {
        use std::{hint::black_box, time::Instant};
        let mut w = World::new(60, 0);
        let mut leaves = Vec::new();
        for _ in 0..1000 {
            let root = w.spawn(Visible(true));
            let middle = w.spawn(Parent(root));
            leaves.push(w.spawn(Parent(middle)));
        }
        let mut deep = w.spawn(());
        for _ in 0..256 {
            deep = w.spawn(Parent(deep));
        }
        let start = Instant::now();
        for _ in 0..100 {
            for &e in &leaves {
                assert!(black_box(w.is_visible(black_box(e))));
            }
        }
        let multipart = start.elapsed().as_micros() as f64 / 100.;
        let start = Instant::now();
        for _ in 0..1000 {
            assert!(black_box(w.is_visible(black_box(deep))));
        }
        eprintln!(
            "visibility: 1000 multipart leaves {multipart:.1}us/pass; 257-deep chain {:.2}us/query",
            start.elapsed().as_micros() as f64 / 1000.
        );
    }

    #[test]
    fn hidden_emission_survives_fresh_restore_and_reveal() {
        use crate::{Emitter, Game, Input, Sim};
        struct Fixture;
        impl Game for Fixture {
            const ID: &'static str = "inherited-visibility-save";
            type Args = ();
            fn setup(w: &mut World, _: &()) {
                let root = w.spawn_named("root", (Transform::default(), Visible(false)));
                w.spawn_named(
                    "emitter",
                    (Transform::default(), Parent(root), Emitter::sparks()),
                );
            }
            fn tick(w: &mut World, _: &Input, _: &()) {
                crate::emitter::step(w);
            }
        }
        let mut sim = Sim::<Fixture>::new(()).unwrap();
        sim.run(500.);
        let e = sim.world().named("emitter").unwrap();
        assert!(!sim.world().is_visible(e));
        assert!(!sim.world().require::<Emitter>(e).state.births.is_empty());
        let saved = sim.save().unwrap();
        let mut fresh = Sim::<Fixture>::new(()).unwrap();
        fresh.restore(&saved).unwrap();
        assert!(!fresh.world().is_visible(e));
        for s in [&mut sim, &mut fresh] {
            let root = s.world().named("root").unwrap();
            s.world_mut().remove::<Visible>(root);
            assert!(s.world().is_visible(e));
            s.run(500.);
        }
        assert_eq!(sim.save().unwrap(), fresh.save().unwrap());
    }

    #[test]
    fn drawn_poses_and_opacity_inherit_down_the_parent_chain() {
        use crate::{Offset, Opacity, Vec3};
        let mut w = World::new(60, 0);
        let root = w.spawn((
            Transform::at(1., 0., 0.),
            Offset(Transform::at(0., 0.5, 0.)),
            Opacity(0.5),
        ));
        let child = w.spawn((Transform::at(0., 2., 0.), Parent(root), Opacity(0.5)));
        let leaf = w.spawn((
            Transform::at(0., 0., 3.).with_scale(2.),
            Parent(child),
            Offset(Transform::at(1., 0., 0.)),
        ));
        w.propagate();
        let at = |e| w.drawn(e).unwrap();
        // The root's offset moves its displayed hierarchy; the leaf's own is in its frame.
        assert_eq!(at(root).pose.translation, Vec3::new(1., 0.5, 0.).into());
        assert_eq!(at(child).pose.translation, Vec3::new(1., 2.5, 0.).into());
        assert_eq!(at(leaf).pose.translation, Vec3::new(3., 2.5, 3.).into());
        assert_eq!(
            (at(root).opacity, at(child).opacity, at(leaf).opacity),
            (0.5, 0.25, 0.25)
        );
        // Below an offset, a plain child composes from it.
        let plain = w.spawn((Transform::at(0., 1., 0.), Parent(child)));
        w.propagate();
        let expected =
            w.drawn(child).unwrap().pose * Affine3A::from_translation(Vec3::new(0., 1., 0.));
        assert!(w.drawn(plain).unwrap().pose.abs_diff_eq(expected, 1e-6));
        // Without any offset on the chain, drawn is the propagated global exactly.
        let lone = w.spawn(Transform::at(0.1, 0.2, 0.3).with_scale(1.7));
        let under = w.spawn((Transform::at(0.7, 0.1, 0.9), Parent(lone)));
        w.propagate();
        assert_eq!(w.drawn(under).unwrap().pose, w.global(under).unwrap());
        // A hidden ancestor hides the drawn chain whatever the opacity.
        w.insert(root, Visible(false));
        assert!(w.drawn(leaf).is_none());
        w.remove::<Visible>(root);
        // Cycles and dangling parents are not drawn.
        w.insert(root, Parent(leaf));
        assert!(w.drawn(child).is_none());
        w.remove::<Parent>(root);
        w.despawn(root);
        assert!(w.drawn(child).is_none());
    }

    #[test]
    #[should_panic(expected = "a tick read or wrote presentation component")]
    fn a_tick_cannot_read_drawn_poses() {
        let mut w = World::new(60, 0);
        let e = w.spawn(Transform::default());
        w.begin_tick();
        let _ = w.drawn(e);
    }
}
