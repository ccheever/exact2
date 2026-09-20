use super::*;
type StaticText = std::borrow::Cow<'static, str>;
/// Retained structural suffix. Lagging consumers resynchronize; edits never wait.
pub const CHANGE_LIMIT: usize = 4096;
/// Independent acknowledgement; dropping the handle releases its retention claim.
pub struct ChangeConsumer(std::rc::Rc<std::cell::Cell<u64>>);
pub struct Changes<'a> {
    pub next: u64,
    pub resync: bool,
    pub events: std::collections::vec_deque::Iter<'a, Change>,
}
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
        self.healthy()?;
        if self.change_next.checked_add(count as u64).is_none()
            || self.journal_next.get().checked_add(count as u64).is_none()
        {
            return Err(DataError::new("journal cursor exhausted"));
        }
        Ok(())
    }
    pub(super) fn record_change(&mut self, entity: Entity, kind: ChangeKind) {
        self.prune_changes();
        let sequence = self.change_next;
        self.change_next = sequence
            .checked_add(1)
            .expect("structural sequence exhausted");
        if kind != ChangeKind::Reset {
            self.event(EventKind::Structural(kind.clone(), entity));
        }
        if self.consumers.is_empty() {
            return;
        }
        if self.changes.len() == CHANGE_LIMIT {
            self.changes.pop_front();
        }
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
    pub fn subscribe_changes(&mut self) -> Result<ChangeConsumer, DataError> {
        self.prune_changes();
        if self.consumers.len() == 64 {
            return Err(DataError::new("structural consumer limit (64)"));
        }
        let cursor = std::rc::Rc::new(std::cell::Cell::new(self.change_next));
        self.consumers.push(std::rc::Rc::downgrade(&cursor));
        Ok(ChangeConsumer(cursor))
    }
    pub fn changes(&self, consumer: &ChangeConsumer) -> Result<Changes<'_>, DataError> {
        if !self
            .consumers
            .iter()
            .any(|c| c.ptr_eq(&std::rc::Rc::downgrade(&consumer.0)))
        {
            return Err(DataError::new("consumer belongs to another world"));
        }
        let first = self.change_next - self.changes.len() as u64;
        let since = consumer.0.get();
        let resync = since < first;
        let offset = if resync {
            self.changes.len()
        } else {
            (since - first) as usize
        };
        Ok(Changes {
            next: self.change_next,
            resync,
            events: self.changes.range(offset..),
        })
    }
    pub fn acknowledge_changes(
        &mut self,
        consumer: &ChangeConsumer,
        through: u64,
    ) -> Result<(), DataError> {
        self.changes(consumer)?;
        if through < consumer.0.get() || through > self.change_next {
            return Err(DataError::new("invalid structural acknowledgement"));
        }
        consumer.0.set(through);
        self.prune_changes();
        Ok(())
    }
    fn prune_changes(&mut self) {
        self.consumers.retain(|c| c.strong_count() != 0);
        let min = self
            .consumers
            .iter()
            .filter_map(|c| c.upgrade())
            .map(|c| c.get())
            .min()
            .unwrap_or(self.change_next);
        let first = self.change_next.saturating_sub(self.changes.len() as u64);
        let count = min.saturating_sub(first).min(self.changes.len() as u64) as usize;
        self.changes.drain(..count);
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
        let game = self.journal.borrow();
        let session = self.session_journal.borrow();
        let mut game = game.iter().filter(|e| e.index >= since).peekable();
        let mut session = session.iter().filter(|e| e.index >= since).peekable();
        let events: Vec<_> = std::iter::from_fn(|| {
            if session
                .peek()
                .is_some_and(|s| game.peek().is_none_or(|g| s.index <= g.index))
            {
                session.next()
            } else {
                game.next()
            }
        })
        .take(512)
        .cloned()
        .collect();
        crate::json::to_string(&events)
    }
    /// Unsaved host telemetry, anchored before the next deterministic game event.
    pub fn session_log(&self, message: &str) -> Result<(), DataError> {
        if message.len() > 4096 {
            return Err(DataError::new("session log exceeds 4096 bytes"));
        }
        let mut events = self.session_journal.borrow_mut();
        if events.len() == 4096 {
            events.pop_front();
        }
        events.push_back(Event {
            index: self.journal_next(),
            tick: self.tick(),
            kind: EventKind::Message(message.into()),
        });
        Ok(())
    }
    pub(crate) fn write_journal(&self, w: &mut dyn Writer) {
        w.item();
        self.journal_next().write(w);
        let events = self.journal.borrow();
        w.item();
        w.begin_seq(events.len());
        for e in events.iter() {
            w.item();
            e.write(w);
        }
        w.end_seq();
    }
    pub(crate) fn read_journal(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        if !r.item()? {
            return Err(DataError::new("missing journal cursor"));
        }
        self.journal_next.get_mut().read(r)?;
        if !r.item()? {
            return Err(DataError::new("missing journal entries"));
        }
        let mut events: Vec<Event> = Vec::new();
        crate::data::limits::read_vec(r, &mut events, 4096)?;
        let first = self
            .journal_next()
            .checked_sub(events.len() as u64)
            .ok_or_else(|| DataError::new("journal cursor precedes entries"))?;
        let mut tick = 0;
        for (i, e) in events.iter().enumerate() {
            let text = match &e.kind {
                EventKind::Message(s) | EventKind::Published(s) => s.as_str(),
                EventKind::Structural(
                    ChangeKind::Insert(s) | ChangeKind::Replace(s) | ChangeKind::Remove(s),
                    _,
                ) => s.as_ref(),
                EventKind::Structural(ChangeKind::Reset, _) => {
                    return Err(DataError::new("reset is not a game event"))
                }
                _ => "",
            };
            if e.index != first + i as u64
                || e.tick < tick
                || e.tick > self.tick()
                || text.len() > 4096
            {
                return Err(DataError::new("invalid saved journal"));
            }
            tick = e.tick;
        }
        *self.journal.get_mut() = events.into();
        Ok(())
    }
    pub fn journal_next(&self) -> u64 {
        self.journal_next.get()
    }
    pub fn replacement(&self) -> u64 {
        self.replacement
    }
    pub(crate) fn adopt(&mut self, mut next: Self) -> Result<(), DataError> {
        self.change_room(1)?;
        next.replacement = self
            .replacement
            .checked_add(1)
            .ok_or_else(|| DataError::new("replacement exhausted"))?;
        next.changes = std::mem::take(&mut self.changes);
        next.consumers = std::mem::take(&mut self.consumers);
        next.change_next = self.change_next;
        next.record_change(Entity::default(), ChangeKind::Reset);
        next.session_journal = std::mem::take(&mut self.session_journal);
        for e in next.session_journal.get_mut() {
            e.index = next.journal_next.get();
        }
        std::mem::swap(self, &mut next);
        self.mutation(|_| drop(next));
        Ok(())
    }
}

#[cfg(test)]
mod atomic_tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    #[derive(Default, crate::Component)]
    struct C(u32);
    #[test]
    fn no_consumer_retains_nothing_and_full_history_never_refuses_spawn() {
        let mut w = World::new(60, 0);
        w.register::<C>().unwrap();
        w.spawn(C(7)).unwrap();
        assert_eq!(w.changes.capacity(), 0);
        let consumer = w.subscribe_changes().unwrap();
        for _ in 0..CHANGE_LIMIT {
            w.spawn(C(7)).unwrap();
        }
        assert!(w.changes(&consumer).unwrap().resync);
        assert_eq!(w.changes.len(), CHANGE_LIMIT);
        drop(consumer);
        w.spawn(()).unwrap();
        assert!(w.changes.is_empty());
    }
    #[test]
    fn adopt_cursor_exhaustion_preserves_both_journals() {
        let mut w = World::new(60, 0);
        w.spawn(()).unwrap();
        w.session_log("retained").unwrap();
        w.change_next = u64::MAX;
        let before = w.changes.clone();
        let logs = w.logs(0).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| w.adopt(World::new(60, 0))));
        assert_eq!(w.changes, before);
        assert_eq!(w.logs(0).unwrap(), logs);
        assert!(result.unwrap().is_err());
    }
}
