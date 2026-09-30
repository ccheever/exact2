//! Popups (workspace.rs `popup_items_for`, `open_popup`, `run_popup_item`):
//! the row "⋯" menu, the rail tab menu, "View ▾", and the launch dialog's
//! account and permissions dropdowns.

use super::{Overlay, Workspace};
use crate::keymap::{self, Scope};
use crate::palette::Command;
use crate::picker::{self, PopupAction, PopupItem, TabAction};

pub use crate::picker::{Popup, PopupKind};

impl Workspace {
    /// The rows a popup shows.
    pub fn popup_items_for(&self, kind: PopupKind) -> Vec<PopupItem> {
        match kind {
            PopupKind::RowMenu(ix) => self.row_menu_items(ix),
            PopupKind::TabMenu(pos) => self.tab_menu_items(pos),
            PopupKind::ViewMenu => vec![
                PopupItem::choice(
                    "Untracked & archived",
                    self.all_sessions,
                    PopupAction::Command(Command::ToggleOthers.id()),
                )
                .with_hint(self.hint(Command::ToggleOthers)),
                PopupItem::choice(
                    "History",
                    self.history,
                    PopupAction::Command(Command::ToggleHistory.id()),
                )
                .with_hint(self.hint(Command::ToggleHistory)),
                PopupItem::choice(
                    "Hide non-running",
                    self.hide_non_running,
                    PopupAction::Command(Command::ToggleNonRunning.id()),
                )
                .with_hint(self.hint(Command::ToggleNonRunning)),
            ],
            PopupKind::FieldChoice(_) | PopupKind::LaunchAccount | PopupKind::LaunchPermissions => {
                match &self.overlay {
                    Overlay::Launch(launch) => launch.popup_items(kind, &self.state),
                    _ => Vec::new(),
                }
            }
        }
    }

    /// The rows of the open popup.
    pub fn popup_items(&self) -> Vec<PopupItem> {
        match self.popup {
            Some(popup) => self.popup_items_for(popup.kind),
            None => Vec::new(),
        }
    }

    fn row_menu_items(&self, ix: usize) -> Vec<PopupItem> {
        let archived = self
            .sessions()
            .get(ix)
            .is_some_and(|item| item.session.archived);
        let machine = super::exec::machine_at(self, ix);
        crate::rows::menu(
            self.page,
            archived,
            machine.as_ref(),
            &self.latest_provider_versions,
        )
        .into_iter()
        .map(|item| {
            let id = item.id.replace('_', "-");
            let hint = Command::from_id(&id)
                .map(|c| self.hint(c))
                .unwrap_or_default();
            PopupItem::command(item.label, hint, icon_name(&item.icon), &id)
        })
        .collect()
    }

    fn tab_menu_items(&self, pos: usize) -> Vec<PopupItem> {
        let Some(tab) = self.tabs.tabs.get(pos) else {
            return Vec::new();
        };
        let rail = &[Scope::Rail, Scope::Global];
        let mut items = Vec::new();
        if tab.folder {
            items.push(
                PopupItem::command(
                    "Rename folder…",
                    String::new(),
                    "pencil",
                    &Command::Edit.id(),
                )
                .with_action(PopupAction::Tab(pos, TabAction::Rename)),
            );
        } else if tab.session.is_some() {
            items.push(
                PopupItem::command(
                    "Rename session…",
                    keymap::hint(Command::Edit, rail),
                    "tag",
                    &Command::Edit.id(),
                )
                .with_action(PopupAction::Tab(pos, TabAction::Rename)),
            );
            items.push(
                PopupItem::command(
                    "Move to another machine…",
                    String::new(),
                    "arrow-right-left",
                    &Command::Move.id(),
                )
                .with_action(PopupAction::Tab(pos, TabAction::Move)),
            );
        }
        if self.exited.contains(&tab.key) {
            items.push(
                PopupItem::command(
                    "Reconnect",
                    keymap::hint(Command::ReconnectTab, rail),
                    "rotate-ccw",
                    &Command::ReconnectTab.id(),
                )
                .with_action(PopupAction::Tab(pos, TabAction::Reconnect)),
            );
        }
        if tab.folder || self.tabs.has_children(pos) {
            items.push(
                PopupItem::command(
                    if tab.collapsed { "Expand" } else { "Collapse" },
                    String::new(),
                    "chevron-down",
                    &Command::ExitNav.id(),
                )
                .with_action(PopupAction::Tab(pos, TabAction::Collapse)),
            );
        }
        items.push(
            PopupItem::command(
                "New folder…",
                String::new(),
                "plus",
                &Command::NewFolder.id(),
            )
            .with_action(PopupAction::Tab(pos, TabAction::NewFolder)),
        );
        items.push(
            PopupItem::command(
                if tab.folder {
                    "Remove folder"
                } else {
                    "Close tab (agent keeps running)"
                },
                keymap::hint(Command::CloseTab, rail),
                "square",
                &Command::CloseTab.id(),
            )
            .with_action(PopupAction::Tab(pos, TabAction::Close)),
        );
        items
    }

    /// Open a popup, anchored to its trigger or at a pointer position.
    pub fn open_popup(&mut self, kind: PopupKind, at: Option<(f64, f64)>) {
        if let PopupKind::RowMenu(ix) = kind {
            self.index = ix;
        }
        if let (PopupKind::LaunchAccount | PopupKind::LaunchPermissions, Overlay::Launch(launch)) =
            (kind, &mut self.overlay)
        {
            let _ = launch.press(
                if kind == PopupKind::LaunchAccount {
                    "popup:account"
                } else {
                    "popup:permissions"
                },
                &self.state,
            );
        }
        let items = self.popup_items_for(kind);
        self.popup = Some(Popup::open(kind, &items, at));
    }

    /// A trigger's click: open its popup, or close it when open.
    pub fn toggle_popup(&mut self, kind: PopupKind, at: Option<(f64, f64)>) {
        if self.popup.is_some_and(|p| p.kind == kind) {
            self.popup = None;
            return;
        }
        self.open_popup(kind, at);
    }

    /// Run the popup's row at `index`.
    pub fn run_popup_item(&mut self, index: usize) {
        let Some(item) = self.popup_items().into_iter().nth(index) else {
            return;
        };
        self.popup = None;
        match item.action {
            PopupAction::Command(id) => {
                if let Some(cmd) = Command::from_id(&id) {
                    self.execute(cmd);
                }
            }
            PopupAction::Tab(pos, action) => self.run_tab_action(pos, action),
            PopupAction::Choice(_, _)
            | PopupAction::LaunchAccount(_)
            | PopupAction::LaunchPermissions(_) => {
                if let Overlay::Launch(launch) = &mut self.overlay {
                    let id = match &item.action {
                        PopupAction::LaunchAccount(handle) => format!("account:{handle}"),
                        PopupAction::LaunchPermissions(mode) => format!("permissions:{mode}"),
                        PopupAction::Choice(ix, value) => format!("choice:{ix}:{value}"),
                        _ => String::new(),
                    };
                    let outcome = launch.press(&id, &self.state);
                    match outcome {
                        crate::launch::Outcome::Close => self.close_launch(),
                        crate::launch::Outcome::Submit => self.submit_launch(),
                        crate::launch::Outcome::None => self.sync_launch_catalogs(false),
                    }
                }
            }
        }
    }

    fn run_tab_action(&mut self, pos: usize, action: TabAction) {
        if pos >= self.tabs.tabs.len() {
            return;
        }
        match action {
            TabAction::Rename => self.set_message("Renaming is not available yet in this client"),
            TabAction::Reconnect => {
                if self.exited.contains(&self.tabs.tabs[pos].key) {
                    self.reconnect_tab(pos);
                }
            }
            TabAction::Close => self.close_tab(pos),
            TabAction::Move => {
                self.tabs.select(pos);
                self.nav = false;
                self.execute(Command::Move);
            }
            TabAction::Collapse => {
                if self.tabs.toggle_collapsed(pos) {
                    self.nav = self.nav && self.tabs.active > 0;
                }
                self.persist_tabs();
            }
            TabAction::NewFolder => self.execute(Command::NewFolder),
        }
    }

    /// The popup as the contract's `Popup` shape.
    pub fn popup_view(&self) -> serde_json::Value {
        let Some(popup) = self.popup else {
            return serde_json::Value::Null;
        };
        let items = self.popup_items_for(popup.kind);
        // The View menu hangs from its right edge; everything else from its left.
        let (x, y) = popup.at.unwrap_or((0.0, 0.0));
        let (anchor, x, right) = match popup.kind {
            PopupKind::ViewMenu => ("view-menu".to_string(), 0.0, x),
            PopupKind::RowMenu(ix) => (format!("more-{}", self.row_id(ix)), x, 0.0),
            PopupKind::TabMenu(pos) => (format!("tab-{}", self.tabs.tabs[pos].key), x, 0.0),
            _ => (String::new(), x, 0.0),
        };
        let kind = match popup.kind {
            PopupKind::RowMenu(_) => "row",
            PopupKind::TabMenu(_) => "tab",
            PopupKind::ViewMenu => "view",
            PopupKind::FieldChoice(_) => "field",
            PopupKind::LaunchAccount => "account",
            PopupKind::LaunchPermissions => "permissions",
        };
        let rows: Vec<serde_json::Value> = items
            .iter()
            .enumerate()
            .map(|(i, item)| picker::item(item, &i.to_string(), i == popup.index))
            .collect();
        serde_json::json!({
            "kind": kind,
            "anchor": anchor,
            "x": x,
            "y": y,
            "right": right,
            "title": "",
            "items": rows,
            "empty": "Nothing to choose",
        })
    }
}

/// The Lucide icons the menus use, as static names (picker's items hold `&'static str`).
fn icon_name(name: &str) -> &'static str {
    const NAMES: &[&str] = &[
        "archive",
        "archive-restore",
        "arrow-right-left",
        "chevron-down",
        "chevron-right",
        "ellipsis",
        "eye",
        "key-round",
        "list",
        "log-in",
        "message-square",
        "messages",
        "monitor",
        "pause",
        "pencil",
        "pin",
        "play",
        "plus",
        "rocket",
        "rotate-ccw",
        "square",
        "tag",
        "terminal",
        "trash",
    ];
    NAMES.iter().copied().find(|n| *n == name).unwrap_or("")
}
