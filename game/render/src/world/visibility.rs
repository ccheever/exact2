use exact_game::{Entity, Parent, Visible, World};

/// Scratch for one immutable feed's membership scan. Ancestors shared by many
/// draws are read once. Nothing survives into the next scan except capacity.
#[derive(Default)]
pub(super) struct Visibility {
    states: Vec<u8>,
    path: Vec<Entity>,
}

impl Visibility {
    pub fn begin(&mut self) {
        self.states.fill(0);
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
            if w.get::<Visible>(entity).is_some_and(|v| !v.0) {
                break false;
            }
            match w.get::<Parent>(entity) {
                Some(parent) => entity = parent.0,
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
    fn shared_visibility_matches_current_rows_after_edits_cycles_and_recycling() {
        let mut w = World::new(60, 0);
        let root = w.spawn(());
        let middle = w.spawn(Parent(root));
        let leaves: Vec<_> = (0..12)
            .map(|_| w.spawn((Transform::default(), Parent(middle))))
            .collect();
        let mut cache = Visibility::default();
        let check = |w: &World, cache: &mut Visibility| {
            cache.begin();
            for &entity in leaves.iter().chain([&middle, &root]).rev() {
                assert_eq!(cache.get(w, entity), w.is_visible(entity));
            }
            cache.begin();
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
        cache.begin();
        assert!(cache.get(&w, replacement));
        assert!(!cache.get(&w, middle)); // Old parent generation at a cached slot.
        check(&w, &mut cache);
        w.insert(middle, Parent(replacement));
        check(&w, &mut cache);
        let mut other = World::new(60, 0);
        let entity = other.spawn(Visible(false));
        cache.begin();
        assert!(!cache.get(&other, entity)); // Scratch can serve another world.
    }
}
