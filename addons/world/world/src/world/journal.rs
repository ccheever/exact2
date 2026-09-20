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
    Published(std::rc::Rc<str>),
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
            EventKind::Message(s) => s,
            EventKind::Published(s) => s,
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
            || self
                .journal_next
                .get()
                .checked_add(count as u64)
                .is_none_or(|n| n > CURSOR_LIMIT)
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
            self.event(EventKind::Structural(kind.clone(), entity))
                .expect("preflighted journal cursor");
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
    pub(super) fn event(&self, kind: EventKind) -> Result<(), DataError> {
        self.healthy()?;
        let index = self.journal_next.get();
        let next = index
            .checked_add(1)
            .filter(|n| *n <= CURSOR_LIMIT)
            .ok_or_else(|| DataError::new("journal cursor exhausted"))?;
        let mut journal = self.journal.borrow_mut();
        if journal.len() == 4096 {
            journal.pop_front();
        }
        self.journal_next.set(next);
        journal.push_back(Event {
            index,
            tick: self.tick(),
            kind,
        });
        Ok(())
    }
    pub fn log(&self, message: &str) -> Result<(), DataError> {
        if message.len() > 4096 {
            return Err(DataError::new("log exceeds 4096 bytes"));
        }
        self.event(EventKind::Message(message.into()))
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
        if self.journal_next() > CURSOR_LIMIT {
            return Err(DataError::new("cursor beyond supported range"));
        }
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
    pub(crate) fn adopt(&mut self, next: Self) -> Result<(), DataError> {
        drop(self.exchange(next)?);
        Ok(())
    }
    pub(crate) fn exchange(&mut self, mut next: Self) -> Result<Self, DataError> {
        // Reset consumes only the unsaved structural cursor. A healthy candidate
        // may replace a poisoned destination; all refusal precedes moving journals.
        self.change_next
            .checked_add(1)
            .ok_or_else(|| DataError::new("structural cursor exhausted"))?;
        next.replacement = self
            .replacement
            .checked_add(1)
            .ok_or_else(|| DataError::new("replacement exhausted"))?;
        next.driver_owned = self.driver_owned;
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
        Ok(next)
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

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn conflicting_parent_lease_during_remove_poisons_recorded_partial_mutation() {
        let mut w = World::new(60, 0);
        let parent = w.spawn(()).unwrap();
        let child = w.spawn(()).unwrap();
        w.set_parent(child, Some(parent)).unwrap();
        let cursor = w.journal_next();
        // Inject a conflicting internal lease: public APIs prohibit mutable Parent access.
        std::mem::forget(
            w.storage::<Parent>()
                .unwrap()
                .get_mut(child.index() as usize)
                .unwrap(),
        );
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || w.remove::<Parent>(child)
        ))
        .is_err());
        assert!(w.has::<Parent>(child));
        assert_eq!(w.journal_next(), cursor + 1);
        assert!(
            w.healthy().is_err(),
            "recorded Remove with Parent still present must poison"
        );
        assert!(w.save().unwrap_err().message.contains("poisoned"));
    }
    #[test]
    fn adoption_preflight_keeps_delivery_and_subscriptions_on_replacement_exhaustion() {
        let mut w = World::new(60, 0);
        let consumer = w.subscribe_changes().unwrap();
        w.spawn(()).unwrap();
        w.publish("kept", 3u32).unwrap();
        w.emit("queued").unwrap();
        w.session_log("session").unwrap();
        w.replacement = u64::MAX;
        let bytes = w.save().unwrap();
        let logs = w.logs(crate::LogCursor::default()).unwrap();
        let events: Vec<_> = w.changes(&consumer).unwrap().events.cloned().collect();
        assert!(w.adopt(World::new(60, 0)).is_err());
        assert_eq!(w.save().unwrap(), bytes);
        assert_eq!(w.logs(crate::LogCursor::default()).unwrap(), logs);
        assert_eq!(
            w.changes(&consumer)
                .unwrap()
                .events
                .cloned()
                .collect::<Vec<_>>(),
            events
        );
        assert_eq!(
            w.publications().get("kept"),
            Some(&crate::Published::Number(3.))
        );
        assert_eq!(w.take_messages(), ["queued"]);
    }
    #[test]
    fn exhausted_game_cursor_refuses_without_losing_history() {
        let mut w = World::new(60, 0);
        for _ in 0..4096 {
            w.log("kept").unwrap();
        }
        w.journal_next.set(u64::MAX);
        for (i, e) in w.journal.get_mut().iter_mut().enumerate() {
            e.index = u64::MAX - 4096 + i as u64;
        }
        let before = w.logs(LogCursor::default()).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.log("refused")));
        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
        assert_eq!(w.logs(LogCursor::default()).unwrap(), before);
    }
    #[test]
    fn exhausted_publication_cursor_refuses_without_mutation() {
        let w = World::new(60, 0);
        w.publish("kept", 1u32).unwrap();
        w.journal_next.set(u64::MAX - 1);
        let before = w.publications().clone();
        let cursor = w.journal_next();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.publish("new", 2u32)));
        assert!(result.is_ok(), "publish must return an error");
        assert!(result.unwrap().is_err());
        assert_eq!(*w.publications(), before);
        assert_eq!(w.journal_next(), cursor);
    }
    #[test]
    fn removal_cursor_exhaustion_returns_errors_without_changing_state() {
        #[derive(Default, crate::Component)]
        struct Kept(u32);
        for structural in [false, true] {
            let mut w = World::new(60, 0);
            w.register::<Kept>().unwrap();
            let e = w.spawn(Kept(7)).unwrap();
            if structural {
                w.change_next = u64::MAX;
            } else {
                w.journal_next.set(u64::MAX - 1);
            }
            let before = w.save().unwrap();
            let despawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.despawn(e)));
            assert!(despawn.is_ok(), "despawn panicked on cursor exhaustion");
            assert!(despawn.unwrap().is_err());
            let remove =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.remove::<Kept>(e)));
            assert!(remove.is_ok(), "remove panicked on cursor exhaustion");
            assert!(remove.unwrap().is_err());
            assert!(w.contains(e));
            assert_eq!(w.get::<Kept>(e).unwrap().0, 7);
            assert_eq!(w.save().unwrap(), before);
        }
    }
    #[test]
    fn live_exhausted_generation_refuses_decode_but_retired_slot_does_not_block_spawn() {
        let mut w = World::new(60, 0);
        let e = w.spawn(()).unwrap();
        w.state.slots[0].generation = u32::MAX;
        let mut out = bin::Encoder::prefixed(super::super::MAGIC);
        w.write(&mut out, true);
        let bytes = out.finish().unwrap();
        assert!(World::new(60, 0).load(&bytes).is_err());
        w.state.slots[0].generation = u32::MAX - 1;
        assert!(w
            .despawn(Entity {
                generation: u32::MAX - 1,
                ..e
            })
            .unwrap());
        let saved = w.save().unwrap();
        w.load(&saved).unwrap();
        let next = w.spawn(()).unwrap();
        assert_eq!(next.index(), 1);
        assert!(!w.contains(e));
        w.despawn(next).unwrap();
        assert_eq!(w.spawn(()).unwrap().index(), 1);
        assert_eq!(w.len(), 1);
    }
}

#[cfg(test)]
mod supported_cursor_tests {
    use super::*;
    use crate::{Game, Input, Sim};
    struct G;
    impl Game for G {
        const ID: &'static str = "cursor-admission";
        type Args = ();
        fn setup(w: &mut World, _: &()) -> Result<(), DataError> {
            let parent = w.spawn_named("parent", ())?;
            let child = w.spawn(())?;
            w.set_parent(child, Some(parent))
        }
        fn tick(w: &mut World, _: &Input, _: &()) -> Result<(), DataError> {
            if let Some(parent) = w.named("parent") {
                w.despawn(parent)?;
            }
            Ok(())
        }
    }
    #[test]
    fn saved_cursor_limit_refuses_atomically_on_every_open_path_and_boundary_continues() {
        const LIMIT: u64 = 1 << 62;
        for cursor in [LIMIT + 1, LIMIT - 2] {
            let mut source = Sim::<G>::new(()).unwrap();
            let w = source.world_mut();
            w.journal_next.set(cursor);
            let len = w.journal.borrow().len() as u64;
            for (i, event) in w.journal.get_mut().iter_mut().enumerate() {
                event.index = cursor - len + i as u64;
            }
            let bytes = source.save().unwrap();
            let mut dest = Sim::<G>::new(()).unwrap();
            dest.world().session_log("keep").unwrap();
            let consumer = dest.world_mut().subscribe_changes().unwrap();
            let before = dest.save().unwrap();
            let logs = dest.world().logs(LogCursor::default()).unwrap();
            for carry in [false, true] {
                let result = if carry {
                    dest.carry(&bytes).map(|_| ())
                } else {
                    dest.restore(&bytes)
                };
                if cursor > LIMIT {
                    assert!(result
                        .unwrap_err()
                        .message
                        .contains("cursor beyond supported range"));
                    assert_eq!(dest.save().unwrap(), before);
                    assert_eq!(dest.world().logs(LogCursor::default()).unwrap(), logs);
                    assert_eq!(dest.world().changes(&consumer).unwrap().events.len(), 0);
                } else {
                    result.unwrap();
                    assert_eq!(dest.save().unwrap(), bytes);
                    assert_eq!(dest.run(17.).unwrap(), 1);
                    assert!(dest.world().is_empty(), "tick and orphan reap continue");
                    assert_eq!(dest.world().journal_next(), LIMIT);
                    let continued = dest.save().unwrap();
                    assert_eq!(
                        Sim::<G>::from_save(&continued).unwrap().save().unwrap(),
                        continued
                    );
                    let epoch = dest.world().mutation_epoch();
                    assert!(dest.world_mut().spawn(()).is_err());
                    assert!(dest.world().log("past boundary").is_err());
                    assert_eq!(dest.world().mutation_epoch(), epoch);
                    assert_eq!(dest.save().unwrap(), continued);
                    dest.world().validate().unwrap();
                }
            }
            let fresh = Sim::<G>::from_save(&bytes);
            assert_eq!(fresh.is_ok(), cursor == LIMIT - 2);
            if let Err(error) = fresh {
                assert!(error.message.contains("cursor beyond supported range"));
            }

            // EXGAME omits game/structural/session journals; tick is its saved sequence cursor.
            let mut source = World::new(60, 0);
            source.spawn(()).unwrap();
            source.state.tick = cursor;
            let bytes = source.save().unwrap();
            let mut dest = World::new(60, 0);
            dest.spawn_named("keep", ()).unwrap();
            let before = dest.save().unwrap();
            let logs = dest.logs(LogCursor::default()).unwrap();
            for carry in [false, true] {
                let result = if carry {
                    dest.carry(&bytes).map(|_| ())
                } else {
                    dest.load(&bytes)
                };
                if cursor > LIMIT {
                    assert!(result
                        .unwrap_err()
                        .message
                        .contains("cursor beyond supported range"));
                    assert_eq!(dest.save().unwrap(), before);
                    assert_eq!(dest.logs(LogCursor::default()).unwrap(), logs);
                } else {
                    result.unwrap();
                    assert_eq!(dest.save().unwrap(), bytes);
                    dest.step_clock();
                    assert_eq!(dest.tick(), LIMIT - 1);
                    let continued = dest.save().unwrap();
                    dest.load(&continued).unwrap();
                    dest.spawn(()).unwrap();
                }
            }
        }
    }
    #[test]
    fn preflight_reserves_only_records_the_removal_will_emit() {
        #[derive(Default, crate::Component)]
        struct C;
        let mut w = World::new(60, 0);
        w.register::<C>().unwrap();
        let e = w.spawn(C).unwrap();
        w.journal_next.set((1 << 62) - 1);
        assert!(w.remove::<C>(e).unwrap().is_some());
        assert_eq!(w.journal_next(), 1 << 62);
        let epoch = w.mutation_epoch();
        w.set_parent(e, None).unwrap();
        assert_eq!(w.mutation_epoch(), epoch);
        assert_eq!(w.journal_next(), 1 << 62);
    }
}
