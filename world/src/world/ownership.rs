use super::*;
/// Ownership edge. Mutate only through World::set_parent, preserving cycle and journal rules.
#[derive(Clone, Copy, Default, crate::Component)]
pub struct Parent(Entity);
impl Parent {
    pub fn entity(self) -> Entity {
        self.0
    }
}
impl World {
    pub fn set_parent(&mut self, child: Entity, parent: Option<Entity>) -> Result<(), DataError> {
        if !self.contains(child) {
            return Err(DataError::new("stale child"));
        }
        match parent {
            Some(parent) => {
                self.check_parent(child, parent)?;
                self.change_room(2)?;
                self.register::<Parent>()?;
                self.insert(child, Parent(parent))?;
            }
            None => {
                self.remove::<Parent>(child)?;
            }
        }
        Ok(())
    }
    pub(super) fn check_parent(&self, child: Entity, mut parent: Entity) -> Result<(), DataError> {
        for _ in 0..256 {
            if child == parent {
                return Err(DataError::new("ownership cycle"));
            }
            if !self.contains(parent) {
                return Err(DataError::new("stale parent"));
            }
            match self.get::<Parent>(parent) {
                Some(next) => parent = next.entity(),
                None => return Ok(()),
            }
        }
        Err(DataError::new("ownership ancestry work exceeds 256 edges"))
    }
    pub(super) fn change_owner(&mut self, child: Entity, old: Option<Entity>, new: Option<Entity>) {
        if old == new {
            return;
        }
        if let Some(old) = old {
            if let Some(children) = self.owners.get_mut(&old) {
                children.remove(&child);
                if children.is_empty() {
                    self.owners.remove(&old);
                }
            }
        }
        if let Some(new) = new {
            self.owners.entry(new).or_default().insert(child);
        }
    }
    pub(super) fn rebuild_owners(&mut self) {
        let owners = self.query::<&Parent>().iter().fold(
            BTreeMap::<Entity, BTreeSet<Entity>>::new(),
            |mut owners, (child, p)| {
                owners.entry(p.entity()).or_default().insert(child);
                owners
            },
        );
        self.owners = owners;
    }
    // A three-colour walk visits each edge at most twice, including reverse chains.
    fn ownership_status(&self) -> Result<std::cell::RefMut<'_, Vec<u8>>, DataError> {
        let count = self.state.slots.len();
        if count > crate::MAX_ENTITIES {
            return Err(DataError::new("ownership entity limit"));
        }
        let mut parents = self.query::<&Parent>();
        let mut parent = |at| parents.get(self.entity_at(at).unwrap()).map(|p| p.entity());
        let mut status = self.ownership.borrow_mut();
        status.resize(count, 0);
        status.fill(0);
        for start in 0..count {
            if status[start] != 0 || !self.state.slots[start].alive {
                continue;
            }
            let mut at = start;
            let result = loop {
                if status[at] == 1 {
                    return Err(DataError::new("ownership cycle").at(at));
                }
                if status[at] > 1 {
                    break status[at];
                }
                status[at] = 1;
                match parent(at) {
                    None => break 2,
                    Some(parent) if !self.contains(parent) => break 3,
                    Some(parent) => at = parent.index() as usize,
                }
            };
            at = start;
            while status[at] == 1 {
                status[at] = result;
                match parent(at) {
                    Some(p) if self.contains(p) => at = p.index() as usize,
                    _ => break,
                }
            }
        }
        Ok(status)
    }
    pub(crate) fn validate_ownership(&self) -> Result<(), DataError> {
        if self.storage::<Parent>().is_none_or(|s| s.len() == 0) {
            return Ok(());
        }
        if self.ownership_status()?.contains(&3) {
            return Err(DataError::new("ownership has a dead parent"));
        }
        Ok(())
    }
    /// Validate without transforms. No mutation on refusal.
    pub fn validate(&self) -> Result<(), DataError> {
        self.healthy()?;
        self.validate_state()?;
        self.validate_ownership()
    }
    /// Despawn leaves descendants until this boundary. Reap in ascending slot order.
    pub fn reap_orphans(&mut self) -> Result<(), DataError> {
        if self.orphans.is_empty() {
            return Ok(());
        }
        let mut todo: Vec<_> = self.orphans.iter().copied().collect();
        let mut remove = BTreeSet::new();
        let mut at = 0;
        while let Some(&e) = todo.get(at) {
            at += 1;
            if !self.contains(e) || remove.contains(&e) {
                continue;
            }
            let orphan = self
                .get::<Parent>(e)
                .is_some_and(|p| !self.contains(p.entity()) || remove.contains(&p.entity()));
            if !orphan {
                continue;
            }
            if self.state.slots[e.index as usize].generation == u32::MAX {
                return Err(DataError::new("entity generation exhausted"));
            }
            remove.insert(e);
            if let Some(children) = self.owners.get(&e) {
                todo.extend(children);
            }
            if todo.len() > crate::MAX_ENTITIES {
                return Err(DataError::new("orphan work limit"));
            }
        }
        self.change_room(remove.len())?;
        for e in remove {
            self.despawn(e)?;
        }
        self.orphans.clear();
        Ok(())
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn refused_parent_edge_does_not_declare_parent() {
        for stale in [false, true] {
            let mut w = World::new(60, 0);
            let child = w.spawn(()).unwrap();
            let parent = if stale { Entity::default() } else { child };
            let before = w.save().unwrap();
            let epoch = w.mutation_epoch();
            let cursor = w.journal_next();
            assert!(w.set_parent(child, Some(parent)).is_err());
            assert!(w.registry.is_empty(), "refused edge declared Parent");
            assert_eq!(w.save().unwrap(), before);
            assert_eq!(w.mutation_epoch(), epoch);
            assert_eq!(w.journal_next(), cursor);
            let parent = w.spawn(()).unwrap();
            w.set_parent(child, Some(parent)).unwrap();
            assert!(w.registry.contains_key("Parent"));
            assert_eq!(w.get::<Parent>(child).unwrap().entity(), parent);
        }
    }
    #[test]
    fn unchanged_boundary_does_not_visit_ownership_slots() {
        let mut w = World::new(60, 0);
        let root = w.spawn(()).unwrap();
        let child = w.spawn(()).unwrap();
        w.set_parent(child, Some(root)).unwrap();
        w.reap_orphans().unwrap();
        assert!(
            w.ownership.borrow().is_empty(),
            "valid Parent insertion needs no reap"
        );
        let transient = w.spawn(()).unwrap();
        w.reap_orphans().unwrap();
        assert!(
            w.ownership.borrow().is_empty(),
            "spawning cannot orphan an owner"
        );
        w.despawn(transient).unwrap();
        w.reap_orphans().unwrap();
        w.ownership.get_mut().fill(42);
        for _ in 0..1000 {
            w.reap_orphans().unwrap();
        }
        assert!(w.ownership.borrow().is_empty());
        w.despawn(root).unwrap();
        w.reap_orphans().unwrap();
        assert!(!w.contains(child));
    }
    #[test]
    fn unrelated_despawns_in_a_200k_world_never_scan_ownership() {
        let mut w = World::new(60, 0);
        for _ in 0..crate::MAX_ENTITIES {
            w.spawn(()).unwrap();
        }
        let root = w.entity_at(0).unwrap();
        let child = w.entity_at(199_999).unwrap();
        w.set_parent(child, Some(root)).unwrap();
        for _ in 0..1000 {
            let unrelated = w.entity_at(100_000).unwrap();
            w.despawn(unrelated).unwrap();
            w.reap_orphans().unwrap();
            w.spawn(()).unwrap();
        }
        assert!(w.ownership.borrow().is_empty());
        w.despawn(root).unwrap();
        w.reap_orphans().unwrap();
        assert!(!w.contains(child));
    }
    #[test]
    fn destroying_one_owner_in_200k_slots_reaps_only_its_descendants() {
        let mut w = World::new(60, 0);
        for _ in 0..crate::MAX_ENTITIES {
            w.spawn(()).unwrap();
        }
        let root = w.entity_at(100_000).unwrap();
        let child = w.entity_at(199_999).unwrap();
        let grandchild = w.entity_at(1).unwrap();
        w.set_parent(child, Some(root)).unwrap();
        w.set_parent(grandchild, Some(child)).unwrap();
        w.despawn(root).unwrap();
        w.spawn(()).unwrap(); // recycled root must not rescue the old subtree
        w.reap_orphans().unwrap();
        assert!(w.ownership.borrow().is_empty(), "reap scanned all slots");
        assert!(!w.contains(child) && !w.contains(grandchild));
        assert_eq!(w.len(), crate::MAX_ENTITIES - 2);
    }
    #[test]
    fn orphan_generation_exhaustion_refuses_before_removing_any_child() {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let mut w = World::new(60, 0);
        let parent = w.spawn(()).unwrap();
        let first = w.spawn(()).unwrap();
        let last = w.spawn(()).unwrap();
        w.set_parent(first, Some(parent)).unwrap();
        w.set_parent(last, Some(parent)).unwrap();
        // A valid loaded generation can reach this boundary; no billions of
        // churn operations are necessary to exercise the unfavourable suffix.
        w.state.slots[last.index() as usize].generation = u32::MAX;
        let last = w.entity_at(last.index() as usize).unwrap();
        w.despawn(parent).unwrap();
        w.orphans.insert(last);
        let cursor = w.journal_next();
        let result = catch_unwind(AssertUnwindSafe(|| w.reap_orphans()));
        assert!(
            w.contains(first),
            "earlier orphan was removed before refusal"
        );
        assert!(w.contains(last));
        assert_eq!(w.journal_next(), cursor);
        assert!(result.unwrap().unwrap_err().message.contains("generation"));
    }
    #[test]
    fn queued_retired_orphans_are_ignored_before_and_after_exact_restore() {
        let mut fixture = World::new(60, 0);
        fixture.spawn(()).unwrap();
        fixture.spawn(()).unwrap();
        fixture.state.slots[1].generation = u32::MAX - 1;
        let initial = fixture.save().unwrap();
        let mut w = World::new(60, 0);
        w.load(&initial).unwrap(); // Start from a canonical, admitted boundary.
        let parent = w.entity_at(0).unwrap();
        let child = w.entity_at(1).unwrap();
        w.set_parent(child, Some(parent)).unwrap();
        assert!(w.despawn(parent).unwrap());
        assert!(w.despawn(child).unwrap());
        w.validate().unwrap();
        assert!(w.is_empty());
        let retired = w.save().unwrap();
        let mut restored = World::new(60, 0);
        restored.register::<Parent>().unwrap();
        restored.load(&retired).unwrap();
        restored.reap_orphans().unwrap();
        w.reap_orphans().unwrap();
        assert_eq!(w.save().unwrap(), restored.save().unwrap());
        // A stale retired entry must not prevent the remaining live subtree's removal.
        let owner = w.spawn(()).unwrap();
        let live = w.spawn(()).unwrap();
        w.set_parent(live, Some(owner)).unwrap();
        w.despawn(owner).unwrap();
        w.orphans.insert(child);
        w.reap_orphans().unwrap();
        assert!(!w.contains(live));
    }
    #[test]
    fn ownership_scratch_and_validation_share_the_resident_budget() {
        let mut w = World::new(60, 0);
        let root = w.spawn(()).unwrap();
        for _ in 0..100 {
            let e = w.spawn(()).unwrap();
            w.set_parent(e, Some(root)).unwrap();
        }
        let bytes = w.save().unwrap();
        let decodes = |bytes_budget| {
            let mut next = World::new(60, 0);
            next.register::<Parent>().unwrap();
            let budget = crate::data::LoadBudget::new(bytes_budget);
            next.load_in(&bytes, Some(&budget), false).is_ok()
        };
        let (mut low, mut high) = (0, 1_000_000);
        while low < high {
            let mid = (low + high) / 2;
            if decodes(mid) {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        assert!(!decodes(low - 1));
        let budget = crate::data::LoadBudget::new(low);
        w.load_in(&bytes, Some(&budget), false).unwrap();
    }
}
