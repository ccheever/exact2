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
        if kind != ChangeKind::Reset {
            self.event(EventKind::Structural(kind.clone(), entity));
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
        next.session_journal = std::mem::take(&mut self.session_journal);
        for e in next.session_journal.get_mut() {
            e.index = next.journal_next.get();
        }
        *self = next;
        Ok(())
    }
}
