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
        self.change_room(2)?;
        match parent {
            Some(parent) => {
                self.register::<Parent>()?;
                self.insert(child, Parent(parent))?;
            }
            None => {
                self.remove::<Parent>(child);
            }
        }
        Ok(())
    }
    pub(super) fn check_parent(&self, child: Entity, mut parent: Entity) -> Result<(), DataError> {
        for _ in 0..=self.state.slots.len() {
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
        Err(DataError::new("ownership traversal exceeds entity bound"))
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
        if !self.reap_dirty || self.storage::<Parent>().is_none_or(|s| s.len() == 0) {
            return Ok(());
        }
        let mut status = self.ownership_status()?;
        if status
            .iter()
            .enumerate()
            .any(|(i, &s)| s == 3 && self.state.slots[i].generation == u32::MAX)
        {
            return Err(DataError::new("entity generation exhausted"));
        }
        self.change_room(status.iter().filter(|&&s| s == 3).count())?;
        let states = std::mem::take(&mut *status);
        drop(status);
        for (index, &state) in states.iter().enumerate() {
            if state == 3 {
                self.despawn(self.entity_at(index).unwrap());
            }
        }
        *self.ownership.get_mut() = states;
        self.reap_dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
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
        w.despawn(transient);
        w.reap_orphans().unwrap();
        w.ownership.get_mut().fill(42);
        for _ in 0..1000 {
            w.reap_orphans().unwrap();
        }
        assert_eq!(*w.ownership.borrow(), [42, 42, 42]);
        w.despawn(root);
        w.reap_orphans().unwrap();
        assert!(!w.contains(child));
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
        w.despawn(parent);
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
    fn ownership_scratch_does_not_consume_the_state_decode_allowance() {
        let mut w = World::new(60, 0);
        let root = w.spawn(()).unwrap();
        for _ in 0..100 {
            let e = w.spawn(()).unwrap();
            w.set_parent(e, Some(root)).unwrap();
        }
        let bytes = w.save().unwrap();
        let decodes = |budget| {
            let mut next = World::new(60, 0);
            next.register::<Parent>().unwrap();
            let mut r = bin::Decoder::with_budget(&bytes[8..], budget);
            next.read(&mut r).and_then(|()| r.finish()).is_ok()
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
