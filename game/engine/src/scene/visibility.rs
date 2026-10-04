// @ref LLP 1046.008#a-effective-visibility — one current-row presentation predicate.
use crate::{Entity, Parent, Visible, World};

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
}
