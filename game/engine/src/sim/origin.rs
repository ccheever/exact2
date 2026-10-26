//! A world's message carries what caused it (LLP 1133.022 D6, D10 item 6).
//!
//! A message the runner posts from a commit that was not a person's or the
//! app's own (a tool invocation, a wake, a package's event) arrives with
//! its origin in front of the text, [`ORIGIN_MARK`]`<word>\u{1}`. The world
//! keeps the least privileged origin posted into it since its fresh start,
//! and every message it emits carries that mark back, so the canvas's
//! `message=` handler runs under it: a tool that posts into a world cannot
//! start a device through the world's later emissions. The mark is
//! simulation state, saved and restored with the world; only a restart
//! that builds a fresh world from its arguments clears it, since a tool's
//! message can leave state that emits much later and the world's code is
//! not traced.
//!
//! The save writes the mark only when a world has one, so a save of a world
//! no tool reached is byte-for-byte what it was before the mark existed.

use super::*;

/// What precedes a marked message's text; the runner writes and reads the
/// same (`runner/src/runner/world_origin.rs`).
pub const ORIGIN_MARK: &str = "\u{1}origin:";

/// The origins a mark names, least privileged last; 0, the app's, is no mark.
const WORDS: [&str; 4] = ["app", "tool", "wake", "device"];

/// `text`'s origin rank and the text after its mark; rank 0 for none.
pub fn unmark(text: &str) -> (u8, &str) {
    let Some(rest) = text.strip_prefix(ORIGIN_MARK) else {
        return (0, text);
    };
    let Some((word, text)) = rest.split_once('\u{1}') else {
        return (0, text);
    };
    match WORDS.iter().position(|w| *w == word) {
        Some(rank) => (rank as u8, text),
        // An unknown word is the least privileged: never a way to raise one.
        None => (WORDS.len() as u8 - 1, text),
    }
}

/// `text` marked with origin rank `rank`; unchanged for the app's.
pub fn mark(rank: u8, text: String) -> String {
    match WORDS.get(rank as usize) {
        None | Some(&"app") => text,
        Some(word) => format!("{ORIGIN_MARK}{word}\u{1}{text}"),
    }
}

impl<G: Game> Sim<G> {
    /// A posted event: a message's mark is taken off its text and joins the
    /// world's.
    pub(super) fn unmarked(&mut self, event: InputEvent) -> InputEvent {
        match event {
            InputEvent::Message {
                text,
                at_ms,
                player,
            } => {
                let (rank, rest) = unmark(&text);
                if rank > self.origin {
                    self.origin = rank;
                    self.world
                        .log(format_args!("posted by a {} commit", WORDS[rank as usize]));
                }
                let text = if rank == 0 && rest.len() == text.len() {
                    text
                } else {
                    rest.to_owned()
                };
                InputEvent::Message {
                    text,
                    at_ms,
                    player,
                }
            }
            event => event,
        }
    }

    /// The world's emitted messages, each with its mark.
    pub(super) fn marked(&self, messages: Vec<String>) -> Vec<String> {
        let rank = self.origin;
        messages.into_iter().map(|m| mark(rank, m)).collect()
    }

    /// The world's mark as an origin's word, `app` for none.
    pub fn origin(&self) -> &'static str {
        WORDS.get(self.origin as usize).copied().unwrap_or("device")
    }

    /// `,"origin":"<word>"` for the agent's `state` of a marked world.
    pub(crate) fn origin_json(&self) -> String {
        match self.origin {
            0 => String::new(),
            _ => format!(",\"origin\":\"{}\"", self.origin()),
        }
    }
}

impl Data for Saved {
    fn write(&self, w: &mut dyn crate::Writer) {
        w.begin_struct();
        w.field("game");
        self.game.write(w);
        w.field("world");
        self.world.write(w);
        w.field("args");
        self.args.write(w);
        w.field("input");
        self.input.write(w);
        w.field("queue");
        self.queue.write(w);
        w.field("world_us");
        self.world_us.write(w);
        w.field("journal");
        self.journal.write(w);
        w.field("journal_next");
        self.journal_next.write(w);
        w.field("overflow_logged");
        self.overflow_logged.write(w);
        w.field("published");
        self.published.write(w);
        // Only a marked world's save carries its mark (module doc).
        if self.origin != 0 {
            w.field("origin");
            self.origin.write(w);
        }
        w.end_struct();
    }

    fn read(&mut self, r: &mut dyn crate::Reader) -> Result<(), DataError> {
        const FIELDS: [&str; 10] = [
            "game",
            "world",
            "args",
            "input",
            "queue",
            "world_us",
            "journal",
            "journal_next",
            "overflow_logged",
            "published",
        ];
        let mut seen = [false; FIELDS.len()];
        self.origin = 0;
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            let at = field.as_str();
            let read = match at {
                "game" => self.game.read(r),
                "world" => self.world.read(r),
                "args" => self.args.read(r),
                "input" => self.input.read(r),
                "queue" => self.queue.read(r),
                "world_us" => self.world_us.read(r),
                "journal" => self.journal.read(r),
                "journal_next" => self.journal_next.read(r),
                "overflow_logged" => self.overflow_logged.read(r),
                "published" => self.published.read(r),
                "origin" => self.origin.read(r),
                _ => r.skip(),
            };
            read.map_err(|e| e.at(at))?;
            if let Some(i) = FIELDS.iter().position(|f| *f == at) {
                seen[i] = true;
            }
        }
        if self.origin as usize >= WORDS.len() {
            return Err(DataError::new("an origin mark names no origin").at("origin"));
        }
        if r.complete() {
            if let Some(i) = seen.iter().position(|s| !s) {
                return Err(DataError::new("missing field").at(FIELDS[i]));
            }
        }
        Ok(())
    }
}
