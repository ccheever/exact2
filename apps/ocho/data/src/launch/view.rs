//! The composer's `OVERLAY` JSON (ui.rs `render_launch`,
//! `render_launch_composer`, `launch_control`, `render_launch_line`,
//! `render_launch_menu`, `render_menu_row`). Colours are theme names the
//! caller resolves; the ids are what `press` answers to.
//!
//! How the existing shapes carry the composer:
//! - `fields`: the prompt (`launch-prompt`, kind `prompt`), then the line's
//!   controls in order — `launch-where`, `launch-what`, `launch-who` (only
//!   when the harness has more than one account), all kind `control` — then,
//!   with a menu open, its machine bar (`launch-machine-tab:{i}`, kind `tab`)
//!   and its search box (`launch-menu-query`, kind `search`). A control's
//!   `label` is its main word, `value` the small trailing text, `hint` its
//!   dot's colour name, `focused` whether the keyboard is on it, and
//!   `options` its flags: `open` (its menu is open), `muted` (no project),
//!   `danger` (a risky guardrail).
//! - `rows`: the open menu's rows; `hint` says `group`, `note`, `folder`
//!   (monospace) or `item`, `chip` is the ✓ tick, `swatch` the provider
//!   dot's colour name, `accent` a risky mode (drawn in danger).
//! - `buttons`: the send button (`launch-send`, "↑").

use serde_json::{json, Value as Json};

use super::{
    effort_label, keys::launchable, machine, machine_label, machine_permission_default, menu::Row,
    provider_dot, provider_label, Focus, Launch, MenuKind,
};
use crate::permissions;
use crate::picker;
use crate::types::State;

/// An `OVERLAY` with its title and mode pill, everything else zero.
pub fn overlay(kind: &str, title: &str, pill: &str, pill_color: &str) -> Json {
    json!({
        "kind": kind,
        "width": match kind {
            "quick" => 600,
            "launch" => 560,
            _ => 680,
        },
        "top": false,
        "title": title,
        "subtitle": "",
        "pill": pill,
        "pillColor": pill_color,
        "glyph": "",
        "placeholder": "",
        "query": "",
        "status": "",
        "statusError": false,
        "rows": [],
        "index": 0,
        "footer": "",
        "body": "",
        "bodyMarkdown": false,
        "fields": [],
        "buttons": [],
        "hint": "",
        "focusId": "",
    })
}

fn control(id: &str, label: &str, value: &str, focused: bool, flags: &[&str]) -> Json {
    let mut c = picker::field(id, label, value, "", "control", focused);
    picker::set(&mut c, "options", json!(flags));
    c
}

impl Launch {
    /// The `OVERLAY`: the quick launch, or the composer with its menu.
    pub fn view(&self, state: &State) -> Json {
        if self.quick.is_some() {
            return crate::quick::view(self, state);
        }
        let open = self.menu.as_ref().map(|m| m.kind);
        let mut fields = vec![self.prompt_field()];
        fields.push(self.where_control(state, open));
        fields.push(self.what_control(state, open));
        let accounts = self.accounts(state);
        // Who runs it: silent when the harness has a single login.
        if accounts.len() > 1 {
            let label = accounts
                .iter()
                .find(|a| a.handle == self.account)
                .map(|a| a.email.clone())
                .unwrap_or_else(|| {
                    if self.account.is_empty() {
                        "machine login".into()
                    } else {
                        self.account.clone()
                    }
                });
            let flags: &[&str] = if open == Some(MenuKind::Account) {
                &["open"]
            } else {
                &[]
            };
            let focused = self.focus == Focus::Account;
            fields.push(control("launch-who", &label, "", focused, flags));
        }
        let mut v = overlay("launch", "", "", "");
        picker::set(&mut v, "status", json!(self.error));
        picker::set(&mut v, "statusError", json!(!self.error.is_empty()));
        let send = picker::button("launch-send", "↑", "", true);
        picker::set(&mut v, "buttons", json!([send]));
        let focus_id = match &self.menu {
            None => "field-launch-prompt",
            Some(menu) if menu.searchable() => "field-launch-menu-query",
            Some(_) => "",
        };
        picker::set(&mut v, "focusId", json!(focus_id));
        if let Some(menu) = &self.menu {
            // The machine sits in a bar across the top of the project menu:
            // which machine you are looking at frames the list.
            let machines = launchable(state);
            if menu.kind == MenuKind::Project && machines.len() > 1 {
                for (i, m) in machines.iter().enumerate() {
                    let id = format!("launch-machine-tab:{i}");
                    let here = m.id == self.machine;
                    fields.push(picker::field(&id, &machine_label(m), "", "", "tab", here));
                }
            }
            if menu.searchable() {
                let search = picker::field(
                    "launch-menu-query",
                    "",
                    &menu.query,
                    menu.placeholder(),
                    "search",
                    true,
                );
                fields.push(search);
                picker::set(&mut v, "placeholder", json!(menu.placeholder()));
                picker::set(&mut v, "query", json!(menu.query));
            }
            picker::set(&mut v, "rows", json!(self.menu_view(state, menu.index)));
            picker::set(&mut v, "index", json!(menu.index));
        }
        picker::set(&mut v, "fields", json!(fields));
        v
    }

    fn prompt_field(&self) -> Json {
        let focused = self.focus == Focus::Prompt && self.menu.is_none();
        let mut prompt = picker::field(
            "launch-prompt",
            "",
            &self.prompt,
            "Do anything",
            "prompt",
            focused,
        );
        picker::set(&mut prompt, "multiline", json!(true));
        prompt
    }

    /// Where it runs: the project's name ("no project" for home) and the
    /// machine, small, after it.
    fn where_control(&self, state: &State, open: Option<MenuKind>) -> Json {
        let machine_name = machine(state, &self.machine)
            .map(machine_label)
            .unwrap_or_else(|| self.machine.clone());
        let label = if self.cwd == "~" {
            "no project".to_string()
        } else {
            self.projects
                .iter()
                .find(|p| p.machine == self.machine && p.cwd == self.cwd)
                .map(|p| p.name())
                .unwrap_or_else(|| self.cwd.clone())
        };
        let mut flags = Vec::new();
        if open == Some(MenuKind::Project) {
            flags.push("open");
        }
        if self.cwd == "~" {
            flags.push("muted");
        }
        let focused = self.focus == Focus::Project;
        control("launch-where", &label, &machine_name, focused, &flags)
    }

    /// What runs it. A native default is not a model name, so it reads as
    /// the harness; effort and a guardrail only appear when they are set (or
    /// the machine's default guardrail is a risky one).
    fn what_control(&self, state: &State, open: Option<MenuKind>) -> Json {
        let label = if self.model.is_empty() {
            provider_label(&self.provider).to_string()
        } else {
            self.model_name()
        };
        let mut extras: Vec<String> = Vec::new();
        if !self.effort.is_empty() {
            extras.push(effort_label(&self.effort).to_string());
        }
        let machine_default = machine_permission_default(state, &self.machine, &self.provider);
        let effective = if self.permissions.is_empty() {
            machine_default
        } else {
            self.permissions.clone()
        };
        let risky = permissions::risky(&effective);
        if !self.permissions.is_empty() || risky {
            extras.push(permissions::short(&effective));
        }
        let mut flags = Vec::new();
        if matches!(open, Some(MenuKind::Model | MenuKind::More)) {
            flags.push("open");
        }
        if risky {
            flags.push("danger");
        }
        let focused = self.focus == Focus::Model;
        let mut c = control("launch-what", &label, &extras.join(" · "), focused, &flags);
        picker::set(&mut c, "hint", json!(provider_dot(&self.provider)));
        c
    }

    /// The open menu's rows as `PICKER_ROW`s.
    fn menu_view(&self, state: &State, selected: usize) -> Vec<Json> {
        let machine_default = machine_permission_default(state, &self.machine, &self.provider);
        self.menu_rows(state)
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let id = format!("launch-menu-row:{i}");
                let (kind, name, detail, danger, dot) = match row {
                    Row::Group(name) => ("group", name.clone(), String::new(), false, ""),
                    Row::Note(text) => ("note", text.clone(), String::new(), false, ""),
                    Row::NoProject => (
                        "item",
                        "No project".into(),
                        "start in the home folder".into(),
                        false,
                        "",
                    ),
                    Row::Project(p) => {
                        let p = &self.projects[*p];
                        ("item", p.name(), p.cwd.clone(), false, "")
                    }
                    Row::Folder(path) => {
                        ("folder", path.clone(), "use this folder".into(), false, "")
                    }
                    Row::Model(m) => {
                        let name = if m.id.is_empty() {
                            format!("{} default", provider_label(&m.provider))
                        } else {
                            m.name.clone()
                        };
                        let dot = provider_dot(&m.provider);
                        ("item", name, m.description.clone(), false, dot)
                    }
                    Row::Account(a) => ("item", a.email.clone(), a.profile.clone(), false, ""),
                    Row::Effort(level) => {
                        let detail = if level.is_empty() {
                            "let the harness decide"
                        } else {
                            "reasoning effort"
                        };
                        let name = effort_label(level).to_string();
                        ("item", name, detail.to_string(), false, "")
                    }
                    Row::Perm(mode) => {
                        let label = if mode.is_empty() && !machine_default.is_empty() {
                            format!(
                                "{} · {}",
                                permissions::label(mode),
                                permissions::short(&machine_default)
                            )
                        } else {
                            permissions::label(mode)
                        };
                        let detail = permissions::describe(mode).to_string();
                        ("item", label, detail, permissions::risky(mode), "")
                    }
                };
                let tick = if self.row_is_current(row) { "✓" } else { "" };
                let selected = i == selected && row.selectable();
                let mut r = picker::row(&id, tick, &name, &detail, kind, selected);
                picker::set(&mut r, "swatch", json!(dot));
                picker::set(&mut r, "accent", json!(danger));
                r
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::tests::{fresh, state};
    use crate::launch::LaunchMemory;
    use crate::picker::Mods;
    use crate::types::Account;

    fn ids(v: &Json) -> Vec<String> {
        v["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["id"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn the_composer_is_a_prompt_and_one_line() {
        let st = state();
        let launch = Launch::open(&LaunchMemory::default(), &st, None, "Fix it".into()).unwrap();
        let v = launch.view(&st);
        assert_eq!(v["kind"], "launch");
        assert_eq!(v["width"], 560);
        assert_eq!(
            (v["title"].as_str(), v["pill"].as_str()),
            (Some(""), Some(""))
        );
        assert_eq!(v["footer"], "");
        assert_eq!(ids(&v), ["launch-prompt", "launch-where", "launch-what"]);
        let f = &v["fields"];
        assert_eq!(f[0]["value"], "Fix it");
        assert_eq!(f[0]["placeholder"], "Do anything");
        assert_eq!(f[0]["focused"], true);
        assert_eq!(f[0]["multiline"], true);
        assert_eq!(f[1]["label"], "fleet");
        assert_eq!(f[1]["value"], "LAPTOP");
        assert_eq!(f[2]["label"], "Codex");
        assert_eq!(f[2]["value"], "");
        assert_eq!(f[2]["hint"], "good");
        assert_eq!(v["buttons"][0]["id"], "launch-send");
        assert_eq!(v["buttons"][0]["label"], "↑");
        assert_eq!(v["focusId"], "field-launch-prompt");
        assert_eq!(v["rows"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn the_line_names_effort_risky_guardrails_and_a_second_account() {
        let mut st = state();
        st.machines[0].local = true;
        st.machines[0]
            .permissions
            .insert("claude".into(), "bypassPermissions".into());
        st.accounts.push(Account {
            name: "work".into(),
            email: "work@example.com".into(),
            provider: "claude".into(),
            shared: true,
            ..Default::default()
        });
        let mut launch = fresh(&st);
        launch.cwd = "~".into();
        launch.provider = "claude".into();
        launch.account = "claude:work@example.com".into();
        launch.model = "opus".into();
        launch.effort = "high".into();
        launch.error = "Choose a project folder".into();
        let v = launch.view(&st);
        assert_eq!(
            ids(&v),
            ["launch-prompt", "launch-where", "launch-what", "launch-who"]
        );
        let f = &v["fields"];
        assert_eq!(f[1]["label"], "no project");
        assert_eq!(f[1]["value"], "this Mac");
        assert_eq!(f[1]["options"], json!(["muted"]));
        assert_eq!(f[2]["label"], "opus");
        assert_eq!(f[2]["value"], "High · Bypass permissions");
        assert_eq!(f[2]["options"], json!(["danger"]));
        assert_eq!(f[2]["hint"], "warn");
        assert_eq!(f[3]["label"], "work@example.com");
        assert_eq!(v["status"], "Choose a project folder");
        assert_eq!(v["statusError"], true);
        // A chosen safe mode shows; the machine's safe default does not.
        launch.permissions = "plan".into();
        launch.effort.clear();
        assert_eq!(launch.view(&st)["fields"][2]["value"], "Plan");
    }

    #[test]
    fn an_open_menu_brings_its_bar_search_and_rows() {
        let st = state();
        let mut launch = fresh(&st);
        launch.key("l", &Mods::META, false, &st);
        let v = launch.view(&st);
        assert_eq!(
            ids(&v),
            [
                "launch-prompt",
                "launch-where",
                "launch-what",
                "launch-machine-tab:0",
                "launch-machine-tab:1",
                "launch-machine-tab:2",
                "launch-menu-query",
            ]
        );
        assert_eq!(v["fields"][1]["options"], json!(["open"]));
        assert_eq!(v["fields"][0]["focused"], false);
        assert_eq!(v["fields"][3]["label"], "LAPTOP");
        assert_eq!(v["fields"][3]["focused"], true);
        assert_eq!(
            v["fields"][6]["placeholder"],
            "Search projects, or type a path"
        );
        assert_eq!(v["focusId"], "field-launch-menu-query");
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows[0]["label"], "No project");
        assert_eq!(rows[0]["detail"], "start in the home folder");
        assert_eq!(rows[1]["label"], "fleet");
        assert_eq!(rows[1]["detail"], "~/Developer/fleet");
        assert_eq!(rows[1]["chip"], "✓");
        assert_eq!(rows[1]["selected"], true);
        assert_eq!(v["index"], 1);

        launch.key("m", &Mods::META, false, &st);
        let v = launch.view(&st);
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows[0]["hint"], "group");
        assert_eq!(rows[0]["label"], "Codex");
        assert_eq!(rows[1]["label"], "Codex default");
        assert_eq!(rows[1]["detail"], "Whatever Codex is set up with");
        assert_eq!(rows[1]["swatch"], "good");
        assert_eq!(rows[1]["chip"], "✓");

        let mut st = st.clone();
        st.machines[0]
            .permissions
            .insert("codex".into(), "yolo".into());
        launch.key("e", &Mods::META, false, &st);
        let v = launch.view(&st);
        assert_eq!(v["focusId"], "");
        assert_eq!(ids(&v), ["launch-prompt", "launch-where", "launch-what"]);
        let rows = v["rows"].as_array().unwrap();
        let labels: Vec<&str> = rows.iter().map(|r| r["label"].as_str().unwrap()).collect();
        assert_eq!(labels[0], "Effort");
        assert_eq!(labels[1], "Auto");
        assert_eq!(rows[1]["detail"], "let the harness decide");
        assert_eq!(rows[2]["detail"], "reasoning effort");
        let guard = labels.iter().position(|l| *l == "Guardrail").unwrap();
        assert_eq!(labels[guard + 1], "Machine default · YOLO");
        assert_eq!(rows[guard + 1]["detail"], "whatever the machine is set to");
        assert_eq!(labels[guard + 2], "Provider default");
        let yolo = rows.last().unwrap();
        assert_eq!(yolo["label"], "YOLO · no approvals, no sandbox");
        assert_eq!(yolo["accent"], true);

        launch.key("l", &Mods::META, false, &st);
        launch.input("launch-menu-query", "/srv");
        let v = launch.view(&st);
        assert_eq!(v["rows"][0]["hint"], "folder");
        assert_eq!(v["rows"][0]["label"], "/srv");
        assert_eq!(v["rows"][0]["detail"], "use this folder");
    }
}
