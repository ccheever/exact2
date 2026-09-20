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
    fn ownership_status(&self) -> Result<Vec<u8>, DataError> {
        let count = self.state.slots.len();
        if count > crate::MAX_ENTITIES {
            return Err(DataError::new("ownership entity limit"));
        }
        let mut parents = vec![None; count];
        for (e, parent) in self.query::<&Parent>().iter() {
            parents[e.index() as usize] = Some(parent.entity());
        }
        let mut status = vec![0u8; count];
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
                match parents[at] {
                    None => break 2,
                    Some(parent) if !self.contains(parent) => break 3,
                    Some(parent) => at = parent.index() as usize,
                }
            };
            at = start;
            while status[at] == 1 {
                status[at] = result;
                match parents[at] {
                    Some(p) if self.contains(p) => at = p.index() as usize,
                    _ => break,
                }
            }
        }
        Ok(status)
    }
    pub(crate) fn validate_ownership(&self) -> Result<(), DataError> {
        if self.storage::<Parent>().is_none_or(|s| s.is_empty()) {
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
        if self.storage::<Parent>().is_none_or(|s| s.is_empty()) {
            return Ok(());
        }
        let status = self.ownership_status()?;
        self.change_room(status.iter().filter(|&&s| s == 3).count())?;
        for (index, state) in status.into_iter().enumerate() {
            if state == 3 {
                self.despawn(self.entity_at(index));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
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
