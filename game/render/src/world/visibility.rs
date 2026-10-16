use exact_game::{Entity, Parent, Visible, World};

/// Scratch for one immutable feed's membership scan. Ancestors shared by many
/// draws are read once. Nothing survives into the next scan except capacity.
#[derive(Default)]
pub(super) struct Visibility {
    states: Vec<u8>,
    parents: Vec<Option<Entity>>,
    path: Vec<Entity>,
}

impl Visibility {
    pub fn begin(&mut self, w: &World) {
        self.states.fill(0);
        self.parents.fill(None);
        // Read each component column once, rather than looking it up for every
        // draw and ancestor. `get` still checks full entity liveness before
        // consulting these slot-indexed rows, including cached hidden parents.
        for (entity, visible) in w.query::<&Visible>().iter() {
            if !visible.0 {
                let index = entity.index() as usize;
                if self.states.len() <= index {
                    self.states.resize(index + 1, 0);
                }
                self.states[index] = 3;
            }
        }
        for (entity, parent) in w.query::<&Parent>().iter() {
            let index = entity.index() as usize;
            if self.parents.len() <= index {
                self.parents.resize(index + 1, None);
            }
            self.parents[index] = Some(parent.0);
        }
    }

    pub fn get(&mut self, w: &World, mut entity: Entity) -> bool {
        self.path.clear();
        let visible = loop {
            // A cached slot must never make a dangling old generation visible.
            if !w.contains(entity) {
                break false;
            }
            let index = entity.index() as usize;
            if self.states.len() <= index {
                self.states.resize(index + 1, 0);
            }
            match self.states[index] {
                2 => break true,
                1 | 3 => break false, // Cycle in this walk, or a hidden ancestor.
                _ => {}
            }
            self.states[index] = 1;
            self.path.push(entity);
            match self.parents.get(index).copied().flatten() {
                Some(parent) => entity = parent,
                None => break true,
            }
        };
        for entity in &self.path {
            self.states[entity.index() as usize] = if visible { 2 } else { 3 };
        }
        visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_game::Transform;

    #[test]
    fn changed_parent_subtrees_survive_retries_and_cycles() {
        struct Scene;
        impl exact_game::Game for Scene {
            type Args = ();
            const ID: &'static str = "parent-reset-cache";
            fn setup(w: &mut World, _: &()) {
                let a = w.spawn_named("a", Transform::default());
                let b = w.spawn_named("b", Transform::default());
                let middle = w.spawn_named("middle", (Transform::default(), Parent(a)));
                w.spawn_named("leaf", (Transform::default(), Parent(middle)));
                w.spawn_named("other", (Transform::default(), Parent(b)));
            }
            fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
        }
        let mut sim = exact_game::Sim::<Scene>::new(()).unwrap();
        sim.run(1000. / 60.);
        let middle = sim.world().named("middle").unwrap();
        let leaf = sim.world().named("leaf").unwrap();
        let other = sim.world().named("other").unwrap();
        let b = sim.world().named("b").unwrap();
        let mut cache = ParentChanges::default();
        cache.begin(sim.world());
        cache.commit();
        cache.begin(sim.world());
        assert!(!cache.get(sim.world(), leaf));
        sim.world_mut().insert(middle, Parent(b));
        for _ in 0..2 {
            cache.begin(sim.world());
            assert!(cache.get(sim.world(), leaf));
            assert!(!cache.get(sim.world(), other));
        }
        cache.commit();
        cache.begin(sim.world());
        assert!(!cache.get(sim.world(), leaf));
        sim.world_mut().insert(b, Parent(leaf));
        cache.begin(sim.world());
        assert!(cache.get(sim.world(), leaf));
        cache.commit();
        cache.begin(sim.world());
        assert!(!cache.get(sim.world(), leaf));
        sim.world_mut().remove::<Parent>(middle);
        cache.begin(sim.world());
        assert!(cache.get(sim.world(), leaf));
        cache.reset();
        cache.begin(sim.world());
        assert!(cache.get(sim.world(), leaf));
    }

    #[test]
    fn sparse_columns_clear_removed_rows_between_scans() {
        let mut w = World::new(60, 0);
        let leaf = w.spawn(());
        for _ in 0..(exact_game::PAGE * 3) {
            w.spawn(());
        }
        let hidden = w.spawn(Visible(false));
        let parent = w.spawn(Parent(hidden));
        w.insert(leaf, Parent(parent));
        let mut cache = Visibility::default();
        let check = |w: &World, cache: &mut Visibility| {
            cache.begin(w);
            // Querying a leaf first must include ancestors outside its page.
            for e in [leaf, parent, hidden] {
                assert_eq!(cache.get(w, e), w.is_visible(e));
            }
        };
        check(&w, &mut cache);
        w.remove::<Visible>(hidden);
        check(&w, &mut cache);
        w.insert(hidden, Parent(leaf));
        check(&w, &mut cache);
        w.remove::<Parent>(parent);
        check(&w, &mut cache);
        w.despawn(hidden);
        check(&w, &mut cache);
    }

    #[test]
    fn shared_visibility_matches_current_rows_after_edits_cycles_and_recycling() {
        let mut w = World::new(60, 0);
        let root = w.spawn(());
        let middle = w.spawn(Parent(root));
        let leaves: Vec<_> = (0..12)
            .map(|_| w.spawn((Transform::default(), Parent(middle))))
            .collect();
        let mut cache = Visibility::default();
        let check = |w: &World, cache: &mut Visibility| {
            cache.begin(w);
            for &entity in leaves.iter().chain([&middle, &root]).rev() {
                assert_eq!(cache.get(w, entity), w.is_visible(entity));
            }
            cache.begin(w);
            for &entity in leaves.iter().chain([&middle, &root]) {
                assert_eq!(cache.get(w, entity), w.is_visible(entity));
            }
        };
        check(&w, &mut cache);
        w.insert(root, Visible(false));
        w.insert(leaves[0], Visible(true));
        check(&w, &mut cache);
        w.remove::<Visible>(root);
        w.insert(root, Parent(leaves[0]));
        check(&w, &mut cache); // No propagation: the current rows form a cycle.
        w.remove::<Parent>(root);
        check(&w, &mut cache);
        w.despawn(root);
        let replacement = w.spawn(Visible(true));
        assert_eq!(replacement.index(), root.index());
        assert_ne!(replacement, root);
        cache.begin(&w);
        assert!(cache.get(&w, replacement));
        assert!(!cache.get(&w, middle)); // Old parent generation at a cached slot.
        check(&w, &mut cache);
        w.insert(middle, Parent(replacement));
        check(&w, &mut cache);
        let mut other = World::new(60, 0);
        let entity = other.spawn(Visible(false));
        cache.begin(&other);
        assert!(!cache.get(&other, entity)); // Scratch can serve another world.
    }
}

/// Parent edges at the last successful feed. A changed edge resets its subtree,
/// not every unrelated hierarchy in the scene.
#[derive(Default)]
pub(super) struct ParentChanges {
    previous: Vec<Option<(Entity, Entity)>>,
    current: Vec<Option<(Entity, Entity)>>,
    states: Vec<u8>,
    path: Vec<Entity>,
}
impl ParentChanges {
    pub fn reset(&mut self) {
        self.previous.clear();
        self.current.clear();
        self.states.clear();
    }
    pub fn begin(&mut self, w: &World) {
        self.current.fill(None);
        for (entity, parent) in w.query::<&Parent>().iter() {
            let index = entity.index() as usize;
            if self.current.len() <= index {
                self.current.resize(index + 1, None);
            }
            self.current[index] = Some((entity, parent.0));
        }
        self.states
            .resize(self.previous.len().max(self.current.len()), 0);
        for (index, state) in self.states.iter_mut().enumerate() {
            *state = if self.previous.get(index).copied().flatten()
                != self.current.get(index).copied().flatten()
            {
                2
            } else {
                0
            };
        }
        // A fresh ancestor also cuts interpolation; memoize that walk together
        // with parent changes instead of rereading every shared ancestor.
        for entity in w.fresh() {
            let index = entity.index() as usize;
            if self.states.len() <= index {
                self.states.resize(index + 1, 0);
            }
            self.states[index] = 2;
        }
    }
    pub fn get(&mut self, w: &World, mut entity: Entity) -> bool {
        self.path.clear();
        let changed = loop {
            if !w.contains(entity) {
                break false;
            }
            let index = entity.index() as usize;
            match self.states.get(index).copied().unwrap_or(3) {
                2 => break true,
                1 | 3 => break false,
                _ => {}
            }
            self.states[index] = 1;
            self.path.push(entity);
            match self.current.get(index).copied().flatten() {
                Some((owner, parent)) if owner == entity => entity = parent,
                _ => break false,
            }
        };
        for entity in &self.path {
            self.states[entity.index() as usize] = if changed { 2 } else { 3 };
        }
        changed
    }
    pub fn commit(&mut self) {
        std::mem::swap(&mut self.previous, &mut self.current);
    }
}
