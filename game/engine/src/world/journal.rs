use super::{Event, World};
use std::{collections::VecDeque, fmt::Write};

const LINES: usize = 4096;
const SESSION_LINE_BYTES: usize = 65_536;

pub(super) struct Entry {
    event: Event,
    game: bool,
    before_game: u64,
}

fn append<T>(lines: &mut VecDeque<T>, event: T) {
    if lines.len() == LINES {
        lines.pop_front();
    }
    lines.push_back(event);
}

impl World {
    /// Append a deterministic game event. Saved by Sim, outside the world hash.
    pub fn log(&self, line: impl std::fmt::Display) {
        let mut event = self.event(line);
        event.index = self.saved_journal_next.get();
        self.saved_journal_next.set(event.index + 1);
        append(&mut self.saved_journal.borrow_mut(), event.clone());
        self.append_session(event, true);
    }
    /// Append host/adapter diagnostics, never saved or hashed. Logs merge these
    /// with game events in append order, using an independent session cursor.
    /// Retains 4,096 combined lines. Oversized text becomes an explicit refusal.
    pub fn session_log(&self, line: impl std::fmt::Display) {
        struct Bounded(String);
        impl Write for Bounded {
            fn write_str(&mut self, text: &str) -> std::fmt::Result {
                if text.len() > SESSION_LINE_BYTES - self.0.len() {
                    return Err(std::fmt::Error);
                }
                self.0.push_str(text);
                Ok(())
            }
        }
        let mut text = Bounded(String::new());
        if write!(text, "{line}").is_err() {
            text.0 = "session journal refused: line exceeds 65536-byte budget".into();
        }
        self.session_count.set(self.session_count.get() + 1);
        self.append_session(self.event(text.0), false);
    }
    fn event(&self, line: impl std::fmt::Display) -> Event {
        Event {
            index: 0,
            tick: self.tick(),
            seconds: self.seconds(),
            line: format!(
                "t={} tick={} {line}",
                self.tick() as u128 * 1000 / self.hz() as u128,
                self.tick()
            ),
        }
    }
    fn append_session(&self, event: Event, game: bool) {
        self.append_entry(Entry {
            event,
            game,
            before_game: self.saved_journal_next.get(),
        });
    }
    fn append_entry(&self, mut entry: Entry) {
        entry.event.index = self.journal_next.get();
        self.journal_next.set(entry.event.index + 1);
        append(&mut self.journal.borrow_mut(), entry);
    }
    /// Combined game/session history; reading does not change simulation state.
    pub fn journal(&self) -> Vec<Event> {
        self.journal
            .borrow()
            .iter()
            .map(|e| e.event.clone())
            .collect()
    }
    /// Next combined cursor. Draining a host never erases agent history.
    pub fn journal_next(&self) -> u64 {
        self.journal_next.get()
    }
    pub(crate) fn saved_journal(&self) -> Vec<Event> {
        self.saved_journal.borrow().iter().cloned().collect()
    }
    pub(crate) fn saved_journal_next(&self) -> u64 {
        self.saved_journal_next.get()
    }
    pub(crate) fn restore_journal(&mut self, lines: Vec<Event>, next: u64) {
        *self.saved_journal.get_mut() = lines.clone().into();
        self.saved_journal_next.set(next);
        *self.journal.get_mut() = lines
            .into_iter()
            .map(|event| Entry {
                before_game: event.index,
                event,
                game: true,
            })
            .collect();
        self.journal_next.set(next);
    }
    // Restore the saved game history and merge surviving session diagnostics at
    // their game-event boundaries. Neither the diagnostic nor its ordering key
    // enters the save. Setup appends to the current session in execution order.
    pub(crate) fn continue_journal(&mut self, old: &mut World, setup: bool) {
        let events = std::mem::take(self.journal.get_mut());
        let previous = std::mem::take(old.journal.get_mut());
        self.session_count.set(old.session_count.get());
        if setup {
            *self.journal.get_mut() = previous;
            self.journal_next.set(old.journal_next.get());
            for event in events {
                self.append_entry(event);
            }
        } else {
            let mut session: VecDeque<_> = previous.into_iter().filter(|e| !e.game).collect();
            let start = events
                .front()
                .map_or(self.saved_journal_next.get(), |e| e.event.index);
            self.journal_next
                .set(start + self.session_count.get() - session.len() as u64);
            for event in events {
                while session
                    .front()
                    .is_some_and(|e| e.before_game <= event.event.index)
                {
                    self.append_entry(session.pop_front().unwrap());
                }
                self.append_entry(event);
            }
            for event in session {
                self.append_entry(event);
            }
        }
    }
}
