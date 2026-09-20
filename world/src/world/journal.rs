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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogCursor {
    replacement: u64,
    game: u64,
    session: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Logs {
    pub next: LogCursor,
    pub reset: bool,
    pub truncated: bool,
    pub entries: String,
}
impl Event {
    fn text(&self) -> &str {
        match &self.kind {
            EventKind::Message(s) | EventKind::Published(s) => s,
            EventKind::Structural(
                ChangeKind::Insert(s) | ChangeKind::Replace(s) | ChangeKind::Remove(s),
                _,
            ) => s,
            _ => "",
        }
    }
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
    pub fn logs(&self, mut cursor: LogCursor) -> Result<Logs, DataError> {
        let reset = cursor.replacement != self.replacement;
        if reset {
            cursor = LogCursor {
                replacement: self.replacement,
                ..LogCursor::default()
            };
        }
        let game = self.journal.borrow();
        let session = self.session_journal.borrow();
        let first_game = self.journal_next() - game.len() as u64;
        let first_session = self.session_next.get() - session.len() as u64;
        let truncated = cursor.game < first_game || cursor.session < first_session;
        if cursor.game > self.journal_next() || cursor.session > self.session_next.get() {
            return Err(DataError::new("future log cursor"));
        }
        cursor.game = cursor.game.max(first_game);
        cursor.session = cursor.session.max(first_session);
        let mut game = game.range((cursor.game - first_game) as usize..).peekable();
        let mut session = session
            .range((cursor.session - first_session) as usize..)
            .peekable();
        let mut w = crate::json::Encoder::default();
        let mut budget = crate::json::LIMIT - 2;
        w.begin_seq(0);
        for _ in 0..512 {
            let host = session
                .peek()
                .is_some_and(|(at, _)| game.peek().is_none_or(|g| *at <= g.index));
            let event = if host {
                session.peek().map(|(_, e)| e)
            } else {
                game.peek().copied()
            };
            let Some(event) = event else {
                break;
            };
            let Some(left) = budget.checked_sub(256 + 6 * event.text().len()) else {
                break;
            };
            budget = left;
            w.item();
            event.write(&mut w);
            if host {
                cursor.session = event.index + 1;
                session.next();
            } else {
                cursor.game = event.index + 1;
                game.next();
            }
        }
        w.end_seq();
        Ok(Logs {
            next: cursor,
            reset,
            truncated,
            entries: w.finish()?,
        })
    }
    /// Unsaved host telemetry, anchored before the next deterministic game event.
    pub fn session_log(&self, message: &str) -> Result<(), DataError> {
        if message.len() > 4096 {
            return Err(DataError::new("session log exceeds 4096 bytes"));
        }
        let index = self.session_next.get();
        self.session_next.set(
            index
                .checked_add(1)
                .ok_or_else(|| DataError::new("session cursor exhausted"))?,
        );
        let mut events = self.session_journal.borrow_mut();
        if events.len() == 4096 {
            events.pop_front();
        }
        events.push_back((
            self.journal_next(),
            Event {
                index,
                tick: self.tick(),
                kind: EventKind::Message(message.into()),
            },
        ));
        Ok(())
    }
    pub(crate) fn write_journal(&self, w: &mut dyn Writer) {
        w.item();
        self.journal_next().write(w);
        let events = self.journal.borrow();
        w.item();
        w.begin_seq(events.len());
        for e in events.iter() {
            if w.stopped() {
                break;
            }
            w.item();
            e.write(w);
        }
        w.end_seq();
    }
    pub(crate) fn read_journal(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.required_item("missing journal cursor")?;
        self.journal_next.get_mut().read(r)?;
        r.required_item("missing journal entries")?;
        let mut events: Vec<Event> = Vec::new();
        crate::data::limits::read_vec(r, &mut events, 4096)?;
        let first = self
            .journal_next()
            .checked_sub(events.len() as u64)
            .ok_or_else(|| DataError::new("journal cursor precedes entries"))?;
        let mut tick = 0;
        for (i, e) in events.iter().enumerate() {
            if e.index != first + i as u64
                || e.tick < tick
                || e.tick > self.tick()
                || e.text().len() > 4096
                || matches!(e.kind, EventKind::Structural(ChangeKind::Reset, _))
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
        next.session_next = self.session_next.clone();
        for (at, _) in next.session_journal.get_mut() {
            *at = next.journal_next.get();
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
    fn session_cursor_refusal_does_not_drop_the_oldest_record() {
        let mut w = World::new(60, 0);
        for _ in 0..4096 {
            w.session_log("kept").unwrap();
        }
        w.session_next.set(u64::MAX);
        for (i, (_, event)) in w.session_journal.get_mut().iter_mut().enumerate() {
            event.index = u64::MAX - 4096 + i as u64;
        }
        let before = w.logs(LogCursor::default()).unwrap();
        assert!(w.session_log("refused").is_err());
        assert_eq!(w.session_journal.borrow().len(), 4096);
        assert_eq!(w.logs(LogCursor::default()).unwrap(), before);
    }
    #[test]
    fn adopt_cursor_exhaustion_preserves_both_journals() {
        let mut w = World::new(60, 0);
        w.spawn(()).unwrap();
        w.session_log("retained").unwrap();
        w.change_next = u64::MAX;
        let before = w.changes.clone();
        let logs = w.logs(LogCursor::default()).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| w.adopt(World::new(60, 0))));
        assert_eq!(w.changes, before);
        assert_eq!(w.logs(LogCursor::default()).unwrap(), logs);
        assert!(result.unwrap().is_err());
    }
}
