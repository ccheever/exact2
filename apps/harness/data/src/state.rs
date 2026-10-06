//! The session's state: plain Rust records mirroring `shapes.contract`,
//! held in one mutex that the runner's thread reads and the agent's
//! threads write. A writer calls [`Shared::changed`] after it unlocks; the
//! runner then asks `session()` again.

use crate::keys::Keys;
use crate::providers::Sources;
use exact_runner::Native;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// `shape Run`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub text: String,
    pub fg: String,
    pub bg: String,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
    pub under: bool,
    pub strike: bool,
    /// A link target, or empty.
    pub href: String,
}

impl Run {
    /// A plain run.
    pub fn plain(text: impl Into<String>) -> Run {
        Run {
            text: text.into(),
            ..Run::default()
        }
    }

    /// A run in one foreground colour.
    pub fn fg(text: impl Into<String>, fg: &str) -> Run {
        Run {
            text: text.into(),
            fg: fg.to_string(),
            ..Run::default()
        }
    }

    /// A dim run.
    pub fn dim(text: impl Into<String>) -> Run {
        Run {
            text: text.into(),
            dim: true,
            ..Run::default()
        }
    }

    /// The same style, other text.
    pub fn same_style(&self, other: &Run) -> bool {
        self.fg == other.fg
            && self.bg == other.bg
            && self.bold == other.bold
            && self.italic == other.italic
            && self.dim == other.dim
            && self.under == other.under
            && self.strike == other.strike
    }
}

/// `shape Line`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line {
    pub runs: Vec<Run>,
}

impl Line {
    /// One plain run.
    pub fn plain(text: impl Into<String>) -> Line {
        Line {
            runs: vec![Run::plain(text)],
        }
    }

    /// Append a run, merging it into the last when the styles match.
    pub fn push(&mut self, run: Run) {
        if run.text.is_empty() {
            return;
        }
        if let Some(last) = self.runs.last_mut() {
            if last.same_style(&run) {
                last.text.push_str(&run.text);
                return;
            }
        }
        self.runs.push(run);
    }

    /// The line's text.
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// `shape Block`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Block {
    pub kind: String,
    pub depth: f64,
    pub marker: String,
    pub lang: String,
    pub runs: Vec<Run>,
    pub lines: Vec<Line>,
}

impl Block {
    /// A paragraph of runs.
    pub fn p(runs: Vec<Run>) -> Block {
        Block {
            kind: "p".into(),
            runs,
            ..Block::default()
        }
    }

    /// Pre-coloured lines.
    pub fn lines(lines: Vec<Line>) -> Block {
        Block {
            kind: "lines".into(),
            lines,
            ..Block::default()
        }
    }
}

/// `shape Entry`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry {
    pub id: String,
    pub kind: String,
    pub busy: bool,
    pub title: String,
    pub status: String,
    pub blocks: Vec<Block>,
    pub more: f64,
    pub image: String,
    pub cols: f64,
    pub rows: f64,
    /// What the title links to, or empty.
    pub link: String,
}

/// `shape Approval`; `id` empty when nothing waits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Approval {
    pub id: String,
    pub tool: String,
    pub summary: String,
    pub lines: Vec<Line>,
}

/// `shape ModelChoice`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelChoice {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub available: bool,
}

/// One message of the conversation, provider-neutral.
#[derive(Debug, Clone, PartialEq)]
pub struct Msg {
    pub role: Role,
    pub parts: Vec<Part>,
}

/// Who said it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// A piece of a message.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
}

impl Msg {
    /// A user's text.
    pub fn user(text: &str) -> Msg {
        Msg {
            role: Role::User,
            parts: vec![Part::Text(text.to_string())],
        }
    }

    /// The message's text parts, joined.
    pub fn text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|p| match p {
                Part::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// A decision on a pending approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Yes,
    Always,
    No,
}

/// The running turn's handle: its number and its cancel flag.
#[derive(Clone)]
pub struct Turn {
    pub id: u64,
    pub cancel: Arc<AtomicBool>,
}

impl Turn {
    /// Whether the turn was interrupted.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Everything the session holds.
#[derive(Default)]
pub struct State {
    pub model: String,
    pub cwd: String,
    pub branch: String,
    pub phase: String,
    pub tokens_in: f64,
    pub tokens_out: f64,
    /// The last request's input tokens: how full the context is.
    pub context: f64,
    pub entries: Vec<Entry>,
    pub approval: Approval,
    pub toast: String,
    pub toast_gen: u64,
    pub models: Vec<ModelChoice>,
    pub history: Vec<Msg>,
    /// The turn running now, if any.
    pub turn: Option<Turn>,
    pub next_turn: u64,
    /// The answer to the approval with this id.
    pub decision: Option<(String, Choice)>,
    /// Tools the person allowed for the rest of the session.
    pub always: Vec<String>,
    pub next_entry: u64,
    pub next_file: u64,
    /// The round in progress: its assistant message so far, and the
    /// results its tools have answered. An interrupt finishes it.
    pub round: Option<(Msg, Vec<Part>)>,
    /// Provider keys. Never copied into anything the session shows.
    pub keys: Keys,
    /// Whether Ollama answered the startup probe.
    pub ollama_up: bool,
    /// OpenRouter's models, from the catalog, its cache, or the fallback.
    pub catalog: Vec<ModelChoice>,
    /// OpenRouter's API (a test points it at a local server).
    pub openrouter_base: String,
    /// `~/.config/exact-harness`, when there is one.
    pub config_dir: Option<std::path::PathBuf>,
    /// Whether background requests (the catalog, the Ollama probe) run.
    pub network: bool,
    /// The streaming round's text so far, for an interrupt to keep.
    pub round_text: String,
    /// Prompts sent while a turn ran, oldest first (P16).
    pub queue: std::collections::VecDeque<String>,
    /// Entries retired so far: the host printed them, the session no
    /// longer answers them (P1).
    pub retired: f64,
}

impl State {
    /// A fresh entry id: a decimal number, strictly increasing, never
    /// reused (not even across `/clear`).
    pub fn id(&mut self) -> String {
        self.next_entry += 1;
        self.next_entry.to_string()
    }

    /// Stop answering every settled entry whose id is at most `id` (they
    /// stay in the model's history). Idempotent; an older id or one that
    /// is not a number does nothing. A busy entry is never retired.
    pub fn retire(&mut self, id: &str) -> bool {
        let Ok(through) = id.trim().parse::<u64>() else {
            return false;
        };
        let before = self.entries.len();
        self.entries
            .retain(|e| e.busy || e.id.parse::<u64>().map_or(true, |n| n > through));
        self.retired += (before - self.entries.len()) as f64;
        true
    }

    /// Append an entry and return its id.
    pub fn push(&mut self, mut entry: Entry) -> String {
        if entry.id.is_empty() {
            entry.id = self.id();
        }
        let id = entry.id.clone();
        self.entries.push(entry);
        id
    }

    /// A busy entry to change. A settled one is never changed again, so
    /// this answers `None` for it.
    pub fn busy_entry(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries
            .iter_mut()
            .rev()
            .find(|e| e.id == id)
            .filter(|e| e.busy)
    }

    /// Whether a turn is running.
    pub fn busy(&self) -> bool {
        self.turn.is_some()
    }

    /// Whether `turn` is still the running one and not interrupted.
    pub fn current(&self, turn: &Turn) -> bool {
        self.turn.as_ref().map(|t| t.id) == Some(turn.id) && !turn.cancelled()
    }

    /// What the model list and the routes are made from.
    pub fn sources(&self) -> Sources<'_> {
        Sources {
            keys: &self.keys,
            ollama_up: self.ollama_up,
            catalog: &self.catalog,
            openrouter_base: &self.openrouter_base,
        }
    }

    /// Rebuild the model list from the keys and the catalog. The current
    /// model stays listed even when the catalog no longer has it.
    pub fn refresh_models(&mut self) {
        let mut models = crate::providers::models(&self.sources());
        if !models.iter().any(|m| m.id == self.model) {
            if let Some(old) = self.models.iter().find(|m| m.id == self.model) {
                let mut kept = old.clone();
                kept.available = models
                    .iter()
                    .any(|m| m.provider == kept.provider && m.available);
                models.push(kept);
            }
        }
        self.models = models;
    }

    /// An error entry with every key scrubbed out of it.
    pub fn error_scrubbed(&mut self, message: &str) -> String {
        let message = crate::keys::scrub(message, &self.keys.secrets());
        self.error(&message)
    }

    /// The selected model's provider.
    pub fn provider(&self) -> String {
        self.models
            .iter()
            .find(|m| m.id == self.model)
            .map(|m| m.provider.clone())
            .unwrap_or_default()
    }

    /// Add a notice entry (settled).
    pub fn notice(&mut self, blocks: Vec<Block>) -> String {
        self.push(Entry {
            kind: "notice".into(),
            blocks,
            ..Entry::default()
        })
    }

    /// Add an error entry (settled).
    pub fn error(&mut self, message: &str) -> String {
        self.push(Entry {
            kind: "error".into(),
            blocks: vec![Block::p(vec![Run::plain(message)])],
            ..Entry::default()
        })
    }
}

/// The state, the condition the agent waits on, and the announcer.
pub struct Shared {
    state: Mutex<State>,
    pub wake: Condvar,
    pub native: Native,
}

impl Shared {
    /// Shared state around `state`.
    pub fn new(state: State, native: Native) -> Arc<Shared> {
        Arc::new(Shared {
            state: Mutex::new(state),
            wake: Condvar::new(),
            native,
        })
    }

    /// The state, locked; a poisoned lock is taken anyway.
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Tell the host the session changed.
    pub fn changed(&self) {
        self.native.changed("session");
    }

    /// Show a transient notice for three seconds.
    pub fn toast(self: &Arc<Shared>, text: &str) {
        let generation = {
            let mut s = self.lock();
            s.toast = text.to_string();
            s.toast_gen += 1;
            s.toast_gen
        };
        self.changed();
        let shared = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let cleared = {
                let mut s = shared.lock();
                let mine = s.toast_gen == generation;
                if mine {
                    s.toast.clear();
                }
                mine
            };
            if cleared {
                shared.changed();
            }
        });
    }
}
