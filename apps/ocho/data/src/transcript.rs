//! A tab's conversation view (transcript.rs `TranscriptView`, `Transcript`,
//! `Entry`; workspace.rs `refresh_transcript`, `send_transcript`, the
//! transcript keys; ui.rs `render_transcript`), without the PTY, the timers
//! and the list state. Time is host milliseconds (`now_ms`); the model
//! asks `wanted` what to fetch and `set`s the reply; the composer's send
//! hands back what the PTY receives.
//!
//! `view` is the `Transcript` shape:
//!
//! ```text
//! shape TranscriptEntry
//!   id: string          // "entry-N"
//!   kind: string        // user | assistant | tool-group | agents-md
//!   subdued: bool       // an assistant progress message: italic muted
//!   first: bool         // index 0: a user bubble has no top gap
//!   blocks: list<MarkdownBlock>   // the Markdown (markdown_doc), empty for tool groups
//!   summary: string     // a tool group's "N tools · 1m 5s"
//!   live: string        // the working indicator's text on the newest tool group, "" elsewhere
//!   collapsed: bool     // an agents-md entry folded (▸); ▾ when expanded
//!   toggleId: string    // the press id that folds or unfolds it
//! shape Transcript
//!   visible: bool
//!   entries: list<TranscriptEntry>
//!   empty: string       // "No messages yet." | "Loading conversation…" | "No conversation available." | ""
//!   error: string       // "Couldn't refresh. {e}" over rows, "{e}" alone; "" when none
//!   working: string     // the working indicator under the last row, "" when idle or shown on a tool group
//!   provider: string    // the working indicator's provider
//!   draft: string       // the composer's text
//!   rows: number        // the composer's height in lines: its line count, 1-6
//!   placeholder: string // "Message the agent…"
//!   editing: bool       // the composer has the keyboard
//!   sendLabel: string   // "Send ↑" | "Sending…"
//!   canSend: bool       // the button is live
//!   scrollSeq: number   // moves when a scroll is asked; apply once per value
//!   scrollBy: number    // ±60 px per j / k / ↑ / ↓
//!   scrollTo: string    // "entry-N" (Home, End, a search hit) or ""
//! ```
//!
//! Entry blocks are `markdown_doc`'s `MarkdownBlock`s (that module's header
//! has their drawing, LaTeX included).
//!
//! Drawing (ui.rs `render_transcript`, 1:1; TEXT_SM 12 px): a column, full
//! size. The error first: `px_6 py_3` (24 × 12), 12 px `warn`. Then the
//! body: with no rows, `p_8` (32) `muted` (the empty text) with the working
//! indicator under it; with rows, a list anchored to the bottom (it starts
//! at the latest message and follows new ones until scrolled), `py_4` (16)
//! top and bottom. Each row: `px_6 py_3`, `pt_6` (24) instead for a user
//! message that is not first, 15 px text, line height 1.7, `text` colour,
//! its content centred at most 736 px wide:
//! - user: right-aligned bubble, at most 85 % wide, `px_4 py_3` (16 × 12),
//!   radius 20, bg `elevated`, the blocks inside;
//! - assistant: the blocks (subdued: every run `muted` italic);
//! - tool-group: 13 px, line height 20 px, `muted`: the summary, or on the
//!   newest group while live the working indicator (12 px) with `live`;
//! - agents-md: a pressable row (`gap_2`, 13 px `muted`, hover `text`) of
//!   "▸"/"▾" and "AGENTS.md instructions"; expanded, the blocks under it in
//!   a `pl_4` column with `border_l_1` `border`, the column `gap_3`.
//!
//! The working indicator (status_indicator.rs `working_indicator`, 12 px)
//! goes under the last row (`mt_2`, 736 px centred) when `working` is set.
//! The composer: `border_t_1` `border`, `px_6 py_3`, a row (736 px centred,
//! `items_end`, `gap_3`): the input (`flex_1`, radius 16, bg `elevated`,
//! `px_4 py_2`, `rows` lines tall, placeholder) and the send button
//! (`rounded_full`, `px_3 py_2`; live: bg `accent`, text `surface`; else bg
//! `elevated`, text `muted`).

use crate::markdown_doc::{self, Block};
use serde::Deserialize;
use serde_json::{json, Value as Json};
use std::collections::HashSet;

/// Background refresh interval (workspace.rs `REFRESH_INTERVAL`).
pub const REFRESH_INTERVAL_MS: f64 = 8_000.0;
/// The visible, running transcript's interval (`LIVE_TRANSCRIPT_INTERVAL`).
pub const LIVE_INTERVAL_MS: f64 = 2_000.0;
/// The provider writes its transcript after Enter; read it again then.
pub const AFTER_SEND_MS: f64 = 800.0;
/// Scroll step for j / k / ↑ / ↓, px.
pub const SCROLL_STEP: f64 = 60.0;
/// The composer's placeholder.
pub const PLACEHOLDER: &str = "Message the agent…";
// Realized by the host: Terminal.swift `submit` pastes, then presses Return 150 ms later.
#[allow(dead_code)]
/// What follows the pasted draft on the PTY.
pub const ENTER: &str = "\r";
// Realized by the host: Terminal.swift `submit` pastes, then presses Return 150 ms later.
#[allow(dead_code)]
/// Bracketed paste lands before Enter is pressed this much later.
pub const PASTE_SETTLE_MS: f64 = 150.0;
/// A `fleet transcript` read gives up after this long (`run_within`).
pub const TIMEOUT_MS: f64 = 10_000.0;
/// The composer grows to this many lines.
pub const MAX_COMPOSER_LINES: usize = 6;
// Drawn by transcript.contract (max-width 736).
#[allow(dead_code)]
/// Rows and the composer are centred at most this wide, px.
pub const COLUMN_WIDTH: f64 = 736.0;
/// Sending with the session's terminal gone.
pub const NOT_CONNECTED: &str =
    "The session terminal is no longer connected; your draft is intact.";
/// Enter could not be pressed in a connected terminal.
pub const NOT_SUBMITTED: &str =
    "Could not submit to the session terminal; check its composer before retrying.";

/// An assistant message's place in its turn.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum MessagePhase {
    /// Progress, shown subdued.
    #[serde(rename = "commentary")]
    Commentary,
    /// The turn's reply.
    #[serde(rename = "final")]
    Final,
    /// Older helpers omit the phase.
    #[default]
    #[serde(other)]
    Unknown,
}

/// One entry of a transcript.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Entry {
    /// The parsed text, empty for tool groups.
    #[serde(skip)]
    pub blocks: Vec<Block>,
    /// `user`, `assistant` or `tools`.
    pub kind: String,
    /// Where an assistant message sits in its turn.
    #[serde(default)]
    pub phase: MessagePhase,
    /// The Markdown.
    #[serde(default)]
    pub text: String,
    /// A tool group's tool count.
    #[serde(default)]
    pub count: usize,
    /// A tool group's span, seconds.
    #[serde(default)]
    pub seconds: u64,
}

/// A conversation as `fleet transcript` prints it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Transcript {
    /// The entries, oldest first.
    #[serde(default)]
    pub entries: Vec<Entry>,
}

/// One `fleet transcript M S --revision R` reply, or fleet serve's (which
/// can also answer only what changed: `from`, `base`).
#[derive(Default, Deserialize)]
pub struct TranscriptUpdate {
    /// The entries, when they changed (from `from`, when it is set).
    #[serde(skip)]
    pub transcript: Transcript,
    /// The entries as read; `decode` moves them into `transcript`. Read
    /// directly rather than through `#[serde(flatten)]`, which buffers the
    /// whole reply first (hundreds of kilobytes, every few seconds).
    #[serde(default)]
    entries: Vec<Entry>,
    /// The revision the reply describes.
    #[serde(default)]
    pub revision: String,
    /// Nothing changed since the revision asked for.
    #[serde(default)]
    pub unchanged: bool,
    /// The entries replace the reader's from this index on.
    #[serde(default)]
    pub from: Option<usize>,
    /// Fingerprints every entry but the last, for the next ask.
    #[serde(default)]
    pub base: String,
}

impl TranscriptUpdate {
    /// Parse a reply and its Markdown (only the entries it carries).
    pub fn decode(json: &str) -> serde_json::Result<Self> {
        let mut update: Self = serde_json::from_str(json)?;
        update.transcript.entries = std::mem::take(&mut update.entries);
        update.transcript.prepare_markdown();
        Ok(update)
    }
}

impl Transcript {
    /// An assistant entry shown subdued: commentary, or (for helpers without
    /// phases) any reply that is not the last of its turn.
    pub fn is_progress(&self, index: usize) -> bool {
        let entry = &self.entries[index];
        if entry.kind != "assistant" {
            return false;
        }
        match entry.phase {
            MessagePhase::Commentary => true,
            MessagePhase::Final => false,
            MessagePhase::Unknown => self
                .entries
                .get(index + 1)
                .is_some_and(|next| next.kind != "user"),
        }
    }

    fn prepare_markdown(&mut self) {
        for entry in &mut self.entries {
            if entry.kind != "tools" {
                entry.blocks = markdown_doc::parse(&entry.text);
            }
        }
    }
}

impl Entry {
    /// A user entry with its Markdown parsed.
    pub fn user(text: String) -> Self {
        Self {
            blocks: markdown_doc::parse(&text),
            kind: "user".into(),
            phase: MessagePhase::Unknown,
            text,
            count: 0,
            seconds: 0,
        }
    }

    /// The `# AGENTS.md instructions for PATH` wrapper a provider injects,
    /// with nothing but instructions and environment context inside.
    pub fn is_injected_instructions(&self) -> bool {
        if self.kind != "user" {
            return false;
        }
        let Some(header) = self
            .text
            .trim()
            .strip_prefix("# AGENTS.md instructions for ")
        else {
            return false;
        };
        let Some((path, body)) = header.split_once('\n') else {
            return false;
        };
        if path.trim().is_empty() {
            return false;
        }
        let Some(body) = body.trim_start().strip_prefix("<INSTRUCTIONS>") else {
            return false;
        };
        let Some((_, tail)) = body.split_once("</INSTRUCTIONS>") else {
            return false;
        };
        let tail = tail.trim();
        // Only collapse the injected wrapper, never a real request appended to it.
        tail.is_empty()
            || tail
                .strip_prefix("<environment_context>")
                .and_then(|context| context.split_once("</environment_context>"))
                .is_some_and(|(_, rest)| rest.trim().is_empty())
    }

    /// A tool group's line: "28 tools · 7m", "1 tool".
    pub fn summary(&self) -> String {
        let tools = if self.count == 1 { "tool" } else { "tools" };
        let duration = match self.seconds {
            0 => return format!("{} {tools}", self.count),
            s if s < 60 => format!("{s}s"),
            s if s < 3600 && s % 60 == 0 => format!("{}m", s / 60),
            s if s < 3600 => format!("{}m {}s", s / 60, s % 60),
            s if s % 3600 == 0 => format!("{}h", s / 3600),
            s => format!("{}h {}m", s / 3600, (s % 3600) / 60),
        };
        format!("{} {tools} · {duration}", self.count)
    }
}

/// A message sent through the PTY but not yet in the provider's transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingMessage {
    text: String,
    after_index: usize,
}

/// What `send` asks of the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Send {
    /// Nothing to send (empty, or a send in flight).
    Nothing,
    /// The terminal is gone: show [`NOT_CONNECTED`]; the draft stays.
    NotConnected,
    /// Paste this into the PTY, press [`ENTER`] [`PASTE_SETTLE_MS`] later,
    /// then call `sent`.
    Paste(String),
}

/// A provider reply landed (workspace.rs `transcript_reply_observed`): a
/// newer observation of the same session with a new last message, or the
/// session going idle. Clock-only status ticks do not count. The model
/// calls `refresh_now` on the active, visible transcript when this holds.
pub fn reply_observed(current: &crate::types::Session, incoming: &crate::types::Session) -> bool {
    current.id == incoming.id
        && incoming.status_observed_at > current.status_observed_at
        && ((!incoming.last_message.is_empty() && incoming.last_message != current.last_message)
            || (current.state != "idle" && incoming.state == "idle"))
}

/// What a key did in the transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptKey {
    /// Esc outside the composer: leave transcript mode.
    Close,
    /// i or Enter: the composer takes the keyboard.
    Compose,
    /// The composer gave the keyboard back (Esc while editing).
    StopEditing,
    /// Enter in the composer: `send` was asked.
    Send,
    /// A scroll was asked (`scrollSeq` moved).
    Scrolled,
    /// Nothing the transcript handles.
    None,
}

/// A tab's conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct TranscriptView {
    /// The machine and session `fleet transcript` reads.
    pub machine: String,
    /// The session id.
    pub session: String,
    /// Transcript mode is on.
    pub visible: bool,
    /// The conversation, once read.
    pub data: Option<Transcript>,
    /// The last read's failure.
    pub error: Option<String>,
    /// A read is in flight.
    pub loading: bool,
    /// A live reply arrived while a read was in flight: read again after.
    pub pending_refresh: bool,
    /// When the last read landed, host ms.
    pub fetched_at: Option<f64>,
    /// The read in flight (a counter; a stale reply is dropped).
    pub request: u64,
    /// The revision the data answers.
    pub revision: String,
    /// Entries whose AGENTS.md disclosure is open.
    pub expanded_instructions: HashSet<usize>,
    /// Entry to scroll to once the next transcript arrives (a search hit).
    pub reveal: Option<usize>,
    /// The composer's text.
    pub draft: String,
    /// The composer has the keyboard.
    pub editing: bool,
    /// A send is in flight.
    pub sending: bool,
    /// Read now (or at this host time), ignoring the interval.
    force_at: Option<f64>,
    scroll_seq: u64,
    scroll_by: f64,
    scroll_to: Option<usize>,
    pending_messages: Vec<PendingMessage>,
    source_entries: usize,
}

impl Default for TranscriptView {
    fn default() -> Self {
        TranscriptView::new("", "")
    }
}

impl TranscriptView {
    /// The view for a session.
    pub fn new(machine: &str, session: &str) -> Self {
        Self {
            machine: machine.into(),
            session: session.into(),
            visible: false,
            data: None,
            error: None,
            loading: false,
            pending_refresh: false,
            fetched_at: None,
            request: 0,
            revision: String::new(),
            expanded_instructions: HashSet::new(),
            reveal: None,
            draft: String::new(),
            editing: false,
            sending: false,
            force_at: None,
            scroll_seq: 0,
            scroll_by: 0.0,
            scroll_to: None,
            pending_messages: Vec::new(),
            source_entries: 0,
        }
    }

    // ----- reading ------------------------------------------------------------

    /// The interval at `live` (the transcript visible on a RUNNING session).
    pub fn interval_ms(live: bool) -> f64 {
        if live {
            LIVE_INTERVAL_MS
        } else {
            REFRESH_INTERVAL_MS
        }
    }

    /// Ask for a read at the next `wanted`, whatever the interval (a reply was
    /// observed, the mode was toggled, a hit opened). While a read is in
    /// flight the ask is kept for after it.
    pub fn refresh_now(&mut self) {
        if self.loading {
            self.pending_refresh = true;
        } else {
            self.force_at = Some(0.0);
        }
    }

    /// A read is due: forced, never read, or the interval passed.
    pub fn due(&self, now_ms: f64, live: bool) -> bool {
        if self.loading {
            return false;
        }
        if self.force_at.is_some_and(|at| at <= now_ms) {
            return true;
        }
        match self.fetched_at {
            None => true,
            Some(at) => now_ms - at >= Self::interval_ms(live),
        }
    }

    /// The `fleet` argv to run now, if a read is due: `transcript M S
    /// --revision R` (an empty revision when forced). Call `begin` once it
    /// is queued.
    pub fn wanted(&self, now_ms: f64, live: bool) -> Option<Vec<String>> {
        if !self.due(now_ms, live) || self.machine.is_empty() || self.session.is_empty() {
            return None;
        }
        let forced = self.force_at.is_some_and(|at| at <= now_ms);
        Some(vec![
            "transcript".into(),
            self.machine.clone(),
            self.session.clone(),
            "--revision".into(),
            if forced {
                String::new()
            } else {
                self.revision.clone()
            },
        ])
    }

    /// The read `wanted` gave was queued: its request number, for `set`.
    pub fn begin(&mut self) -> u64 {
        self.loading = true;
        self.request += 1;
        self.request
    }

    /// The reply to read `request` (dropped when a newer read superseded
    /// it). `active` is whether the tab is still the active one: a refresh
    /// asked while the read was in flight is forced at once on the active
    /// tab, and only clears the interval on another (it reads when shown).
    pub fn set(&mut self, request: u64, now_ms: f64, result: Result<String, String>, active: bool) {
        if request != self.request || !self.loading {
            return;
        }
        self.loading = false;
        self.fetched_at = Some(now_ms);
        // A forced read that came due has been made; one still ahead (the
        // refresh after a send) stays scheduled.
        if self.force_at.is_some_and(|at| at <= now_ms) {
            self.force_at = None;
        }
        let refresh_again = std::mem::take(&mut self.pending_refresh);
        match result.and_then(|text| {
            TranscriptUpdate::decode(&text).map_err(|e| format!("Read transcript: {e}"))
        }) {
            Ok(update) => {
                self.accept_update(update);
                if let Some(target) = self.reveal.take() {
                    let len = self.data.as_ref().map_or(0, |d| d.entries.len());
                    if target < len {
                        self.scroll_to_entry(target);
                    }
                }
            }
            Err(error) => self.error = Some(error),
        }
        if refresh_again {
            if active {
                self.force_at = Some(0.0);
            } else {
                self.fetched_at = None;
            }
        }
    }

    /// Apply a decoded reply.
    pub fn accept_update(&mut self, update: TranscriptUpdate) {
        if !update.unchanged {
            self.accept(update.transcript);
        }
        self.revision = update.revision;
        self.error = None;
    }

    /// Apply a transcript, reconciling messages sent optimistically.
    pub fn accept(&mut self, mut data: Transcript) {
        let mut matched_through = 0;
        self.pending_messages.retain_mut(|pending| {
            let found = data
                .entries
                .iter()
                .enumerate()
                .skip(pending.after_index.max(matched_through))
                .find(|(_, entry)| {
                    entry.kind == "user" && entry.text.trim() == pending.text.trim()
                });
            if let Some((index, _)) = found {
                matched_through = index + 1;
                false
            } else {
                pending.after_index = pending.after_index.max(matched_through);
                true
            }
        });
        self.source_entries = data.entries.len();
        for pending in &self.pending_messages {
            data.entries.push(Entry::user(pending.text.clone()));
        }
        self.replace_display(data);
    }

    fn replace_display(&mut self, data: Transcript) {
        let old = self
            .data
            .as_ref()
            .map(|d| d.entries.as_slice())
            .unwrap_or(&[]);
        let unchanged = old
            .iter()
            .zip(&data.entries)
            .take_while(|(a, b)| a == b)
            .count();
        self.expanded_instructions
            .retain(|index| *index < unchanged);
        self.data = Some(data);
        self.error = None;
    }

    /// Show a sent message before the provider's transcript has it.
    pub fn add_optimistic_user(&mut self, text: String) {
        self.pending_messages.push(PendingMessage {
            text: text.clone(),
            after_index: self.source_entries,
        });
        let mut data = self.data.clone().unwrap_or_default();
        data.entries.push(Entry::user(text));
        self.replace_display(data);
    }

    /// Messages sent but not yet in the transcript.
    pub fn pending_count(&self) -> usize {
        self.pending_messages.len()
    }

    /// Fold or unfold an AGENTS.md entry (a no-op on any other entry).
    pub fn toggle_instructions(&mut self, index: usize) {
        if !self
            .data
            .as_ref()
            .and_then(|data| data.entries.get(index))
            .is_some_and(Entry::is_injected_instructions)
        {
            return;
        }
        if !self.expanded_instructions.insert(index) {
            self.expanded_instructions.remove(&index);
        }
    }

    // ----- scrolling ----------------------------------------------------------

    fn scroll_to_entry(&mut self, index: usize) {
        self.scroll_seq += 1;
        self.scroll_by = 0.0;
        self.scroll_to = Some(index);
    }

    fn scroll_by(&mut self, delta: f64) {
        self.scroll_seq += 1;
        self.scroll_by = delta;
        self.scroll_to = None;
    }

    // ----- composer -----------------------------------------------------------

    /// The composer's text changed.
    pub fn input(&mut self, text: &str) {
        self.draft = text.to_string();
    }

    /// Send the draft (workspace.rs `send_transcript`); `connected` is
    /// whether the tab's terminal is up.
    pub fn send(&mut self, connected: bool) -> Send {
        if self.sending || self.draft.trim().is_empty() {
            return Send::Nothing;
        }
        if !connected {
            return Send::NotConnected;
        }
        self.sending = true;
        Send::Paste(self.draft.clone())
    }

    /// Enter was pressed `PASTE_SETTLE_MS` after the paste: `connected`,
    /// whether the same terminal is still the tab's; `sent`, whether the key
    /// went through. On success the bubble appears, the draft clears if
    /// untouched, and the transcript is read again `AFTER_SEND_MS` later
    /// (the model reads only the active tab's). Returns the error to show.
    pub fn sent(
        &mut self,
        draft: &str,
        connected: bool,
        sent: bool,
        now_ms: f64,
    ) -> Option<&'static str> {
        self.sending = false;
        let sent = connected && sent;
        if sent {
            self.add_optimistic_user(draft.to_string());
            if self.draft == draft {
                self.draft.clear();
            }
            self.force_at = Some(now_ms + AFTER_SEND_MS);
            return None;
        }
        connected.then_some(NOT_SUBMITTED)
    }

    /// The button is live: not sending, a draft, a connected terminal.
    pub fn can_send(&self, connected: bool) -> bool {
        !self.sending && !self.draft.trim().is_empty() && connected
    }

    // ----- keys ---------------------------------------------------------------

    /// A key while the transcript (or its composer, when `editing`) has the
    /// keyboard: `name` is the canonical key, `mods` the modifiers held.
    pub fn key(&mut self, name: &str, mods: &crate::picker::Mods) -> TranscriptKey {
        let name = crate::picker::canon(name);
        let plain = !mods.meta && !mods.ctrl && !mods.alt;
        if self.editing {
            return match name.as_str() {
                "enter" if plain && !mods.shift => TranscriptKey::Send,
                "escape" => {
                    self.editing = false;
                    TranscriptKey::StopEditing
                }
                _ => TranscriptKey::None,
            };
        }
        match name.as_str() {
            "escape" => TranscriptKey::Close,
            "i" | "enter" if plain => {
                self.editing = true;
                TranscriptKey::Compose
            }
            "up" | "k" => {
                self.scroll_by(-SCROLL_STEP);
                TranscriptKey::Scrolled
            }
            "down" | "j" => {
                self.scroll_by(SCROLL_STEP);
                TranscriptKey::Scrolled
            }
            "home" => {
                self.scroll_to_entry(0);
                TranscriptKey::Scrolled
            }
            "end" => {
                let len = self.data.as_ref().map_or(0, |d| d.entries.len());
                self.scroll_to_entry(len.saturating_sub(1));
                TranscriptKey::Scrolled
            }
            _ => TranscriptKey::None,
        }
    }

    // ----- view ---------------------------------------------------------------

    /// The `Transcript` shape. `live` is the working indicator's (provider,
    /// status) while the session is RUNNING and observed ("Working…" when
    /// the status is empty); `connected` whether the tab's terminal is up.
    pub fn view(&self, live: Option<(&str, &str)>, connected: bool) -> Json {
        let live = live.map(|(provider, status)| {
            (
                provider.to_string(),
                if status.is_empty() {
                    "Working…".to_string()
                } else {
                    status.to_string()
                },
            )
        });
        let mut entries = Vec::new();
        let mut empty = "";
        let mut working = String::new();
        match &self.data {
            Some(data) if data.entries.is_empty() => {
                empty = "No messages yet.";
                if let Some((_, status)) = &live {
                    working = status.clone();
                }
            }
            Some(data) => {
                let latest_tools = live.as_ref().and_then(|_| {
                    data.entries
                        .iter()
                        .enumerate()
                        .rev()
                        .take_while(|(_, entry)| entry.kind != "user")
                        .find(|(_, entry)| entry.kind == "tools")
                        .map(|(index, _)| index)
                });
                for (index, entry) in data.entries.iter().enumerate() {
                    let injected = entry.is_injected_instructions();
                    let subdued = data.is_progress(index);
                    let kind = match entry.kind.as_str() {
                        "user" if injected => "agents-md",
                        "user" => "user",
                        "tools" => "tool-group",
                        _ => "assistant",
                    };
                    let mut live_text = String::new();
                    if latest_tools == Some(index) {
                        if let Some((_, status)) = &live {
                            let tools = if entry.count == 1 { "tool" } else { "tools" };
                            let separator = if status.ends_with('…') { " " } else { ". " };
                            live_text = format!(
                                "{}{separator}Called {} {tools}.",
                                status.trim_end_matches('.'),
                                entry.count
                            );
                        }
                    }
                    let collapsed = injected && !self.expanded_instructions.contains(&index);
                    let blocks = if entry.kind == "tools" || collapsed {
                        Vec::new()
                    } else {
                        markdown_doc::to_json(&entry.blocks, subdued)
                    };
                    entries.push(json!({
                        "id": format!("entry-{index}"),
                        "kind": kind,
                        "subdued": subdued,
                        "first": index == 0,
                        "blocks": blocks,
                        "summary": if entry.kind == "tools" { entry.summary() } else { String::new() },
                        "live": live_text,
                        "collapsed": collapsed,
                        "toggleId": if injected { format!("transcript-toggle-{index}") } else { String::new() },
                    }));
                }
                if latest_tools.is_none() {
                    if let Some((_, status)) = &live {
                        working = status.clone();
                    }
                }
            }
            None => {
                empty = if self.error.is_none() {
                    "Loading conversation…"
                } else {
                    "No conversation available."
                };
                if let Some((_, status)) = &live {
                    working = status.clone();
                }
            }
        }
        let error = match &self.error {
            Some(error) if self.data.is_some() => format!("Couldn't refresh. {error}"),
            Some(error) => error.clone(),
            None => String::new(),
        };
        json!({
            "visible": self.visible,
            "entries": entries,
            "empty": empty,
            "error": error,
            "working": working,
            "provider": live.as_ref().map(|(p, _)| p.clone()).unwrap_or_default(),
            "draft": self.draft,
            "rows": (self.draft.matches('\n').count() + 1).min(MAX_COMPOSER_LINES),
            "placeholder": PLACEHOLDER,
            "editing": self.editing,
            "sendLabel": if self.sending { "Sending…" } else { "Send ↑" },
            "canSend": self.can_send(connected),
            "scrollSeq": self.scroll_seq,
            "scrollBy": self.scroll_by,
            "scrollTo": self.scroll_to.map(|i| format!("entry-{i}")).unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picker::Mods;

    #[test]
    fn optimistic_user_appears_immediately_and_reconciles_without_duplicates() {
        let mut view = TranscriptView::default();
        view.accept(
            TranscriptUpdate::decode(r#"{"entries":[{"kind":"user","text":"repeat"}]}"#)
                .unwrap()
                .transcript,
        );
        view.add_optimistic_user("repeat".into());
        assert_eq!(view.data.as_ref().unwrap().entries.len(), 2);

        // A cached or not-yet-updated source must not erase the local bubble.
        view.accept_update(
            TranscriptUpdate::decode(r#"{"revision":"old","unchanged":true}"#).unwrap(),
        );
        view.accept(
            TranscriptUpdate::decode(r#"{"entries":[{"kind":"user","text":"repeat"}]}"#)
                .unwrap()
                .transcript,
        );
        assert_eq!(view.data.as_ref().unwrap().entries.len(), 2);
        assert_eq!(view.pending_messages.len(), 1);

        view.accept(
            TranscriptUpdate::decode(
                r#"{"entries":[{"kind":"user","text":"repeat"},{"kind":"user","text":"repeat"}]}"#,
            )
            .unwrap()
            .transcript,
        );
        assert_eq!(view.data.as_ref().unwrap().entries.len(), 2);
        assert!(view.pending_messages.is_empty());
    }

    #[test]
    fn repeated_pending_messages_match_distinct_source_entries() {
        let mut view = TranscriptView::default();
        view.add_optimistic_user("again".into());
        view.add_optimistic_user("again".into());
        let source = |count| Transcript {
            entries: (0..count).map(|_| Entry::user("again".into())).collect(),
        };
        view.accept(source(1));
        assert_eq!(view.pending_messages.len(), 1);
        assert_eq!(view.data.as_ref().unwrap().entries.len(), 2);
        view.accept(source(1));
        assert_eq!(view.pending_messages.len(), 1);
        view.accept(source(2));
        assert!(view.pending_messages.is_empty());
        assert_eq!(view.data.as_ref().unwrap().entries.len(), 2);
    }

    #[test]
    fn unchanged_updates_keep_rows_and_legacy_responses_still_replace_them() {
        let mut view = TranscriptView::default();
        view.accept_update(
            TranscriptUpdate::decode(
                r#"{"entries":[{"kind":"user","text":"hello"}],"revision":"one"}"#,
            )
            .unwrap(),
        );
        let original = view.data.clone().unwrap();
        view.accept_update(
            TranscriptUpdate::decode(r#"{"entries":[],"revision":"one","unchanged":true}"#)
                .unwrap(),
        );
        assert_eq!(&original, view.data.as_ref().unwrap());
        assert_eq!(view.revision, "one");
        view.accept_update(
            TranscriptUpdate::decode(r#"{"entries":[{"kind":"user","text":"legacy"}]}"#).unwrap(),
        );
        assert_eq!(view.data.as_ref().unwrap().entries[0].text, "legacy");
        assert!(view.revision.is_empty());
    }

    fn message(text: &str) -> Entry {
        Entry {
            blocks: markdown_doc::parse(text),
            phase: MessagePhase::Unknown,
            kind: "assistant".into(),
            text: text.into(),
            count: 0,
            seconds: 0,
        }
    }

    fn instructions() -> Entry {
        let mut entry = message(
            "# AGENTS.md instructions for /project\n\n<INSTRUCTIONS>\nUse the project conventions.\n</INSTRUCTIONS>",
        );
        entry.kind = "user".into();
        entry
    }

    #[test]
    fn recognizes_injected_instructions_without_collapsing_real_requests() {
        let mut entry = instructions();
        assert!(entry.is_injected_instructions());
        entry
            .text
            .push_str("\n<environment_context>Project directory</environment_context>");
        assert!(entry.is_injected_instructions());
        entry.text.push_str("\nPlease fix the login button.");
        assert!(!entry.is_injected_instructions());
        entry.text = "Please update AGENTS.md with our conventions.".into();
        assert!(!entry.is_injected_instructions());
        entry = instructions();
        entry.kind = "assistant".into();
        assert!(!entry.is_injected_instructions());
        entry.kind = "user".into();
        entry.text = "# AGENTS.md instructions for /project\nExplain these instructions.".into();
        assert!(!entry.is_injected_instructions());
    }

    #[test]
    fn instruction_disclosures_start_collapsed_and_survive_refreshes() {
        let mut view = TranscriptView::default();
        let entries = vec![instructions(), message("Ready")];
        view.accept(Transcript {
            entries: entries.clone(),
        });
        assert!(view.expanded_instructions.is_empty());
        view.toggle_instructions(0);
        assert!(view.expanded_instructions.contains(&0));
        view.accept(Transcript {
            entries: entries.clone(),
        });
        assert!(view.expanded_instructions.contains(&0));
        let mut updated = entries;
        updated.push(message("Working"));
        view.accept(Transcript { entries: updated });
        assert!(view.expanded_instructions.contains(&0));
        view.toggle_instructions(0);
        assert!(view.expanded_instructions.is_empty());
        view.toggle_instructions(0);
        view.accept(Transcript {
            entries: vec![message("Different conversation")],
        });
        assert!(view.expanded_instructions.is_empty());
        view.toggle_instructions(0);
        assert!(view.expanded_instructions.is_empty());
    }

    #[test]
    fn progress_is_subdued_even_when_it_is_the_newest_message() {
        let transcript = TranscriptUpdate::decode(
            r#"{"entries":[
            {"kind":"user","text":"Do it"},
            {"kind":"assistant","phase":"commentary","text":"On it"},
            {"kind":"assistant","phase":"final","text":"Done"},
            {"kind":"user","text":"Next task"},
            {"kind":"assistant","phase":"commentary","text":"Still working"}
        ]}"#,
        )
        .unwrap()
        .transcript;
        assert_eq!(
            (0..5)
                .map(|i| transcript.is_progress(i))
                .collect::<Vec<_>>(),
            vec![false, true, false, false, true]
        );
    }

    #[test]
    fn old_helpers_keep_only_the_last_reply_in_each_turn_prominent() {
        let transcript = TranscriptUpdate::decode(
            r#"{"entries":[
            {"kind":"assistant","text":"On it"},
            {"kind":"tools","count":1},
            {"kind":"assistant","text":"Done"},
            {"kind":"user","text":"Next task"},
            {"kind":"assistant","text":"First check"},
            {"kind":"assistant","text":"Done again"}
        ]}"#,
        )
        .unwrap()
        .transcript;
        assert_eq!(
            (0..6)
                .map(|i| transcript.is_progress(i))
                .collect::<Vec<_>>(),
            vec![true, false, false, false, true, false]
        );
    }

    #[test]
    fn formats_tool_spans_without_inventing_missing_timing() {
        let mut entry = Entry {
            blocks: Vec::new(),
            phase: MessagePhase::Unknown,
            kind: "tools".into(),
            text: String::new(),
            count: 28,
            seconds: 420,
        };
        assert_eq!(entry.summary(), "28 tools · 7m");
        entry.count = 1;
        entry.seconds = 0;
        assert_eq!(entry.summary(), "1 tool");
        entry.seconds = 1;
        assert_eq!(entry.summary(), "1 tool · 1s");
        entry.seconds = 65;
        assert_eq!(entry.summary(), "1 tool · 1m 5s");
        entry.seconds = 3600;
        assert_eq!(entry.summary(), "1 tool · 1h");
        entry.seconds = 3660;
        assert_eq!(entry.summary(), "1 tool · 1h 1m");
    }

    #[test]
    fn reads_follow_the_interval_and_a_forced_read_ignores_it() {
        let mut view = TranscriptView::new("mac", "s1");
        let argv = view.wanted(1000.0, false).unwrap();
        assert_eq!(argv, vec!["transcript", "mac", "s1", "--revision", ""]);
        let request = view.begin();
        assert!(view.wanted(1500.0, false).is_none());
        view.set(
            request,
            2000.0,
            Ok(r#"{"entries":[{"kind":"user","text":"hi"}],"revision":"r1"}"#.into()),
            true,
        );
        assert!(!view.loading);
        assert_eq!(view.revision, "r1");
        assert!(view.wanted(3999.0, true).is_none());
        assert!(view.wanted(4000.0, false).is_none());
        assert_eq!(
            view.wanted(4001.0, true).unwrap()[4],
            "r1",
            "a due read asks with its revision"
        );
        assert!(view.wanted(10_000.0, false).is_some());
        view.refresh_now();
        assert_eq!(view.wanted(4000.0, false).unwrap()[4], "");
        // A forced read while loading is kept for after the reply.
        let request = view.begin();
        view.refresh_now();
        assert!(view.pending_refresh);
        view.set(request, 4100.0, Err("boom".into()), true);
        assert_eq!(view.error.as_deref(), Some("boom"));
        assert_eq!(view.wanted(4100.0, false).unwrap()[4], "", "forced");
        // On an inactive tab the queued refresh only clears the interval.
        let request = view.begin();
        view.refresh_now();
        view.set(
            request,
            4150.0,
            Ok(r#"{"revision":"r2","unchanged":true}"#.into()),
            false,
        );
        assert_eq!(view.fetched_at, None);
        assert_eq!(view.wanted(4150.0, false).unwrap()[4], "r2", "not forced");
        // A stale reply is dropped.
        let request = view.begin();
        view.set(request - 1, 4200.0, Ok("{}".into()), true);
        assert!(view.loading);
        assert!(TranscriptView::default().wanted(0.0, false).is_none());
    }

    #[test]
    fn keys_scroll_compose_send_and_close() {
        let mut view = TranscriptView::new("m", "s");
        view.accept(Transcript {
            entries: vec![message("a"), message("b"), message("c")],
        });
        assert_eq!(view.key("Escape", &Mods::NONE), TranscriptKey::Close);
        assert_eq!(view.key("j", &Mods::NONE), TranscriptKey::Scrolled);
        assert_eq!(view.scroll_by, SCROLL_STEP);
        assert_eq!(view.key("ArrowUp", &Mods::NONE), TranscriptKey::Scrolled);
        assert_eq!(view.scroll_by, -SCROLL_STEP);
        assert_eq!(view.key("End", &Mods::NONE), TranscriptKey::Scrolled);
        assert_eq!(view.scroll_to, Some(2));
        assert_eq!(view.key("Home", &Mods::NONE), TranscriptKey::Scrolled);
        assert_eq!(view.scroll_to, Some(0));
        assert_eq!(view.scroll_seq, 4);
        assert_eq!(view.key("i", &Mods::CTRL), TranscriptKey::None);
        assert_eq!(view.key("i", &Mods::NONE), TranscriptKey::Compose);
        assert!(view.editing);
        assert_eq!(view.key("j", &Mods::NONE), TranscriptKey::None);
        assert_eq!(view.key("Enter", &Mods::SHIFT), TranscriptKey::None);
        assert_eq!(view.key("Enter", &Mods::NONE), TranscriptKey::Send);
        assert_eq!(view.key("Escape", &Mods::NONE), TranscriptKey::StopEditing);
        assert!(!view.editing);
        assert_eq!(view.key("Enter", &Mods::NONE), TranscriptKey::Compose);
    }

    #[test]
    fn sending_pastes_the_draft_then_shows_the_bubble() {
        let mut view = TranscriptView::new("m", "s");
        assert_eq!(view.send(true), Send::Nothing);
        view.input("  ");
        assert_eq!(view.send(true), Send::Nothing);
        view.input("fix it");
        assert!(view.can_send(true));
        assert!(!view.can_send(false));
        assert_eq!(view.send(false), Send::NotConnected);
        assert!(!view.sending);
        assert_eq!(view.send(true), Send::Paste("fix it".into()));
        assert!(view.sending);
        assert_eq!(view.send(true), Send::Nothing);
        assert_eq!(view.view(None, true)["sendLabel"], "Sending…");
        assert_eq!(view.sent("fix it", true, true, 1000.0), None);
        assert!(!view.sending);
        assert!(view.draft.is_empty());
        assert_eq!(view.data.as_ref().unwrap().entries[0].text, "fix it");
        view.begin();
        view.set(1, 1100.0, Ok(r#"{"entries":[]}"#.into()), true);
        assert!(view.wanted(1500.0, false).is_none());
        assert!(view.wanted(1800.0, false).is_some());
        // A failed send keeps the draft.
        view.input("again\nand\nmore");
        assert_eq!(view.view(None, true)["rows"], 3);
        view.send(true);
        assert_eq!(
            view.sent("again\nand\nmore", true, false, 2000.0),
            Some(NOT_SUBMITTED)
        );
        assert_eq!(view.draft, "again\nand\nmore");
        view.send(true);
        assert_eq!(view.sent("again\nand\nmore", false, false, 2000.0), None);
        assert!(!view.sending);
    }

    #[test]
    fn replies_not_status_ticks_refresh_the_transcript() {
        use crate::types::Session;
        let current = Session {
            id: "session".into(),
            state: "running".into(),
            status_text: "Working (1s)".into(),
            status_observed_at: 100,
            ..Default::default()
        };
        let mut incoming = current.clone();
        incoming.status_observed_at = 101;
        incoming.status_text = "Working (2s)".into();
        assert!(!reply_observed(&current, &incoming));
        incoming.last_message = "Done".into();
        assert!(reply_observed(&current, &incoming));
        incoming.last_message.clear();
        incoming.state = "idle".into();
        assert!(reply_observed(&current, &incoming));
        incoming.status_observed_at = 99;
        assert!(!reply_observed(&current, &incoming));
        incoming.status_observed_at = 102;
        incoming.id = "another session".into();
        assert!(!reply_observed(&current, &incoming));
    }

    #[test]
    fn view_names_kinds_states_and_the_live_indicator() {
        let mut view = TranscriptView::new("m", "s");
        assert_eq!(view.view(None, false)["empty"], "Loading conversation…");
        view.error = Some("x".into());
        let v = view.view(None, false);
        assert_eq!(v["empty"], "No conversation available.");
        assert_eq!(v["error"], "x");
        view.error = None;
        view.accept(Transcript::default());
        let v = view.view(Some(("claude", "")), true);
        assert_eq!(v["empty"], "No messages yet.");
        assert_eq!(v["working"], "Working…");
        let mut entries = vec![instructions(), message("Thinking")];
        entries.push(Entry {
            blocks: Vec::new(),
            phase: MessagePhase::Unknown,
            kind: "tools".into(),
            text: String::new(),
            count: 2,
            seconds: 65,
        });
        entries.push(message("Done"));
        view.accept(Transcript { entries });
        view.error = Some("late".into());
        let v = view.view(Some(("codex", "Reading files…")), true);
        assert_eq!(v["error"], "Couldn't refresh. late");
        let rows = v["entries"].as_array().unwrap();
        assert_eq!(rows[0]["kind"], "agents-md");
        assert_eq!(rows[0]["collapsed"], true);
        assert!(rows[0]["blocks"].as_array().unwrap().is_empty());
        assert_eq!(rows[0]["toggleId"], "transcript-toggle-0");
        assert_eq!(rows[1]["kind"], "assistant");
        assert_eq!(rows[1]["subdued"], true);
        assert_eq!(rows[2]["kind"], "tool-group");
        assert_eq!(rows[2]["summary"], "2 tools · 1m 5s");
        assert_eq!(rows[2]["live"], "Reading files… Called 2 tools.");
        assert_eq!(v["working"], "");
        assert_eq!(rows[3]["subdued"], false);
        view.toggle_instructions(0);
        let v = view.view(None, false);
        assert_eq!(v["entries"][0]["collapsed"], false);
        assert!(!v["entries"][0]["blocks"].as_array().unwrap().is_empty());
        assert_eq!(v["entries"][2]["live"], "");
        assert_eq!(v["canSend"], false);
        assert_eq!(v["placeholder"], PLACEHOLDER);
    }

    #[test]
    fn a_search_hit_scrolls_to_its_turn_once_rows_exist() {
        let mut view = TranscriptView::new("m", "s");
        view.reveal = Some(1);
        let request = view.begin();
        view.set(
            request,
            0.0,
            Ok(
                r#"{"entries":[{"kind":"user","text":"a"},{"kind":"assistant","text":"b"}]}"#
                    .into(),
            ),
            true,
        );
        assert_eq!(view.view(None, false)["scrollTo"], "entry-1");
        assert!(view.reveal.is_none());
    }
}
