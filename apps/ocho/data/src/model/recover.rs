//! Terminal-tab recovery wired to the model (workspace.rs
//! `assess_recovery`, `launch_active_recovery`,
//! `recover_active_exited_session`, the `Recovery*` and `UnpauseTab`
//! arms): each tab's [`Recovery`] lives here by tab key, its `fleet
//! recovery` replies come back as [`Reply::Recovery`], and a reattach or
//! relaunch re-keys the tab, whose new argv the host then starts.

use super::exec::ConfirmAction;
use super::*;
use crate::recovery::{self, Assessed, LaunchMode, Recovery};

impl Workspace {
    /// The observed (machine, session) of tab `pos`.
    fn tab_observed(&self, pos: usize) -> Option<SessionItem> {
        let (machine, session, _) = self.tabs.tabs.get(pos)?.session.as_ref()?;
        self.session_item(machine, session)
    }

    /// Ask `fleet recovery` about tab `pos` (`assess_recovery`).
    pub fn assess_recovery(&mut self, pos: usize, continue_search: bool) {
        let Some(tab) = self.tabs.tabs.get(pos) else {
            return;
        };
        let Some((machine, session, read_only)) = tab.session.clone() else {
            return;
        };
        let key = tab.key.clone();
        let entry = self.recoveries.entry(key.clone()).or_default();
        if let Some(argv) = entry.assess(&machine, &session, continue_search) {
            self.queue(
                "recovery",
                argv,
                String::new(),
                Reply::Recovery {
                    key,
                    machine,
                    read_only,
                },
            );
        }
    }

    /// A `fleet recovery` reply for the tab that was `key`.
    pub(super) fn recovery_arrived(
        &mut self,
        key: &str,
        machine: &str,
        read_only: bool,
        result: Result<String, String>,
    ) {
        let Some(pos) = self.tabs.position(key) else {
            return;
        };
        let active = self.tabs.active == pos + 1;
        let now = self.now;
        let Some(entry) = self.recoveries.get_mut(key) else {
            return;
        };
        match entry.set_assessment(result, machine, read_only, active, now) {
            Assessed::Shown => {}
            Assessed::Reattach {
                session,
                key: new_key,
                argv,
            } => {
                self.rekey_tab(
                    pos,
                    &new_key,
                    argv,
                    (machine.to_string(), session, read_only),
                );
                self.set_message(recovery::REATTACHED);
                self.persist_tabs();
            }
            Assessed::Launch(mode) => self.launch_recovery(pos, mode),
        }
    }

    /// Move tab `pos` to `key` with a new terminal `argv` and session: the
    /// old terminal goes, and the view starts the new one.
    fn rekey_tab(
        &mut self,
        pos: usize,
        key: &str,
        argv: Vec<String>,
        session: (String, String, bool),
    ) {
        let old = self.tabs.tabs[pos].key.clone();
        let mut recovery = self.recoveries.remove(&old).unwrap_or_default();
        if old != key {
            self.host(vec!["close-terminal".into(), old.clone()], String::new());
            self.transcripts.remove(&old);
            self.transcript_scroll.remove(&old);
        } else {
            self.host(
                vec!["reconnect-terminal".into(), old.clone()],
                String::new(),
            );
        }
        self.exited.remove(&old);
        self.exit_codes.remove(&old);
        self.connections.remove(&old);
        let tab = &mut self.tabs.tabs[pos];
        tab.key = key.to_string();
        tab.reconnect = argv;
        tab.machine = Some(session.0.clone());
        tab.session = Some(session);
        recovery.launched();
        self.recoveries.insert(key.to_string(), recovery);
    }

    /// Relaunch tab `pos` in `mode` (`launch_active_recovery`).
    fn launch_recovery(&mut self, pos: usize, mode: LaunchMode) {
        let Some(tab) = self.tabs.tabs.get(pos) else {
            return;
        };
        let Some((machine, _, read_only)) = tab.session.clone() else {
            return;
        };
        let key = tab.key.clone();
        let request = self.request_id();
        let Some(relaunch) = self
            .recoveries
            .entry(key)
            .or_default()
            .launch(mode, &machine, &request, read_only)
        else {
            return;
        };
        self.rekey_tab(pos, &relaunch.key, relaunch.argv, relaunch.session);
        self.set_message(relaunch.message);
        self.persist_tabs();
    }

    /// The active tab's position, when it is a terminal.
    fn active_pos(&self) -> Option<usize> {
        self.tabs.active.checked_sub(1)
    }

    /// The `Recovery*` commands; `true` when `cmd` was one.
    pub(super) fn recovery_command(&mut self, cmd: Command) -> bool {
        let Some(pos) = self.active_pos() else {
            return matches!(
                cmd,
                Command::RecoveryContinue
                    | Command::RecoveryRestore
                    | Command::RecoveryFork
                    | Command::RecoveryReplace
                    | Command::RecoveryDetails
            );
        };
        match cmd {
            Command::RecoveryContinue => self.assess_recovery(pos, true),
            Command::RecoveryRestore => self.launch_recovery(pos, LaunchMode::Resume),
            Command::RecoveryReplace => self.launch_recovery(pos, LaunchMode::Replace),
            Command::RecoveryFork => {
                let key = self.tabs.tabs[pos].key.clone();
                if self
                    .recoveries
                    .get(&key)
                    .is_some_and(|r| r.state.can_fork())
                {
                    let tab = self.tabs.tabs[pos].clone();
                    let item = self.tab_observed(pos).unwrap_or_else(|| {
                        // Not observed: a session built from the tab's own ids.
                        let (machine, session, _) = tab.session.clone().unwrap_or_default();
                        let mut item = SessionItem::default();
                        item.machine.id = machine.clone();
                        item.machine.name = machine;
                        item.session.id = session;
                        item.session.title = tab.title.clone();
                        item
                    });
                    self.overlay = Overlay::Confirm {
                        action: ConfirmAction::ForkRecovery,
                        item,
                    };
                }
            }
            Command::RecoveryDetails => {
                let key = self.tabs.tabs[pos].key.clone();
                self.recoveries.entry(key).or_default().toggle_details();
            }
            _ => return false,
        }
        true
    }

    /// The fork confirmation's yes.
    pub(super) fn confirm_fork_recovery(&mut self) {
        if let Some(pos) = self.active_pos() {
            self.launch_recovery(pos, LaunchMode::Fork);
        }
    }

    /// UnpauseTab (workspace.rs): `unpause M S`, the tab marked resuming.
    pub(super) fn unpause_tab(&mut self) {
        let Some(pos) = self.active_pos() else {
            return;
        };
        let observed = self.tab_observed(pos);
        match recovery::unpause_argv(observed.as_ref().map(|i| (&i.machine, &i.session))) {
            Ok(argv) => {
                let key = self.tabs.tabs[pos].key.clone();
                self.resuming_tabs.insert(key);
                self.run_cli(argv, recovery::UNPAUSE_DONE);
            }
            Err(message) => self.set_message(message),
        }
    }

    /// Reconnect (workspace.rs `reconnect_tab`): retry the transport in
    /// place, else assess a recoverable session, else start the frozen
    /// argv again.
    pub fn reconnect_tab(&mut self, pos: usize) {
        let key = self.tabs.tabs[pos].key.clone();
        if self.retry_connection(&key) {
            return;
        }
        let observed = self.tab_observed(pos);
        if recovery::reconnect_assesses(observed.as_ref().map(|i| &i.session)) {
            self.assess_recovery(pos, false);
            return;
        }
        self.exited.remove(&key);
        self.exit_codes.remove(&key);
        self.connections.remove(&key);
        self.host(vec!["reconnect-terminal".into(), key], String::new());
        self.set_message("Reconnecting terminal…");
    }

    /// Tab `key`'s terminal exited: a recoverable session is assessed
    /// (workspace.rs `observe_terminal`).
    pub(super) fn terminal_exited(&mut self, key: &str) {
        let Some(pos) = self.tabs.position(key) else {
            return;
        };
        let observed = self.tab_observed(pos);
        let recovery = self.recoveries.get(key).cloned().unwrap_or_default();
        if recovery::should_assess_on_exit(observed.as_ref().map(|i| &i.session), &recovery) {
            self.assess_recovery(pos, false);
        }
    }

    /// The active tab's agent may have exited under a live terminal, or its
    /// machine came back (`recover_active_exited_session`).
    pub fn recover_active_exited_session(&mut self) {
        let Some(pos) = self.active_pos() else {
            return;
        };
        let tab = &self.tabs.tabs[pos];
        let observed = self.tab_observed(pos);
        let recovery = self.recoveries.get(&tab.key).cloned().unwrap_or_default();
        if recovery::should_check_active(
            tab.session.as_ref(),
            observed.as_ref().map(|i| (&i.machine, &i.session)),
            &recovery,
            self.now,
        ) {
            self.assess_recovery(pos, false);
        }
    }

    /// A machine was observed: resumes that finished leave the set.
    pub(super) fn settle_resuming(&mut self, machine: &crate::types::Machine) {
        let tabs = &self.tabs.tabs;
        self.resuming_tabs.retain(|key| {
            tabs.iter()
                .find(|tab| &tab.key == key)
                .is_some_and(|tab| recovery::still_resuming(tab.session.as_ref(), machine))
        });
    }

    /// The overlay, banners and cards for the active tab.
    pub fn tab_chrome(&self) -> serde_json::Value {
        let Some(tab) = self.tabs.active_tab() else {
            return serde_json::json!({});
        };
        let none = Recovery::default();
        let recovery = self.recoveries.get(&tab.key).unwrap_or(&none);
        let observed = tab
            .session
            .as_ref()
            .and_then(|(m, s, _)| self.session_item(m, s));
        let connection = self.connections.get(&tab.key).map(|c| {
            (
                c.state.as_str(),
                c.transport.as_str(),
                c.retryable,
                c.detail.clone(),
            )
        });
        let remote_machine = tab
            .machine
            .as_deref()
            .and_then(|id| self.state.machines.iter().find(|m| m.id == id))
            .is_some_and(|m| !m.local);
        let chrome = recovery::Chrome {
            recovery,
            observed: observed.as_ref().map(|i| (&i.machine, &i.session)),
            has_session: tab.session.is_some(),
            remote_machine,
            connection,
            exited: self.exited.contains(&tab.key),
            exit_code: self.exit_codes.get(&tab.key).copied(),
            resuming: self.resuming_tabs.contains(&tab.key),
        };
        recovery::tab_chrome(&chrome, &|cmd| self.hint(cmd))
    }

    /// Recovery is showing on the active tab (the strip stays hidden).
    pub fn active_recovering(&self) -> bool {
        self.tabs
            .active_tab()
            .and_then(|tab| self.recoveries.get(&tab.key))
            .is_some_and(|r| r.state != recovery::RecoveryState::None)
    }

    /// The exit banner's buttons (ui.rs): Reconnect, Close tab, Ocho
    /// manager, each with its key hint where the user is.
    pub fn tab_view_exit_buttons(&self) -> Vec<serde_json::Value> {
        [
            ("reconnect", "Reconnect", Command::ReconnectTab, true),
            ("close", "Close tab", Command::CloseTab, false),
            ("manager", "Ocho manager", Command::SelectTab(0), false),
        ]
        .into_iter()
        .map(|(id, label, cmd, primary)| {
            serde_json::json!({
                "id": id, "label": label, "hint": self.hint(cmd), "primary": primary, "disabled": false,
            })
        })
        .collect()
    }
}
