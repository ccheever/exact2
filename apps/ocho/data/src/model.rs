//! The workspace: the GPUI desktop's `Workspace` (workspace.rs) without its
//! entities and windows. One per app; the host has one window (LLP 1030),
//! so there is one.

mod dialogs;
mod exec;
mod extras;
mod keys;
mod menus;
#[cfg(test)]
mod tests;

use crate::keymap::Keystroke;
use crate::palette::Command;
use crate::settings::DesktopSettings;
use crate::tab_tree::{TabDrop, TabTree};
use crate::theme::Theme;
use crate::types::{FleetEvent, Machine, Session, State};
use serde::Serialize;
use std::collections::HashMap;

pub use exec::{exec_confirm_texts, row_command, Overlay, SessionItem, UndoAction};
pub use menus::{Popup, PopupKind};

/// The manager's four pages, in rail order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub enum Page {
    /// Machines.
    Machines,
    /// Sessions, the page the app opens on.
    #[default]
    Sessions,
    /// Accounts.
    Accounts,
}

impl Page {
    /// Every page, in rail order.
    pub const ALL: [Page; 3] = [Page::Machines, Page::Sessions, Page::Accounts];

    /// Its place in [`Page::ALL`].
    pub fn index(self) -> usize {
        Page::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    /// The page at `i`, wrapping.
    pub fn from_index(i: usize) -> Page {
        Page::ALL[i % Page::ALL.len()]
    }

    /// The header's title.
    pub fn title(self) -> &'static str {
        match self {
            Page::Machines => "Machines",
            Page::Sessions => "Sessions",
            Page::Accounts => "Accounts",
        }
    }

    /// The Lucide icon beside the page in the rail.
    pub fn icon(self) -> &'static str {
        match self {
            Page::Machines => "monitor",
            Page::Sessions => "message-square",
            Page::Accounts => "key-round",
        }
    }

    /// The page's id in the contract.
    pub fn id(self) -> &'static str {
        match self {
            Page::Machines => "machines",
            Page::Sessions => "sessions",
            Page::Accounts => "accounts",
        }
    }

    /// The page an id names.
    pub fn from_id(id: &str) -> Option<Page> {
        Page::ALL.iter().copied().find(|p| p.id() == id)
    }
}

/// The part of the window that owns the keyboard (workspace.rs `Region`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Region {
    /// The rail, while NAV is on.
    Rail,
    /// The manager's list.
    Manager,
    /// The active tab's terminal.
    Terminal,
    /// The docked shell under the tab.
    Panel,
    /// The tab's transcript view.
    Transcript,
}

/// One thing the contract saw happen.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A key: the web's `KeyboardEvent.key` name, the modifiers held
    /// (`meta`, `shift`, `alt`, `ctrl` joined by `+`) and where it landed
    /// (`terminal`, `search`, `overlay`, or empty for the window).
    Key {
        /// The key's name.
        name: String,
        /// The modifiers held.
        mods: String,
        /// Where the key landed.
        at: String,
    },
    /// A press on the node with this id.
    Press(String),
    /// A press that opens a popup, with its trigger's top-left corner.
    PressAt(String, f64, f64),
    /// A double click on the node.
    DoubleClick(String),
    /// A secondary click on the node, at a window point.
    ContextMenu(String, f64, f64),
    /// The pointer entered (`true`) or left a node.
    Hover(String, bool),
    /// Text in an input changed.
    Input(String, String),
    /// A pan on a node moved by (dx, dy) since the last event.
    Pan(String, f64, f64),
    /// The host clock, in milliseconds, and the window's size.
    Tick(f64, f64, f64),
    /// Something the contract has no name for; ignored.
    Unknown,
}

impl Event {
    /// The event `dispatch(kind, a, b, x, y)` names.
    pub fn parse(kind: &str, a: &str, b: &str, x: f64, y: f64) -> Event {
        match kind {
            "key" => Event::Key {
                name: a.into(),
                mods: String::new(),
                at: String::new(),
            },
            "key-at" => Event::Key {
                name: a.into(),
                mods: String::new(),
                at: b.into(),
            },
            "press" => Event::Press(a.into()),
            "press-at" => Event::PressAt(a.into(), x, y),
            "dblclick" => Event::DoubleClick(a.into()),
            "contextmenu" => Event::ContextMenu(a.into(), x, y),
            "hover" => Event::Hover(a.into(), b == "true"),
            "input" => Event::Input(a.into(), b.into()),
            "pan" => Event::Pan(a.into(), x, y),
            "tick" => Event::Tick(a.parse().unwrap_or(0.0), x, y),
            _ => Event::Unknown,
        }
    }
}

/// A fleet command for the module to run: `fleet` plus `argv`, and how the
/// model wants the reply back.
#[derive(Clone, Debug, Serialize)]
pub struct Job {
    /// A token the reply carries.
    pub id: u64,
    /// The command's arguments after `fleet`.
    pub argv: Vec<String>,
    /// What the reply is for.
    pub kind: String,
    /// Standard input, when the command takes any.
    pub stdin: String,
}

/// What a reply does when it comes back (the closure a GPUI task held).
#[derive(Clone, Debug)]
pub enum Reply {
    /// Toast `done` on success, the error on failure, then a refresh.
    Cli {
        /// The success toast.
        done: String,
        /// Recorded on success.
        undo_on_success: Option<UndoAction>,
        /// Recorded on failure.
        undo_on_failure: Option<UndoAction>,
    },
    /// A session flag write: confirm or roll back the optimistic patch.
    Flag {
        /// (machine, session, flag).
        key: FlagKey,
        /// The write this reply answers.
        revision: u64,
        /// The success toast.
        done: String,
        /// Recorded on success.
        undo: Option<UndoAction>,
    },
    /// `fleet accounts usage A`: the meters for an account.
    Usage(String),
    /// `fleet models …`: a catalog for the launch dialog.
    Models(crate::launch::CatalogKey),
    /// `fleet search …`: the topic finder's search or index run.
    Topics(Box<crate::finders::TopicJob>),
    /// The theme files the module listed.
    ThemeFiles,
    /// The build's change history for What's New.
    WhatsNewHistory,
    /// `fleet serve --describe` for an iMessage pairing request.
    PairDescribe(u64),
    /// `fleet machine pair-imessage …` for a pairing request.
    Pairing(u64),
    /// `fleet directories M PATH`: folder completion for that machine and path.
    Directories(String, String),
    /// A host job; nothing to apply.
    Host,
}

/// (machine id, session id, flag name).
pub type FlagKey = (String, String, String);

/// An optimistic session flag write: (revision, previous, desired, committed).
pub type PendingFlag = (u64, bool, bool, bool);

/// The toast at the window's bottom right (ui.rs:1932).
#[derive(Clone, Debug, Default, Serialize)]
pub struct Toast {
    /// The text, empty for none.
    pub text: String,
    /// Shown as a warning.
    pub error: bool,
    /// Stays until dismissed.
    pub persistent: bool,
    /// When it was posted, host milliseconds.
    pub at: f64,
}

/// An account's usage meters as last loaded (workspace.rs `UsageLoad`).
#[derive(Clone, Debug, Default)]
pub struct UsageLoad {
    /// When it was loaded, host milliseconds.
    pub at: f64,
    /// The meters, `None` while loading.
    pub usage: Option<crate::types::ProviderUsage>,
    /// The load failed.
    pub error: bool,
}

/// The model.
#[derive(Debug)]
pub struct Workspace {
    /// Moves on every dispatch; the contract's `ui.version`.
    pub version: u64,
    /// Moves when jobs are queued; the contract's `ui.io`.
    pub io: u64,
    /// Moves when a feed event was applied.
    pub feed_version: u64,
    /// Moves when the desktop.json mirror was applied.
    pub desktop_version: u64,
    /// The first desktop.json text was applied (its windows restored).
    pub desktop_loaded: bool,
    /// The last desktop.json text could not be parsed; the file is not written.
    pub desktop_broken: bool,
    /// Moves when jobs' replies were applied.
    pub io_version: u64,
    /// The fleet, as the feed last showed it.
    pub state: State,
    /// Whether a first feed event arrived.
    pub loaded: bool,
    /// A refresh is in flight.
    pub loading: bool,
    /// The feed's account error, shown on the Accounts page.
    pub account_error: String,
    /// The feed's own failure (no binary, could not start).
    pub feed_failure: String,
    /// Newest provider versions the feed reported.
    pub latest_provider_versions: HashMap<String, String>,
    /// The theme in use.
    pub theme: Theme,
    /// `desktop.json` as last read, and the launch memory it carries.
    pub settings: DesktopSettings,
    /// The manager page.
    pub page: Page,
    /// The selected row on the page.
    pub index: usize,
    /// The search text.
    pub query: String,
    /// Whether the search box is being typed into.
    pub searching: bool,
    /// View ▾: untracked & archived.
    pub all_sessions: bool,
    /// View ▾: history.
    pub history: bool,
    /// View ▾: hide non-running.
    pub hide_non_running: bool,
    /// The rail has the keyboard.
    pub nav: bool,
    /// The rail's TERMINALS tree and which tab is active.
    pub tabs: TabTree,
    /// Keys of tabs whose terminal has exited.
    pub exited: std::collections::HashSet<String>,
    /// The drop target while a tab is dragged.
    pub tab_drop: Option<TabDrop>,
    /// The tab being dragged, and the pointer's y offset from its row's top.
    pub drag: Option<(usize, f64)>,
    /// Rail width in px.
    pub rail_width: f64,
    /// Rail resize drag in progress.
    pub rail_resizing: bool,
    /// The window's size, for clamps.
    pub window: (f64, f64),
    /// The one open menu or dropdown.
    pub popup: Option<Popup>,
    /// The one open overlay.
    pub overlay: Overlay,
    /// The toast.
    pub toast: Toast,
    /// The first key of a two-key sequence waiting for its second.
    pub pending_keys: Option<Keystroke>,
    /// The launch prompt typed when the dialog closed without launching.
    pub launch_draft: String,
    /// The session-title prompt (`titles.json`), as the module read it.
    pub title_prompt: String,
    /// The host clock at the last event, milliseconds.
    pub now: f64,
    /// The wall clock (seconds since the epoch) the module last reported, and
    /// the host clock then: the app's clock may be frozen or monotonic.
    pub wall: (f64, f64),
    /// Usage meters per account name.
    pub usage: HashMap<String, UsageLoad>,
    /// Optimistic session flag writes awaiting the CLI, by (machine, session, flag).
    pub pending_flags: HashMap<FlagKey, PendingFlag>,
    /// The undo stack (20 entries).
    pub undo_stack: Vec<UndoAction>,
    /// Commands in flight.
    pub busy: usize,
    jobs: Vec<Job>,
    replies: HashMap<u64, Reply>,
    next_job: u64,
    operation_revision: u64,
    /// The hovered node id (row actions brighten, the rail's close shows).
    pub hovered: Option<String>,
    /// The node to focus after this frame (an input in an overlay).
    pub focus_id: String,
    /// The list row to scroll into view after this frame.
    pub scroll_to: String,
    /// The window title last handed to the host.
    pub window_title: String,
    /// A key an input's own handler just took: its keydown bubbles to the
    /// window's handler next and must not run twice (a web keydown bubbles).
    bubbling_key: Option<String>,
    /// The sessions watched for notifications.
    pub notifications: crate::notifications::Notifications,
    /// Field values of forms closed without submitting, by kind.
    pub form_drafts: HashMap<crate::forms::FormKind, Vec<String>>,
    /// The topic finder (it keeps its index time between openings).
    pub topics: crate::finders::TopicFinder,
    /// Theme files the module found, parsed.
    pub theme_files: Vec<crate::themes::ThemeEntry>,
    /// What's New: the build's recent changes.
    pub whats_new: crate::whats_new::WhatsNew,
    /// The history was asked for.
    pub whats_new_asked: bool,
    /// The iMessage pairing card.
    pub imessage: crate::imessage_pair::ImessagePair,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace::new()
    }
}

impl Workspace {
    /// A workspace before any feed event: the GPUI app's `Workspace::new`.
    pub fn new() -> Workspace {
        Workspace {
            version: 0,
            io: 0,
            feed_version: 0,
            desktop_version: 0,
            desktop_loaded: false,
            desktop_broken: false,
            io_version: 0,
            state: State::default(),
            loaded: false,
            loading: true,
            account_error: String::new(),
            feed_failure: String::new(),
            latest_provider_versions: HashMap::new(),
            theme: Theme::ocho_dark(),
            settings: DesktopSettings::default(),
            page: Page::Sessions,
            index: 0,
            query: String::new(),
            searching: false,
            all_sessions: false,
            history: false,
            hide_non_running: false,
            nav: false,
            tabs: TabTree::new(),
            exited: Default::default(),
            tab_drop: None,
            drag: None,
            rail_width: 236.0,
            rail_resizing: false,
            window: (1180.0, 760.0),
            popup: None,
            overlay: Overlay::None,
            toast: Toast::default(),
            pending_keys: None,
            launch_draft: String::new(),
            title_prompt: String::new(),
            now: 0.0,
            wall: (0.0, 0.0),
            usage: HashMap::new(),
            pending_flags: HashMap::new(),
            undo_stack: Vec::new(),
            busy: 0,
            jobs: Vec::new(),
            replies: HashMap::new(),
            next_job: 0,
            operation_revision: 0,
            hovered: None,
            focus_id: String::new(),
            scroll_to: String::new(),
            window_title: String::new(),
            bubbling_key: None,
            notifications: Default::default(),
            form_drafts: HashMap::new(),
            topics: Default::default(),
            theme_files: Vec::new(),
            whats_new: Default::default(),
            whats_new_asked: false,
            imessage: Default::default(),
        }
    }

    /// Apply one event.
    pub fn dispatch(&mut self, event: Event) {
        self.version += 1;
        if !matches!(event, Event::Key { .. }) {
            self.bubbling_key = None;
        }
        self.focus_id.clear();
        self.scroll_to.clear();
        match event {
            Event::Tick(now, w, h) => self.tick(now, w, h),
            Event::Hover(id, on) => {
                if on {
                    self.hovered = Some(id);
                } else if self.hovered.as_deref() == Some(id.as_str()) {
                    self.hovered = None;
                }
            }
            Event::Press(id) => self.press(&id),
            Event::PressAt(id, x, y) => self.press_at(&id, x, y),
            Event::Key { name, mods, at } => {
                if at.is_empty() && self.bubbling_key.take().as_deref() == Some(name.as_str()) {
                    // The same keydown, bubbled from the input that handled it.
                } else {
                    if !at.is_empty() {
                        self.bubbling_key = Some(name.clone());
                    }
                    self.key(&Keystroke::new(&name, &mods), &at)
                }
            }
            Event::Input(id, value) => self.input(&id, &value),
            Event::Pan(id, dx, dy) => self.pan(&id, dx, dy),
            Event::DoubleClick(id) => self.double_click(&id),
            Event::ContextMenu(id, x, y) => self.context_menu(&id, x, y),
            Event::Unknown => {}
        }
        self.sync_window_title();
    }

    /// "{tab} — Ocho" on a tab, "Ocho" on the manager (workspace.rs `window_title`).
    pub fn sync_window_title(&mut self) {
        let title = match self.tabs.active_tab() {
            Some(tab) => format!("{} — Ocho", tab.title),
            None => "Ocho".to_string(),
        };
        if title != self.window_title {
            self.window_title = title.clone();
            self.host(vec!["window-title".into(), title], String::new());
        }
    }

    fn tick(&mut self, now: f64, w: f64, h: f64) {
        self.now = now;
        if w > 0.0 && h > 0.0 && (w, h) != self.window {
            self.window = (w, h);
            self.rail_width = clamp_rail(self.rail_width, w);
        }
        if !self.toast.persistent && !self.toast.text.is_empty() && now - self.toast.at > 6000.0 {
            self.toast = Toast::default();
        }
        self.refresh_account_usage(false);
        if matches!(self.overlay, Overlay::Conversations) {
            // The debounced search comes due between keystrokes (upstream
            // spawns a timer; the tick is the contract's only clock).
            self.poll_topics();
        }
    }

    /// Post a toast (workspace.rs `set_message`): plain ones fade after 6 s.
    pub fn set_message(&mut self, text: impl Into<String>) {
        self.toast = Toast {
            text: text.into(),
            error: false,
            persistent: false,
            at: self.now,
        };
    }

    /// Post an error toast (workspace.rs `set_error`): stays until dismissed.
    pub fn set_error(&mut self, text: impl Into<String>) {
        self.toast = Toast {
            text: text.into(),
            error: true,
            persistent: true,
            at: self.now,
        };
    }

    /// Queue a job for the module and say what its reply does.
    pub fn queue(&mut self, kind: &str, argv: Vec<String>, stdin: String, reply: Reply) -> u64 {
        self.next_job += 1;
        let id = self.next_job;
        self.jobs.push(Job {
            id,
            argv,
            kind: kind.into(),
            stdin,
        });
        self.replies.insert(id, reply);
        self.io += 1;
        id
    }

    /// Run a fleet command with a toast when it lands (workspace.rs `run_cli`).
    pub fn run_cli(&mut self, args: Vec<String>, done: impl Into<String>) {
        self.run_cli_with_history(args, None, done, None, None);
    }

    /// `run_cli` with undo entries recorded on success or failure.
    pub fn run_cli_with_history(
        &mut self,
        args: Vec<String>,
        input: Option<String>,
        done: impl Into<String>,
        undo_on_success: Option<UndoAction>,
        undo_on_failure: Option<UndoAction>,
    ) {
        self.busy += 1;
        self.set_message(format!(
            "Running fleet {}…",
            args.first().cloned().unwrap_or_default()
        ));
        self.queue(
            "cli",
            args,
            input.unwrap_or_default(),
            Reply::Cli {
                done: done.into(),
                undo_on_success,
                undo_on_failure,
            },
        );
    }

    /// A host job (the window's title, the clipboard, a URL).
    pub fn host(&mut self, argv: Vec<String>, stdin: String) {
        self.queue("host", argv, stdin, Reply::Host);
    }

    /// The jobs queued since the last take.
    pub fn take_jobs(&mut self) -> Vec<Job> {
        std::mem::take(&mut self.jobs)
    }

    /// The module could not run the queued jobs at all.
    pub fn io_failed(&mut self, message: &str) {
        self.io_version += 1;
        self.busy = 0;
        self.replies.clear();
        self.set_error(format!("fleet: {message}"));
    }

    /// A module reply's wall clock, kept beside the host clock.
    fn stamp_wall(&mut self, json: &serde_json::Value) {
        if let Some(now) = json.get("now").and_then(|n| n.as_f64()) {
            self.wall = (now, self.now);
        }
    }

    /// Seconds since the epoch now, from the module's last stamp plus the
    /// host clock since.
    pub fn epoch_s(&self) -> f64 {
        if self.wall.0 == 0.0 {
            return 0.0;
        }
        self.wall.0 + (self.now - self.wall.1).max(0.0) / 1000.0
    }

    /// Replies to jobs: `{"replies":[{"id","kind","status","stdout","stderr"}]}`.
    pub fn apply_io(&mut self, json: &serde_json::Value) {
        self.io_version += 1;
        self.stamp_wall(json);
        let Some(replies) = json.get("replies").and_then(|r| r.as_array()) else {
            return;
        };
        for reply in replies {
            let id = reply.get("id").and_then(|k| k.as_u64()).unwrap_or(0);
            let status = reply.get("status").and_then(|s| s.as_i64()).unwrap_or(0);
            let stdout = reply
                .get("stdout")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let stderr = reply
                .get("stderr")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let Some(what) = self.replies.remove(&id) else {
                continue;
            };
            let result: Result<String, String> = if status == 0 {
                Ok(stdout)
            } else {
                Err(if stderr.is_empty() {
                    format!("fleet exited with status {status}")
                } else {
                    stderr
                })
            };
            self.apply_reply(what, result);
        }
    }

    fn apply_reply(&mut self, what: Reply, result: Result<String, String>) {
        match what {
            Reply::Cli {
                done,
                undo_on_success,
                undo_on_failure,
            } => {
                self.busy = self.busy.saturating_sub(1);
                match result {
                    Ok(_) => {
                        if let Some(undo) = undo_on_success {
                            self.record_undo(undo);
                        }
                        self.set_message(done);
                    }
                    Err(error) => {
                        if let Some(undo) = undo_on_failure {
                            self.record_undo(undo);
                        }
                        self.set_error(error);
                    }
                }
                self.refresh_feed();
            }
            Reply::Flag {
                key,
                revision,
                done,
                undo,
            } => {
                self.busy = self.busy.saturating_sub(1);
                let current = self
                    .pending_flags
                    .get(&key)
                    .is_some_and(|p| p.0 == revision);
                if current {
                    match result {
                        Ok(_) => {
                            if let Some(p) = self.pending_flags.get_mut(&key) {
                                p.3 = true;
                            }
                            if let Some(undo) = undo {
                                self.record_undo(undo);
                            }
                            self.set_message(done);
                        }
                        Err(error) => {
                            if let Some((_, previous, _, _)) = self.pending_flags.remove(&key) {
                                let flag = key.2.clone();
                                self.patch_session(&key.0, &key.1, |s| {
                                    set_flag(s, &flag, previous)
                                });
                            }
                            self.set_error(error);
                        }
                    }
                }
                self.refresh_feed();
            }
            Reply::Usage(name) => {
                let at = self.now;
                match result {
                    Ok(text) => {
                        let usage = serde_json::from_str::<crate::types::ProviderUsage>(&text).ok();
                        let error = usage.is_none();
                        self.usage.insert(name, UsageLoad { at, usage, error });
                    }
                    Err(_) => {
                        self.usage.insert(
                            name,
                            UsageLoad {
                                at,
                                usage: None,
                                error: true,
                            },
                        );
                    }
                }
            }
            Reply::Models(key) => exec::models_arrived(self, key, result),
            Reply::Directories(machine, path) => {
                exec::directories_arrived(self, &machine, &path, result)
            }
            Reply::Topics(job) => self.topics_arrived(*job, result),
            Reply::WhatsNewHistory => self.whats_new_arrived(result),
            Reply::PairDescribe(request) => self.pair_describe_arrived(request, result),
            Reply::Pairing(request) => self.imessage.set_pairing(request, result),
            Reply::ThemeFiles => {
                if let Ok(text) = result {
                    self.theme_files_arrived(&text);
                }
            }
            Reply::Host => {}
        }
    }

    /// Ask the feed for a fresh round (the module wakes the watcher).
    pub fn refresh_feed(&mut self) {
        self.host(vec!["refresh-feed".into()], String::new());
    }

    /// A feed batch from the module: `{"events":[…]}`, each one a
    /// `fleet snapshot --watch` NDJSON object, merged as workspace.rs
    /// `apply_watch` merges it.
    pub fn apply_feed(&mut self, json: &serde_json::Value) {
        self.stamp_wall(json);
        if let Some(failure) = json.get("failure").and_then(|f| f.as_str()) {
            if self.feed_failure != failure {
                self.feed_failure = failure.to_string();
                self.set_error(failure.to_string());
            }
        }
        let events: Vec<&serde_json::Value> = match json.get("events").and_then(|e| e.as_array()) {
            Some(batch) => batch.iter().collect(),
            None => Vec::new(),
        };
        for event in events {
            if let Ok(mut event) = serde_json::from_value::<FleetEvent>(event.clone()) {
                event.index();
                self.apply_event(event);
            }
        }
        self.feed_version += 1;
    }

    fn apply_event(&mut self, mut event: FleetEvent) {
        let now = self.now;
        // Notifications watch the sessions the event carries (notifications.rs `observe`).
        let mut notes = Vec::new();
        {
            let settings = &self.settings;
            let n = &mut self.notifications;
            let mut watch = |machine: &Machine, sessions: &[Session]| {
                for s in sessions {
                    notes.extend(n.observe(machine, s, settings));
                }
            };
            if let Some(state) = &event.state {
                for m in &state.machines {
                    if let Some(last) = &m.last {
                        watch(m, &last.sessions);
                    }
                }
            }
            if let Some(m) = &event.machine {
                if let Some(last) = &m.last {
                    watch(m, &last.sessions);
                }
            }
            if let Some(update) = &event.indicators {
                if let Some(m) = self
                    .state
                    .machines
                    .iter()
                    .find(|m| m.id == update.machine_id)
                {
                    watch(m, &update.sessions);
                }
            }
        }
        for note in notes {
            match note {
                crate::notifications::Note::Post {
                    kind,
                    title,
                    body,
                    thread,
                } => {
                    self.host(vec!["notify".into(), title, thread, kind.into()], body);
                }
                crate::notifications::Note::Remove { thread } => {
                    self.host(vec!["notify-remove".into(), thread], String::new());
                }
            }
        }
        if !event.latest_provider_versions.is_empty() {
            self.latest_provider_versions = event.latest_provider_versions.clone();
        }
        // Every session the event carries was observed now (backend.rs
        // stamps `observation_received_at` on receipt).
        let stamp = |sessions: &mut Vec<Session>| {
            for s in sessions {
                s.observation_received_at = Some(now);
            }
        };
        match event.kind.as_str() {
            "state" => {
                let Some(mut state) = event.state.take() else {
                    return;
                };
                for machine in &mut state.machines {
                    if let Some(last) = machine.last.as_mut() {
                        stamp(&mut last.sessions);
                    }
                    if let Some(known) = self.state.machines.iter().find(|m| m.id == machine.id) {
                        // The stream's first state carries snapshots; a
                        // round-closing one relies on the machine events
                        // before it (workspace.rs `merge_state`).
                        if machine.last.is_none() {
                            machine.last = known.last.clone();
                            machine.error = known.error.clone();
                        }
                    }
                }
                self.state = state;
                self.loaded = true;
                self.loading = false;
                self.account_error = event.account_error;
                self.reapply_pending_flags();
                self.tabs.promote_established(&self.state);
                self.clamp_index();
            }
            "machine" => {
                let Some(mut update) = event.machine.take() else {
                    return;
                };
                if let Some(last) = update.last.as_mut() {
                    stamp(&mut last.sessions);
                }
                if let Some(known) = self.state.machines.iter_mut().find(|m| m.id == update.id) {
                    if event.unchanged {
                        // The vitals without the sessions: the sessions shown stay.
                        known.error = update.error;
                        if let Some(mut fresh) = update.last.take() {
                            if let Some(old) = known.last.as_mut() {
                                fresh.sessions = std::mem::take(&mut old.sessions);
                            }
                            known.last = Some(fresh);
                        }
                    } else {
                        let previous = known.last.take();
                        let live = update.last.as_ref().is_some_and(|l| l.live_inventory);
                        if let (Some(fresh), Some(old)) = (update.last.as_mut(), previous.as_ref())
                        {
                            if !live {
                                // A partial scan keeps the sessions it did not visit.
                                for s in &old.sessions {
                                    if !fresh.sessions.iter().any(|f| f.id == s.id) {
                                        fresh.sessions.push(s.clone());
                                    }
                                }
                            }
                        }
                        if update.last.is_none() {
                            update.last = previous;
                        }
                        *known = update;
                    }
                } else {
                    self.state.machines.push(update);
                }
                self.loading = false;
                self.reapply_pending_flags();
                self.clamp_index();
            }
            "session-indicators" => {
                let Some(mut update) = event.indicators.take() else {
                    return;
                };
                stamp(&mut update.sessions);
                if let Some(machine) = self
                    .state
                    .machines
                    .iter_mut()
                    .find(|m| m.id == update.machine_id)
                {
                    if let Some(last) = machine.last.as_mut() {
                        for fresh in &update.sessions {
                            if let Some(known) = last.sessions.iter_mut().find(|s| s.id == fresh.id)
                            {
                                known.apply_indicator(fresh);
                            } else {
                                last.sessions.push(fresh.clone());
                            }
                        }
                    }
                }
            }
            "error" if !event.error.is_empty() => self.set_error(event.error),
            _ => {}
        }
    }

    /// The desktop.json mirror from the module: `{"text","mtime","home"}`.
    pub fn apply_desktop(&mut self, json: &serde_json::Value) {
        self.desktop_version += 1;
        self.stamp_wall(json);
        let text = json.get("text").and_then(|t| t.as_str()).unwrap_or("");
        if text.is_empty() {
            return;
        }
        // A file this parser cannot read is left alone: nothing is restored
        // from it and, above all, nothing is written back over it.
        let settings = match DesktopSettings::try_parse(text) {
            Ok(settings) => settings,
            Err(error) => {
                self.desktop_broken = true;
                self.set_error(format!("desktop.json could not be read: {error}"));
                return;
            }
        };
        self.desktop_broken = false;
        if !self.desktop_loaded {
            self.desktop_loaded = true;
            self.theme = match settings.theme.as_deref() {
                Some("Ocho Light") => Theme::ocho_light(),
                _ => Theme::ocho_dark(),
            };
            if let Some(window) = settings.restored_windows().into_iter().next() {
                self.rail_width = clamp_rail(window.rail_width.unwrap_or(236.0), self.window.0);
                self.tabs = TabTree::restore(&window);
            }
        }
        self.settings = settings;
    }

    /// Save the window's tabs to desktop.json (windows.rs `persist`).
    pub fn persist_tabs(&mut self) {
        if self.desktop_broken || !self.desktop_loaded {
            return;
        }
        let window = self.tabs.saved(self.rail_width);
        self.settings.set_windows(vec![window]);
        let text = self.settings.to_json_pretty();
        self.host(vec!["save-desktop".into()], text);
    }

    /// The keyboard's region (workspace.rs `region`).
    pub fn region(&self) -> Region {
        if self.nav {
            return Region::Rail;
        }
        match self.tabs.active_tab() {
            None => Region::Manager,
            Some(_) => Region::Terminal,
        }
    }

    /// The keymap scopes in effect, innermost first.
    pub fn key_scopes(&self) -> Vec<crate::keymap::Scope> {
        use crate::keymap::Scope;
        if self.nav {
            return vec![Scope::Rail, Scope::Global];
        }
        if self.tabs.active > 0 {
            return vec![Scope::Global];
        }
        vec![Scope::for_page(self.page), Scope::Manager, Scope::Global]
    }

    /// The key hint for a command where the user is.
    pub fn hint(&self, cmd: Command) -> String {
        crate::keymap::hint(cmd, &self.key_scopes())
    }

    /// The sessions the page lists, filtered and sorted (workspace.rs `sessions`).
    pub fn sessions(&self) -> Vec<SessionItem> {
        crate::session::session_refs(
            &self.state,
            &self.query,
            self.history,
            self.all_sessions,
            self.hide_non_running,
        )
        .into_iter()
        .map(|(m, s)| SessionItem {
            machine: m.clone(),
            session: s.clone(),
        })
        .collect()
    }

    /// How many rows the page has.
    pub fn count(&self) -> usize {
        match self.page {
            Page::Machines => crate::session::machine_rows(&self.state).count(),
            Page::Sessions => self.sessions().len(),
            Page::Accounts => self.state.accounts.len(),
        }
    }

    fn clamp_index(&mut self) {
        let count = self.count();
        if count == 0 {
            self.index = 0;
        } else if self.index >= count {
            self.index = count - 1;
        }
    }

    /// Select a row, clamped (workspace.rs `select`).
    pub fn select(&mut self, index: usize) {
        let count = self.count();
        self.index = if count == 0 { 0 } else { index.min(count - 1) };
        self.scroll_to = format!("row-{}", self.row_id(self.index));
    }

    /// The contract's id for the row at `index` (rows.rs's ids).
    pub fn row_id(&self, index: usize) -> String {
        match self.page {
            Page::Machines => crate::session::machine_rows(&self.state)
                .nth(index)
                .map(|m| m.id.clone())
                .unwrap_or_default(),
            Page::Sessions => self
                .sessions()
                .get(index)
                .map(|i| format!("{}/{}", i.machine.id, i.session.id))
                .unwrap_or_default(),
            Page::Accounts => self
                .state
                .accounts
                .get(index)
                .map(|a| a.name.clone())
                .unwrap_or_default(),
        }
    }

    /// Half a page of rows (workspace.rs `page_size`).
    pub fn page_size(&self) -> usize {
        let rows = (self.window.1 - 140.0) / 74.0;
        ((rows / 2.0).floor().max(1.0)) as usize
    }

    /// The selected session, with its machine.
    pub fn selected_session(&self) -> Option<SessionItem> {
        if self.page != Page::Sessions {
            return None;
        }
        self.sessions().into_iter().nth(self.index)
    }

    /// The selected machine row.
    pub fn selected_machine(&self) -> Option<Machine> {
        if self.page != Page::Machines {
            return None;
        }
        crate::session::machine_rows(&self.state)
            .nth(self.index)
            .cloned()
    }

    /// The selected account.
    pub fn selected_account(&self) -> Option<crate::types::Account> {
        if self.page != Page::Accounts {
            return None;
        }
        self.state.accounts.get(self.index).cloned()
    }

    /// The session the active tab follows, with its machine.
    pub fn tab_target(&self) -> Option<SessionItem> {
        let tab = self.tabs.active_tab()?;
        let (machine_id, session_id, _) = tab.session.as_ref()?;
        let machine = self.state.machines.iter().find(|m| &m.id == machine_id)?;
        let session = machine
            .last
            .as_ref()?
            .sessions
            .iter()
            .find(|s| &s.id == session_id)?;
        Some(SessionItem {
            machine: machine.clone(),
            session: session.clone(),
        })
    }

    /// The session a command acts on: the tab's while one is active, else
    /// the manager's selected row.
    pub fn command_session(&self) -> Option<SessionItem> {
        if self.tabs.active > 0 {
            self.tab_target()
        } else {
            self.selected_session()
        }
    }

    /// Change a session in the model (an optimistic patch before the CLI lands).
    pub fn patch_session(
        &mut self,
        machine_id: &str,
        session_id: &str,
        patch: impl FnOnce(&mut Session),
    ) {
        if let Some(machine) = self.state.machines.iter_mut().find(|m| m.id == machine_id) {
            if let Some(last) = machine.last.as_mut() {
                if let Some(session) = last.sessions.iter_mut().find(|s| s.id == session_id) {
                    patch(session);
                }
            }
        }
    }

    fn reapply_pending_flags(&mut self) {
        let pending: Vec<_> = self
            .pending_flags
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        for ((machine, session, flag), (_, _, desired, _)) in pending {
            self.patch_session(&machine, &session, |s| set_flag(s, &flag, desired));
        }
    }

    /// Record an undo entry (20 kept).
    pub fn record_undo(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
        if self.undo_stack.len() > 20 {
            self.undo_stack.remove(0);
        }
    }

    /// The next operation revision (optimistic writes are keyed by it).
    pub fn next_revision(&mut self) -> u64 {
        self.operation_revision = self.operation_revision.wrapping_add(1);
        self.operation_revision
    }

    /// Load usage meters for the Accounts page (workspace.rs
    /// `refresh_account_usage`): cached 5 minutes, connected accounts only.
    pub fn refresh_account_usage(&mut self, force: bool) {
        if self.page != Page::Accounts || self.tabs.active > 0 {
            return;
        }
        let names: Vec<String> = self
            .state
            .accounts
            .iter()
            .filter(|a| a.shared && a.status == "connected")
            .map(|a| a.name.clone())
            .collect();
        for name in names {
            let stale = match self.usage.get(&name) {
                None => true,
                Some(load) => {
                    (force || self.now - load.at > 300_000.0)
                        && (load.usage.is_some() || load.error)
                }
            };
            if stale {
                self.usage.insert(
                    name.clone(),
                    UsageLoad {
                        at: self.now,
                        usage: None,
                        error: false,
                    },
                );
                self.queue(
                    "usage",
                    vec!["accounts".into(), "usage".into(), name.clone()],
                    String::new(),
                    Reply::Usage(name),
                );
            }
        }
    }
}

/// Set one of a session's flags by name (workspace.rs `SessionFlag::set`).
pub fn set_flag(session: &mut Session, flag: &str, value: bool) {
    match flag {
        "tracked" => session.tracked = value,
        "pinned" => session.pinned = value,
        "archived" => session.archived = value,
        "hidden" => session.hidden = value,
        _ => {}
    }
}

/// A session's flag by name.
pub fn get_flag(session: &Session, flag: &str) -> bool {
    match flag {
        "tracked" => session.tracked,
        "pinned" => session.pinned,
        "archived" => session.archived,
        "hidden" => session.hidden,
        _ => false,
    }
}

/// workspace.rs:10556: `clamp(w, 180, min(480, max(window_w − 320, 180)))`.
pub fn clamp_rail(w: f64, window_w: f64) -> f64 {
    let max = 480f64.min((window_w - 320.0).max(180.0));
    w.clamp(180.0, max)
}
