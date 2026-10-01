//! Commands (workspace.rs `execute`), the overlays they open, the terminals
//! they spawn and the undo stack.

use super::{get_flag, set_flag, Page, Reply, Workspace};
#[allow(unused_imports)]
use super::{FlagKey, PendingFlag};
use crate::keymap::Scope;
use crate::launch::{CatalogKey, Launch, LaunchMemory, Submission};
use crate::palette::{self, Command, CommandInfo};
use crate::picker::PickerState;
use crate::preferences::SettingsPage;
use crate::session::{account_label, machine_rows, profiles, NamedProfile};
use crate::tab_tree::{ClosedTab, Reopened, Tab};
use crate::types::{Machine, Session};

/// A session with its machine (workspace.rs `SessionItem`).
#[derive(Clone, Debug, PartialEq)]
pub struct SessionItem {
    /// The machine the session runs on.
    pub machine: Machine,
    /// The session.
    pub session: Session,
}

/// What a confirmation dialog will do to a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    /// `fleet terminate`.
    Stop,
    /// `fleet delete`.
    Delete,
    /// `fleet interrupt`.
    Interrupt,
}

/// The one open overlay (workspace.rs `Overlay`).
#[derive(Clone, Debug, Default)]
pub enum Overlay {
    /// Nothing open.
    #[default]
    None,
    /// The keyboard help.
    Help,
    /// A session's last message.
    Message(SessionItem),
    /// A yes/no question about a session.
    Confirm {
        /// What yes does.
        action: ConfirmAction,
        /// To which session.
        item: SessionItem,
    },
    /// "Remove {machine} from Ocho?".
    MachineDelete(Machine),
    /// "Install/Update {provider} on {machine}?".
    ProviderUpdate {
        /// The machine.
        machine: Machine,
        /// The provider.
        provider: String,
    },
    /// "Remove launch profile {name}?".
    ProfileDelete(String),
    /// The launch dialog, or quick launch inside it.
    Launch(Box<Launch>),
    /// The command palette.
    Palette(PickerState),
    Settings(SettingsPage),
    /// A form (add machine, label, profile, …).
    Form(Box<crate::forms::Form>),
    /// "Open the session for PR …".
    PullRequests(crate::finders::PullRequestFinder),
    /// "Find the session where I worked on …" (state on `Workspace::topics`).
    Conversations,
    /// The theme picker.
    Themes(Box<crate::themes::ThemePicker>),
    /// What's New (state on `Workspace::whats_new`).
    WhatsNew,
    /// The iMessage pairing card (state on `Workspace::imessage`).
    PairIMessage,
}

/// An entry on the undo stack (workspace.rs `UndoAction`).
#[derive(Clone, Debug, PartialEq)]
pub enum UndoAction {
    /// A closed tab.
    ClosedTab(ClosedTab),
    /// A session flag change (`tracked`, `pinned`, `archived`, `hidden`).
    SessionFlag {
        /// The machine.
        machine_id: String,
        /// The session.
        session_id: String,
        /// The flag's name.
        flag: String,
        /// Its value before.
        previous: bool,
        /// Its value after.
        desired: bool,
    },
}

/// A fresh 32-hex request id (backend.rs `new_request_id`), from the clock
/// and a counter: unique enough within one app.
fn new_request_id(seed: f64, n: u64) -> String {
    let mut x = (seed as u64) ^ (n.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mut out = String::with_capacity(32);
    for _ in 0..4 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push_str(&format!("{:08x}", (x & 0xFFFF_FFFF) as u32));
    }
    out
}

impl Workspace {
    /// Run a command (workspace.rs `execute`).
    pub fn execute(&mut self, cmd: Command) {
        self.pending_keys = None;
        self.popup = None;
        if matches!(
            cmd,
            Command::NextTab
                | Command::PrevTab
                | Command::SelectTab(_)
                | Command::Page(_)
                | Command::CloseTab
        ) {
            self.secrets.blur();
        }
        match cmd {
            Command::Undo => self.undo(),
            Command::ReopenClosedTab => self.reopen_closed_tab(),
            Command::ClearScrollback => {
                if let Some(tab) = self.tabs.active_tab() {
                    let key = tab.key.clone();
                    self.host(vec!["clear-terminal".into(), key], String::new());
                }
            }
            Command::Palette => self.overlay = Overlay::Palette(PickerState::new()),
            Command::ToggleSecrets => self.toggle_secrets(),
            Command::ToggleTranscript => self.toggle_transcript(),
            Command::AttachIMessage => {
                match self.tab_target().as_ref().and_then(imessage_attach_command) {
                    Some(args) => self.run_cli(args, "iMessage attached to this conversation"),
                    None => self.set_error("Open a live Codex app-server tab to attach iMessage"),
                }
            }
            Command::Help => self.overlay = Overlay::Help,
            Command::Settings => self.open_settings(),
            Command::Quit => self.host(vec!["quit".into()], String::new()),
            Command::CloseWindow => self.host(vec!["close-window".into()], String::new()),
            Command::Refresh => {
                self.refresh_account_usage(true);
                self.loading = true;
                self.refresh_feed();
            }
            Command::NextTab => {
                self.tabs.step(true);
                self.tabs.reveal_active();
                self.persist_tabs();
            }
            Command::PrevTab => {
                self.tabs.step(false);
                self.tabs.reveal_active();
                self.persist_tabs();
            }
            Command::SelectTab(n) => {
                if let Some(slot) = self.tabs.numbered_slot(n) {
                    self.tabs.active = slot;
                    self.tabs.reveal_active();
                    self.persist_tabs();
                }
            }
            Command::CloseTab => {
                if self.tabs.active > 0 {
                    self.close_tab(self.tabs.active - 1);
                } else {
                    self.host(vec!["close-window".into()], String::new());
                }
            }
            Command::ReconnectTab => {
                if self.tabs.active > 0 {
                    let pos = self.tabs.active - 1;
                    if self.terminal_can_reconnect(&self.tabs.tabs[pos].key) {
                        self.reconnect_tab(pos);
                    } else {
                        self.set_message("This terminal is still connected");
                    }
                }
            }
            Command::New => self.open_launch(),
            Command::QuickLaunch => self.open_quick_launch(),
            Command::EnterNav => self.nav = !self.nav,
            Command::ExitNav => self.nav = false,
            Command::RailDown => self.move_nav_selection(1),
            Command::RailUp => self.move_nav_selection(-1),
            Command::LaunchProfile(slot) => {
                match profiles(&self.state)
                    .into_iter()
                    .find(|p| p.profile.shortcut == slot as i64)
                {
                    Some(p) => self.launch_profile(p),
                    None => self.set_error(format!(
                        "No profile assigned to ⌘⌥{slot}. Use Add profile… in the command palette to create one."
                    )),
                }
            }
            _ if self.tabs.active > 0 && !palette::TAB_SESSION.iter().any(|i| i.command == cmd) => {
                // Manager-only commands leave the terminal tab first.
                self.tabs.select_manager();
                self.nav = false;
                self.persist_tabs();
                self.execute(cmd);
                return;
            }
            Command::NextPage => self.set_page(Page::from_index(self.page.index() + 1)),
            Command::PrevPage => {
                self.set_page(Page::from_index(self.page.index() + Page::ALL.len() - 1))
            }
            Command::Page(n) => self.set_page(Page::from_index(n)),
            Command::Down => self.select(self.index + 1),
            Command::Up => self.select(self.index.saturating_sub(1)),
            Command::First => self.select(0),
            Command::Last => self.select(self.count().saturating_sub(1)),
            Command::HalfDown => self.select(self.index + self.page_size()),
            Command::HalfUp => self.select(self.index.saturating_sub(self.page_size())),
            Command::Search => {
                if self.page != Page::Sessions {
                    self.set_page(Page::Sessions);
                }
                self.searching = true;
                self.focus_id = "search".into();
            }
            Command::ClearFilter => {
                self.query.clear();
                self.searching = false;
                self.index = 0;
            }
            Command::ToggleOthers => {
                self.all_sessions = !self.all_sessions;
                self.index = 0;
            }
            Command::ToggleHistory => {
                self.history = !self.history;
                self.index = 0;
            }
            Command::ToggleNonRunning => {
                self.hide_non_running = !self.hide_non_running;
                self.index = 0;
            }
            Command::Open => match self.page {
                Page::Machines => {
                    if let Some(m) = self.selected_machine() {
                        self.set_page(Page::Sessions);
                        self.query = m.name.clone();
                        self.set_message(format!(
                            "Sessions on {} · Esc shows every machine",
                            m.name
                        ));
                    } else {
                        self.set_page(Page::Sessions);
                    }
                }
                Page::Sessions => self.attach(false),
                Page::Accounts => {
                    if let Some(a) = self.selected_account() {
                        if !a.shared || a.status != "connected" {
                            let args = vec!["accounts".to_string(), "login".into(), a.name.clone()];
                            self.open_terminal_tab(
                                format!("login:{}", a.name),
                                format!("Sign in · {}", account_label(&a)),
                                args.clone(),
                                args,
                                None,
                            );
                        } else {
                            let args =
                                vec!["shell".to_string(), "--account".into(), a.name.clone()];
                            self.open_terminal_tab(
                                format!("shell:{}", a.name),
                                format!("Shell · {}", account_label(&a)),
                                args.clone(),
                                args,
                                None,
                            );
                        }
                    }
                }
            },
            Command::ViewReadOnly => self.attach(true),
            Command::UpdateMachine => {
                if let Some(m) = self.selected_machine() {
                    if m.local {
                        self.set_error(
                            "This Mac runs the desktop's own Ocho binary; nothing to update.",
                        );
                    } else {
                        let args = vec!["machine".to_string(), "update".into(), m.id.clone()];
                        self.run_cli(args, format!("Ocho helper updated on {}", m.name));
                    }
                }
            }
            Command::UpdateCodex
            | Command::UpdateClaude
            | Command::UpdateOpenCode
            | Command::UpdateGrok
            | Command::UpdateAntigravity => {
                if let Some(machine) = self.selected_machine() {
                    let provider = match cmd {
                        Command::UpdateCodex => "codex",
                        Command::UpdateClaude => "claude",
                        Command::UpdateAntigravity => "antigravity",
                        Command::UpdateGrok => "grok",
                        _ => "opencode",
                    };
                    self.overlay = Overlay::ProviderUpdate {
                        machine,
                        provider: provider.into(),
                    };
                }
            }
            Command::ConnectFly => {
                let args = vec!["sprites".to_string(), "connect".into()];
                let key = format!("fly:{}", self.request_id());
                self.open_terminal_tab(key, "Connect Fly.io".into(), args.clone(), args, None);
            }
            Command::Message => {
                if let Some(item) = self.selected_session() {
                    self.overlay = Overlay::Message(item);
                }
            }
            Command::Track => {
                if let Some(item) = self.selected_session() {
                    self.run_session_flag(item, "tracked", true, "track", "Session tracked");
                }
            }
            Command::Pin => {
                if let Some(item) = self.selected_session() {
                    let desired = !item.session.pinned;
                    let (action, done) = if desired {
                        ("pin", "Pinned · ⌘Z to undo")
                    } else {
                        ("unpin", "Unpinned · ⌘Z to undo")
                    };
                    self.run_session_flag(item, "pinned", desired, action, done);
                }
            }
            Command::Archive => {
                if let Some(item) = self.selected_session() {
                    let desired = !item.session.archived;
                    let (action, done) = if desired {
                        ("archive", "Archived · ⌘Z to undo")
                    } else {
                        ("unarchive", "Unarchived · ⌘Z to undo")
                    };
                    self.run_session_flag(item, "archived", desired, action, done);
                }
            }
            Command::Interrupt => {
                if let Some(item) = self.command_session() {
                    self.overlay = Overlay::Confirm {
                        action: ConfirmAction::Interrupt,
                        item,
                    };
                }
            }
            Command::Pause => {
                if let Some(item) = self.selected_session() {
                    if item.session.state == "paused" {
                        self.run_cli(
                            vec!["unpause".into(), item.machine.id, item.session.id],
                            "Session resumed",
                        );
                    } else {
                        self.run_cli(
                            vec![
                                "pause".into(),
                                item.machine.id,
                                item.session.id,
                                "--yes".into(),
                            ],
                            "Session paused; opening it resumes",
                        );
                    }
                }
            }
            Command::UnpauseTab => {
                let target = self.tab_target().filter(|i| i.session.state == "paused");
                match target {
                    Some(item) => self.run_cli(
                        vec!["unpause".into(), item.machine.id, item.session.id],
                        "Session resumed",
                    ),
                    None => self.set_message("This session is not paused"),
                }
            }
            Command::Stop => {
                if let Some(item) = self.selected_session() {
                    self.overlay = Overlay::Confirm {
                        action: ConfirmAction::Stop,
                        item,
                    };
                }
            }
            Command::Delete => match self.page {
                Page::Machines => {
                    if let Some(machine) = self.selected_machine() {
                        self.overlay = Overlay::MachineDelete(machine);
                    }
                }
                Page::Sessions => {
                    if let Some(item) = self.selected_session() {
                        self.overlay = Overlay::Confirm {
                            action: ConfirmAction::Delete,
                            item,
                        };
                    }
                }
                _ => {}
            },
            Command::Shell => {
                if let Some(m) = self.selected_machine() {
                    let args = vec!["connect".to_string(), m.id.clone()];
                    let key = format!("shell:{}", m.id);
                    self.open_terminal_tab(
                        key.clone(),
                        format!("{} shell", m.name),
                        args.clone(),
                        args,
                        None,
                    );
                    if let Some(tab) = self.tabs.tabs.iter_mut().find(|t| t.key == key) {
                        tab.machine = Some(m.id.clone());
                    }
                }
            }
            Command::NewFolder => self.open_form(crate::forms::Form::folder(None)),
            Command::PullRequests => self.open_pull_requests(),
            Command::Conversations => self.open_conversations(),
            Command::IndexConversations => {
                self.topics.index_conversations(true, self.now);
                self.poll_topics();
            }
            Command::Themes => self.open_themes(false),
            Command::WhatsNew => self.open_whats_new(),
            Command::PairIMessage => self.open_pair_imessage(),
            Command::BackupRecovery => self.open_form(crate::forms::Form::recovery_backup()),
            Command::ConnectEAS => {
                let args = vec!["eas".to_string(), "connect".into()];
                let key = format!("eas-connect:{}", self.request_id());
                self.open_terminal_tab(key, "Connect EAS".into(), args.clone(), args, None);
            }
            Command::AddMachine => self.open_form(crate::forms::Form::machine()),
            Command::AddAccount => self.open_form(crate::forms::Form::account()),
            Command::AddProfile => {
                let form = crate::forms::Form::profile(None, &self.state);
                self.open_form_result(form);
            }
            Command::Edit => match self.page {
                Page::Sessions => {
                    if let Some(item) = self.selected_session() {
                        self.open_form(crate::forms::Form::label(item));
                    }
                }
                Page::Machines => {
                    if let Some(m) = self.selected_machine() {
                        self.open_form(crate::forms::Form::machine_edit(&m));
                    }
                }
                Page::Accounts => {
                    if let Some(a) = self.selected_account() {
                        let args = vec!["accounts".to_string(), "login".into(), a.name.clone()];
                        self.open_terminal_tab(
                            format!("login:{}", a.name),
                            format!("Sign in · {}", account_label(&a)),
                            args.clone(),
                            args,
                            None,
                        );
                    }
                }
            },
            Command::Resume => {
                if let Some(item) = self.selected_session() {
                    // An EAS conversation reattaches rather than resuming.
                    if item.session.eas {
                        self.attach_item(item, false);
                    } else {
                        self.open_form(crate::forms::Form::resume(item));
                    }
                }
            }
            Command::Handoff => {
                if let Some(item) = self.command_session() {
                    self.tabs.select_manager();
                    let form = crate::forms::Form::handoff(item, &self.state);
                    self.open_form_result(form);
                }
            }
            Command::SwitchAccount => {
                if let Some(item) = self.command_session() {
                    self.tabs.select_manager();
                    let form = crate::forms::Form::switch_account(item, &self.state);
                    self.open_form_result(form);
                }
            }
            Command::Move => {
                if let Some(item) = self.command_session() {
                    self.tabs.select_manager();
                    let form = crate::forms::Form::migrate(item, &self.state);
                    self.open_form_result(form);
                }
            }
            other => self.set_message(format!(
                "{} is not available yet in this client",
                palette::label(other)
            )),
        }
        self.refresh_account_usage(false);
    }

    /// Switch pages (workspace.rs `set_page`).
    pub fn set_page(&mut self, page: Page) {
        if page != self.page {
            self.index = 0;
        }
        self.page = page;
        self.popup = None;
        self.searching = false;
        self.refresh_account_usage(false);
    }

    fn move_nav_selection(&mut self, step: i64) {
        // The rail's rows: the four pages, then the visible terminals, wrapping.
        let visible = self.tabs.visible();
        let rows = 4 + visible.len();
        let current = if self.tabs.active == 0 {
            self.page.index()
        } else {
            4 + visible
                .iter()
                .position(|&i| i + 1 == self.tabs.active)
                .unwrap_or(0)
        };
        let next = ((current as i64 + step).rem_euclid(rows as i64)) as usize;
        if next < 4 {
            self.tabs.select_manager();
            self.set_page(Page::from_index(next));
        } else {
            self.tabs.active = visible[next - 4] + 1;
        }
        self.tabs.reveal_active();
        self.persist_tabs();
    }

    /// A request id for a new terminal or launch.
    pub fn request_id(&mut self) -> String {
        let n = self.next_revision();
        new_request_id(self.now, n)
    }

    /// Open the session's terminal (workspace.rs `attach`).
    pub fn attach(&mut self, read_only: bool) {
        let Some(item) = self.selected_session() else {
            return;
        };
        self.attach_item(item, read_only);
    }

    /// Open a session's terminal in a tab.
    pub fn attach_item(&mut self, item: SessionItem, read_only: bool) {
        if !item.session.can_attach() {
            self.set_error("No attachable terminal. Press o to explicitly resume saved history.");
            return;
        }
        let args =
            crate::tab_tree::session_attach_command(&item.machine.id, &item.session.id, read_only);
        let key = format!("{}:{}:{}", item.machine.id, item.session.id, read_only);
        let title = session_tab_title(&item.session.title, &item.machine.name, read_only);
        self.open_terminal_tab(
            key,
            title,
            args.clone(),
            args,
            Some((item.machine.id, item.session.id, read_only)),
        );
    }

    /// Open or select a terminal tab (workspace.rs `open_terminal_tab`).
    pub fn open_terminal_tab(
        &mut self,
        key: String,
        title: String,
        command: Vec<String>,
        reconnect: Vec<String>,
        session: Option<(String, String, bool)>,
    ) {
        let _ = command;
        if let Some(pos) = self.tabs.position(&key) {
            self.tabs.select(pos);
            self.nav = false;
            self.tabs.reveal_active();
            if self.exited.contains(&key) {
                self.reconnect_tab(pos);
            }
            self.persist_tabs();
            return;
        }
        let machine = session.as_ref().map(|s| s.0.clone());
        self.tabs.tabs.push(Tab {
            key,
            title,
            reconnect: crate::tab_tree::restore_terminal_command(&reconnect),
            session,
            machine,
            depth: 0,
            collapsed: false,
            folder: false,
        });
        self.tabs.active = self.tabs.tabs.len();
        self.nav = false;
        self.set_message("");
        self.persist_tabs();
    }

    /// Spawn the terminal again (workspace.rs `reconnect_tab`).
    pub fn reconnect_tab(&mut self, pos: usize) {
        let key = self.tabs.tabs[pos].key.clone();
        // A retryable transport reconnects in place.
        if self.retry_connection(&key) {
            return;
        }
        self.exited.remove(&key);
        self.exit_codes.remove(&key);
        self.connections.remove(&key);
        self.host(vec!["reconnect-terminal".into(), key], String::new());
    }

    /// Close a tab (workspace.rs `close_tab`): its rows are promoted, the
    /// close goes on the undo stack.
    pub fn close_tab(&mut self, pos: usize) {
        let Some(closed) = self.tabs.close(pos) else {
            return;
        };
        self.exited.remove(&closed.tab.key);
        self.host(
            vec!["close-terminal".into(), closed.tab.key.clone()],
            String::new(),
        );
        self.set_message(crate::tab_tree::TabTree::closed_message(&closed));
        self.record_undo(UndoAction::ClosedTab(closed));
        self.persist_tabs();
    }

    fn undo(&mut self) {
        let Some(action) = self.undo_stack.pop() else {
            self.set_message("Nothing to undo");
            return;
        };
        match action {
            UndoAction::ClosedTab(closed) => {
                let title = closed.tab.title.clone();
                match self.tabs.reopen(closed) {
                    Reopened::AlreadyOpen(_) => {
                        self.set_message(format!("{title} is already open"))
                    }
                    Reopened::Folder(_) | Reopened::Tab(_) => {
                        self.set_message(format!("Reopened {title}"))
                    }
                }
                self.persist_tabs();
            }
            UndoAction::SessionFlag {
                machine_id,
                session_id,
                flag,
                previous,
                desired,
            } => {
                let action = match (flag.as_str(), previous) {
                    ("pinned", true) => "pin",
                    ("pinned", false) => "unpin",
                    ("archived", true) => "archive",
                    ("archived", false) => "unarchive",
                    ("tracked", true) => "track",
                    ("hidden", true) => "hide",
                    ("hidden", false) => "unhide",
                    _ => return,
                };
                let item = SessionItem {
                    machine: Machine {
                        id: machine_id,
                        ..Default::default()
                    },
                    session: Session {
                        id: session_id,
                        ..Default::default()
                    },
                };
                let _ = desired;
                self.run_session_flag(item, &flag, previous, action, "Undone");
            }
        }
    }

    fn reopen_closed_tab(&mut self) {
        let at = self
            .undo_stack
            .iter()
            .rposition(|a| matches!(a, UndoAction::ClosedTab(_)));
        match at {
            Some(i) => {
                let action = self.undo_stack.remove(i);
                self.undo_stack.push(action);
                self.undo();
            }
            None => self.set_message("No closed tab to reopen"),
        }
    }

    /// Write a session flag optimistically, then through the CLI
    /// (workspace.rs `run_session_flag`).
    pub fn run_session_flag(
        &mut self,
        item: SessionItem,
        flag: &str,
        desired: bool,
        action: &str,
        done: &str,
    ) {
        let previous = get_flag(&item.session, flag);
        let undo = (previous != desired).then(|| UndoAction::SessionFlag {
            machine_id: item.machine.id.clone(),
            session_id: item.session.id.clone(),
            flag: flag.to_string(),
            previous,
            desired,
        });
        let revision = self.next_revision();
        let key = (
            item.machine.id.clone(),
            item.session.id.clone(),
            flag.to_string(),
        );
        self.pending_flags
            .insert(key.clone(), (revision, previous, desired, false));
        let flag_name = flag.to_string();
        self.patch_session(&item.machine.id, &item.session.id, move |s| {
            set_flag(s, &flag_name, desired)
        });
        self.busy += 1;
        self.set_message(format!("Running fleet {action}…"));
        self.queue(
            "flag",
            vec![action.into(), item.machine.id, item.session.id],
            String::new(),
            Reply::Flag {
                key,
                revision,
                done: done.into(),
                undo,
            },
        );
    }

    /// Yes on a confirmation (workspace.rs `confirm_yes`).
    pub fn confirm_yes(&mut self) {
        match std::mem::replace(&mut self.overlay, Overlay::None) {
            Overlay::Confirm { action, item } => {
                let (verb, done) = match action {
                    ConfirmAction::Stop => ("terminate", "Session stopped"),
                    ConfirmAction::Delete => ("delete", "Session stopped and removed"),
                    ConfirmAction::Interrupt => ("interrupt", "Interrupt sent"),
                };
                self.run_cli(
                    vec![
                        verb.into(),
                        item.machine.id,
                        item.session.id,
                        "--yes".into(),
                    ],
                    done,
                );
            }
            Overlay::MachineDelete(machine) => {
                self.run_cli(
                    vec![
                        "machine".into(),
                        "remove".into(),
                        machine.id.clone(),
                        "--emit-removed".into(),
                    ],
                    format!("Removed {} from Ocho", machine.name),
                );
            }
            Overlay::ProviderUpdate { machine, provider } => {
                let label = crate::session::provider_label(&provider).to_string();
                self.run_cli(
                    vec![
                        "machine".into(),
                        "update-provider".into(),
                        machine.id.clone(),
                        provider,
                    ],
                    format!("{label} updated on {}", machine.name),
                );
            }
            Overlay::ProfileDelete(name) => {
                self.run_cli(
                    vec!["profiles".into(), "remove".into(), name.clone()],
                    format!("Removed profile {name}"),
                );
            }
            other => self.overlay = other,
        }
    }

    /// The launch memory desktop.json carries, in launch.rs's terms.
    pub fn launch_memory(&self) -> LaunchMemory {
        let s = &self.settings;
        LaunchMemory {
            launch: s.launch.as_ref().map(|d| crate::launch::LaunchDefaults {
                machine: d.machine.clone(),
                provider: d.provider.clone(),
                account: d.account.clone(),
                cwd: d.cwd.clone(),
                model: d.model.clone(),
                effort: d.effort.clone(),
                permissions: d.permissions.clone(),
            }),
            recent_projects: s
                .recent_projects
                .iter()
                .map(|p| crate::launch::RecentProject {
                    machine: p.machine.clone(),
                    cwd: p.cwd.clone(),
                })
                .collect(),
            recent_models: s
                .recent_models
                .iter()
                .map(|m| crate::launch::RecentModel {
                    provider: m.provider.clone(),
                    id: m.id.clone(),
                    name: m.name.clone(),
                })
                .collect(),
            model_efforts: s.model_efforts.clone(),
        }
    }

    fn store_launch_memory(&mut self, memory: LaunchMemory) {
        let s = &mut self.settings;
        s.launch = memory.launch.map(|d| crate::settings::LaunchDefaults {
            machine: d.machine,
            provider: d.provider,
            account: d.account,
            cwd: d.cwd,
            model: d.model,
            effort: d.effort,
            permissions: d.permissions,
        });
        s.recent_projects = memory
            .recent_projects
            .into_iter()
            .map(|p| crate::settings::RecentProject {
                machine: p.machine,
                cwd: p.cwd,
            })
            .collect();
        s.recent_models = memory
            .recent_models
            .into_iter()
            .map(|m| crate::settings::RecentModel {
                provider: m.provider,
                id: m.id,
                name: m.name,
            })
            .collect();
        s.model_efforts = memory.model_efforts;
    }

    /// Open the launch dialog (workspace.rs `open_launch`).
    pub fn open_launch(&mut self) {
        let draft = self.launch_draft.clone();
        let selected = self.selected_machine().map(|m| m.id);
        match Launch::open(
            &self.launch_memory(),
            &self.state,
            selected.as_deref(),
            draft,
        ) {
            Ok(launch) => {
                self.overlay = Overlay::Launch(Box::new(launch));
                self.focus_id = "field-launch-prompt".into();
                self.sync_launch_catalogs(false);
            }
            Err(e) => self.set_error(e),
        }
    }

    /// Open the numbered quick launch (workspace.rs `open_quick_launch`).
    pub fn open_quick_launch(&mut self) {
        let draft = self.launch_draft.clone();
        let selected = self.selected_machine().map(|m| m.id);
        let mut shortcuts = crate::quick::Shortcuts {
            accounts: self.settings.launch_shortcuts.accounts.clone(),
            machines: self.settings.launch_shortcuts.machines.clone(),
        };
        crate::quick::enroll_state(&mut shortcuts, &self.state);
        if shortcuts.accounts != self.settings.launch_shortcuts.accounts
            || shortcuts.machines != self.settings.launch_shortcuts.machines
        {
            self.settings.launch_shortcuts.accounts = shortcuts.accounts.clone();
            self.settings.launch_shortcuts.machines = shortcuts.machines.clone();
            let text = self.settings.to_json_pretty();
            self.host(vec!["save-desktop".into()], text);
        }
        match crate::quick::open(
            &self.launch_memory(),
            &self.state,
            selected.as_deref(),
            &shortcuts,
            draft,
        ) {
            Ok(launch) => {
                self.overlay = Overlay::Launch(Box::new(launch));
                self.sync_launch_catalogs(false);
            }
            Err(e) => self.set_error(e),
        }
    }

    /// Close the launch dialog, keeping the prompt as a draft.
    pub fn close_launch(&mut self) {
        if let Overlay::Launch(launch) = &self.overlay {
            // workspace.rs `close_launch`: a non-blank prompt is kept and said so,
            // from quick launch too.
            let draft = launch.close();
            self.launch_draft = draft;
            self.overlay = Overlay::None;
            if !self.launch_draft.trim().is_empty() {
                self.set_message(crate::launch::DRAFT_KEPT);
            }
        }
    }

    /// Ask the module for the catalog the dialog wants, once.
    pub fn sync_launch_catalogs(&mut self, force: bool) {
        let Overlay::Launch(launch) = &mut self.overlay else {
            return;
        };
        launch.sync_catalogs(&self.state, force);
        if let Some(key) = launch.wanted_catalog() {
            launch.catalog_requested(&key);
            let argv = key.argv(&self.state);
            self.queue("models", argv, String::new(), Reply::Models(key));
        }
        let Overlay::Launch(launch) = &mut self.overlay else {
            return;
        };
        if let Some((machine, path)) = launch.wanted_directories() {
            launch.directories_requested();
            self.queue(
                "directories",
                vec!["directories".into(), machine.clone(), path.clone()],
                String::new(),
                Reply::Directories(machine, path),
            );
        }
    }

    /// Launch what the dialog holds (workspace.rs `launch`).
    pub fn submit_launch(&mut self) {
        let request = self.request_id();
        let mut memory = self.launch_memory();
        let Overlay::Launch(launch) = &mut self.overlay else {
            return;
        };
        match launch.submit(&self.state, &mut memory, &request) {
            Ok(Submission {
                machine,
                tab_key,
                title,
                argv,
                request: _,
            }) => {
                self.overlay = Overlay::None;
                self.launch_draft.clear();
                self.store_launch_memory(memory);
                let reconnect = argv.clone();
                self.open_terminal_tab(tab_key.clone(), title, argv, reconnect, None);
                if let Some(tab) = self.tabs.tabs.iter_mut().find(|t| t.key == tab_key) {
                    tab.machine = Some(machine);
                }
            }
            Err(e) => self.set_error(e),
        }
    }

    fn launch_profile(&mut self, p: NamedProfile) {
        if p.profile.machine_id.is_empty() {
            self.set_error("This older preset needs a machine. Press 4, select it, and press e.");
            return;
        }
        let request = self.request_id();
        let pr = &p.profile;
        let mut argv = vec![
            "run".to_string(),
            pr.machine_id.clone(),
            "--request-id".into(),
            request.clone(),
            "--provider".into(),
            pr.provider.clone(),
            "--account".into(),
            pr.account.clone(),
            "--cwd".into(),
            pr.cwd.clone(),
            "--model".into(),
            pr.model.clone(),
            "--effort".into(),
            pr.effort.clone(),
            "--prompt".into(),
            pr.prompt.clone(),
            "--attach".into(),
        ];
        argv = crate::launch::with_permissions(argv, &pr.permissions);
        let machine_name = crate::session::machine(&self.state, &pr.machine_id)
            .map(|m| m.name.clone())
            .unwrap_or(pr.machine_id.clone());
        let key = format!("{}:{}:false", pr.machine_id, request);
        let title = format!(
            "{} · {}",
            crate::session::provider_label(&pr.provider),
            machine_name
        );
        let machine = pr.machine_id.clone();
        self.open_terminal_tab(key.clone(), title, argv.clone(), argv, None);
        if let Some(tab) = self.tabs.tabs.iter_mut().find(|t| t.key == key) {
            tab.machine = Some(machine);
        }
    }

    /// Open the settings page (workspace.rs `open_settings`).
    pub fn open_settings(&mut self) {
        self.overlay = Overlay::Settings(SettingsPage::new(self.title_prompt.clone()));
    }

    /// Apply what a settings key or press asked for.
    pub fn apply_settings_effect(&mut self, effect: crate::preferences::Effect) {
        if effect.save_settings {
            let text = self.settings.to_json_pretty();
            self.host(vec!["save-desktop".into()], text);
        }
        if effect.save_prompt {
            if let Overlay::Settings(page) = &self.overlay {
                self.host(vec!["save-title-prompt".into()], page.prompt.clone());
            }
        }
        if effect.reset_theme {
            self.theme = crate::theme::Theme::bundled(self.theme.dark);
            self.settings.theme = None;
            let text = self.settings.to_json_pretty();
            self.host(vec!["save-desktop".into()], text);
            self.set_message(format!("Theme: {} (default)", self.theme.name));
        }
        if effect.open_themes {
            self.open_themes(true);
            return;
        }
        if effect.close {
            self.overlay = Overlay::None;
        }
    }

    /// The palette's rows where the user is (workspace.rs `palette_items`).
    pub fn palette_items(&self) -> Vec<(&'static CommandInfo, i32)> {
        let Overlay::Palette(picker) = &self.overlay else {
            return Vec::new();
        };
        let query = picker.query.trim();
        let selected_machine = if self.page == Page::Machines {
            crate::session::machine_rows(&self.state).nth(self.index)
        } else {
            None
        };
        let tab_session = self.tab_target();
        let mut items: Vec<&'static CommandInfo> = Vec::new();
        if self.tabs.active == 0 {
            items.extend(palette::page_commands(self.page).iter());
            items.extend(palette::MOTION.iter());
        } else if let Some(tab) = self.tabs.active_tab() {
            if let (Some((_, _, false)), Some(item)) = (tab.session.as_ref(), tab_session.as_ref())
            {
                if item.session.managed {
                    if item.session.provider == "codex" {
                        items.extend(palette::TERMINAL.iter());
                    } else if item.session.claude_remote {
                        items.extend(palette::CLAUDE_REMOTE.iter().filter(|info| {
                            info.command != Command::SwitchClaudeWorkerTerminal
                                || !tab.reconnect.iter().any(|arg| arg == "--worker-terminal")
                        }));
                    }
                }
            }
        }
        // A session is usually moved from inside its own terminal, where the
        // Sessions page's row keys are typed into the agent instead.
        if self.tabs.active != 0 && tab_session.is_some() {
            items.extend(palette::TAB_SESSION.iter());
        }
        let imessage_target = tab_session
            .as_ref()
            .is_some_and(|item| can_attach_imessage(&item.session));
        // The docked shell panel is not ported: FocusTerminalPanel never shows.
        let has_panel = false;
        if self.tabs.active != 0 && self.tabs.active_tab().is_some_and(|tab| !tab.is_folder()) {
            items.extend(palette::TAB_TERMINAL.iter());
        }
        items.extend(palette::GLOBAL.iter());
        let codex_target = items
            .iter()
            .any(|info| info.command == Command::SwitchAccount)
            && self
                .command_session()
                .is_some_and(|item| crate::forms::offers_account_switch(&item.session.provider));
        let latest = &self.latest_provider_versions;
        let update = |provider: &str| {
            selected_machine.is_some_and(|machine| {
                crate::session::provider_update_action_available(machine, latest, provider)
            })
        };
        let items: Vec<&'static CommandInfo> = items
            .into_iter()
            .filter(|info| match info.command {
                Command::FocusTerminalPanel => has_panel,
                Command::ToggleSecrets => {
                    self.secrets.demo || self.tabs.active_tab().is_some_and(|t| t.session.is_some())
                }
                Command::SwitchAccount => codex_target,
                Command::AttachIMessage => imessage_target,
                Command::UpdateCodex => update("codex"),
                Command::UpdateClaude => update("claude"),
                Command::UpdateOpenCode => update("opencode"),
                Command::UpdateAntigravity => update("antigravity"),
                Command::UpdateGrok => selected_machine.is_some(),
                _ => true,
            })
            .collect();
        palette::rank(&items, query)
    }

    /// Run the palette's highlighted row.
    pub fn run_palette_selection(&mut self) {
        let items = self.palette_items();
        let Overlay::Palette(picker) = &self.overlay else {
            return;
        };
        let Some((info, _)) = items.get(picker.index) else {
            return;
        };
        let cmd = info.command;
        self.overlay = Overlay::None;
        self.execute(cmd);
    }

    /// The help dialog's scopes: everything, Info last.
    pub fn help_scopes(&self) -> Vec<Scope> {
        Scope::HELP.to_vec()
    }
}

/// A model catalog came back from `fleet models`.
pub fn models_arrived(ws: &mut Workspace, key: CatalogKey, result: Result<String, String>) {
    let parsed = result.and_then(|text| {
        serde_json::from_str::<Vec<crate::launch::ModelOption>>(&text)
            .map_err(|e| format!("models: {e}"))
    });
    match &mut ws.overlay {
        Overlay::Launch(launch) => {
            let _ = launch.set_models(&key, parsed, &ws.state);
        }
        Overlay::Form(form) => form.set_models(&key, parsed),
        _ => {}
    }
    ws.sync_launch_catalogs(false);
    ws.sync_form_io();
}

/// Folder completion came back from `fleet directories`.
pub fn directories_arrived(
    ws: &mut Workspace,
    machine: &str,
    path: &str,
    result: Result<String, String>,
) {
    let parsed = result.and_then(|text| {
        serde_json::from_str::<crate::launch::DirectoryMatches>(&text)
            .map_err(|e| format!("directories: {e}"))
    });
    match &mut ws.overlay {
        Overlay::Launch(launch) => launch.set_directories(machine, path, parsed),
        Overlay::Form(form) => form.set_directories(machine, path, parsed),
        _ => {}
    }
    ws.sync_launch_catalogs(false);
    ws.sync_form_io();
}

/// "{title} · {machine}" (workspace.rs `session_tab_title`), read-only marked.
pub fn session_tab_title(title: &str, machine: &str, read_only: bool) -> String {
    let title = crate::session::clean(title);
    let title = if title.is_empty() {
        "Session".to_string()
    } else {
        title
    };
    if read_only {
        format!("{title} (read-only) · {machine}")
    } else {
        format!("{title} · {machine}")
    }
}

/// Machines the manager lists, for the popup's update items.
pub fn machine_at(ws: &Workspace, index: usize) -> Option<Machine> {
    machine_rows(&ws.state).nth(index).cloned()
}

/// A confirmation's title and yes label (ui.rs `render_overlay`'s confirms).
pub fn exec_confirm_texts(action: ConfirmAction, name: &str) -> (String, &'static str) {
    match action {
        ConfirmAction::Stop => (format!("Terminate {name}?"), "Terminate"),
        ConfirmAction::Delete => (
            format!("Stop and remove {name} from Ocho?"),
            "Stop and remove",
        ),
        ConfirmAction::Interrupt => (format!("Interrupt {name}?"), "Interrupt"),
    }
}

/// The command a rows.rs action id names: rows spell the variant in snake
/// case (`view_read_only`, `update_open_code`, `new`), the palette in the
/// web's words (`view-read-only`, `update-opencode`, `launch`).
pub fn row_command(id: &str) -> Option<Command> {
    let kebab = id.replace('_', "-");
    let mapped = match kebab.as_str() {
        "new" => "launch",
        "update-open-code" => "update-opencode",
        other => other,
    };
    Command::from_id(mapped)
}

/// iMessage can attach to a live Codex app-server conversation
/// (workspace.rs `can_attach_imessage`).
pub fn can_attach_imessage(session: &Session) -> bool {
    session.provider == "codex"
        && (session.eas || (session.pid > 0 && !session.codex_socket.is_empty()))
        && !session.historical
        && !session.native_id.is_empty()
        && !matches!(
            session.state.as_str(),
            "paused" | "exited" | "gone" | "dead"
        )
}

/// `fleet sessions imessage M S --thread-id T` for a session that can take it.
pub fn imessage_attach_command(item: &SessionItem) -> Option<Vec<String>> {
    can_attach_imessage(&item.session).then(|| {
        vec![
            "sessions".into(),
            "imessage".into(),
            item.machine.id.clone(),
            item.session.id.clone(),
            "--thread-id".into(),
            item.session.native_id.clone(),
        ]
    })
}
