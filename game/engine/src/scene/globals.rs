use super::{Parent, Transform};
use crate::{storage::Storage, Affine3A, Entity, World};

/// Repeated global-pose reads with component columns resolved once.
/// Values remain live: roots read their local pose and parented entities read
/// the last propagated hierarchy, exactly as [`World::global`].
pub struct Globals<'w> {
    world: &'w World,
    parents: Option<&'w Storage<Parent>>,
    transforms: Option<&'w Storage<Transform>>,
}

impl World {
    /// Share component lookup across many [`World::global`] reads. The reader
    /// borrows this world; it neither copies poses nor changes saved state.
    pub fn globals(&self) -> Globals<'_> {
        Globals {
            world: self,
            parents: self.storage::<Parent>(),
            transforms: self.storage::<Transform>(),
        }
    }
}

impl Globals<'_> {
    /// Read one global pose, retaining entity-generation and row-borrow checks.
    #[track_caller]
    pub fn get(&self, e: Entity) -> Option<Affine3A> {
        let at = std::panic::Location::caller();
        let w = self.world;
        if w.attachments.is_some() {
            return w.current_global(e);
        }
        if !w.contains(e) {
            return None;
        }
        let index = e.index() as usize;
        if self.parents.is_some_and(|parents| parents.has(index)) {
            w.hierarchy
                .nodes
                .get(index)
                .filter(|n| n.entity == e && n.done == w.hierarchy.stamp)
                .map(|n| n.global)
        } else {
            let transforms = self.transforms?;
            transforms
                .copied(index, &w.leases)
                .unwrap_or_else(|_| w.refuse(transforms.row_conflict(index, false, &w.leases, at)))
                .map(Transform::affine)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_match_global_through_edits_missing_poses_and_recycled_entities() {
        let mut w = World::new(60, 0);
        let root = w.spawn(Transform::at(1., 2., 3.));
        let child = w.spawn((Transform::at(4., 5., 6.), Parent(root)));
        let empty = w.spawn(());
        let check = |w: &World| {
            let hash = w.hash();
            let globals = w.globals();
            for entity in [root, child, empty] {
                assert_eq!(globals.get(entity), w.global(entity));
            }
            assert_eq!(w.hash(), hash);
        };
        check(&w);
        w.propagate();
        check(&w);
        {
            let globals = w.globals();
            w.get_mut::<Transform>(root).unwrap().position.x = 9.;
            assert_eq!(globals.get(root), w.global(root));
            assert_eq!(globals.get(child), w.global(child));
        }
        w.remove::<Parent>(child);
        check(&w);
        w.despawn(root);
        let replacement = w.spawn((Transform::at(8., 0., 0.), Parent(child)));
        assert_eq!(root.index(), replacement.index());
        w.propagate();
        check(&w);
        assert_eq!(w.globals().get(replacement), w.global(replacement));
    }

    #[test]
    fn root_reads_refuse_exclusive_row_and_query_leases() {
        let mut w = World::new(60, 0);
        let root = w.spawn(Transform::default());
        let globals = w.globals();
        let exclusive = w.get_mut::<Transform>(root).unwrap();
        let read = || globals.get(root);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(read)).is_err());
        drop(exclusive);
        let mut query = w.query::<&mut Transform>();
        let _ = query.iter().next();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(read)).is_err());
        drop(query);
        assert_eq!(globals.get(root), w.global(root));
    }
}
