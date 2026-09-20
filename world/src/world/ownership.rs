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
                self.check_parent(child, parent)?;
                self.insert(child, Parent(parent));
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
    fn ownership_status(&self, r: &mut dyn Reader) -> Result<Vec<u8>, DataError> {
        let count = self.state.slots.len();
        r.claim(count * (std::mem::size_of::<Option<Entity>>() + 1))?;
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
    pub(crate) fn validate_ownership(&self, r: &mut dyn Reader) -> Result<(), DataError> {
        if self.ownership_status(r)?.contains(&3) {
            return Err(DataError::new("ownership has a dead parent"));
        }
        Ok(())
    }
    /// Validate without transforms. No mutation on refusal.
    pub fn validate(&self) -> Result<(), DataError> {
        self.validate_state()?;
        self.validate_ownership(&mut bin::Decoder::new(&[]))
    }
    /// Despawn leaves descendants until this boundary. Reap in ascending slot order.
    pub fn reap_orphans(&mut self) -> Result<(), DataError> {
        if self.storage::<Parent>().is_none_or(|s| s.is_empty()) {
            return Ok(());
        }
        let status = self.ownership_status(&mut bin::Decoder::new(&[]))?;
        self.change_room(status.iter().filter(|&&s| s == 3).count())?;
        for (index, state) in status.into_iter().enumerate() {
            if state == 3 {
                self.despawn(self.entity_at(index));
            }
        }
        Ok(())
    }
}
