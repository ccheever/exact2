//! The launch dialog's keys and presses (workspace.rs `handle_launch_key`
//! and the picker handlers around it). Every key is answered here or
//! swallowed, so nothing leaks into the manager under the dialog.

use super::{
    account, account_email, default_account, machine, machine_permission_default, AddFolder,
    DirectoryState, Focus, Launch, Outcome, PickerRow, ProjectPicker,
};
use crate::picker::{
    canon, typed, Mods, PickerKey, PickerState, Popup, PopupAction, PopupItem, PopupKind,
};
use crate::quick;
use crate::types::State;

/// What a key means in the dialog's navigation state (no picker, not inserting).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavAction {
    /// Esc.
    Close,
    /// Enter.
    Launch,
    /// Tab / arrows / j k: the next control.
    Move(i32),
    /// ← → h l: the next value, or the picker.
    Cycle(i32),
    /// Space / o: open the picker on the project chip; next value elsewhere.
    Open,
    /// i / a.
    Insert,
    /// Backspace / Delete / x.
    Reset,
    /// Ctrl+R.
    RefreshModels,
    /// Anything else is swallowed so no key leaks into a field or the manager.
    Swallow,
}

/// The NAV-mode key map (launch.rs:679-711).
pub fn nav_action(key: &str, m: &Mods) -> NavAction {
    let key = canon(key);
    if m.ctrl {
        return match key.as_str() {
            "r" => NavAction::RefreshModels,
            "n" | "j" => NavAction::Move(1),
            "p" | "k" => NavAction::Move(-1),
            _ => NavAction::Swallow,
        };
    }
    match key.as_str() {
        "escape" => return NavAction::Close,
        "enter" => return NavAction::Launch,
        "tab" => return NavAction::Move(if m.shift { -1 } else { 1 }),
        "down" => return NavAction::Move(1),
        "up" => return NavAction::Move(-1),
        "left" => return NavAction::Cycle(-1),
        "right" => return NavAction::Cycle(1),
        "backspace" | "delete" => return NavAction::Reset,
        _ => {}
    }
    match typed(&key, m).as_deref() {
        Some("j") => NavAction::Move(1),
        Some("k") => NavAction::Move(-1),
        Some("h") => NavAction::Cycle(-1),
        Some("l") => NavAction::Cycle(1),
        Some(" ") | Some("o") => NavAction::Open,
        Some("i") | Some("a") => NavAction::Insert,
        Some("x") => NavAction::Reset,
        _ => NavAction::Swallow,
    }
}

/// What a key did to the "add folder" input.
enum FolderKey {
    Back,
    Accept,
    Complete(usize),
    Highlight(usize),
    Nothing,
}

impl Launch {
    /// One key (workspace.rs `handle_launch_key`): the open popup or picker
    /// takes it first, then INSERT mode's few keys, then the NAV map.
    pub fn key(&mut self, key: &str, mods: &Mods, held: bool, state: &State) -> Outcome {
        if self.quick.is_some() {
            return quick::key(self, key, mods, held, state);
        }
        let key = canon(key);
        // A submit error stays until the next key; whatever happens next may fix it.
        self.error.clear();
        if self.popup.is_some() {
            return self.key_popup(&key, mods, state);
        }
        if self.machine_picker.is_some() {
            return self.key_machine_picker(&key, mods, state);
        }
        if self.picker.is_some() {
            return self.key_project_picker(&key, mods, state);
        }
        if self.model_picker.is_some() {
            return self.key_model_picker(&key, mods, state);
        }
        if let Some(index) = self.effort_picker {
            return self.key_effort_picker(&key, mods, index);
        }
        if self.insert {
            match key.as_str() {
                "escape" => self.insert = false,
                "enter" if !mods.shift => return Outcome::Submit,
                "tab" => self.move_focus(if mods.shift { -1 } else { 1 }, state),
                "down" => self.move_focus(1, state),
                "up" => self.move_focus(-1, state),
                _ => {}
            }
            return Outcome::None;
        }
        match nav_action(&key, mods) {
            NavAction::Close => return Outcome::Close,
            NavAction::Launch => return Outcome::Submit,
            NavAction::Move(step) => self.move_focus(step, state),
            NavAction::Cycle(step) => self.cycle(step, state),
            NavAction::Open => match self.focus {
                Focus::Project => self.open_project_picker(state),
                Focus::Machine => self.open_machine_picker(state),
                Focus::Model => self.open_model_picker(state),
                Focus::Effort => self.toggle_effort_picker(),
                Focus::Account => self.open_popup(PopupKind::LaunchAccount, state),
                Focus::Permissions => self.open_popup(PopupKind::LaunchPermissions, state),
                Focus::Prompt => self.insert_mode(),
            },
            NavAction::Insert => self.insert_mode(),
            NavAction::Reset => self.reset(state),
            NavAction::RefreshModels => self.sync_catalogs(state, true),
            NavAction::Swallow => {}
        }
        Outcome::None
    }

    fn key_popup(&mut self, key: &str, mods: &Mods, state: &State) -> Outcome {
        let Some(mut popup) = self.popup else {
            return Outcome::None;
        };
        let items = self.popup_items(popup.kind, state);
        match popup.key(key, mods, items.len()) {
            PickerKey::Chosen => {
                if let Some(item) = items.get(popup.index) {
                    self.run_popup_action(item.action.clone());
                }
                self.popup = None;
            }
            PickerKey::Closed => self.popup = None,
            _ => self.popup = Some(popup),
        }
        Outcome::None
    }

    fn run_popup_action(&mut self, action: PopupAction) {
        match action {
            PopupAction::LaunchAccount(handle) => self.account = handle,
            PopupAction::LaunchPermissions(mode) => self.permissions = mode,
            _ => {}
        }
    }

    fn key_machine_picker(&mut self, key: &str, mods: &Mods, state: &State) -> Outcome {
        let count = state.machines.iter().filter(|m| m.sprite.is_none()).count();
        let Some(picker) = self.machine_picker.as_mut() else {
            return Outcome::None;
        };
        if key == "space" {
            let index = picker.index;
            self.pick_machine(index, state);
            return Outcome::None;
        }
        match picker.key(key, mods, count) {
            PickerKey::Chosen => {
                let index = picker.index;
                self.pick_machine(index, state);
            }
            PickerKey::Closed => self.machine_picker = None,
            _ => {}
        }
        Outcome::None
    }

    fn key_project_picker(&mut self, key: &str, mods: &Mods, state: &State) -> Outcome {
        let count = self.picker_rows().len();
        let current_machine = self.machine.clone();
        let Some(picker) = self.picker.as_mut() else {
            return Outcome::None;
        };
        if let Some(adding) = picker.adding.as_mut() {
            let d = &adding.directory;
            let n = d.paths.len();
            let action = match key {
                "escape" => FolderKey::Back,
                "enter" => FolderKey::Accept,
                "tab" if n > 0 && !mods.shift => FolderKey::Complete(d.index),
                "down" if n > 0 => FolderKey::Highlight((d.index + 1) % n),
                "up" if n > 0 => FolderKey::Highlight((d.index + n - 1) % n),
                "n" | "j" if mods.ctrl && n > 0 => FolderKey::Highlight((d.index + 1) % n),
                "p" | "k" if mods.ctrl && n > 0 => FolderKey::Highlight((d.index + n - 1) % n),
                _ => FolderKey::Nothing,
            };
            match action {
                FolderKey::Back => picker.adding = None,
                FolderKey::Accept => self.accept_added_folder(),
                FolderKey::Complete(i) => self.pick_directory(i),
                FolderKey::Highlight(i) => adding.directory.index = i,
                FolderKey::Nothing => {}
            }
            return Outcome::None;
        }
        if picker.list.typing {
            match picker.list.key(key, mods, count) {
                PickerKey::Closed => {
                    picker.list = PickerState {
                        typing: false,
                        ..Default::default()
                    };
                }
                PickerKey::Chosen => {
                    let index = picker.list.index;
                    self.pick_row(index, state);
                }
                _ => {}
            }
            return Outcome::None;
        }
        match typed(key, mods).as_deref() {
            Some("/") => picker.list.typing = true,
            Some("a") => self.start_adding_folder(current_machine, state),
            _ => match picker.list.key(key, mods, count) {
                PickerKey::Closed => self.picker = None,
                PickerKey::Chosen => {
                    let index = picker.list.index;
                    self.pick_row(index, state);
                }
                _ => {}
            },
        }
        Outcome::None
    }

    fn key_model_picker(&mut self, key: &str, mods: &Mods, state: &State) -> Outcome {
        let count = self.model_rows().len();
        let Some(picker) = self.model_picker.as_mut() else {
            return Outcome::None;
        };
        if key == "space" {
            let index = picker.index;
            self.pick_model_row(index, state);
            return Outcome::None;
        }
        if key == "r" && mods.ctrl {
            self.sync_catalogs(state, true);
            return Outcome::None;
        }
        match picker.key(key, mods, count) {
            PickerKey::Chosen => {
                let index = picker.index;
                self.pick_model_row(index, state);
            }
            PickerKey::Closed => self.model_picker = None,
            _ => self.sync_catalogs(state, false),
        }
        Outcome::None
    }

    fn key_effort_picker(&mut self, key: &str, mods: &Mods, index: usize) -> Outcome {
        let count = self.effort_levels().len();
        let mut list = PickerState {
            index,
            ..Default::default()
        };
        if key == "space" {
            self.pick_effort(index);
            return Outcome::None;
        }
        match list.key(key, mods, count) {
            PickerKey::Chosen => self.pick_effort(index),
            PickerKey::Closed => self.effort_picker = None,
            PickerKey::Moved => self.effort_picker = Some(list.index),
            _ => {}
        }
        Outcome::None
    }

    /// Tab / j / k: the next control, skipping an effort half that is not there.
    pub fn move_focus(&mut self, step: i32, state: &State) {
        self.focus = self.focus.moved(step);
        if self.focus == Focus::Effort && !self.has_effort() {
            self.focus = self.focus.moved(step);
        }
        self.insert = false;
        self.effort_picker = None;
        self.sync_catalogs(state, false);
    }

    /// Click or otherwise land on a control. The prompt is a text box, so
    /// landing on it types; chips only navigate until `i`.
    pub fn focus_on(&mut self, focus: Focus, state: &State) {
        self.focus = focus;
        self.insert = focus == Focus::Prompt;
        self.effort_picker = None;
        self.sync_catalogs(state, false);
    }

    /// `i`: type into the prompt, or the model half when that is focused.
    pub fn insert_mode(&mut self) {
        self.focus = self.insert_target();
        self.insert = true;
    }

    /// Backspace / x: clear the focused control.
    pub fn reset(&mut self, state: &State) {
        match self.focus {
            Focus::Prompt => self.prompt.clear(),
            Focus::Model => self.model.clear(),
            Focus::Effort => self.effort.clear(),
            Focus::Account => self.account = default_account(state, &self.provider),
            Focus::Permissions => self.permissions.clear(),
            Focus::Project | Focus::Machine => {}
        }
    }

    /// ← / → on a chip: the next known project or account, the effort
    /// slider's next stop, or the model popup.
    pub fn cycle(&mut self, step: i32, state: &State) {
        let pick = |choices: &[String], current: &str| -> Option<String> {
            if choices.is_empty() {
                return None;
            }
            let next = match choices.iter().position(|c| c == current) {
                Some(at) => (at as i32 + step).rem_euclid(choices.len() as i32) as usize,
                None if step < 0 => choices.len() - 1,
                None => 0,
            };
            Some(choices[next].clone())
        };
        match self.focus {
            Focus::Machine => self.open_machine_picker(state),
            Focus::Project => {
                if self.projects.is_empty() {
                    return;
                }
                let len = self.projects.len() as i32;
                let next = match self.project_index() {
                    Some(at) => (at as i32 + step).rem_euclid(len) as usize,
                    None if step < 0 => self.projects.len() - 1,
                    None => 0,
                };
                self.machine = self.projects[next].machine.clone();
                self.cwd = self.projects[next].cwd.clone();
            }
            Focus::Prompt => {}
            // The model half is a dropdown, not a cycle: any horizontal key opens it.
            Focus::Model => self.open_model_picker(state),
            // The effort half is a list too.
            Focus::Effort => self.toggle_effort_picker(),
            Focus::Permissions => {
                let machine_default =
                    machine_permission_default(state, &self.machine, &self.provider);
                let choices = self.permission_choices(&machine_default);
                if let Some(next) = pick(&choices, &self.permissions) {
                    self.permissions = next;
                }
            }
            Focus::Account => {
                if let Some(next) = pick(&self.account_choices(state), &self.account) {
                    self.account = next;
                }
            }
        }
    }

    /// Show the project list, highlighting the current folder.
    pub fn open_project_picker(&mut self, state: &State) {
        self.focus = Focus::Project;
        self.insert = false;
        let list = PickerState {
            typing: false,
            ..Default::default()
        };
        self.picker = Some(ProjectPicker { list, adding: None });
        let current = self.project_index();
        let index = self
            .picker_rows()
            .iter()
            .position(|r| matches!(r, PickerRow::Project(i) if Some(*i) == current))
            .unwrap_or(0);
        if let Some(picker) = self.picker.as_mut() {
            picker.list.index = index;
        }
        self.sync_catalogs(state, false);
    }

    /// Show the machine list, highlighting the current machine.
    pub fn open_machine_picker(&mut self, state: &State) {
        self.focus = Focus::Machine;
        self.insert = false;
        self.picker = None;
        self.model_picker = None;
        self.effort_picker = None;
        let index = state
            .machines
            .iter()
            .filter(|m| m.sprite.is_none())
            .position(|m| m.id == self.machine)
            .unwrap_or(0);
        self.machine_picker = Some(PickerState {
            index,
            ..Default::default()
        });
    }

    /// Show the model popup on the current model and start its catalogs.
    pub fn open_model_picker(&mut self, state: &State) {
        self.focus = Focus::Model;
        self.insert = false;
        self.effort_picker = None;
        let index = self.current_model_row().unwrap_or(0);
        self.model_picker = Some(PickerState {
            index,
            ..Default::default()
        });
        self.sync_catalogs(state, false);
    }

    /// Show or hide the effort list under the effort half of the model chip,
    /// highlighting the current level.
    pub fn toggle_effort_picker(&mut self) {
        self.focus = Focus::Effort;
        self.insert = false;
        self.model_picker = None;
        self.effort_picker = match self.effort_picker {
            Some(_) => None,
            None => Some(self.effort_index()),
        };
    }

    /// The account or permissions popup's rows.
    pub fn popup_items(&self, kind: PopupKind, state: &State) -> Vec<PopupItem> {
        match kind {
            PopupKind::LaunchPermissions => {
                let machine_default =
                    machine_permission_default(state, &self.machine, &self.provider);
                self.permission_choices(&machine_default)
                    .into_iter()
                    .map(|mode| {
                        let label = if mode.is_empty() && !machine_default.is_empty() {
                            format!("Machine default · {machine_default}")
                        } else if mode.is_empty() {
                            "Machine default · none set, provider default".to_string()
                        } else {
                            crate::permissions::label(&mode)
                        };
                        let checked = mode == self.permissions;
                        PopupItem::choice(label, checked, PopupAction::LaunchPermissions(mode))
                    })
                    .collect()
            }
            PopupKind::LaunchAccount => self
                .account_choices(state)
                .into_iter()
                .map(|handle| {
                    let label = if handle.is_empty() {
                        "Machine login".to_string()
                    } else {
                        account(state, &handle)
                            .map(account_email)
                            .unwrap_or_else(|| handle.clone())
                    };
                    let checked = handle == self.account;
                    PopupItem::choice(label, checked, PopupAction::LaunchAccount(handle))
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Open the account or permissions popup on its current value.
    pub fn open_popup(&mut self, kind: PopupKind, state: &State) {
        self.focus = if kind == PopupKind::LaunchAccount {
            Focus::Account
        } else {
            Focus::Permissions
        };
        self.insert = false;
        self.model_picker = None;
        self.effort_picker = None;
        let items = self.popup_items(kind, state);
        self.popup = Some(Popup::open(kind, &items, None));
    }

    fn toggle_popup(&mut self, kind: PopupKind, state: &State) {
        if self.popup.is_some_and(|p| p.kind == kind) {
            self.popup = None;
        } else {
            self.open_popup(kind, state);
        }
    }

    /// Use the project row at `index`, or start typing a folder.
    pub fn pick_row(&mut self, index: usize, state: &State) {
        match self.picker_rows().get(index).cloned() {
            Some(PickerRow::Project(i)) => {
                let project = self.projects[i].clone();
                self.machine = project.machine;
                self.cwd = project.cwd;
                self.picker = None;
                self.sync_catalogs(state, false);
            }
            Some(PickerRow::AddFolder { machine, .. }) => self.start_adding_folder(machine, state),
            None => {}
        }
    }

    /// Type a path on a machine; completion starts from the folder that holds
    /// the current project, which is usually where new ones live too.
    pub fn start_adding_folder(&mut self, machine_id: String, state: &State) {
        let machine_name = machine(state, &machine_id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| machine_id.clone());
        let path = if self.machine == machine_id {
            match self.cwd.trim_end_matches('/').rfind('/') {
                Some(at) => self.cwd[..=at].to_string(),
                None => "~/".to_string(),
            }
        } else {
            "~/".to_string()
        };
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        picker.adding = Some(AddFolder {
            machine: machine_id,
            machine_name,
            path,
            directory: DirectoryState::default(),
        });
        self.refresh_directories();
    }

    /// Enter on the typed path.
    pub fn accept_added_folder(&mut self) {
        let Some(adding) = self.picker.as_ref().and_then(|p| p.adding.as_ref()) else {
            return;
        };
        let path = adding.path.trim().to_string();
        if path.is_empty() {
            self.error = "Type a folder path first".into();
            return;
        }
        self.machine = adding.machine.clone();
        self.cwd = path;
        self.error.clear();
        self.picker = None;
    }

    /// Tab or a click: complete the path with the match at `index`.
    pub fn pick_directory(&mut self, index: usize) {
        let Some(adding) = self.picker.as_mut().and_then(|p| p.adding.as_mut()) else {
            return;
        };
        if let Some(path) = adding.directory.paths.get(index) {
            adding.path = path.clone();
            self.refresh_directories();
        }
    }

    /// Choose the machine at `index` among the enrolled ones.
    pub fn pick_machine(&mut self, index: usize, state: &State) {
        if let Some(m) = state
            .machines
            .iter()
            .filter(|m| m.sprite.is_none())
            .nth(index)
        {
            self.select_machine(&m.id.clone());
            self.machine_picker = None;
            self.sync_catalogs(state, false);
        }
    }

    /// Choose the model row at `index` and close the popup.
    pub fn pick_model_row(&mut self, index: usize, state: &State) {
        if let Some(row) = self.model_rows().get(index).cloned() {
            self.choose_model(&row, |p| default_account(state, p));
            self.model_picker = None;
            self.sync_catalogs(state, false);
        }
    }

    /// Pick the level at that position and close the list.
    pub fn pick_effort(&mut self, index: usize) {
        if let Some(level) = self.effort_levels().get(index) {
            self.effort = level.clone();
        }
        self.effort_picker = None;
    }

    /// Text typed into one of the dialog's inputs (`Event::Input`).
    pub fn input(&mut self, id: &str, value: &str) {
        match id {
            "launch-prompt" => self.prompt = value.to_string(),
            "launch-model" => self.model = value.to_string(),
            "project-query" => {
                if let Some(picker) = self.picker.as_mut() {
                    picker.list.query = value.to_string();
                    picker.list.index = 0;
                }
            }
            "add-folder" => {
                if let Some(adding) = self.picker.as_mut().and_then(|p| p.adding.as_mut()) {
                    adding.path = value.to_string();
                    self.refresh_directories();
                }
            }
            _ => {}
        }
    }

    /// A press on one of the dialog's nodes, by the id its view gave it.
    pub fn press(&mut self, id: &str, state: &State) -> Outcome {
        let (head, arg) = id.split_once(':').unwrap_or((id, ""));
        let n = arg.parse::<usize>().unwrap_or(0);
        match head {
            "launch-cancel" => return Outcome::Close,
            "launch-submit" => return Outcome::Submit,
            "launch-project" => self.open_project_picker(state),
            "launch-machine" => self.open_machine_picker(state),
            "launch-prompt" => self.focus_on(Focus::Prompt, state),
            "model-pill" => self.focus_on(Focus::Model, state),
            "model-segment" => self.open_model_picker(state),
            "effort-segment" => self.toggle_effort_picker(),
            "account-chip" => self.toggle_popup(PopupKind::LaunchAccount, state),
            "permissions-chip" => self.toggle_popup(PopupKind::LaunchPermissions, state),
            "machine-back" => self.machine_picker = None,
            "project-back" => self.picker = None,
            "project-search" => {
                if let Some(picker) = self.picker.as_mut() {
                    picker.list.typing = true;
                }
            }
            "launch-machine-row" => self.pick_machine(n, state),
            "project" => self.pick_row(n, state),
            "dir" => self.pick_directory(n),
            "model-row" => self.pick_model_row(n, state),
            "effort-row" => self.pick_effort(n),
            "launch-account" => {
                self.account = arg.to_string();
                self.popup = None;
            }
            "launch-permissions" => {
                self.permissions = arg.to_string();
                self.popup = None;
            }
            "quick-slot" => return quick::pick(self, n, state),
            "popup-close" => {
                self.popup = None;
                self.model_picker = None;
                self.effort_picker = None;
            }
            _ => {}
        }
        Outcome::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::tests::{fresh, state};
    use crate::launch::{DirectoryMatches, LaunchMemory};

    const NONE: Mods = Mods::NONE;

    #[test]
    fn navigation_keys_never_type() {
        assert_eq!(nav_action("j", &NONE), NavAction::Move(1));
        assert_eq!(nav_action("k", &NONE), NavAction::Move(-1));
        assert_eq!(nav_action("h", &NONE), NavAction::Cycle(-1));
        assert_eq!(nav_action("l", &NONE), NavAction::Cycle(1));
        assert_eq!(nav_action("i", &NONE), NavAction::Insert);
        assert_eq!(nav_action(" ", &NONE), NavAction::Open);
        assert_eq!(nav_action("Enter", &NONE), NavAction::Launch);
        assert_eq!(nav_action("Escape", &NONE), NavAction::Close);
        assert_eq!(nav_action("Tab", &Mods::SHIFT), NavAction::Move(-1));
        assert_eq!(nav_action("n", &Mods::CTRL), NavAction::Move(1));
        assert_eq!(nav_action("r", &Mods::CTRL), NavAction::RefreshModels);
        assert_eq!(nav_action("Backspace", &NONE), NavAction::Reset);
        // Plain letters and digits are swallowed instead of reaching a field.
        assert_eq!(nav_action("q", &NONE), NavAction::Swallow);
        assert_eq!(nav_action("1", &NONE), NavAction::Swallow);
    }

    #[test]
    fn the_dialog_opens_typing_and_insert_keys_are_few() {
        let st = state();
        let memory = LaunchMemory::default();
        let mut launch = Launch::open(&memory, &st, Some("studio"), "draft".into()).unwrap();
        assert_eq!(launch.machine, "studio");
        assert_eq!((launch.focus, launch.insert), (Focus::Prompt, true));
        assert_eq!(launch.prompt, "draft");
        let none = Launch::open(&memory, &State::default(), None, String::new());
        assert_eq!(
            none.unwrap_err(),
            "Add a machine before launching a session"
        );
        // Shift+Enter is the input's newline; letters go to the input too.
        assert_eq!(launch.key("Enter", &Mods::SHIFT, false, &st), Outcome::None);
        assert_eq!(launch.key("j", &NONE, false, &st), Outcome::None);
        assert_eq!(launch.focus, Focus::Prompt);
        assert_eq!(launch.key("Escape", &NONE, false, &st), Outcome::None);
        assert!(!launch.insert);
        launch.insert = true;
        assert_eq!(launch.key("Tab", &NONE, false, &st), Outcome::None);
        assert_eq!((launch.focus, launch.insert), (Focus::Model, false));
        launch.insert = true;
        assert_eq!(launch.key("Enter", &NONE, false, &st), Outcome::Submit);
    }

    #[test]
    fn nav_keys_move_open_reset_and_close() {
        let st = state();
        let mut launch = fresh(&st);
        launch.focus = Focus::Model;
        // The effort half is skipped when the harness has none.
        launch.provider = "opencode".into();
        launch.key("j", &NONE, false, &st);
        assert_eq!(launch.focus, Focus::Account);
        launch.key("k", &NONE, false, &st);
        assert_eq!(launch.focus, Focus::Model);
        launch.provider = "codex".into();
        // ← → on the model half open the popup; Esc closes it.
        launch.key("l", &NONE, false, &st);
        assert!(launch.model_picker.is_some());
        assert!(launch.wanted_catalog().is_some());
        launch.key("Escape", &NONE, false, &st);
        assert!(launch.model_picker.is_none());
        // Space on the effort half toggles its list; j moves, Enter picks.
        launch.key("j", &NONE, false, &st);
        assert_eq!(launch.focus, Focus::Effort);
        launch.key(" ", &NONE, false, &st);
        assert_eq!(launch.effort_picker, Some(0));
        launch.key("j", &NONE, false, &st);
        launch.key("j", &NONE, false, &st);
        launch.key("Enter", &NONE, false, &st);
        assert_eq!(
            (launch.effort.as_str(), launch.effort_picker),
            ("low", None)
        );
        // x resets the focused control.
        launch.key("x", &NONE, false, &st);
        assert_eq!(launch.effort, "");
        launch.focus = Focus::Prompt;
        launch.prompt = "typed".into();
        launch.key("Backspace", &NONE, false, &st);
        assert_eq!(launch.prompt, "");
        // i types into the prompt from any chip.
        launch.focus = Focus::Permissions;
        launch.key("i", &NONE, false, &st);
        assert_eq!((launch.focus, launch.insert), (Focus::Prompt, true));
        launch.insert = false;
        // Enter launches; Esc closes with the prompt kept as the draft.
        launch.prompt = "draft".into();
        assert_eq!(launch.key("Enter", &NONE, false, &st), Outcome::Submit);
        assert_eq!(launch.key("Escape", &NONE, false, &st), Outcome::Close);
        assert_eq!(launch.close(), "draft");
    }

    #[test]
    fn chips_cycle_their_choices_and_popups_pick_them() {
        let st = state();
        let mut launch = fresh(&st);
        launch.provider = "claude".into();
        launch.focus = Focus::Account;
        launch.key("l", &NONE, false, &st);
        assert_eq!(launch.account, "claude:me@example.com");
        launch.key("h", &NONE, false, &st);
        assert_eq!(launch.account, "");
        // Space opens the account popup on the current value; ↓ Enter picks.
        launch.key(" ", &NONE, false, &st);
        let popup = launch.popup.unwrap();
        assert_eq!((popup.kind, popup.index), (PopupKind::LaunchAccount, 0));
        launch.key("ArrowDown", &NONE, false, &st);
        launch.key("Enter", &NONE, false, &st);
        assert_eq!(launch.account, "claude:me@example.com");
        assert!(launch.popup.is_none());
        // Permissions cycle machine default, then the harness's modes.
        launch.key("j", &NONE, false, &st);
        assert_eq!(launch.focus, Focus::Permissions);
        launch.key("l", &NONE, false, &st);
        assert_eq!(launch.permissions, "manual");
        launch.key("h", &NONE, false, &st);
        launch.key("h", &NONE, false, &st);
        assert_eq!(launch.permissions, "bypassPermissions");
        let items = launch.popup_items(PopupKind::LaunchPermissions, &st);
        assert_eq!(
            items[0].label,
            "Machine default · none set, provider default"
        );
        assert_eq!(items.last().unwrap().label, "Bypass permissions");
        // A press on a popup row sets the value directly.
        launch.press("launch-permissions:plan", &st);
        assert_eq!(launch.permissions, "plan");
        // The project chip cycles known folders across machines.
        launch.focus = Focus::Project;
        assert_eq!(launch.cwd, "~/Developer/fleet");
        launch.key("h", &NONE, false, &st);
        assert_eq!(
            (launch.machine.as_str(), launch.cwd.as_str()),
            ("blank", "~")
        );
        launch.key("l", &NONE, false, &st);
        assert_eq!(launch.cwd, "~/Developer/fleet");
    }

    #[test]
    fn the_project_picker_filters_adds_folders_and_completes_them() {
        let st = state();
        let mut launch = fresh(&st);
        launch.key(" ", &NONE, false, &st);
        let picker = launch.picker.as_ref().unwrap();
        assert_eq!((picker.list.index, picker.list.typing), (0, false));
        // G jumps to the last row (an add-folder row); / filters.
        launch.key("G", &NONE, false, &st);
        let last = launch.picker_rows().len() - 1;
        assert_eq!(launch.picker.as_ref().unwrap().list.index, last);
        launch.key("/", &NONE, false, &st);
        assert!(launch.picker.as_ref().unwrap().list.typing);
        launch.input("project-query", "srv");
        launch.key("Enter", &NONE, false, &st);
        assert_eq!(
            (launch.machine.as_str(), launch.cwd.as_str()),
            ("studio", "/srv/app")
        );
        assert!(launch.picker.is_none());
        // a starts a folder on the current machine, from the parent folder.
        launch.key(" ", &NONE, false, &st);
        launch.key("a", &NONE, false, &st);
        assert_eq!(
            launch.wanted_directories(),
            Some(("studio".into(), "/srv/".into()))
        );
        launch.directories_requested();
        let matches = DirectoryMatches {
            paths: vec!["/srv/app/".into(), "/srv/www/".into()],
            truncated: false,
        };
        launch.set_directories("studio", "/srv/", Ok(matches));
        launch.key("ArrowDown", &NONE, false, &st);
        launch.key("Tab", &NONE, false, &st);
        let adding = launch.picker.as_ref().unwrap().adding.as_ref().unwrap();
        assert_eq!(adding.path, "/srv/www/");
        assert!(adding.directory.loading);
        launch.key("Enter", &NONE, false, &st);
        assert_eq!(
            (launch.cwd.as_str(), launch.picker.is_none()),
            ("/srv/www/", true)
        );
        // An empty path is refused; Esc goes back to the list.
        launch.key(" ", &NONE, false, &st);
        launch.key("a", &NONE, false, &st);
        launch.input("add-folder", "  ");
        launch.key("Enter", &NONE, false, &st);
        assert_eq!(launch.error, "Type a folder path first");
        launch.key("Escape", &NONE, false, &st);
        assert!(launch.picker.as_ref().unwrap().adding.is_none());
        launch.key("Escape", &NONE, false, &st);
        assert!(launch.picker.is_none());
    }

    #[test]
    fn the_machine_picker_and_presses_land_on_rows() {
        let st = state();
        let mut launch = fresh(&st);
        launch.focus = Focus::Machine;
        launch.key("l", &NONE, false, &st);
        assert_eq!(launch.machine_picker.as_ref().unwrap().index, 0);
        launch.key("k", &NONE, false, &st);
        assert_eq!(launch.machine_picker.as_ref().unwrap().index, 2);
        launch.key(" ", &NONE, false, &st);
        assert_eq!(
            (launch.machine.as_str(), launch.cwd.as_str()),
            ("blank", "~")
        );
        assert!(launch.machine_picker.is_none());
        launch.press("launch-machine", &st);
        launch.press("launch-machine-row:1", &st);
        assert_eq!(launch.machine, "studio");
        launch.press("model-segment", &st);
        assert!(launch.model_picker.is_some());
        launch.press("model-row:1", &st);
        assert_eq!(
            (launch.provider.as_str(), launch.model.as_str()),
            ("claude", "")
        );
        assert_eq!(launch.press("launch-cancel", &st), Outcome::Close);
        assert_eq!(launch.press("launch-submit", &st), Outcome::Submit);
    }
}
