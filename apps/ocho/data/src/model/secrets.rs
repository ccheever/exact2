//! The Secrets sidebar and its alerts wired to the model
//! (workspace/secret_requests.rs), and opening a session by id
//! (workspace.rs `session_item`, `open_session_item`,
//! `open_notification_session`).
//!
//! `fleet secret …` replies come back as [`Reply::SecretList`],
//! [`Reply::SecretAlert`], [`Reply::SecretAction`] and [`Reply::SecretLink`].
//! The value typed into the entry travels only as the provide job's stdin.

use super::*;
use crate::finders::{open_session, reveal_message, OpenSession};
use crate::picker::Mods;
use crate::secret_requests::{Requests, SecretKey, Source};

impl Workspace {
    /// The active tab's (machine, session), when it follows a session.
    pub fn active_source(&self) -> Option<Source> {
        let (machine, session, _) = self.tabs.active_tab()?.session.as_ref()?;
        Some((machine.clone(), session.clone()))
    }

    fn attached_sources(&self) -> Vec<Source> {
        self.tabs
            .tabs
            .iter()
            .filter_map(|tab| tab.session.as_ref())
            .map(|(machine, session, _)| (machine.clone(), session.clone()))
            .collect()
    }

    /// Read the open sidebar's list (`refresh_secret_requests`).
    pub fn refresh_secret_requests(&mut self) {
        let active = self.active_source();
        if let Some(argv) = self.secrets.wanted_list(active.as_ref()) {
            if let Some(source) = active {
                self.queue("secrets", argv, String::new(), Reply::SecretList(source));
            }
        }
    }

    /// Read every other attached session's list for alerts
    /// (`refresh_secret_alerts`).
    pub fn refresh_secret_alerts(&mut self) {
        let attached = self.attached_sources();
        let active = self.active_source();
        for (source, argv) in self.secrets.wanted_alerts(&attached, active.as_ref()) {
            self.queue("secrets", argv, String::new(), Reply::SecretAlert(source));
        }
    }

    /// A `fleet secret …` reply.
    pub(super) fn secret_reply(&mut self, what: Reply, result: Result<String, String>) {
        match what {
            Reply::SecretList(source) => {
                let active = self.active_source();
                let epoch = self.epoch_s();
                if self
                    .secrets
                    .set_list(&source, active.as_ref(), result, epoch)
                {
                    self.refresh_secret_requests();
                }
            }
            Reply::SecretAlert(source) => {
                let still_open = self.attached_sources().contains(&source);
                self.secrets.set_alert(&source, still_open, result);
            }
            Reply::SecretAction => {
                self.secrets.set_action(result);
                self.refresh_secret_requests();
            }
            Reply::SecretLink(source) => match Requests::set_link(result) {
                Ok(pending) => {
                    if self.open_notification_session(&source.0, &source.1) {
                        let active = self.active_source();
                        if let Some(argv) = self.secrets.open_from_link(pending, active.as_ref()) {
                            self.queue("secrets", argv, String::new(), Reply::SecretList(source));
                        }
                    }
                }
                Err(message) => self.set_message(message),
            },
            _ => {}
        }
    }

    /// ToggleSecrets (workspace.rs `execute`).
    pub fn toggle_secrets(&mut self) {
        let active = self.active_source();
        self.secrets.toggle(active.as_ref());
        if self.secrets.open {
            self.refresh_secret_requests();
            if self.secrets.focused {
                self.focus_id = "secret-demo-input".into();
            }
        }
    }

    /// A clicked `ocho://secret/NAME` in a session's terminal.
    pub fn open_terminal_secret_link(&mut self, tab_key: &str, uri: &str) {
        let Some(name) = crate::secret_requests::secret_name(uri) else {
            return;
        };
        let Some(source) = self
            .tabs
            .tabs
            .iter()
            .find(|tab| tab.key == tab_key)
            .and_then(|tab| tab.session.as_ref())
            .map(|(m, s, _)| (m.clone(), s.clone()))
        else {
            return;
        };
        let argv = Requests::wanted_link(&source, name);
        self.queue("secrets", argv, String::new(), Reply::SecretLink(source));
    }

    fn submit_secret(&mut self) {
        let epoch = self.epoch_s();
        if let Some(command) = self.secrets.submit(epoch) {
            self.queue("secrets", command.argv, command.stdin, Reply::SecretAction);
        }
    }

    /// A key while the secret entry has the keyboard; `true` when it was taken.
    pub(super) fn secrets_key(&mut self, name: &str, mods: &Mods) -> bool {
        match self.secrets.key(name, mods) {
            SecretKey::None => false,
            SecretKey::Cleared | SecretKey::Swallowed => true,
            SecretKey::Submit => {
                self.submit_secret();
                true
            }
        }
    }

    /// The sidebar's and the alert's presses; `true` when `id` was one.
    pub(super) fn secrets_press(&mut self, id: &str) -> bool {
        if id == "secret-requests-close" {
            self.secrets.close();
        } else if id == "secret-alert-dismiss" {
            self.secrets.dismiss_alert();
        } else if id == "secret-alert-open" {
            let Some(alert) = self.secrets.alerts.front().cloned() else {
                return true;
            };
            let source = (alert.machine.clone(), alert.session.clone());
            if self.open_notification_session(&source.0, &source.1) {
                let active = self.active_source();
                if let Some(argv) = self.secrets.open_from_alert(&source, active.as_ref()) {
                    self.queue("secrets", argv, String::new(), Reply::SecretList(source));
                }
            }
        } else if id == "secret-demo-reset" {
            self.secrets.reset_demo();
        } else if id == "secret-demo-input" {
            self.secrets.focused = true;
        } else if id == "secret-blur" {
            self.secrets.blur();
        } else if let Some(seconds) = id.strip_prefix("secret-lifetime-") {
            if let Ok(seconds) = seconds.parse() {
                self.secrets.set_lifetime(seconds);
            }
        } else if let Some(request) = id.strip_prefix("secret-open-") {
            self.secrets.edit(request);
            self.focus_id = "secret-demo-input".into();
        } else if id.strip_prefix("secret-provide-").is_some() {
            self.submit_secret();
        } else if id.strip_prefix("secret-cancel-").is_some() {
            self.secrets.clear_draft();
        } else if let Some(request) = id.strip_prefix("secret-dismiss-") {
            if let Some(argv) = self.secrets.finish(request, "dismiss") {
                self.queue("secrets", argv, String::new(), Reply::SecretAction);
            }
        } else if let Some(request) = id.strip_prefix("secret-revoke-") {
            if let Some(argv) = self.secrets.finish(request, "revoke") {
                self.queue("secrets", argv, String::new(), Reply::SecretAction);
            }
        } else {
            return false;
        }
        true
    }

    /// The sidebar's view for the active tab.
    pub fn secrets_view(&self) -> serde_json::Value {
        let active = self.active_source();
        self.secrets.view(active.as_ref(), self.epoch_s())
    }

    /// The session `machine_id`/`session_id` as the feed has it now.
    pub fn session_item(&self, machine_id: &str, session_id: &str) -> Option<SessionItem> {
        let machine = self.state.machines.iter().find(|m| m.id == machine_id)?;
        let session = machine
            .last
            .as_ref()?
            .sessions
            .iter()
            .find(|s| s.id == session_id)?;
        Some(SessionItem {
            machine: machine.clone(),
            session: session.clone(),
        })
    }

    /// Attach when the session still has a terminal; otherwise reveal it in
    /// the Sessions list (with archived or historical rows as needed) so `o`
    /// can resume it.
    pub fn open_session_item(&mut self, item: SessionItem) {
        match open_session(&item.session) {
            OpenSession::Attach => self.attach_item(item, false),
            OpenSession::Reveal {
                all_sessions,
                history,
            } => {
                self.all_sessions |= all_sessions;
                self.history |= history;
                self.tabs.select_manager();
                self.nav = false;
                self.set_page(Page::Sessions);
                let index = self
                    .sessions()
                    .iter()
                    .position(|row| {
                        row.session.id == item.session.id && row.machine.id == item.machine.id
                    })
                    .unwrap_or(0);
                self.select(index);
                self.set_message(reveal_message(&item.session.title));
            }
        }
    }

    /// Show a session named by a notification or a secret request: its open
    /// tab if there is one, else open it. `false` when it is gone.
    pub fn open_notification_session(&mut self, machine_id: &str, session_id: &str) -> bool {
        if let Some(pos) = self.tabs.tabs.iter().position(|tab| {
            tab.session
                .as_ref()
                .is_some_and(|(m, s, _)| m == machine_id && s == session_id)
        }) {
            self.tabs.select(pos);
            self.nav = false;
            self.tabs.reveal_active();
        } else if let Some(item) = self.session_item(machine_id, session_id) {
            self.open_session_item(item);
        } else {
            return false;
        }
        self.overlay = Overlay::None;
        true
    }
}
