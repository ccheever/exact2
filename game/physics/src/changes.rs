//! Derived page dirtiness shared by solver reflection and live geometry queries.
use crate::{Body, Collider};
use exact_game::{Component, Entity, Parent, Transform, World, WorldId, PAGE};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Default, PartialEq)]
struct Local {
    pose: Option<Transform>,
    parent: Option<Entity>,
}
impl Local {
    fn of(world: &World, e: Entity) -> Self {
        Self {
            pose: world.get::<Transform>(e).as_deref().copied(),
            parent: world.get::<Parent>(e).map(|p| p.0),
        }
    }
}
#[derive(Default)]
struct PageRows {
    rows: Vec<(Entity, Local)>,
    // Entity identities, including ancestors without a Transform. A write to a
    // body sharing their page is only a reason to compare the ancestor value.
    ancestors: BTreeSet<Entity>,
    parented: bool,
}

#[derive(Default)]
pub(crate) struct Changes {
    identity: Option<(WorldId, u64)>,
    membership: [u64; 2],
    pages: Vec<PageRows>,
    generations: Vec<[u64; 4]>,
    ancestors: BTreeMap<Entity, Local>,
    pub bodies: Vec<Entity>,
    pub statics: Vec<Entity>,
}

pub(crate) struct Changed {
    pub rows: Vec<Entity>,
    pub membership: bool,
    pub parented: bool,
}

fn generations<C: Component>(world: &World, column: usize, result: &mut Vec<[u64; 4]>) {
    for page in world.pages::<C>().iter() {
        let i = page.first as usize / PAGE;
        result.resize(result.len().max(i + 1), [0; 4]);
        result[i][column] = page.generation;
    }
}

impl Changes {
    pub fn refresh(&mut self, world: &World) -> Changed {
        let identity = (world.id(), world.presentation_generation());
        let membership = [world.membership::<Body>(), world.membership::<Collider>()];
        let reset = self.identity.as_ref() != Some(&identity) || self.membership != membership;
        if reset {
            self.pages.clear();
            self.ancestors.clear();
            self.bodies.clear();
            self.statics.clear();
            for (e, b) in world
                .query::<Option<&Body>>()
                .with_any::<Body, Collider>()
                .iter()
            {
                let i = e.index() as usize / PAGE;
                self.pages
                    .resize_with(self.pages.len().max(i + 1), PageRows::default);
                self.pages[i].rows.push((e, Local::default()));
                if b.is_some() {
                    self.bodies.push(e);
                } else {
                    self.statics.push(e);
                }
            }
        }
        let mut next = Vec::new();
        generations::<Body>(world, 0, &mut next);
        generations::<Collider>(world, 1, &mut next);
        generations::<Transform>(world, 2, &mut next);
        generations::<Parent>(world, 3, &mut next);
        let stamp = |v: &Vec<[u64; 4]>, i: usize| v.get(i).copied().unwrap_or_default();
        let ancestor_edits: BTreeSet<_> = self
            .ancestors
            .iter()
            .filter_map(|(&e, local)| {
                let page = e.index() as usize / PAGE;
                (stamp(&self.generations, page)[2..] != stamp(&next, page)[2..]
                    && *local != Local::of(world, e))
                .then_some(e)
            })
            .collect();
        let mut rows = Vec::new();
        for (i, page) in self.pages.iter_mut().enumerate() {
            if page.rows.is_empty() {
                continue;
            }
            let before = stamp(&self.generations, i);
            let after = stamp(&next, i);
            let ancestor_changed = page.ancestors.iter().any(|e| ancestor_edits.contains(e));
            if !reset && before[1..] == after[1..] && !ancestor_changed {
                continue;
            }
            for (e, local) in &mut page.rows {
                let now = Local::of(world, *e);
                if reset || ancestor_changed || before[1] != after[1] || *local != now {
                    rows.push(*e);
                }
                *local = now;
            }
            if !reset && before[3] == after[3] && !ancestor_changed {
                continue;
            }
            page.ancestors.clear();
            page.parented = false;
            for &(e, _) in &page.rows {
                let mut cursor = e;
                for _ in 0..world.len() {
                    let Some(parent) = world.get::<Parent>(cursor) else {
                        break;
                    };
                    page.parented = true;
                    page.ancestors.insert(parent.0);
                    if !world.contains(parent.0) {
                        break;
                    }
                    cursor = parent.0;
                    assert_ne!(cursor, e, "physics: transform cycle");
                }
            }
        }
        let ancestors: BTreeSet<_> = self
            .pages
            .iter()
            .flat_map(|p| p.ancestors.iter().copied())
            .collect();
        self.ancestors.retain(|e, _| ancestors.contains(e));
        for e in ancestors {
            if ancestor_edits.contains(&e) || !self.ancestors.contains_key(&e) {
                self.ancestors.insert(e, Local::of(world, e));
            }
        }
        // Dynamics still reflect each tick, including Rapier's kinematic setter.
        // Sorting the union preserves the old insertion/mutation order exactly.
        rows.extend_from_slice(&self.bodies);
        rows.sort_unstable();
        rows.dedup();
        self.identity = Some(identity);
        self.membership = membership;
        self.generations = next;
        Changed {
            rows,
            membership: reset,
            parented: self.pages.iter().any(|p| p.parented),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interleaved_bodies_do_not_dirty_static_rows_or_a_shared_page_root() {
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        let player = w.spawn((Transform::default(), Body::default()));
        let root = w.spawn(Transform::default());
        let mut bodies = vec![player];
        for _ in 0..PAGE * 3 {
            if w.len().is_multiple_of(PAGE) {
                bodies.push(w.spawn((Transform::default(), Body::default())));
            }
            w.spawn((Transform::default(), Collider::default(), Parent(root)));
        }
        let mut changes = Changes::default();
        changes.refresh(&w);
        for &b in &bodies {
            w.get_mut::<Transform>(b).unwrap().position.x += 1.;
        }
        assert_eq!(changes.refresh(&w).rows, bodies);
        w.get_mut::<Transform>(root).unwrap().position.x += 1.;
        assert_eq!(changes.refresh(&w).rows.len(), PAGE * 3 + bodies.len());
        let grand = w.spawn(Transform::at(5., 0., 0.));
        w.insert(root, Parent(grand));
        assert_eq!(changes.refresh(&w).rows.len(), PAGE * 3 + bodies.len());
        w.get_mut::<Transform>(grand).unwrap().position.y += 1.;
        assert_eq!(changes.refresh(&w).rows.len(), PAGE * 3 + bodies.len());
        w.despawn(grand);
        assert_eq!(changes.refresh(&w).rows.len(), PAGE * 3 + bodies.len());
    }

    #[test]
    fn unchanged_static_pages_are_not_visited_even_when_a_body_moves() {
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        for _ in 0..PAGE * 3 {
            w.spawn((Transform::default(), Collider::default()));
        }
        let b = w.spawn((Transform::default(), Body::default()));
        let mut changes = Changes::default();
        assert_eq!(changes.refresh(&w).rows.len(), PAGE * 3 + 1);
        w.get_mut::<Transform>(b).unwrap().position.x = 1.;
        assert_eq!(changes.refresh(&w).rows, [b]);
        assert_eq!(changes.refresh(&w).rows, [b]);
    }
}

#[cfg(test)]
mod churn_tests {
    use super::*;
    #[test]
    fn unrelated_recycling_does_not_reset_physics_membership() {
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        for _ in 0..2 * PAGE {
            w.spawn((Transform::default(), Collider::default()));
        }
        let mut changes = Changes::default();
        assert!(changes.refresh(&w).membership);
        for _ in 0..10 {
            let e = w.spawn(());
            let c = changes.refresh(&w);
            assert!(!c.membership && c.rows.is_empty());
            w.despawn(e);
            let c = changes.refresh(&w);
            assert!(!c.membership && c.rows.is_empty());
        }
    }
}
