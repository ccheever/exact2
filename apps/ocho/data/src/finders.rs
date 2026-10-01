//! The two session finders (workspace.rs `Overlay::PullRequests` and
//! `Overlay::Conversations`; ui.rs `render_pull_requests` and
//! `render_conversations`): the PR finder over what the feed already holds,
//! and the topic finder over `fleet search`. Both are `OVERLAY` pickers
//! (kind "picker", `PICKER_ROW` rows) with the keys of `PickerState`. Time
//! is host milliseconds; the topic finder's debounce and its index throttle
//! are due-at checks the model polls through `wanted_job`.
//!
//! PR finder rows: `chip` "#N", `detail` the repository, `label` the session
//! title, `hint` "{machine} · {state} · history|attachable". Topic finder
//! rows are two lines: `label` the title, `detail` "{machine} · {Provider} ·
//! history|attachable[ · N matching turns]", `second` the best turn
//! ("you: …" / "agent: …").

use crate::picker::{self, fuzzy_score, Mods, PickerKey, PickerState};
use crate::session::{clean, provider_label, session_state};
use crate::types::{Machine, PullRequest, Session};
use serde::Deserialize;
use serde_json::{json, Value as Json};
use std::collections::HashMap;

/// Conversation search waits for typing to settle (`CONVERSATION_DEBOUNCE`).
pub const DEBOUNCE_MS: f64 = 350.0;
/// Opening the topic finder re-indexes at most this often (`INDEX_INTERVAL`).
pub const INDEX_INTERVAL_MS: f64 = 10.0 * 60.0 * 1000.0;
/// Hits asked for per search.
pub const SEARCH_LIMIT: usize = 12;
/// The PR finder's card width.
pub const PULL_REQUESTS_WIDTH: f64 = 720.0;
/// The topic finder's card width.
pub const CONVERSATIONS_WIDTH: f64 = 720.0;
/// The PR finder's placeholder.
pub const PULL_REQUESTS_PLACEHOLDER: &str = "PR number, repository, or session title…";
/// The topic finder's placeholder.
pub const CONVERSATIONS_PLACEHOLDER: &str = "Search past conversations…";
/// The PR finder with nothing observed yet.
pub const NO_PULL_REQUESTS: &str =
    "No pull requests observed yet. Ocho records GitHub PR links that agents print or report.";
/// The PR finder with a query nothing matches.
pub const NO_PULL_REQUEST_MATCH: &str = "No sessions match this pull request";
/// The topic finder's intro.
pub const CONVERSATIONS_INTRO: &str = "Describe the work, like \"where I added login redirects\". Titles and paths match like /, exact words rank next, and related wording fills in the rest.";
/// The topic finder while a search runs.
pub const SEARCHING: &str = "Searching…";
/// The topic finder with a query nothing matches.
pub const NO_CONVERSATIONS: &str = "No sessions match. Try other words, or index first.";
/// The footer while indexing.
pub const INDEXING: &str = "Indexing new conversations…";

/// A full `OVERLAY` picker card, the rest zero.
#[allow(clippy::too_many_arguments)]
fn overlay(
    width: f64,
    glyph: &str,
    placeholder: &str,
    query: &str,
    status: &str,
    status_error: bool,
    rows: Vec<Json>,
    index: usize,
    footer: &str,
) -> Json {
    json!({
        "kind": "picker", "width": width, "top": true, "title": "", "subtitle": "", "pill": "", "pillColor": "",
        "glyph": glyph, "placeholder": placeholder, "query": query, "status": status, "statusError": status_error,
        "rows": rows, "index": index, "footer": footer, "body": "", "bodyMarkdown": false, "blocks": [], "fields": [],
        "buttons": [], "hint": "", "focusId": "overlay-query-input",
    })
}

// ----- pull requests -------------------------------------------------------------

/// One (pull request, session) pair (workspace.rs `PullRequestEntry`).
#[derive(Clone, Debug, PartialEq)]
pub struct PullRequestEntry {
    /// The pull request.
    pub pull_request: PullRequest,
    /// The machine the session ran on.
    pub machine_id: String,
    /// Its name.
    pub machine_name: String,
    /// The session that made or reported the request.
    pub session: Session,
}

impl PullRequestEntry {
    /// The right-hand hint: "{machine} · {state} · history|attachable".
    pub fn detail(&self) -> String {
        format!(
            "{} · {} · {}",
            self.machine_name,
            session_state(&self.session).to_lowercase(),
            if self.session.can_attach() {
                "attachable"
            } else {
                "history"
            }
        )
    }
}

/// Every (pull request, session) pair Ocho knows about, newest request
/// first (then repository, then the newer session). Hidden, archived,
/// untracked and historical sessions count: the session that made a request
/// is often finished by the time it is opened.
pub fn pull_request_entries(machines: &[Machine]) -> Vec<PullRequestEntry> {
    let mut rows: Vec<(&Machine, &Session, &PullRequest)> = Vec::new();
    for machine in machines {
        let Some(last) = &machine.last else { continue };
        for session in &last.sessions {
            for pull_request in &session.pull_requests {
                rows.push((machine, session, pull_request));
            }
        }
    }
    rows.sort_by(|(_, a_session, a), (_, b_session, b)| {
        b.number
            .cmp(&a.number)
            .then_with(|| a.repo.cmp(&b.repo))
            .then_with(|| {
                b_session
                    .started_epoch
                    .partial_cmp(&a_session.started_epoch)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    rows.into_iter()
        .map(|(machine, session, pull_request)| PullRequestEntry {
            pull_request: pull_request.clone(),
            machine_id: machine.id.clone(),
            machine_name: machine.name.clone(),
            session: session.clone(),
        })
        .collect()
}

/// Entries matching a picker query. A number (with or without "#" or "PR")
/// selects by request number, exact matches before longer ones; anything
/// else fuzzy matches the label, repository, title and machine.
pub fn filter_pull_requests(entries: Vec<PullRequestEntry>, query: &str) -> Vec<PullRequestEntry> {
    let query = query.trim();
    if query.is_empty() {
        return entries;
    }
    let digits = query.trim_start_matches(['#', 'p', 'r', 'P', 'R', ' ']);
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        let mut exact = Vec::new();
        let mut prefix = Vec::new();
        for entry in entries {
            let number = entry.pull_request.number.to_string();
            if number == digits {
                exact.push(entry);
            } else if number.starts_with(digits) {
                prefix.push(entry);
            }
        }
        exact.extend(prefix);
        return exact;
    }
    let mut scored: Vec<(PullRequestEntry, i32)> = entries
        .into_iter()
        .filter_map(|entry| {
            let haystack = format!(
                "{} {}#{} {} {}",
                entry.pull_request.label(),
                entry.pull_request.repo,
                entry.pull_request.number,
                entry.session.title,
                entry.machine_name
            );
            fuzzy_score(query, &haystack).map(|score| (entry, score))
        })
        .collect();
    scored.sort_by_key(|(_, score)| *score);
    scored.into_iter().map(|(entry, _)| entry).collect()
}

/// How a chosen session opens (workspace.rs `open_session_item`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenSession {
    /// It still has a terminal: attach.
    Attach,
    /// Reveal it in the Sessions list so `o` can resume it, turning on the
    /// "untracked & archived" and "history" views as needed.
    Reveal {
        /// Turn on View ▾ "untracked & archived".
        all_sessions: bool,
        /// Turn on View ▾ "history".
        history: bool,
    },
}

/// How to open a session (workspace.rs `open_session_item`): attach when it
/// can be attached (a terminal, or a live EAS conversation), else reveal it.
pub fn open_session(session: &Session) -> OpenSession {
    if session.can_attach() {
        return OpenSession::Attach;
    }
    OpenSession::Reveal {
        all_sessions: session.archived || (!session.managed && !session.tracked),
        history: session.historical,
    }
}

/// The toast when a chosen session is revealed rather than attached
/// (workspace.rs `open_session_item`).
pub fn reveal_message(title: &str) -> String {
    format!(
        "{} has no attachable terminal. Press o to resume its saved history.",
        clean(title)
    )
}

/// What Enter in the PR finder asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct PullRequestChoice {
    /// The entry chosen.
    pub entry: PullRequestEntry,
    /// The session was hidden: run `fleet unhide M S` first, with the toast
    /// "Session restored · press o to resume its saved history".
    pub unhide: bool,
    /// Then attach or reveal.
    pub open: OpenSession,
}

/// The toast after restoring a hidden PR session.
pub const UNHIDE_DONE: &str = "Session restored · press o to resume its saved history";

/// What a finder key did.
#[derive(Clone, Debug, PartialEq)]
pub enum FinderKey<T> {
    /// The highlight moved.
    Moved,
    /// Enter on a row.
    Chosen(T),
    /// Enter with no rows (the topic finder searches at once).
    Search,
    /// Esc.
    Closed,
    /// Printable text the query input holds (`input` brings the value).
    Typed(String),
    /// Nothing.
    None,
}

/// The PR finder (⌘⌥P).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PullRequestFinder {
    /// The query and highlight.
    pub picker: PickerState,
    /// Every pair, newest first.
    pub entries: Vec<PullRequestEntry>,
}

impl PullRequestFinder {
    /// Open over the fleet's machines.
    pub fn open(machines: &[Machine]) -> Self {
        PullRequestFinder {
            picker: PickerState::new(),
            entries: pull_request_entries(machines),
        }
    }

    /// The feed moved: reread the pairs, keeping the query and highlight.
    pub fn refresh(&mut self, machines: &[Machine]) {
        self.entries = pull_request_entries(machines);
        let count = self.matches().len();
        self.picker.select(self.picker.index, count);
    }

    /// The entries the query matches.
    pub fn matches(&self) -> Vec<PullRequestEntry> {
        filter_pull_requests(self.entries.clone(), &self.picker.query)
    }

    /// The query changed.
    pub fn input(&mut self, text: &str) {
        self.picker.query = text.to_string();
        let count = self.matches().len();
        self.picker.select(0, count);
    }

    /// The highlighted entry, and how to open it.
    pub fn choice(&self) -> Option<PullRequestChoice> {
        let entry = self.matches().get(self.picker.index).cloned()?;
        Some(PullRequestChoice {
            unhide: entry.session.hidden,
            open: open_session(&entry.session),
            entry,
        })
    }

    /// One key.
    pub fn key(&mut self, name: &str, mods: &Mods) -> FinderKey<PullRequestChoice> {
        let count = self.matches().len();
        match self.picker.key(name, mods, count) {
            PickerKey::Moved => FinderKey::Moved,
            PickerKey::Chosen => match self.choice() {
                Some(choice) => FinderKey::Chosen(choice),
                None => FinderKey::None,
            },
            PickerKey::Closed => FinderKey::Closed,
            PickerKey::Typed(text) => FinderKey::Typed(text),
            PickerKey::None => FinderKey::None,
        }
    }

    /// A press on row `pull-request-N`: the entry at N.
    pub fn press(&mut self, id: &str) -> Option<PullRequestChoice> {
        let index: usize = id.strip_prefix("pull-request-")?.parse().ok()?;
        let count = self.matches().len();
        self.picker.select(index, count);
        self.choice()
    }

    /// The `OVERLAY` card.
    pub fn view(&self) -> Json {
        let entries = self.matches();
        let rows: Vec<Json> = entries
            .iter()
            .enumerate()
            .map(|(position, entry)| {
                let mut row = picker::row(
                    &format!("pull-request-{position}"),
                    &entry.pull_request.label(),
                    &clean(&entry.session.title),
                    &entry.pull_request.repo,
                    &entry.detail(),
                    position == self.picker.index,
                );
                picker::set(&mut row, "accent", json!(true));
                row
            })
            .collect();
        let status = if entries.is_empty() {
            if self.picker.query.trim().is_empty() {
                NO_PULL_REQUESTS
            } else {
                NO_PULL_REQUEST_MATCH
            }
        } else {
            ""
        };
        overlay(
            PULL_REQUESTS_WIDTH,
            "#",
            PULL_REQUESTS_PLACEHOLDER,
            &self.picker.query,
            status,
            false,
            rows,
            self.picker.index,
            "",
        )
    }
}

// ----- conversations -------------------------------------------------------------

fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// One session ranked by conversation search (backend.rs `SearchHit`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct SearchHit {
    /// The machine id.
    #[serde(default)]
    pub machine: String,
    /// The machine's name.
    #[serde(default)]
    pub machine_name: String,
    /// The session id.
    #[serde(default)]
    pub session: String,
    /// The session's title.
    #[serde(default)]
    pub title: String,
    /// Its folder.
    #[serde(default)]
    pub cwd: String,
    /// Its provider.
    #[serde(default)]
    pub provider: String,
    /// Its account.
    #[serde(default)]
    pub account: String,
    /// The rank.
    #[serde(default)]
    pub score: f64,
    /// How many turns matched.
    #[serde(default)]
    pub matches: i32,
    /// The best turn's index.
    #[serde(default)]
    pub turn: usize,
    /// Who said it: `user` or the agent.
    #[serde(default)]
    pub role: String,
    /// The best turn's text.
    #[serde(default)]
    pub snippet: String,
    /// When the session last changed.
    #[serde(default)]
    pub updated: String,
    /// The session is finished.
    #[serde(default)]
    pub historical: bool,
    /// Its terminal, when it still has one.
    #[serde(default)]
    pub tmux_pane: String,
}

/// `fleet search index --quiet`'s report.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct SearchIndexReport {
    /// Sessions seen.
    #[serde(default)]
    pub sessions: i32,
    /// Sessions with new turns.
    #[serde(default)]
    pub indexed: i32,
    /// New turns embedded.
    #[serde(default)]
    pub chunks: i32,
    /// Sessions skipped.
    #[serde(default)]
    pub skipped: i32,
    /// Sessions that could not be read, by id.
    #[serde(default, deserialize_with = "null_default")]
    pub errors: HashMap<String, String>,
    /// How long it took.
    #[serde(default)]
    pub elapsed: String,
}

/// Row detail for a hit: where it ran, how it opens, and match breadth.
pub fn conversation_detail(hit: &SearchHit) -> String {
    let mut parts = vec![
        hit.machine_name.clone(),
        provider_label(&hit.provider).to_string(),
        if hit.tmux_pane.is_empty() {
            "history".to_string()
        } else {
            "attachable".to_string()
        },
    ];
    if hit.matches > 1 {
        parts.push(format!("{} matching turns", hit.matches));
    }
    parts.join(" · ")
}

/// The best matching turn, attributed to whoever said it.
pub fn conversation_snippet(hit: &SearchHit) -> String {
    let who = if hit.role == "user" { "you" } else { "agent" };
    if hit.snippet.is_empty() {
        format!("{who}, turn {}", hit.turn + 1)
    } else {
        let words: Vec<&str> = hit.snippet.split_whitespace().collect();
        format!("{who}: {}", clean(&words.join(" ")))
    }
}

/// The footer after an index run.
pub fn index_note(report: &SearchIndexReport) -> String {
    let mut note = if report.indexed == 0 {
        "Conversation index is up to date".to_string()
    } else {
        format!(
            "Indexed {} of {} sessions ({} new turns)",
            report.indexed, report.sessions, report.chunks
        )
    };
    if !report.errors.is_empty() {
        note.push_str(&format!(" · {} could not be read", report.errors.len()));
    }
    note
}

/// Search failures that have a known fix get a sentence the user can act on.
pub fn search_error(error: &str) -> String {
    if error.contains("account service is not ready") {
        "Ocho is setting up shared accounts. Conversation search is unavailable for now."
            .to_string()
    } else if error.contains("not deployed") {
        "The account service was deployed without conversation search; see cloud/README.md."
            .to_string()
    } else {
        error.to_string()
    }
}

/// Which `fleet search` job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TopicJobKind {
    /// `search index --quiet`.
    Index,
    /// `search query --limit N -- TEXT`.
    Search,
}

/// A `fleet search` job the finder wants run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopicJob {
    /// Which.
    pub kind: TopicJobKind,
    /// The arguments after `fleet`.
    pub argv: Vec<String>,
    /// The search's revision: a reply for an older one is ignored.
    pub revision: u64,
    /// The query the search answers.
    pub query: String,
}

/// What Enter in the topic finder asks for: attach and show the transcript
/// at the turn, or reveal a finished session in the list.
#[derive(Clone, Debug, PartialEq)]
pub struct ConversationChoice {
    /// The hit.
    pub hit: SearchHit,
    /// Attach (then transcript mode at `hit.turn`) or reveal.
    pub open: OpenSession,
}

/// The toast when a hit's session is no longer in the sessions view.
pub fn conversation_gone(hit: &SearchHit) -> String {
    format!(
        "{} is no longer in the sessions view; refresh and search again",
        clean(&hit.title)
    )
}

/// The topic finder (⌘⇧F) and its search state (workspace.rs
/// `ConversationSearch`); the state outlives the overlay so answers land
/// while the query keeps changing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TopicFinder {
    /// The query and highlight.
    pub picker: PickerState,
    /// The hits for `searched`.
    pub hits: Vec<SearchHit>,
    /// The query the hits answer.
    pub searched: String,
    /// A search is in flight.
    pub loading: bool,
    /// The last failure.
    pub error: Option<String>,
    /// An index run is in flight.
    pub indexing: bool,
    /// When the last index run landed, host ms.
    pub indexed_at: Option<f64>,
    /// The footer.
    pub index_note: String,
    revision: u64,
    search_at: Option<f64>,
    index_wanted: bool,
}

impl TopicFinder {
    /// Open the finder: the query resets, and indexing runs unless it ran
    /// within `INDEX_INTERVAL_MS` (opening is the opt-in; nothing is
    /// embedded until then).
    pub fn open(&mut self, now_ms: f64) {
        self.picker = PickerState::new();
        self.error = None;
        self.search_at = None;
        self.index_conversations(false, now_ms);
    }

    /// Ask for an index run: automatic runs are throttled, the explicit
    /// palette command always runs.
    pub fn index_conversations(&mut self, explicit: bool, now_ms: f64) {
        if self.indexing
            || (!explicit
                && self
                    .indexed_at
                    .is_some_and(|at| now_ms - at < INDEX_INTERVAL_MS))
        {
            return;
        }
        self.index_wanted = true;
    }

    /// The finder closed: a search in flight is dropped.
    pub fn close(&mut self) {
        self.revision += 1;
        self.loading = false;
        self.search_at = None;
    }

    /// The query changed: search once typing settles.
    pub fn input(&mut self, text: &str, now_ms: f64) {
        self.picker.query = text.to_string();
        self.search_conversations(now_ms + DEBOUNCE_MS, now_ms);
    }

    /// Ask for a search at `at` (now for Enter or a finished index); an
    /// empty query clears the hits instead.
    fn search_conversations(&mut self, at: f64, _now_ms: f64) {
        let query = self.picker.query.trim().to_string();
        self.revision += 1;
        if query.is_empty() {
            self.hits.clear();
            self.searched.clear();
            self.loading = false;
            self.error = None;
            self.search_at = None;
            return;
        }
        self.loading = true;
        self.search_at = Some(at);
    }

    /// The job to run now, if any: an index run first, then a due search.
    /// Call `begin` once it is queued.
    pub fn wanted_job(&self, now_ms: f64) -> Option<TopicJob> {
        if self.index_wanted && !self.indexing {
            return Some(TopicJob {
                kind: TopicJobKind::Index,
                argv: vec!["search".into(), "index".into(), "--quiet".into()],
                revision: self.revision,
                query: String::new(),
            });
        }
        if self.search_at.is_some_and(|at| at <= now_ms) {
            let query = self.picker.query.trim().to_string();
            return Some(TopicJob {
                kind: TopicJobKind::Search,
                argv: vec![
                    "search".into(),
                    "query".into(),
                    "--limit".into(),
                    SEARCH_LIMIT.to_string(),
                    "--".into(),
                    query.clone(),
                ],
                revision: self.revision,
                query,
            });
        }
        None
    }

    /// The job `wanted_job` gave was queued.
    pub fn begin(&mut self, job: &TopicJob) {
        match job.kind {
            TopicJobKind::Index => {
                self.index_wanted = false;
                self.indexing = true;
                self.index_note = INDEXING.into();
            }
            TopicJobKind::Search => {
                self.search_at = None;
                self.loading = true;
            }
        }
    }

    /// A job's reply. An index reply returns its note when the run was
    /// explicit and should be toasted (`Ok`) or its error (`Err`).
    pub fn set(
        &mut self,
        job: &TopicJob,
        result: Result<String, String>,
        now_ms: f64,
    ) -> Result<String, String> {
        match job.kind {
            TopicJobKind::Index => {
                self.indexing = false;
                match result.and_then(|out| {
                    serde_json::from_str::<SearchIndexReport>(&out)
                        .map_err(|e| format!("invalid index report: {e}"))
                }) {
                    Ok(report) => {
                        let note = index_note(&report);
                        self.indexed_at = Some(now_ms);
                        self.index_note = note.clone();
                        // Newly embedded turns can change the answer to the open query.
                        self.search_conversations(now_ms, now_ms);
                        Ok(note)
                    }
                    Err(error) => {
                        let error = search_error(&error);
                        self.index_note = String::new();
                        self.error = Some(error.clone());
                        Err(error)
                    }
                }
            }
            TopicJobKind::Search => {
                if job.revision != self.revision {
                    return Ok(String::new());
                }
                self.loading = false;
                match result.and_then(|out| {
                    serde_json::from_str::<Vec<SearchHit>>(&out)
                        .map_err(|e| format!("invalid search results: {e}"))
                }) {
                    Ok(hits) => {
                        self.hits = hits;
                        self.searched = job.query.clone();
                        self.error = None;
                    }
                    Err(error) => {
                        self.hits.clear();
                        self.error = Some(search_error(&error));
                    }
                }
                self.picker.select(0, self.hits.len());
                Ok(String::new())
            }
        }
    }

    /// The highlighted hit, and how to open it as far as the hit tells:
    /// the desktop finds the session and decides with [`open_session`],
    /// which also attaches a pane-less EAS conversation.
    pub fn choice(&self) -> Option<ConversationChoice> {
        let hit = self.hits.get(self.picker.index).cloned()?;
        let open = if hit.tmux_pane.is_empty() {
            OpenSession::Reveal {
                all_sessions: false,
                history: hit.historical,
            }
        } else {
            OpenSession::Attach
        };
        Some(ConversationChoice { hit, open })
    }

    /// One key: Enter with no hits and no search running searches at once.
    pub fn key(&mut self, name: &str, mods: &Mods, now_ms: f64) -> FinderKey<ConversationChoice> {
        let count = self.hits.len();
        match self.picker.key(name, mods, count) {
            PickerKey::Moved => FinderKey::Moved,
            PickerKey::Chosen if count == 0 && !self.loading => {
                self.search_conversations(now_ms, now_ms);
                FinderKey::Search
            }
            PickerKey::Chosen => match self.choice() {
                Some(choice) => FinderKey::Chosen(choice),
                None => FinderKey::None,
            },
            PickerKey::Closed => {
                self.close();
                FinderKey::Closed
            }
            PickerKey::Typed(text) => FinderKey::Typed(text),
            PickerKey::None => FinderKey::None,
        }
    }

    /// A press on row `conversation-N`: the hit at N.
    pub fn press(&mut self, id: &str) -> Option<ConversationChoice> {
        let index: usize = id.strip_prefix("conversation-")?.parse().ok()?;
        self.picker.select(index, self.hits.len());
        self.choice()
    }

    /// The `OVERLAY` card.
    pub fn view(&self) -> Json {
        let query = self.picker.query.trim();
        let (status, status_error) = if let Some(error) = &self.error {
            (error.as_str(), true)
        } else if self.loading {
            (SEARCHING, false)
        } else if query.is_empty() {
            (CONVERSATIONS_INTRO, false)
        } else if self.hits.is_empty() && self.searched == query {
            (NO_CONVERSATIONS, false)
        } else {
            ("", false)
        };
        let rows: Vec<Json> = self
            .hits
            .iter()
            .enumerate()
            .map(|(position, hit)| {
                let mut row = picker::row(
                    &format!("conversation-{position}"),
                    "",
                    &clean(&hit.title),
                    &conversation_detail(hit),
                    "",
                    position == self.picker.index,
                );
                picker::set(&mut row, "second", json!(conversation_snippet(hit)));
                row
            })
            .collect();
        let footer = if self.indexing || !self.index_note.is_empty() {
            self.index_note.as_str()
        } else {
            ""
        };
        overlay(
            CONVERSATIONS_WIDTH,
            "?",
            CONVERSATIONS_PLACEHOLDER,
            &self.picker.query,
            status,
            status_error,
            rows,
            self.picker.index,
            footer,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Snapshot;

    fn session(id: &str, title: &str, pane: &str, prs: &[(&str, i64)], started: f64) -> Session {
        Session {
            id: id.into(),
            title: title.into(),
            tmux_pane: pane.into(),
            state: "idle".into(),
            pid: if pane.is_empty() { 0 } else { 7 },
            started_epoch: started,
            pull_requests: prs
                .iter()
                .map(|(repo, number)| PullRequest {
                    url: format!("https://github.com/{repo}/pull/{number}"),
                    repo: (*repo).into(),
                    number: *number,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn machines() -> Vec<Machine> {
        vec![
            Machine {
                id: "mac".into(),
                name: "Mac".into(),
                last: Some(Snapshot {
                    sessions: vec![
                        session("s1", "Fix login", "%1", &[("acme/web", 41)], 10.0),
                        session("s2", "Docs", "", &[("acme/web", 41), ("acme/api", 7)], 20.0),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            Machine {
                id: "box".into(),
                name: "Box".into(),
                last: Some(Snapshot {
                    sessions: vec![session("s3", "Release", "%2", &[("acme/cli", 410)], 5.0)],
                    ..Default::default()
                }),
                ..Default::default()
            },
        ]
    }

    #[test]
    fn pull_request_pairs_are_newest_first_and_filter_by_number_or_text() {
        let entries = pull_request_entries(&machines());
        let numbers: Vec<(i64, &str)> = entries
            .iter()
            .map(|e| (e.pull_request.number, e.session.id.as_str()))
            .collect();
        assert_eq!(
            numbers,
            vec![(410, "s3"), (41, "s2"), (41, "s1"), (7, "s2")]
        );
        let by_number = filter_pull_requests(entries.clone(), "#41");
        assert_eq!(
            by_number
                .iter()
                .map(|e| e.pull_request.number)
                .collect::<Vec<_>>(),
            vec![41, 41, 410]
        );
        assert_eq!(filter_pull_requests(entries.clone(), "PR 7").len(), 1);
        let by_text = filter_pull_requests(entries.clone(), "release");
        assert_eq!(by_text.len(), 1);
        assert_eq!(by_text[0].session.id, "s3");
        assert_eq!(filter_pull_requests(entries.clone(), "api").len(), 1);
        assert_eq!(filter_pull_requests(entries, "zzz").len(), 0);
    }

    #[test]
    fn pull_request_finder_rows_keys_and_choice() {
        let mut finder = PullRequestFinder::open(&machines());
        let v = finder.view();
        assert_eq!(v["kind"], "picker");
        assert_eq!(v["glyph"], "#");
        assert_eq!(v["placeholder"], PULL_REQUESTS_PLACEHOLDER);
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0]["chip"], "#410");
        assert_eq!(rows[0]["detail"], "acme/cli");
        assert_eq!(rows[0]["label"], "Release");
        assert_eq!(rows[0]["hint"], "Box · idle · attachable");
        assert_eq!(rows[1]["hint"], "Mac · closed · history");
        assert_eq!(rows[0]["selected"], true);
        assert_eq!(finder.key("ArrowDown", &Mods::NONE), FinderKey::Moved);
        let FinderKey::Chosen(choice) = finder.key("Enter", &Mods::NONE) else {
            panic!("enter chooses");
        };
        assert_eq!(choice.entry.session.id, "s2");
        assert_eq!(
            choice.open,
            OpenSession::Reveal {
                all_sessions: true,
                history: false
            }
        );
        assert!(!choice.unhide);
        assert_eq!(finder.key("Escape", &Mods::NONE), FinderKey::Closed);
        finder.input("nothing here");
        assert_eq!(finder.view()["status"], NO_PULL_REQUEST_MATCH);
        assert_eq!(finder.key("Enter", &Mods::NONE), FinderKey::None);
        finder.input("41");
        let choice = finder.press("pull-request-2").unwrap();
        assert_eq!(choice.entry.pull_request.number, 410);
        assert_eq!(choice.open, OpenSession::Attach);
        let empty = PullRequestFinder::open(&[]);
        assert_eq!(empty.view()["status"], NO_PULL_REQUESTS);
        let mut hidden = machines();
        hidden[0].last.as_mut().unwrap().sessions[0].hidden = true;
        finder.refresh(&hidden);
        finder.input("login");
        assert!(finder.choice().unwrap().unhide);

        // An EAS conversation has no pane but attaches all the same.
        let mut cloud = machines();
        let docs = &mut cloud[0].last.as_mut().unwrap().sessions[1];
        docs.eas = true;
        docs.native_id = "thread".into();
        let entry = pull_request_entries(&cloud)
            .into_iter()
            .find(|e| e.pull_request.number == 7)
            .unwrap();
        assert_eq!(entry.detail(), "Mac · closed · attachable");
        assert_eq!(open_session(&entry.session), OpenSession::Attach);
        cloud[0].last.as_mut().unwrap().sessions[1].historical = true;
        let entry = pull_request_entries(&cloud)
            .into_iter()
            .find(|e| e.pull_request.number == 7)
            .unwrap();
        assert_eq!(entry.detail(), "Mac · closed · history");
        assert_eq!(
            reveal_message("Docs\u{7}"),
            "Docs has no attachable terminal. Press o to resume its saved history."
        );
    }

    fn hit(title: &str, pane: &str) -> SearchHit {
        SearchHit {
            machine: "mac".into(),
            machine_name: "Mac".into(),
            session: "s1".into(),
            title: title.into(),
            provider: "claude".into(),
            matches: 3,
            turn: 4,
            role: "user".into(),
            snippet: "  where I   added\nlogin redirects ".into(),
            tmux_pane: pane.into(),
            ..Default::default()
        }
    }

    #[test]
    fn hit_rows_read_like_the_desktop() {
        let h = hit("Login", "%1");
        assert_eq!(
            conversation_detail(&h),
            "Mac · Claude · attachable · 3 matching turns"
        );
        assert_eq!(
            conversation_snippet(&h),
            "you: where I added login redirects"
        );
        let mut quiet = hit("x", "");
        quiet.matches = 1;
        quiet.role = "assistant".into();
        quiet.snippet.clear();
        assert_eq!(conversation_detail(&quiet), "Mac · Claude · history");
        assert_eq!(conversation_snippet(&quiet), "agent, turn 5");
        let mut report = SearchIndexReport::default();
        assert_eq!(index_note(&report), "Conversation index is up to date");
        report.indexed = 2;
        report.sessions = 9;
        report.chunks = 40;
        report.errors.insert("s".into(), "gone".into());
        assert_eq!(
            index_note(&report),
            "Indexed 2 of 9 sessions (40 new turns) · 1 could not be read"
        );
        assert_eq!(
            search_error("Ocho account service is not ready; Ocho sets it up automatically"),
            "Ocho is setting up shared accounts. Conversation search is unavailable for now."
        );
        assert_eq!(
            search_error("run fleet cloud connect first"),
            "run fleet cloud connect first"
        );
        assert!(search_error("search not deployed").starts_with("The account service"));
        assert_eq!(search_error("boom"), "boom");
    }

    #[test]
    fn topic_finder_indexes_on_open_then_debounces_searches() {
        let mut finder = TopicFinder::default();
        finder.open(1000.0);
        let job = finder.wanted_job(1000.0).unwrap();
        assert_eq!(job.kind, TopicJobKind::Index);
        assert_eq!(job.argv, vec!["search", "index", "--quiet"]);
        finder.begin(&job);
        assert_eq!(finder.view()["footer"], INDEXING);
        assert!(finder.wanted_job(1000.0).is_none());
        finder.input("login", 1200.0);
        assert!(finder.loading);
        assert_eq!(finder.view()["status"], SEARCHING);
        assert!(finder.wanted_job(1200.0 + DEBOUNCE_MS - 1.0).is_none());
        let search = finder.wanted_job(1200.0 + DEBOUNCE_MS).unwrap();
        assert_eq!(search.kind, TopicJobKind::Search);
        assert_eq!(
            search.argv,
            vec!["search", "query", "--limit", "12", "--", "login"]
        );
        finder.begin(&search);
        // Typing again supersedes the search in flight.
        finder.input("login redirects", 1600.0);
        assert_eq!(
            finder.set(&search, Ok("[]".into()), 1700.0),
            Ok(String::new())
        );
        assert!(finder.loading, "a stale reply changes nothing");
        let search = finder.wanted_job(2000.0).unwrap();
        finder.begin(&search);
        let hits =
            serde_json::to_string(&json!([{ "title": "Login", "tmux_pane": "%1", "turn": 2 }]))
                .unwrap();
        finder.set(&search, Ok(hits), 2100.0).unwrap();
        assert_eq!(finder.hits.len(), 1);
        assert_eq!(finder.searched, "login redirects");
        let v = finder.view();
        assert_eq!(v["status"], "");
        assert_eq!(v["rows"][0]["label"], "Login");
        assert_eq!(v["rows"][0]["second"], "agent, turn 3");
        assert_eq!(v["glyph"], "?");
        // The index reply lands: the note shows and the query runs again.
        let note = finder
            .set(&job, Ok(r#"{"sessions":3,"indexed":0}"#.into()), 2200.0)
            .unwrap();
        assert_eq!(note, "Conversation index is up to date");
        assert_eq!(finder.indexed_at, Some(2200.0));
        assert_eq!(
            finder.wanted_job(2200.0).unwrap().kind,
            TopicJobKind::Search
        );
        // Reopening within ten minutes does not index again.
        finder.open(3000.0);
        assert!(finder.wanted_job(3000.0).is_none());
        finder.open(3000.0 + INDEX_INTERVAL_MS);
        assert_eq!(
            finder.wanted_job(3000.0 + INDEX_INTERVAL_MS).unwrap().kind,
            TopicJobKind::Index
        );
        // The explicit command always runs, and an index failure is reported.
        let mut other = TopicFinder {
            indexed_at: Some(0.0),
            ..Default::default()
        };
        other.index_conversations(true, 1.0);
        let job = other.wanted_job(1.0).unwrap();
        other.begin(&job);
        assert!(other
            .set(&job, Err("fleet cloud connect".into()), 2.0)
            .is_err());
        assert!(other.view()["statusError"].as_bool().unwrap());
    }

    #[test]
    fn topic_finder_keys_search_choose_and_close() {
        let mut finder = TopicFinder::default();
        finder.open(0.0);
        assert_eq!(finder.view()["status"], CONVERSATIONS_INTRO);
        finder.input("x", 0.0);
        finder.begin(&finder.wanted_job(DEBOUNCE_MS).unwrap());
        let search = TopicJob {
            kind: TopicJobKind::Search,
            argv: Vec::new(),
            revision: finder.revision,
            query: "x".into(),
        };
        finder.set(&search, Ok("[]".into()), 500.0).unwrap();
        assert_eq!(finder.view()["status"], NO_CONVERSATIONS);
        assert_eq!(finder.key("Enter", &Mods::NONE, 600.0), FinderKey::Search);
        assert_eq!(finder.wanted_job(600.0).unwrap().kind, TopicJobKind::Search);
        finder.hits = vec![hit("A", ""), hit("B", "%2")];
        finder.loading = false;
        assert_eq!(
            finder.key("ArrowDown", &Mods::NONE, 700.0),
            FinderKey::Moved
        );
        let FinderKey::Chosen(choice) = finder.key("Enter", &Mods::NONE, 700.0) else {
            panic!("enter chooses");
        };
        assert_eq!(choice.hit.title, "B");
        assert_eq!(choice.open, OpenSession::Attach);
        let first = finder.press("conversation-0").unwrap();
        assert_eq!(
            first.open,
            OpenSession::Reveal {
                all_sessions: false,
                history: false
            }
        );
        assert_eq!(
            finder.key("j", &Mods::NONE, 700.0),
            FinderKey::Typed("j".into())
        );
        assert_eq!(finder.key("Escape", &Mods::NONE, 700.0), FinderKey::Closed);
        assert!(!finder.loading);
        assert!(finder.wanted_job(5000.0).is_none());
        assert_eq!(
            conversation_gone(&hit("Old\u{7}", "")),
            "Old is no longer in the sessions view; refresh and search again"
        );
    }
}
