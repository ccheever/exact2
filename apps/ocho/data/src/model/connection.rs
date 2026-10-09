//! A terminal's transport and exit, as its `<ghostty-terminal>` reports them
//! (terminal/connection.rs `Connection`, ui.rs's connection strip,
//! workspace.rs `reconnect_tab`). The host relays `fleet attach`'s
//! `{"transport","state","retryable"}` datagrams as `connection:{json}`
//! and the exit as `exited` or `exited:CODE`.
//!
//! Mosh's own notice row (`observe_mosh`, read from the grid's top line) is
//! not watched: libghostty does not hand the host its cells.

use super::*;
use crate::session::session_state;
use serde::Deserialize;

/// One transport report.
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Event {
    transport: String,
    state: String,
    #[serde(default)]
    retryable: bool,
}

/// A tab's transport state (terminal/connection.rs `Connection`).
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    /// `ssh` or `mosh`.
    pub transport: String,
    /// `connecting | connected | reconnecting | disconnected | ended`.
    pub state: String,
    /// Retry can reconnect now.
    pub retryable: bool,
    /// When the state began, host ms.
    pub since: f64,
    /// A note that replaces the elapsed-time description.
    pub detail: String,
}

impl Connection {
    fn waiting(&self) -> bool {
        matches!(self.state.as_str(), "connecting" | "reconnecting")
    }

    /// SSH can leave a stale provider error screen after the connection
    /// drops; Mosh keeps useful local prediction while offline.
    pub fn conceals_terminal(&self) -> bool {
        self.transport == "ssh" && matches!(self.state.as_str(), "disconnected" | "reconnecting")
    }

    fn label(&self) -> &'static str {
        match self.state.as_str() {
            "connecting" => "Connecting…",
            "connected" => "Connected",
            "reconnecting" => "Reconnecting…",
            "disconnected" => "Disconnected",
            _ => "Connection ended",
        }
    }

    fn description(&self, now: f64) -> String {
        if !self.detail.is_empty() {
            return self.detail.clone();
        }
        if !self.waiting() {
            return String::new();
        }
        let input = if self.transport == "mosh" {
            "Mosh prediction remains available"
        } else {
            "SSH input resumes when attached"
        };
        format!(
            "{}s · {input}",
            ((now - self.since).max(0.0) / 1000.0) as u64
        )
    }

    fn color(&self, t: &crate::theme::Theme) -> String {
        match self.state.as_str() {
            "connecting" | "reconnecting" => t.warn.css(),
            "disconnected" => t.danger.css(),
            "connected" => t.good.css(),
            _ => t.muted.css(),
        }
    }
}

const ONLINE: &str = "Machine online";

/// Native Claude Remote attaches through a local client with no transport
/// events; the strip shows the worker machine's observation instead (ui.rs
/// `claude_machine_status`): (label, colour).
fn claude_machine_status(
    machine: &crate::types::Machine,
    session: &crate::types::Session,
    viewer_exited: bool,
    now: f64,
    t: &crate::theme::Theme,
) -> Option<(&'static str, String)> {
    if !session.claude_remote || machine.local {
        return None;
    }
    Some(if viewer_exited {
        ("Connection ended", t.muted.css())
    } else if !machine.error.is_empty() {
        ("Machine unreachable", t.danger.css())
    } else if session.pid > 0
        && matches!(
            session_state(session),
            "RUNNING" | "BLOCKED" | "LIMITED" | "IDLE"
        )
        && session.state != "starting"
        && machine
            .last
            .as_ref()
            .is_some_and(|last| last.live_inventory)
        && crate::session::session_observation_available(machine, session, now)
    {
        (ONLINE, t.good.css())
    } else {
        ("Checking machine…", t.warn.css())
    })
}

impl Workspace {
    /// A message from tab `key`'s terminal view.
    pub(super) fn terminal_report(&mut self, key: &str, message: &str) {
        if message == "exited" || message.starts_with("exited:") {
            self.exited.insert(key.to_string());
            if let Some(code) = message.strip_prefix("exited:").and_then(|c| c.parse().ok()) {
                self.exit_codes.insert(key.to_string(), code);
            }
            self.terminal_exited(key);
        } else if message == "focus" {
            // A click on the agent takes typing back from the docked panel.
            self.focus_panel(false);
        } else if let Some(json) = message.strip_prefix("connection:") {
            let Ok(event) = serde_json::from_str::<Event>(json) else {
                return;
            };
            let now = self.now;
            let changed = self.connections.get(key).is_none_or(|c| {
                c.transport != event.transport
                    || c.state != event.state
                    || c.retryable != event.retryable
            });
            if changed {
                self.connections.insert(
                    key.to_string(),
                    Connection {
                        transport: event.transport,
                        state: event.state,
                        retryable: event.retryable,
                        since: now,
                        detail: String::new(),
                    },
                );
            }
        }
    }

    /// Retry or Reconnect can act (workspace.rs `terminal_can_reconnect`).
    pub fn terminal_can_reconnect(&self, key: &str) -> bool {
        self.exited.contains(key) || self.connections.get(key).is_some_and(|c| c.retryable)
    }

    /// A retryable transport is asked to reconnect in place
    /// (terminal.rs `retry_connection`); `true` when it was.
    pub(super) fn retry_connection(&mut self, key: &str) -> bool {
        let now = self.now;
        let Some(connection) = self.connections.get_mut(key).filter(|c| c.retryable) else {
            return false;
        };
        connection.state = "reconnecting".into();
        connection.retryable = false;
        connection.since = now;
        connection.detail.clear();
        self.host(
            vec!["retry-connection".into(), key.to_string()],
            String::new(),
        );
        true
    }

    /// The connection strip's fields for the active tab (ui.rs, 30 tall):
    /// `strip` is "" when none shows.
    pub fn connection_strip(&self, recovering: bool) -> serde_json::Value {
        let none = serde_json::json!({
            "strip": "", "stripLabel": "", "stripDetail": "", "stripColor": "", "stripRetry": false,
        });
        let Some(tab) = self.tabs.active_tab() else {
            return none;
        };
        let t = &self.theme;
        let exited = self.exited.contains(&tab.key);
        let connection = self.connections.get(&tab.key);
        let item = self.tab_target();
        let claude = item.as_ref().and_then(|item| {
            claude_machine_status(&item.machine, &item.session, exited, self.now, t)
        });
        let remote_machine = tab
            .machine
            .as_deref()
            .and_then(|id| self.state.machines.iter().find(|m| m.id == id))
            .is_some_and(|m| !m.local);
        let remote = connection.is_some() || remote_machine;
        let shows = remote
            && !recovering
            && (exited
                || connection.is_some_and(|c| c.state != "connected")
                || claude.as_ref().is_some_and(|(label, _)| *label != ONLINE));
        if !shows {
            return none;
        }
        let (label, color) = match (connection, &claude) {
            (Some(c), _) => (c.label(), c.color(t)),
            (None, Some((label, color))) => (*label, color.clone()),
            (None, None) => (
                if exited {
                    "Connection ended"
                } else {
                    "Remote terminal"
                },
                t.muted.css(),
            ),
        };
        serde_json::json!({
            "strip": "connection",
            "stripLabel": label,
            "stripDetail": connection.map(|c| c.description(self.now)).unwrap_or_default(),
            "stripColor": color,
            "stripRetry": self.terminal_can_reconnect(&tab.key),
        })
    }
}

impl Workspace {
    /// Managed Codex app-server and Claude Remote viewers learn the theme
    /// only when they start: start them again (workspace.rs
    /// `refresh_remote_theme_tabs`). Worker terminals answer theme reports
    /// themselves.
    pub fn refresh_remote_theme_tabs(&mut self) {
        let keys: Vec<String> = self
            .tabs
            .tabs
            .iter()
            .filter(|tab| matches!(tab.session, Some((_, _, false))))
            .filter(|tab| !tab.reconnect.iter().any(|arg| arg == "--worker-terminal"))
            .filter(|tab| !self.exited.contains(&tab.key))
            .filter(|tab| {
                let (machine, session, _) = tab.session.as_ref().unwrap();
                self.session_item(machine, session).is_some_and(|item| {
                    item.session.managed
                        && (item.session.claude_remote || !item.session.codex_socket.is_empty())
                })
            })
            .map(|tab| tab.key.clone())
            .collect();
        for key in keys {
            self.host(vec!["reconnect-terminal".into(), key], String::new());
        }
    }

    /// RefreshSessionTheme (workspace.rs `refresh_active_session_theme`).
    pub fn refresh_active_session_theme(&mut self) {
        let item = self.tabs.active_tab().and_then(|tab| {
            let (machine, session, read_only) = tab.session.as_ref()?;
            if *read_only {
                return None;
            }
            self.session_item(machine, session)
        });
        let Some(item) = item else {
            self.set_error("This session is not available for a theme refresh");
            return;
        };
        if item.session.managed
            && (item.session.claude_remote || !item.session.codex_socket.is_empty())
        {
            let tab = self.tabs.active_tab().cloned().unwrap_or_default();
            if self.exited.contains(&tab.key) {
                self.set_error("Reconnect the terminal before refreshing its theme");
            } else if tab.reconnect.iter().any(|arg| arg == "--worker-terminal") {
                self.set_message("Worker terminal theme refreshed");
            } else {
                self.host(vec!["reconnect-terminal".into(), tab.key], String::new());
                self.set_message("Session theme refreshed");
            }
            return;
        }
        if item.session.provider != "codex" || !item.session.managed {
            self.set_error("Theme refresh is only available for Ocho-managed Codex sessions");
            return;
        }
        if item.session.state != "idle" {
            self.set_error(
                "Theme refresh is available when the current turn is idle, so it cannot interrupt the agent.",
            );
            return;
        }
        self.run_cli(
            vec![
                "refresh-theme".into(),
                item.machine.id,
                item.session.id,
                "--yes".into(),
            ],
            "Session theme refreshed",
        );
    }

    /// SwitchClaudeWorkerTerminal: the tab attaches to Claude Remote's worker
    /// pane instead (workspace.rs `switch_claude_worker_terminal`).
    pub fn switch_claude_worker_terminal(&mut self) {
        let Some(pos) = self.tabs.active.checked_sub(1) else {
            return;
        };
        let Some((machine, session, false)) =
            self.tabs.tabs.get(pos).and_then(|t| t.session.clone())
        else {
            return;
        };
        let Some(item) = self.session_item(&machine, &session) else {
            return;
        };
        if !item.session.claude_remote || !item.session.managed || item.session.tmux_pane.is_empty()
        {
            self.set_error("This tab has no Claude Remote Control worker terminal");
            return;
        }
        let mut args = crate::tab_tree::session_attach_command(&machine, &session, false);
        args.push("--worker-terminal".into());
        let key = self.tabs.tabs[pos].key.clone();
        self.tabs.tabs[pos].reconnect = args;
        // The view hands the terminal its new argv, which starts it again.
        self.exited.remove(&key);
        self.exit_codes.remove(&key);
        self.connections.remove(&key);
        self.set_message("Attached to the Claude worker terminal");
        self.persist_tabs();
    }
}
