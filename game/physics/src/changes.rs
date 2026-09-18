//! Derived page dirtiness shared by solver reflection and live geometry queries.
use crate::{Body, Collider};
use exact_game::{Component, Entity, Parent, Transform, World, WorldId, PAGE};
use std::collections::BTreeSet;

#[derive(Default)]
struct PageRows {
    rows: Vec<Entity>,
    // Ancestor pages, including ancestors without a Transform. Adding a pose or
    // changing a grandparent edge must invalidate the descendant page as well.
    ancestors: BTreeSet<usize>,
    parented: bool,
}

#[derive(Default)]
pub(crate) struct Changes {
    identity: Option<(WorldId, u64)>,
    membership: [u64; 2],
    pages: Vec<PageRows>,
    generations: Vec<[u64; 4]>,
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
                self.pages[i].rows.push(e);
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
        let mut rows = Vec::new();
        for (i, page) in self.pages.iter_mut().enumerate() {
            if page.rows.is_empty() {
                continue;
            }
            if !reset
                && stamp(&self.generations, i) == stamp(&next, i)
                && page
                    .ancestors
                    .iter()
                    .all(|&a| stamp(&self.generations, a)[2..] == stamp(&next, a)[2..])
            {
                continue;
            }
            rows.extend_from_slice(&page.rows);
            if !reset
                && stamp(&self.generations, i)[3] == stamp(&next, i)[3]
                && page
                    .ancestors
                    .iter()
                    .all(|&a| stamp(&self.generations, a)[3] == stamp(&next, a)[3])
            {
                continue;
            }
            page.ancestors.clear();
            page.parented = false;
            for &e in &page.rows {
                let mut cursor = e;
                for _ in 0..world.len() {
                    let Some(parent) = world.get::<Parent>(cursor) else {
                        break;
                    };
                    page.parented = true;
                    page.ancestors.insert(parent.0.index() as usize / PAGE);
                    if !world.contains(parent.0) {
                        break;
                    }
                    cursor = parent.0;
                    assert_ne!(cursor, e, "physics: transform cycle");
                }
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
