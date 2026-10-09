//! The shell docked under a terminal tab (workspace.rs
//! `toggle_terminal_panel`, `close_terminal_panel`, `focus_panel`,
//! `resize_panel`, `panel_shell_args`; ui.rs `render_terminal_tab`,
//! `render_terminal_panel`). The panel's terminal is its own
//! `<ghostty-terminal>` under the tab key plus [`PANEL_SUFFIX`]; a shell that
//! exits takes its panel with it.

use super::*;

/// The panel's height before it is dragged, px.
pub const DEFAULT_PANEL_HEIGHT: f64 = 260.0;
/// The panel never gets shorter than this, px.
pub const MIN_PANEL_HEIGHT: f64 = 96.0;
/// The agent keeps at least this much above the panel, px.
pub const MIN_TAB_HEIGHT: f64 = 160.0;
/// The panel terminal's key is the tab's plus this.
pub const PANEL_SUFFIX: &str = "#panel";

/// A tab's docked shell (workspace.rs `TerminalPanel`).
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    /// `connect M [--cwd C] [--persistent S]`.
    pub argv: Vec<String>,
    /// Keys and pastes go to the panel instead of the agent while set.
    pub focused: bool,
    /// Where the shell was asked to start, for the header.
    pub cwd: Option<String>,
}

/// `fleet connect` for a shell on `machine`, started in `cwd` when the
/// session reports one; session panels reattach their tmux shell by id.
pub fn panel_shell_args(machine: &str, cwd: Option<&str>, session: Option<&str>) -> Vec<String> {
    let mut args = vec!["connect".to_string(), machine.to_string()];
    if let Some(cwd) = cwd.map(str::trim).filter(|cwd| !cwd.is_empty()) {
        args.push("--cwd".into());
        args.push(cwd.to_string());
    }
    if let Some(id) = session {
        args.push("--persistent".into());
        args.push(id.to_string());
    }
    args
}

/// The panel's height within a tab body `body` px tall.
pub fn clamp_panel_height(height: f64, body: f64) -> f64 {
    let max = (body - MIN_TAB_HEIGHT).max(MIN_PANEL_HEIGHT);
    height.clamp(MIN_PANEL_HEIGHT, max)
}

impl Workspace {
    /// The active tab has a docked shell.
    pub fn active_panel(&self) -> Option<&Panel> {
        self.panels.get(&self.tabs.active_tab()?.key)
    }

    /// ToggleTerminalPanel.
    pub fn toggle_terminal_panel(&mut self) {
        let Some(tab) = self.tabs.active_tab().cloned() else {
            self.set_message("Terminal panels belong to terminal tabs");
            return;
        };
        if self.panels.contains_key(&tab.key) {
            self.close_terminal_panel();
            return;
        }
        let Some(machine) = tab
            .session
            .as_ref()
            .map(|s| s.0.clone())
            .or_else(|| tab.machine.clone())
        else {
            self.set_error("This tab does not belong to a machine");
            return;
        };
        let cwd = self
            .tab_target()
            .map(|item| item.session.cwd.trim().to_string())
            .filter(|cwd| !cwd.is_empty());
        let session = tab.session.as_ref().map(|(_, id, _)| id.as_str());
        let argv = panel_shell_args(&machine, cwd.as_deref(), session);
        let message = match &cwd {
            Some(cwd) => format!("Terminal panel opened in {cwd}"),
            None => "Terminal panel opened".to_string(),
        };
        self.panels.insert(
            tab.key,
            Panel {
                argv,
                focused: true,
                cwd,
            },
        );
        self.nav = false;
        self.set_message(message);
    }

    /// The panel's "×" (`close_terminal_panel`).
    pub fn close_terminal_panel(&mut self) {
        let Some(key) = self.tabs.active_tab().map(|t| t.key.clone()) else {
            return;
        };
        if self.panels.remove(&key).is_some() {
            self.host(
                vec!["close-terminal".into(), format!("{key}{PANEL_SUFFIX}")],
                String::new(),
            );
            self.set_message("Terminal panel closed");
        }
    }

    /// FocusTerminalPanel: typing moves between the agent and the panel.
    pub fn focus_terminal_panel(&mut self) {
        let Some(key) = self.tabs.active_tab().map(|t| t.key.clone()) else {
            return;
        };
        match self.panels.get_mut(&key) {
            Some(panel) => {
                panel.focused = !panel.focused;
                self.nav = false;
            }
            None => self.set_message("No terminal panel on this tab"),
        }
    }

    /// A click in the panel (`true`) or the agent above it (`focus_panel`).
    pub(super) fn focus_panel(&mut self, panel: bool) {
        if let Some(key) = self.tabs.active_tab().map(|t| t.key.clone()) {
            if let Some(current) = self.panels.get_mut(&key) {
                current.focused = panel;
            }
        }
    }

    /// The panel's terminal reported (`exited…`: the shell ended and the
    /// panel goes; the tab is untouched).
    pub(super) fn panel_report(&mut self, message: &str) {
        if message == "exited" || message.starts_with("exited:") {
            if let Some(key) = self.tabs.active_tab().map(|t| t.key.clone()) {
                if self.panels.remove(&key).is_some() {
                    self.host(
                        vec!["close-terminal".into(), format!("{key}{PANEL_SUFFIX}")],
                        String::new(),
                    );
                }
            }
        }
    }

    /// The resize handle moved by `dy` (up is taller).
    pub(super) fn resize_panel(&mut self, dy: f64) {
        let body = self.window.1;
        self.panel_height = clamp_panel_height(self.panel_height - dy, body);
    }

    /// The panel's `TabView` fields.
    pub fn panel_view(&self) -> serde_json::Value {
        let Some(tab) = self.tabs.active_tab() else {
            return serde_json::json!({ "panelOpen": false });
        };
        let Some(panel) = self.panels.get(&tab.key) else {
            return serde_json::json!({ "panelOpen": false, "panelHeight": self.panel_height });
        };
        let label = match &panel.cwd {
            Some(cwd) => format!("Terminal · {cwd}"),
            None => "Terminal".to_string(),
        };
        serde_json::json!({
            "panelOpen": true,
            "panelKey": format!("{}{PANEL_SUFFIX}", tab.key),
            "panelArgv": panel.argv,
            "panelArgvJson": serde_json::to_string(&panel.argv).unwrap_or_default(),
            "panelCwd": panel.cwd.clone().unwrap_or_default(),
            "panelLabel": label,
            "panelHeight": self.panel_height,
            "panelFocused": panel.focused && !self.nav,
        })
    }
}
