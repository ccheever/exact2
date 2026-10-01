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

    /// The disconnected scrim covers the grid (ui.rs `render_tab_body`):
    /// SSH dropped, or a remote session's terminal exited with an error.
    /// Returns (covered, reconnecting).
    pub fn connection_scrim(&self) -> (bool, bool) {
        let Some(tab) = self.tabs.active_tab() else {
            return (false, false);
        };
        let connection = self.connections.get(&tab.key);
        let remote_machine = tab
            .machine
            .as_deref()
            .and_then(|id| self.state.machines.iter().find(|m| m.id == id))
            .is_some_and(|m| !m.local);
        let remote = connection.is_some() || remote_machine;
        let failed = self.exit_codes.get(&tab.key).is_some_and(|code| *code != 0);
        let covered = connection.is_some_and(Connection::conceals_terminal)
            || (remote && tab.session.is_some() && failed);
        (
            covered,
            connection.is_some_and(|c| c.state == "reconnecting"),
        )
    }
}
