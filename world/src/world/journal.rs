use super::*;
type StaticText = std::borrow::Cow<'static, str>;
/// Retained module journal capacity. Exhaustion refuses before the next edit.
pub const CHANGE_LIMIT: usize = 1_000_000;
#[derive(Clone, Debug, Default, Data, PartialEq, Eq)]
pub enum ChangeKind {
    #[default]
    Spawn,
    Despawn,
    Insert(StaticText),
    Replace(StaticText),
    Remove(StaticText),
    Reparent(Option<Entity>),
    /// All derived state must be reconstructed after this event.
    Reset,
}
#[derive(Clone, Debug, Default, Data, PartialEq, Eq)]
pub struct Change {
    pub sequence: u64,
    pub tick: u64,
    pub entity: Entity,
    pub kind: ChangeKind,
}
#[derive(Clone, Debug, Default, Data)]
pub enum EventKind {
    #[default]
    Started,
    Structural(ChangeKind, Entity),
    Published(String),
    Message(String),
}
#[derive(Clone, Debug, Default, Data)]
pub struct Event {
    pub index: u64,
    pub tick: u64,
    pub kind: EventKind,
}
impl World {
    pub(super) fn change_room(&self, count: usize) -> Result<(), DataError> {
        if self.changes.len().saturating_add(count) > CHANGE_LIMIT {
            Err(DataError::new("structural journal full; consume changes"))
        } else {
            Ok(())
        }
    }
    pub(super) fn record_change(&mut self, entity: Entity, kind: ChangeKind) {
        let sequence = self.change_next;
        self.change_next = sequence
            .checked_add(1)
            .expect("structural sequence exhausted");
        self.event(EventKind::Structural(kind.clone(), entity));
        self.changes.push_back(Change {
            sequence,
            tick: self.tick(),
            entity,
            kind,
        });
    }
    /// Next structural cursor. Reading never acknowledges events.
    pub fn change_cursor(&self) -> u64 {
        self.change_next
    }
    pub fn changes(&self, since: u64) -> Result<impl Iterator<Item = &Change>, DataError> {
        if since > self.change_next
            || self
                .changes
                .front()
                .map_or(self.change_next, |c| c.sequence)
                > since
        {
            return Err(DataError::new("structural cursor no longer retained"));
        }
        Ok(self.changes.iter().filter(move |c| c.sequence >= since))
    }
    /// Acknowledge every event before through. Multiple consumers acknowledge their minimum cursor.
    pub fn consume_changes(&mut self, through: u64) -> Result<(), DataError> {
        if through > self.change_next {
            return Err(DataError::new("future structural cursor"));
        }
        while self.changes.front().is_some_and(|c| c.sequence < through) {
            self.changes.pop_front();
        }
        Ok(())
    }
    pub(super) fn event(&self, kind: EventKind) {
        let mut journal = self.journal.borrow_mut();
        if journal.len() == 4096 {
            journal.pop_front();
        }
        let index = self.journal_next.get();
        self.journal_next
            .set(index.checked_add(1).expect("journal cursor exhausted"));
        journal.push_back(Event {
            index,
            tick: self.tick(),
            kind,
        });
    }
    pub fn log(&self, message: &str) -> Result<(), DataError> {
        if message.len() > 4096 {
            return Err(DataError::new("log exceeds 4096 bytes"));
        }
        self.event(EventKind::Message(message.into()));
        Ok(())
    }
    /// At most 512 structured records; formatting happens in the caller or logs().
    pub fn journal(&self, since: u64) -> Vec<Event> {
        self.journal
            .borrow()
            .iter()
            .filter(|e| e.index >= since)
            .take(512)
            .cloned()
            .collect()
    }
    pub fn logs(&self, since: u64) -> Result<String, DataError> {
        crate::json::to_string(&self.journal(since))
    }
    pub fn journal_next(&self) -> u64 {
        self.journal_next.get()
    }
    pub fn replacement(&self) -> u64 {
        self.replacement
    }
    pub(crate) fn inherit_registry(&mut self, live: &Self) {
        self.registry = live.registry.clone();
    }
    pub(crate) fn adopt(&mut self, mut next: Self) -> Result<(), DataError> {
        self.change_room(1)?;
        next.replacement = self
            .replacement
            .checked_add(1)
            .ok_or_else(|| DataError::new("replacement exhausted"))?;
        next.changes = std::mem::take(&mut self.changes);
        next.change_next = self.change_next;
        next.record_change(Entity::default(), ChangeKind::Reset);
        *self = next;
        Ok(())
    }
}
