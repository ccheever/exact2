//! Where events go (workspace.rs `dispatch`, `handle_*_key`, `click_row`):
//! presses by node id, keys by scope, text by input.

use super::{Overlay, Page, PopupKind, Workspace};
use crate::keymap::{self, Keystroke, Resolution, Scope};
use crate::palette::Command;
use crate::picker::{Mods, PickerKey};

impl Workspace {
    /// A press on a node, by the id the contract gave it.
    pub(super) fn press(&mut self, id: &str) {
        if id.starts_with("secret-") {
            self.secrets_press(id);
            return;
        }
        if id.starts_with("transcript-") && self.transcript_press(id) {
            return;
        }
        if id.starts_with("pair-") && self.phone_press(id) {
            return;
        }
        // A click anywhere else takes the keyboard from the secret entry
        // (ui.rs: a terminal click; the entry is the only focus Ocho tracks).
        if !id.starts_with("key:") {
            self.secrets.blur();
        }
        let (kind, rest) = id.split_once(':').unwrap_or((id, ""));
        match kind {
            // ⌘ chords the contract declares (app.contract's shortcut buttons).
            "key" => {
                let mut parts: Vec<&str> = rest.split('+').collect();
                let key = parts.pop().unwrap_or("");
                let mods = parts.join("+");
                let ks = Keystroke::new(&web_key(key), &mods);
                self.key(&ks, "");
            }
            "page" => {
                self.nav = false;
                if let Some(page) = Page::from_id(rest) {
                    self.tabs.select_manager();
                    self.set_page(page);
                    self.persist_tabs();
                }
            }
            "tab" if crate::recovery::press_command(rest).is_some() => {
                if let Some(cmd) = crate::recovery::press_command(rest) {
                    self.execute(cmd);
                }
            }
            "tab"
                if rest == "reconnect"
                    || rest == "close"
                    || rest == "manager"
                    || rest == "retry"
                    || rest == "handoff" =>
            {
                match rest {
                    "reconnect" => self.execute(Command::ReconnectTab),
                    "close" => self.execute(Command::CloseTab),
                    "manager" => self.execute(Command::SelectTab(0)),
                    "handoff" => self.execute(Command::Handoff),
                    _ => self.execute(Command::ReconnectTab),
                }
            }
            "tab" => {
                if let Some(pos) = self.tabs.position(rest) {
                    if self.tabs.tabs[pos].folder {
                        self.tabs.toggle_collapsed(pos);
                        self.persist_tabs();
                    } else {
                        self.nav = false;
                        self.tabs.select(pos);
                        self.tabs.reveal_active();
                        self.persist_tabs();
                        self.recover_active_exited_session();
                    }
                }
            }
            "fold" => {
                if let Some(pos) = self.tabs.position(rest) {
                    if self.tabs.toggle_collapsed(pos) {
                        self.nav = self.nav && self.tabs.active > 0;
                    }
                    self.persist_tabs();
                }
            }
            "close" => {
                if let Some(pos) = self.tabs.position(rest) {
                    self.close_tab(pos);
                }
            }
            // The active tab's terminal view reported: "exited" when its
            // process ended, "missing"/"unavailable" when it could not show one.
            // The active tab's terminal view reported: "exited[:CODE]" when
            // its process ended, "connection:{json}" for its transport,
            // "missing"/"unavailable" when it could not show one.
            // The docked shell: its view's reports, its "×", and clicks
            // that move typing between it and the agent.
            "panel" => match rest {
                "close" => self.close_terminal_panel(),
                "focus" => self.focus_panel(true),
                "blur" => self.focus_panel(false),
                other => self.panel_report(other),
            },
            "terminal" => {
                if let Some(tab) = self.tabs.active_tab() {
                    let key = tab.key.clone();
                    match rest {
                        "unavailable" => {
                            self.set_error("No terminal: libghostty is not available in this build")
                        }
                        link if link.starts_with("secret:") => {
                            self.open_terminal_secret_link(&key, &link["secret:".len()..]);
                        }
                        other => self.terminal_report(&key, other),
                    }
                }
            }
            "row" => {
                self.nav = false;
                self.popup = None;
                if let Some(index) = self.row_index(rest) {
                    self.select(index);
                }
            }
            "act" => {
                let (row, action) = rest.split_once(':').unwrap_or((rest, ""));
                if let Some(index) = self.row_index(row) {
                    self.index = index;
                    if let Some(cmd) = super::row_command(action) {
                        self.execute(cmd);
                    }
                }
            }
            "more" => {
                if let Some(index) = self.row_index(rest) {
                    self.toggle_popup(PopupKind::RowMenu(index), None);
                }
            }
            "menu" => {
                if let Ok(index) = rest.parse::<usize>() {
                    self.run_popup_item(index);
                }
            }
            "popup-close" => self.popup = None,
            "view-menu" => self.toggle_popup(PopupKind::ViewMenu, None),
            "primary" => {
                let (id, _) = crate::rows::primary(self.page);
                if let Some(cmd) = super::row_command(id) {
                    self.execute(cmd);
                }
            }
            "settings" => self.execute(Command::Settings),
            "help" => self.execute(Command::Help),
            "update" => self.execute(Command::UpdateDesktop),
            "toast-dismiss" => self.toast = Default::default(),
            "focus" => {
                if rest == "search" {
                    self.searching = true;
                    if self.page != Page::Sessions {
                        self.set_page(Page::Sessions);
                        self.searching = true;
                    }
                } else if let Overlay::Launch(launch) = &mut self.overlay {
                    let _ = launch.press(&format!("focus:{rest}"), &self.state);
                }
            }
            "blur" => {
                if rest == "search" {
                    self.searching = false;
                }
            }
            "backdrop" | "message-close" => self.close_overlay(),
            "button" | "pick" if self.extras_press(rest) => {}
            "whats-new" => self.execute(Command::WhatsNew),
            "pick" | "button" | "field" | "choice" | "dir" if self.dialog_press(kind, rest) => {}
            "button" => self.overlay_button(rest),
            "pick" => self.pick(rest),
            "field" | "toggle" | "step-up" | "step-down" | "reset" => {
                self.overlay_press(kind, rest)
            }
            _ => {}
        }
    }

    /// A popup trigger's press, with the trigger's frame: "View ▾" hangs its
    /// top-right corner 56 right and 28 down of the trigger's origin, a row's
    /// "⋯" its top-left 28 down (ui.rs `anchored_popup` offsets).
    pub(super) fn press_at(&mut self, id: &str, x: f64, y: f64) {
        match id {
            "view-menu" => self.toggle_popup(PopupKind::ViewMenu, Some((x + 56.0, y + 28.0))),
            other => {
                if other.starts_with("field:") {
                    // A form's choice field: its dropdown hangs under the field.
                    self.press(other);
                    if let Some(popup) = self.popup.as_mut() {
                        if popup.at.is_none() {
                            popup.at = Some((x, y + 32.0));
                        }
                    }
                    return;
                }
                if let Some(row) = other.strip_prefix("more:") {
                    if let Some(index) = self.row_index(row) {
                        self.toggle_popup(PopupKind::RowMenu(index), Some((x, y + 28.0)));
                    }
                } else {
                    self.press(other);
                }
            }
        }
    }

    /// The row index for a row id on the current page.
    fn row_index(&self, id: &str) -> Option<usize> {
        (0..self.count()).find(|&i| self.row_id(i) == id)
    }

    /// Close whatever overlay is open, keeping drafts (ui.rs backdrop click).
    pub fn close_overlay(&mut self) {
        match &self.overlay {
            Overlay::Launch(_) => self.close_launch(),
            Overlay::Settings(_) => {
                if let Overlay::Settings(page) = &mut self.overlay {
                    let effect = page.close();
                    self.apply_settings_effect(effect);
                }
                self.overlay = Overlay::None;
            }
            Overlay::Form(_) => self.close_form(),
            Overlay::Conversations => {
                self.topics.close();
                self.overlay = Overlay::None;
            }
            Overlay::Themes(picker) => {
                if let crate::themes::ThemeKey::Revert { theme, .. } = picker.cancel() {
                    self.theme = theme;
                }
                self.overlay = Overlay::None;
            }
            Overlay::None => {}
            _ => self.overlay = Overlay::None,
        }
    }

    fn overlay_button(&mut self, id: &str) {
        match id {
            "confirm-no" => self.overlay = Overlay::None,
            "confirm-yes" => self.confirm_yes(),
            "launch" => self.submit_launch(),
            "cancel" => self.close_overlay(),
            _ => {
                if let Overlay::Launch(launch) = &mut self.overlay {
                    let outcome = launch.press(&format!("button:{id}"), &self.state);
                    self.launch_outcome(outcome);
                }
            }
        }
    }

    fn pick(&mut self, id: &str) {
        match &mut self.overlay {
            Overlay::Palette(picker) => {
                if let Ok(index) = id.parse::<usize>() {
                    let count = self.palette_items().len();
                    if let Overlay::Palette(picker) = &mut self.overlay {
                        picker.select(index, count);
                    }
                    self.run_palette_selection();
                } else {
                    let _ = picker;
                }
            }
            Overlay::Launch(launch) => {
                let outcome = launch.press(&format!("pick:{id}"), &self.state);
                self.launch_outcome(outcome);
            }
            _ => {}
        }
    }

    fn overlay_press(&mut self, kind: &str, rest: &str) {
        match &mut self.overlay {
            Overlay::Settings(page) => {
                let Some(index) = rest
                    .strip_prefix("setting:")
                    .and_then(|n| n.parse::<usize>().ok())
                else {
                    return;
                };
                let effect = match kind {
                    "toggle" | "field" => page.activate(&mut self.settings, index),
                    "step-up" => page.step(&mut self.settings, index, 1.0),
                    "step-down" => page.step(&mut self.settings, index, -1.0),
                    "reset" => page.reset(&mut self.settings, index),
                    _ => return,
                };
                self.apply_settings_effect(effect);
            }
            Overlay::Launch(launch) => {
                let outcome = launch.press(&format!("{kind}:{rest}"), &self.state);
                self.launch_outcome(outcome);
            }
            _ => {}
        }
    }

    fn launch_outcome(&mut self, outcome: crate::launch::Outcome) {
        match outcome {
            crate::launch::Outcome::None => self.sync_launch_catalogs(false),
            crate::launch::Outcome::Close => self.close_launch(),
            crate::launch::Outcome::Submit => self.submit_launch(),
        }
    }

    /// A key (workspace.rs `dispatch`). `at` says where it landed: the
    /// search box, an overlay's input, the terminal, or the window.
    pub(super) fn key(&mut self, ks: &Keystroke, at: &str) {
        let mods = Mods {
            meta: ks.meta,
            shift: ks.shift,
            alt: ks.alt,
            ctrl: ks.ctrl,
        };
        // ⇧⌘N opens quick launch from anywhere, over anything.
        if matches!(
            keymap::resolve(&[Scope::Global], ks, None),
            Resolution::Command(Command::QuickLaunch)
        ) {
            self.popup = None;
            self.open_quick_launch();
            return;
        }
        if let Overlay::Launch(launch) = &mut self.overlay {
            let outcome = if launch.quick.is_some() {
                crate::quick::key(launch, &ks.key, &mods, false, &self.state)
            } else {
                launch.key(&ks.key, &mods, false, &self.state)
            };
            self.launch_outcome(outcome);
            return;
        }
        if self.popup.is_some() && self.handle_popup_key(ks, &mods) {
            return;
        }
        // The secret entry (input_view.rs `active_input` SecretDemo).
        if self.secrets.open
            && self.secrets.editing.is_some()
            && self.secrets.focused
            && matches!(self.overlay, Overlay::None)
            && self.secrets_key(&ks.key, &mods)
        {
            return;
        }
        if self.handle_global_key(ks) {
            return;
        }
        if self.dialog_key(&ks.key, &mods) || self.extras_key(&ks.key, &mods) {
            return;
        }
        // Transcript mode takes the tab's keys (terminal_key_target).
        if matches!(self.overlay, Overlay::None)
            && self.tabs.active > 0
            && !self.nav
            && self.transcript_key(&ks.key, &mods)
        {
            return;
        }
        if at == "terminal" && matches!(self.overlay, Overlay::None) {
            // The terminal took the key itself; only NAV's entry is ours.
            return;
        }
        match &self.overlay {
            Overlay::Palette(_) => {
                self.handle_picker_key(ks, &mods);
            }
            Overlay::None => {
                if self.nav {
                    self.handle_nav_key(ks);
                } else if self.tabs.active > 0 {
                    // Bare keys over a terminal reach it, not the manager.
                } else {
                    self.handle_manager_key(ks, at);
                }
            }
            _ => {
                self.handle_manager_key(ks, at);
            }
        }
    }

    fn handle_global_key(&mut self, ks: &Keystroke) -> bool {
        if !ks.meta {
            return false;
        }
        let pending = self.pending_keys.take();
        match keymap::resolve(&[Scope::Global], ks, pending.as_ref()) {
            Resolution::Command(cmd) => {
                self.execute(cmd);
                true
            }
            Resolution::Pending => {
                self.pending_keys = Some(ks.clone());
                true
            }
            Resolution::None => false,
        }
    }

    fn handle_popup_key(&mut self, ks: &Keystroke, mods: &Mods) -> bool {
        if ks.meta {
            self.popup = None;
            return false;
        }
        let count = self.popup_items().len();
        let Some(popup) = self.popup.as_mut() else {
            return false;
        };
        match popup.key(&ks.key, mods, count) {
            PickerKey::Chosen => {
                let index = popup.index;
                self.run_popup_item(index);
            }
            PickerKey::Closed => self.popup = None,
            PickerKey::Typed(_) | PickerKey::None => {
                if let Some(t) = ks.typed() {
                    match t {
                        " " => {
                            let index = popup.index;
                            self.run_popup_item(index);
                        }
                        "j" => popup.index = crate::picker::wrapped(popup.index, 1, count),
                        "k" => popup.index = crate::picker::wrapped(popup.index, -1, count),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn handle_picker_key(&mut self, ks: &Keystroke, mods: &Mods) -> bool {
        let count = self.palette_items().len();
        let Overlay::Palette(picker) = &mut self.overlay else {
            return false;
        };
        match picker.key(&ks.key, mods, count) {
            PickerKey::Closed => self.overlay = Overlay::None,
            PickerKey::Chosen => self.run_palette_selection(),
            _ => {}
        }
        true
    }

    fn handle_nav_key(&mut self, ks: &Keystroke) -> bool {
        if ks.key == "a" && ks.ctrl {
            self.nav = false;
            return true;
        }
        if let Resolution::Command(cmd) = keymap::resolve(&[Scope::Rail], ks, None) {
            match cmd {
                Command::CloseTab | Command::ReconnectTab if self.tabs.active == 0 => {}
                Command::Edit => {}
                other => self.execute(other),
            }
        }
        true
    }

    fn handle_manager_key(&mut self, ks: &Keystroke, at: &str) -> bool {
        let t = ks.typed().unwrap_or_default().to_string();
        let key = ks.key.as_str();
        match &mut self.overlay {
            Overlay::Help => {
                if matches!(key, "Escape" | "Enter") || matches!(t.as_str(), "q" | "?") {
                    self.overlay = Overlay::None;
                }
                return true;
            }
            Overlay::Message(_) => {
                if matches!(key, "Escape" | "Enter") || matches!(t.as_str(), "q" | "m") {
                    self.overlay = Overlay::None;
                }
                return true;
            }
            Overlay::Confirm { .. }
            | Overlay::MachineDelete(_)
            | Overlay::ProviderUpdate { .. }
            | Overlay::ProviderPathFix { .. } => {
                if t == "y" {
                    self.confirm_yes();
                } else if t == "n" || key == "Escape" {
                    self.overlay = Overlay::None;
                }
                return true;
            }
            Overlay::Settings(page) => {
                let mods = mods_text(ks);
                let effect = page.key(&mut self.settings, key, &mods);
                self.apply_settings_effect(effect);
                return true;
            }
            Overlay::Launch(_)
            | Overlay::Palette(_)
            | Overlay::Form(_)
            | Overlay::PullRequests(_)
            | Overlay::Conversations
            | Overlay::Themes(_)
            | Overlay::WhatsNew
            | Overlay::PairIMessage
            | Overlay::PairPhone(_) => return false,
            Overlay::None => {}
        }
        if self.searching || at == "search" {
            match key {
                "Enter" | "Escape" => {
                    self.searching = false;
                    self.focus_id = "manager".into();
                }
                "ArrowDown" => self.execute(Command::Down),
                "ArrowUp" => self.execute(Command::Up),
                _ => {}
            }
            return true;
        }
        let scopes = self.key_scopes();
        let pending = self.pending_keys.take();
        match keymap::resolve(&scopes, ks, pending.as_ref()) {
            Resolution::Command(cmd) => {
                self.execute(cmd);
                true
            }
            Resolution::Pending => {
                self.pending_keys = Some(ks.clone());
                true
            }
            Resolution::None => false,
        }
    }

    /// Text typed into an input, by id.
    pub(super) fn input(&mut self, id: &str, value: &str) {
        if self.dialog_input(id, value) {
            return;
        }
        match id {
            "secret-demo-input" => self.secrets.input(value),
            "transcript-input" => self.transcript_input(value),
            "search" => {
                self.query = value.to_string();
                self.index = 0;
            }
            "query" => {
                if let Overlay::Palette(picker) = &mut self.overlay {
                    picker.query = value.to_string();
                    picker.index = 0;
                }
            }
            other => {
                if let Some(field) = other.strip_prefix("field:") {
                    match &mut self.overlay {
                        Overlay::Launch(launch) => {
                            launch.input(field, value);
                            self.sync_launch_catalogs(false);
                        }
                        Overlay::Settings(page) => {
                            page.prompt = value.to_string();
                            page.editing_prompt = true;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// A pan: the rail's resize handle, or a tab being dragged.
    pub(super) fn pan(&mut self, id: &str, dx: f64, dy: f64) {
        if id == "panel-resize" {
            self.resize_panel(dy);
            return;
        }
        if id == "rail-resize" {
            self.rail_resizing = true;
            self.rail_width = super::clamp_rail(self.rail_width + dx, self.window.0);
            if dx == 0.0 && dy == 0.0 {
                self.rail_resizing = false;
                self.persist_tabs();
            }
            return;
        }
        if let Some(key) = id.strip_prefix("tab:") {
            let Some(from) = self.tabs.position(key) else {
                return;
            };
            let visible = self.tabs.visible();
            let Some(row) = visible.iter().position(|&i| i == from) else {
                return;
            };
            // Rows are 29 px tall (py_1 + one 21 px line); a status line adds 15.
            let (offset_row, y) = match self.drag {
                Some((f, y)) if f == from => (row, y + dy),
                _ => (row, dy),
            };
            let _ = offset_row;
            if dx == 0.0 && dy == 0.0 {
                // The pan ended: drop where the target says.
                if let Some(drop) = self.tab_drop.take() {
                    if self.tabs.move_tab(from, drop) {
                        self.persist_tabs();
                    }
                }
                self.drag = None;
                return;
            }
            let row_h = 29.0;
            let target_row = ((row as f64) + (y / row_h).floor())
                .clamp(0.0, (visible.len() - 1) as f64) as usize;
            let within = y.rem_euclid(row_h);
            let target = visible[target_row];
            self.drag = Some((from, y));
            self.tab_drop = if target != from && self.tabs.can_drop(from, target) {
                Some(crate::tab_tree::TabDrop {
                    target,
                    zone: crate::tab_tree::zone_for(within, 0.0, row_h),
                })
            } else {
                None
            };
        }
    }

    /// A double click: rows open, folders rename, sessions label.
    pub(super) fn double_click(&mut self, id: &str) {
        if let Some(row) = id.strip_prefix("row:") {
            if let Some(index) = self.row_index(row) {
                self.index = index;
                self.execute(Command::Open);
            }
        }
    }

    /// A right click: the row's or the tab's menu, at the pointer.
    pub(super) fn context_menu(&mut self, id: &str, x: f64, y: f64) {
        if let Some(row) = id.strip_prefix("row:") {
            if let Some(index) = self.row_index(row) {
                self.open_popup(PopupKind::RowMenu(index), Some((x, y)));
            }
        } else if let Some(key) = id.strip_prefix("tab:") {
            if let Some(pos) = self.tabs.position(key) {
                self.open_popup(PopupKind::TabMenu(pos), Some((x, y)));
            }
        }
    }
}

/// The contract's shortcut spelling to the web's key name.
fn web_key(key: &str) -> String {
    match key {
        "tab" => "Tab".into(),
        "enter" => "Enter".into(),
        "escape" => "Escape".into(),
        "space" => " ".into(),
        other => other.into(),
    }
}

/// Modifiers as the preferences module reads them.
fn mods_text(ks: &Keystroke) -> String {
    let mut out = Vec::new();
    if ks.meta {
        out.push("meta");
    }
    if ks.shift {
        out.push("shift");
    }
    if ks.alt {
        out.push("alt");
    }
    if ks.ctrl {
        out.push("ctrl");
    }
    out.join("+")
}
