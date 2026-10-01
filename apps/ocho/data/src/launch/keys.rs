//! The composer's keys, presses and typed text (workspace.rs
//! `launch_owns_key`, `handle_launch_key`, `handle_menu_key`, the `step_*`
//! walks, `open_menu`, `menu_move`, `pick_row` and `launch_query_changed`).
//! Typing always types: the prompt (or an open menu's search box) keeps the
//! keyboard, and only the handful of keys `owns_key` names are the composer's.

use super::{
    account_choices, default_account, menu::Row, Focus, Launch, Menu, MenuKind, ModelRow, Outcome,
    ProjectSource,
};
use crate::picker::{canon, Mods};
use crate::quick;
use crate::types::State;

/// The machines a launch can start on: one bound to an existing Sprite
/// session belongs to that session, not to new work.
pub fn launchable(state: &State) -> Vec<&crate::types::Machine> {
    state
        .machines
        .iter()
        .filter(|m| m.sprite.is_none())
        .collect()
}

fn ring_step<T: Clone + PartialEq>(ring: &[T], current: &T, step: i32) -> T {
    let len = ring.len() as i32;
    let at = ring.iter().position(|c| c == current).unwrap_or(0) as i32;
    ring[(((at + step) % len + len) % len) as usize].clone()
}

impl Launch {
    /// The keys the composer takes back from its own text box: Enter
    /// launches, Shift-Enter is a newline, Tab walks the line, Escape closes,
    /// ⌘L ⌘M ⌘U ⌘E open the menus and ⌘Enter launches. Arrows (and Space)
    /// only steer once focus has left the prompt, so the text's selection
    /// stays intact; with a menu open the list keys are its own. Everything
    /// else belongs to the focused input.
    pub fn owns_key(&self, key: &str, mods: &Mods) -> bool {
        if self.quick.is_some() {
            return true;
        }
        let key = canon(key);
        let key = key.as_str();
        if mods.meta {
            return matches!(key, "l" | "m" | "u" | "e" | "enter");
        }
        if mods.ctrl || mods.alt {
            return false;
        }
        if self.menu.is_some() {
            return matches!(
                key,
                "escape" | "enter" | "tab" | "up" | "down" | "left" | "right"
            );
        }
        match key {
            "escape" | "tab" => true,
            "enter" => !mods.shift,
            "up" | "down" | "left" | "right" | "space" => self.focus != Focus::Prompt,
            _ => false,
        }
    }

    /// One key (workspace.rs `handle_launch_key`). Keys `owns_key` does not
    /// name are the input's and change nothing here.
    pub fn key(&mut self, key: &str, mods: &Mods, held: bool, state: &State) -> Outcome {
        if self.quick.is_some() {
            return quick::key(self, key, mods, held, state);
        }
        let key = canon(key);
        let key = key.as_str();
        if mods.meta {
            // ⌘R refreshes a form's model list, not the composer's.
            if !matches!(key, "l" | "m" | "u" | "e" | "enter") {
                return Outcome::None;
            }
            // A submit error stays until the next key; whatever happens next may fix it.
            self.error.clear();
            match key {
                "l" => self.open_menu(MenuKind::Project, state),
                "m" => self.open_menu(MenuKind::Model, state),
                "u" => self.open_menu(MenuKind::Account, state),
                "e" => self.open_menu(MenuKind::More, state),
                _ => return Outcome::Submit,
            }
            return Outcome::None;
        }
        if !self.owns_key(key, mods) {
            return Outcome::None;
        }
        self.error.clear();
        if let Some(kind) = self.menu.as_ref().map(|m| m.kind) {
            match key {
                "escape" => self.menu = None,
                // Tab walks the list the way the arrows do, and Shift-Tab goes
                // back up it rather than further down.
                "down" => self.menu_move(1, state),
                "up" => self.menu_move(-1, state),
                "tab" => self.menu_move(if mods.shift { -1 } else { 1 }, state),
                "enter" => self.menu_pick(state),
                // The machine bar sits across the top of the project menu.
                "left" | "right" if kind == MenuKind::Project => {
                    self.step_machine(if key == "right" { 1 } else { -1 }, state)
                }
                _ => {}
            }
            return Outcome::None;
        }
        match key {
            "escape" => return Outcome::Close,
            "enter" => return Outcome::Submit,
            "tab" => self.launch_move(if mods.shift { -1 } else { 1 }, state),
            "up" | "down" | "left" | "right" => {
                let step = if matches!(key, "down" | "right") {
                    1
                } else {
                    -1
                };
                self.arrow(matches!(key, "up" | "down"), step, state);
            }
            "space" => {
                let kind = match self.focus {
                    Focus::Project => MenuKind::Project,
                    Focus::Model => MenuKind::Model,
                    _ => MenuKind::Account,
                };
                self.open_menu(kind, state);
            }
            _ => {}
        }
        Outcome::None
    }

    /// Tab order is prompt → project → model → account → prompt, skipping
    /// the account when there is nothing to choose between.
    pub fn launch_move(&mut self, step: i32, state: &State) {
        let mut next = self.focus.moved(step);
        if next == Focus::Account && account_choices(state, &self.provider).len() < 2 {
            next = next.moved(step);
        }
        self.focus = next;
    }

    /// Arrows on a focused control: up/down walks projects, left/right walks
    /// machines on the project control and models on the model control.
    pub fn arrow(&mut self, vertical: bool, step: i32, state: &State) {
        match (self.focus, vertical) {
            (Focus::Project, true) => self.step_project(step, state),
            (Focus::Project, false) => self.step_machine(step, state),
            (Focus::Model, false) => self.step_model(step, state),
            (Focus::Model, true) => self.open_menu(MenuKind::Model, state),
            (Focus::Account, false) => self.step_account(step, state),
            (Focus::Account, true) => self.open_menu(MenuKind::Account, state),
            _ => {}
        }
    }

    /// Cycling walks the folders this machine has actually launched in, then
    /// its home folder; the whole list belongs in the menu, not in a cycle.
    pub fn step_project(&mut self, step: i32, state: &State) {
        let mut ring: Vec<String> = self
            .projects
            .iter()
            .filter(|p| p.machine == self.machine)
            .filter(|p| p.source == ProjectSource::Recent || p.cwd == self.cwd)
            .map(|p| p.cwd.clone())
            .collect();
        if !ring.iter().any(|c| c == "~") {
            ring.push("~".into());
        }
        if ring.len() < 2 {
            return;
        }
        self.cwd = ring_step(&ring, &self.cwd, step);
        self.sync_catalogs(state, false);
    }

    /// The next launchable machine.
    pub fn step_machine(&mut self, step: i32, state: &State) {
        let ids: Vec<String> = launchable(state).iter().map(|m| m.id.clone()).collect();
        if ids.len() < 2 {
            return;
        }
        let next = ring_step(&ids, &self.machine, step);
        self.set_machine(&next, state);
    }

    /// The models this account reached for last, in that order.
    pub fn step_model(&mut self, step: i32, state: &State) {
        let ring = self.recent_model_ring();
        if ring.len() < 2 {
            return;
        }
        let here = (self.provider.clone(), self.model.clone());
        let (provider, id) = ring_step(&ring, &here, step);
        let name = self
            .recent_models
            .iter()
            .find(|m| m.provider == provider && m.id == id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| id.clone());
        let row = ModelRow {
            provider,
            id,
            name,
            description: String::new(),
            recent: true,
        };
        self.choose_model(&row, |p| default_account(state, p));
        self.sync_catalogs(state, false);
    }

    /// The next shared account for the harness.
    pub fn step_account(&mut self, step: i32, state: &State) {
        let choices = account_choices(state, &self.provider);
        if choices.len() < 2 {
            return;
        }
        self.account = ring_step(&choices, &self.account, step);
        self.sync_catalogs(state, false);
    }

    /// Open a menu, or close it when it is the one open. It opens on what is
    /// already chosen, so Enter changes nothing by accident.
    pub fn open_menu(&mut self, kind: MenuKind, state: &State) {
        if self.menu.as_ref().is_some_and(|m| m.kind == kind) {
            self.menu = None;
            return;
        }
        self.focus = match kind {
            MenuKind::Project => Focus::Project,
            MenuKind::Account => Focus::Account,
            _ => Focus::Model,
        };
        self.menu = Some(Menu::new(kind));
        let rows = self.menu_rows(state);
        let at = rows
            .iter()
            .position(|row| self.row_is_current(row))
            .unwrap_or_else(|| rows.iter().position(Row::selectable).unwrap_or(0));
        if let Some(menu) = self.menu.as_mut() {
            menu.index = at;
        }
        self.sync_catalogs(state, false);
    }

    /// Move the menu's highlight, skipping groups and notes, wrapping.
    pub fn menu_move(&mut self, step: i32, state: &State) {
        let rows = self.menu_rows(state);
        let Some(menu) = self.menu.as_mut() else {
            return;
        };
        if rows.is_empty() {
            return;
        }
        let len = rows.len() as i32;
        let mut at = menu.index as i32;
        for _ in 0..rows.len() {
            at = ((at + step) % len + len) % len;
            if rows[at as usize].selectable() {
                break;
            }
        }
        menu.index = at as usize;
    }

    /// Enter in a menu: the highlighted row.
    pub fn menu_pick(&mut self, state: &State) {
        let Some(index) = self.menu.as_ref().map(|m| m.index) else {
            return;
        };
        if let Some(row) = self.menu_rows(state).get(index).cloned() {
            self.pick_row(row, state);
        }
    }

    /// A click on the menu row at `index`.
    pub fn pick_menu_index(&mut self, index: usize, state: &State) {
        if let Some(row) = self.menu_rows(state).get(index).cloned() {
            if let Some(menu) = self.menu.as_mut() {
                menu.index = index;
            }
            self.pick_row(row, state);
        }
    }

    /// Use a row. Effort and guardrails keep the drawer open: they are
    /// usually set together on the rare visit; everything else closes it.
    pub fn pick_row(&mut self, row: Row, state: &State) {
        match row {
            Row::Group(_) | Row::Note(_) => return,
            Row::NoProject => {
                self.cwd = "~".into();
                self.menu = None;
            }
            Row::Project(i) => {
                if let Some(p) = self.projects.get(i).cloned() {
                    self.machine = p.machine;
                    self.cwd = p.cwd;
                }
                self.menu = None;
            }
            Row::Folder(path) => {
                self.cwd = path.trim_end_matches('/').to_string();
                if self.cwd.is_empty() {
                    self.cwd = "/".into();
                }
                self.menu = None;
            }
            Row::Model(m) => {
                self.menu = None;
                self.choose_model(&m, |p| default_account(state, p));
            }
            Row::Account(a) => {
                self.account = a.handle;
                self.menu = None;
            }
            Row::Effort(level) => self.effort = level,
            Row::Perm(mode) => self.permissions = mode,
        }
        self.sync_catalogs(state, false);
    }

    /// Text typed into one of the composer's inputs (`Event::Input`):
    /// `launch-prompt` is the prompt, `launch-menu-query` the open menu's
    /// search box, which re-filters the list and asks the machine for
    /// folders when the query reads like a path.
    pub fn input(&mut self, id: &str, value: &str) {
        let id = id.strip_prefix("field:").unwrap_or(id);
        match id {
            "launch-prompt" => {
                self.prompt = value.to_string();
                self.error.clear();
            }
            "launch-menu-query" => {
                let Some(menu) = self.menu.as_mut() else {
                    return;
                };
                menu.query = value.to_string();
                menu.index = 0;
                // The first selectable row is computed without the fleet: the
                // rows that need it (accounts) are all selectable anyway.
                let rows = self.rows(&[], "");
                let first = rows.iter().position(Row::selectable).unwrap_or(0);
                if let Some(menu) = self.menu.as_mut() {
                    menu.index = first;
                }
                self.refresh_directories();
            }
            _ => {}
        }
    }

    /// A press on one of the composer's nodes, by the id its view gave it,
    /// with or without the contract's `field:` / `button:` / `pick:` /
    /// `focus:` prefix.
    pub fn press(&mut self, id: &str, state: &State) -> Outcome {
        let id = ["field:", "button:", "pick:", "focus:"]
            .iter()
            .find_map(|p| id.strip_prefix(p))
            .unwrap_or(id);
        let (head, arg) = id.split_once(':').unwrap_or((id, ""));
        let n = arg.parse::<usize>().unwrap_or(0);
        match head {
            "quick-slot" => return quick::pick(self, n, state),
            "launch-send" => return Outcome::Submit,
            // A click into the prompt puts the keyboard there and closes a menu.
            "launch-prompt" => {
                self.focus = Focus::Prompt;
                self.menu = None;
            }
            "launch-where" => self.click_control(Focus::Project, MenuKind::Project, state),
            "launch-what" => self.click_control(Focus::Model, MenuKind::Model, state),
            "launch-who" => self.click_control(Focus::Account, MenuKind::Account, state),
            "launch-machine-tab" => {
                let id = launchable(state).get(n).map(|m| m.id.clone());
                if let Some(id) = id {
                    self.set_machine(&id, state);
                }
            }
            "launch-menu-row" => self.pick_menu_index(n, state),
            "popup-close" => self.menu = None,
            _ => {}
        }
        Outcome::None
    }

    fn click_control(&mut self, focus: Focus, kind: MenuKind, state: &State) {
        self.focus = focus;
        self.open_menu(kind, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::tests::{fresh, state};
    use crate::launch::{DirectoryMatches, LaunchMemory, RecentModel};
    use crate::types::Account;

    const NONE: Mods = Mods::NONE;

    fn two_claude_accounts() -> State {
        let mut st = state();
        st.accounts.push(Account {
            name: "work".into(),
            email: "work@example.com".into(),
            provider: "claude".into(),
            shared: true,
            ..Default::default()
        });
        st
    }

    #[test]
    fn typing_always_types_and_only_a_few_keys_are_the_composers() {
        let st = state();
        let memory = LaunchMemory::default();
        let mut launch = Launch::open(&memory, &st, Some("studio"), "draft".into()).unwrap();
        assert_eq!(launch.machine, "studio");
        assert_eq!(launch.focus, Focus::Prompt);
        assert_eq!(launch.prompt, "draft");
        let none = Launch::open(&memory, &State::default(), None, String::new());
        assert_eq!(
            none.unwrap_err(),
            "Add a machine before launching a session"
        );
        // Letters, Shift+Enter and arrows in the prompt are the input's.
        for (key, mods) in [
            ("j", NONE),
            ("Enter", Mods::SHIFT),
            ("ArrowDown", NONE),
            (" ", NONE),
            ("r", Mods::META),
            ("a", Mods::CTRL),
        ] {
            assert!(!launch.owns_key(key, &mods), "{key}");
            assert_eq!(launch.key(key, &mods, false, &st), Outcome::None);
        }
        assert_eq!(launch.focus, Focus::Prompt);
        assert!(launch.owns_key("Tab", &NONE));
        assert!(launch.owns_key("m", &Mods::META));
        assert_eq!(launch.key("Enter", &NONE, false, &st), Outcome::Submit);
        assert_eq!(
            launch.key("Enter", &Mods::META, false, &st),
            Outcome::Submit
        );
        assert_eq!(launch.key("Escape", &NONE, false, &st), Outcome::Close);
        assert_eq!(launch.close(), "draft");
        launch.input("field:launch-prompt", "Fix it");
        assert_eq!(launch.prompt, "Fix it");
    }

    #[test]
    fn tab_walks_the_line_and_skips_a_lone_account() {
        let st = state();
        let mut launch = fresh(&st);
        launch.error = "old".into();
        launch.key("Tab", &NONE, false, &st);
        assert_eq!(launch.focus, Focus::Project);
        assert!(launch.error.is_empty());
        launch.key("Tab", &NONE, false, &st);
        assert_eq!(launch.focus, Focus::Model);
        // Codex has no shared account here, so Tab goes back to the prompt.
        launch.key("Tab", &NONE, false, &st);
        assert_eq!(launch.focus, Focus::Prompt);
        launch.key("Tab", &Mods::SHIFT, false, &st);
        assert_eq!(launch.focus, Focus::Model);
        // Two Claude accounts make the account a stop.
        let st = two_claude_accounts();
        launch.provider = "claude".into();
        launch.key("Tab", &NONE, false, &st);
        assert_eq!(launch.focus, Focus::Account);
        // ← → walk them; ↑ ↓ open the menu.
        launch.account = "claude:me@example.com".into();
        launch.key("ArrowRight", &NONE, false, &st);
        assert_eq!(launch.account, "claude:work@example.com");
        launch.key("ArrowRight", &NONE, false, &st);
        assert_eq!(launch.account, "claude:me@example.com");
        launch.key("ArrowDown", &NONE, false, &st);
        assert_eq!(launch.menu.as_ref().unwrap().kind, MenuKind::Account);
    }

    #[test]
    fn arrows_on_the_line_walk_projects_machines_and_recent_models() {
        let st = state();
        let mut launch = fresh(&st);
        launch.focus = Focus::Project;
        // No recent project on the laptop: the ring is the current folder and home.
        assert_eq!(launch.cwd, "~/Developer/fleet");
        launch.key("ArrowDown", &NONE, false, &st);
        assert_eq!(launch.cwd, "~");
        // ~ is also a session folder, so the ring has only one stop now.
        launch.key("ArrowDown", &NONE, false, &st);
        assert_eq!(launch.cwd, "~");
        launch.key("ArrowRight", &NONE, false, &st);
        assert_eq!(launch.machine, "studio");
        launch.key("ArrowLeft", &NONE, false, &st);
        assert_eq!(launch.machine, "laptop");
        launch.focus = Focus::Model;
        launch.recent_models = vec![
            RecentModel {
                provider: "claude".into(),
                id: "opus".into(),
                name: "Opus".into(),
            },
            RecentModel {
                provider: "codex".into(),
                id: "gpt-6".into(),
                name: "GPT-6".into(),
            },
        ];
        launch.key("ArrowRight", &NONE, false, &st);
        assert_eq!(
            (launch.provider.as_str(), launch.model.as_str()),
            ("codex", "gpt-6")
        );
        launch.key("ArrowRight", &NONE, false, &st);
        assert_eq!(
            (launch.provider.as_str(), launch.account.as_str()),
            ("claude", "claude:me@example.com")
        );
        // Space opens the focused control's menu.
        launch.key(" ", &NONE, false, &st);
        assert_eq!(launch.menu.as_ref().unwrap().kind, MenuKind::Model);
    }

    #[test]
    fn menus_open_on_the_current_value_move_pick_and_toggle() {
        let st = state();
        let mut launch = fresh(&st);
        launch.key("l", &Mods::META, false, &st);
        let menu = launch.menu.as_ref().unwrap();
        assert_eq!(
            (menu.kind, launch.focus),
            (MenuKind::Project, Focus::Project)
        );
        let rows = launch.menu_rows(&st);
        assert!(launch.row_is_current(&rows[menu.index]));
        // Menu keys are the list's; letters belong to its search box.
        assert!(launch.owns_key("Tab", &NONE));
        assert!(!launch.owns_key("j", &NONE));
        launch.key("ArrowUp", &NONE, false, &st);
        launch.key("Enter", &NONE, false, &st);
        assert!(launch.menu.is_none());
        assert_eq!(launch.cwd, "~");
        // ⌘L twice closes it again; Esc closes without leaving the dialog.
        launch.key("l", &Mods::META, false, &st);
        launch.key("l", &Mods::META, false, &st);
        assert!(launch.menu.is_none());
        launch.key("e", &Mods::META, false, &st);
        assert_eq!(launch.focus, Focus::Model);
        assert_eq!(launch.key("Escape", &NONE, false, &st), Outcome::None);
        assert!(launch.menu.is_none());
        // The drawer stays open while effort and guardrails are set.
        launch.key("e", &Mods::META, false, &st);
        let rows = launch.menu_rows(&st);
        let high = rows
            .iter()
            .position(|r| *r == Row::Effort("high".into()))
            .unwrap();
        let plan = rows
            .iter()
            .position(|r| *r == Row::Perm("full-access".into()))
            .unwrap();
        launch.press(&format!("pick:launch-menu-row:{high}"), &st);
        launch.press(&format!("launch-menu-row:{plan}"), &st);
        assert_eq!(
            (launch.effort.as_str(), launch.permissions.as_str()),
            ("high", "full-access")
        );
        assert!(launch.menu.is_some());
        // Groups are skipped by the highlight.
        launch.menu.as_mut().unwrap().index = 1;
        launch.key("ArrowUp", &NONE, false, &st);
        assert!(rows[launch.menu.as_ref().unwrap().index].selectable());
    }

    #[test]
    fn the_project_menu_switches_machines_and_types_paths() {
        let st = state();
        let mut launch = fresh(&st);
        launch.press("field:launch-where", &st);
        assert_eq!(launch.menu.as_ref().unwrap().kind, MenuKind::Project);
        launch.key("ArrowRight", &NONE, false, &st);
        assert_eq!(
            (launch.machine.as_str(), launch.cwd.as_str()),
            ("studio", "~/research")
        );
        launch.press("launch-machine-tab:2", &st);
        assert_eq!(
            (launch.machine.as_str(), launch.cwd.as_str()),
            ("blank", "~")
        );
        launch.input("launch-menu-query", "~/Dev");
        assert_eq!(
            launch.wanted_directories(),
            Some(("blank".into(), "~/Dev".into()))
        );
        launch.directories_requested();
        let found = DirectoryMatches {
            paths: vec!["~/Developer/".into()],
            truncated: false,
        };
        launch.set_directories("blank", "~/Dev", Ok(found));
        assert_eq!(launch.menu.as_ref().unwrap().index, 0);
        launch.key("Enter", &NONE, false, &st);
        assert_eq!(launch.cwd, "~/Developer");
        assert!(launch.menu.is_none());
        // A click into the prompt closes a menu and puts focus there.
        launch.press("launch-what", &st);
        assert!(launch.menu.is_some());
        launch.press("focus:launch-prompt", &st);
        assert!(launch.menu.is_none());
        assert_eq!(launch.focus, Focus::Prompt);
        assert_eq!(launch.press("button:launch-send", &st), Outcome::Submit);
    }

    #[test]
    fn an_eas_machine_switches_the_harness_to_codex() {
        let mut st = state();
        st.machines[1].eas = Some(Default::default());
        st.accounts.push(Account {
            name: "cx".into(),
            email: "cx@example.com".into(),
            provider: "codex".into(),
            shared: true,
            ..Default::default()
        });
        let mut launch = fresh(&st);
        launch.provider = "claude".into();
        launch.model = "opus".into();
        launch.set_machine("studio", &st);
        assert_eq!(
            (
                launch.provider.as_str(),
                launch.model.as_str(),
                launch.account.as_str()
            ),
            ("codex", "", "codex:cx@example.com")
        );
    }
}
