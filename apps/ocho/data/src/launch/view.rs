//! The launch dialog's `OVERLAY` and `POPUP` JSON (ui.rs `render_launch`,
//! `render_launch_composer`, `render_machine_picker`, `render_project_picker`,
//! `render_model_popup`, `render_effort_popup`). Colors are theme names the
//! caller resolves; the ids are what `press` answers to.

use serde_json::{json, Value as Json};

use super::{
    account, account_email, effort_label, machine, machine_permission_default, provider_label,
    AddFolder, Focus, Launch, PickerRow, ProjectPicker,
};
use crate::picker::{self, PickerState, PopupAction, PopupItem, PopupKind};
use crate::quick;
use crate::types::State;

/// An `OVERLAY` with its title and mode pill, everything else zero.
pub fn overlay(kind: &str, title: &str, pill: &str, pill_color: &str) -> Json {
    json!({
        "kind": kind,
        "width": if kind == "quick" { 600 } else { 680 },
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

impl Launch {
    /// The `OVERLAY`: the quick launch, a picker, or the composer.
    pub fn view(&self, state: &State) -> Json {
        if self.quick.is_some() {
            return quick::view(self, state);
        }
        if let Some(picker) = &self.machine_picker {
            return self.machine_picker_view(state, picker);
        }
        if let Some(picker) = &self.picker {
            return match &picker.adding {
                Some(adding) => self.add_folder_view(adding),
                None => self.project_picker_view(picker),
            };
        }
        self.composer_view(state)
    }

    fn composer_view(&self, state: &State) -> Json {
        let f = self.focus;
        let machine_name = machine(state, &self.machine)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| self.machine.clone());
        let account_text = if self.account.is_empty() {
            "machine login".to_string()
        } else {
            account(state, &self.account)
                .map(account_email)
                .unwrap_or_else(|| self.account.clone())
        };
        let machine_default = machine_permission_default(state, &self.machine, &self.provider);
        let permissions_text = if !self.permissions.is_empty() {
            crate::permissions::label(&self.permissions)
        } else if machine_default.is_empty() {
            "provider default".to_string()
        } else {
            format!("{machine_default} · machine default")
        };
        let editing_model = f == Focus::Model && self.insert;
        let placeholder = "What would you like to work on?";
        let mut prompt = picker::field(
            "launch-prompt",
            "",
            &self.prompt,
            placeholder,
            "text",
            f == Focus::Prompt,
        );
        picker::set(&mut prompt, "multiline", json!(true));
        let model_value = if editing_model {
            self.model.clone()
        } else {
            self.model_name()
        };
        let mut pill = picker::field(
            "model-pill",
            provider_label(&self.provider),
            &model_value,
            "Provider default",
            if editing_model { "text" } else { "pill" },
            matches!(f, Focus::Model | Focus::Effort),
        );
        let effort: Vec<&str> = if self.has_effort() {
            vec![effort_label(&self.effort)]
        } else {
            vec![]
        };
        picker::set(&mut pill, "options", json!(effort));
        let half = if f == Focus::Effort {
            "effort"
        } else {
            "model"
        };
        picker::set(&mut pill, "hint", json!(half));
        let chip = |id: &str, label: &str, value: &str, focused: bool| {
            let mut c = picker::field(id, label, value, "", "chip", focused);
            picker::set(&mut c, "hint", json!("▾"));
            c
        };
        let (mode, color) = if self.insert {
            ("INSERT", "good")
        } else {
            ("NAV", "warn")
        };
        let mut v = overlay("launch", "Launch session", mode, color);
        let fields = json!([
            chip("launch-project", "Project", &self.cwd, f == Focus::Project),
            chip(
                "launch-machine",
                "Machine",
                &machine_name,
                f == Focus::Machine
            ),
            prompt,
            pill,
            chip(
                "account-chip",
                "Account",
                &account_text,
                f == Focus::Account
            ),
            chip(
                "permissions-chip",
                "Permissions",
                &permissions_text,
                f == Focus::Permissions
            ),
        ]);
        picker::set(&mut v, "fields", fields);
        let hint = if self.insert {
            "Enter launches · Shift+Enter new line · Tab moves to the project, model, account and permissions · Esc keeps a draft"
        } else {
            "Tab / j / k move · Space or Enter opens a picker · i types · Backspace resets · Esc keeps a draft"
        };
        picker::set(&mut v, "footer", json!(hint));
        picker::set(&mut v, "status", json!(self.error));
        picker::set(&mut v, "statusError", json!(!self.error.is_empty()));
        let buttons = json!([
            picker::button("launch-cancel", "Cancel", "Esc", false),
            picker::button("launch-submit", "Launch", "Enter", true),
        ]);
        picker::set(&mut v, "buttons", buttons);
        let focus_id = match (self.insert, self.insert_target()) {
            (true, Focus::Model) => "launch-model",
            (true, _) => "launch-prompt",
            _ => "",
        };
        picker::set(&mut v, "focusId", json!(focus_id));
        v
    }

    fn machine_picker_view(&self, state: &State, picker: &PickerState) -> Json {
        let rows: Vec<Json> = state
            .machines
            .iter()
            .filter(|m| m.sprite.is_none())
            .enumerate()
            .map(|(i, m)| {
                let current = if m.id == self.machine { "✓" } else { "" };
                let id = format!("launch-machine-row:{i}");
                picker::row(&id, "", &m.name, "", current, i == picker.index)
            })
            .collect();
        let mut v = overlay("machine-picker", "Choose machine", "MACHINE", "accent");
        let status = if rows.is_empty() {
            "No enrolled machines"
        } else {
            ""
        };
        picker::set(&mut v, "status", json!(status));
        picker::set(&mut v, "rows", json!(rows));
        picker::set(&mut v, "index", json!(picker.index));
        picker::set(
            &mut v,
            "footer",
            json!("↑ / ↓ or j / k choose · Enter select · Esc back"),
        );
        picker::set(
            &mut v,
            "buttons",
            json!([picker::button("machine-back", "Back", "Esc", false)]),
        );
        v
    }

    fn project_picker_view(&self, picker: &ProjectPicker) -> Json {
        let rows: Vec<Json> = self
            .picker_rows()
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let id = format!("project:{i}");
                let selected = i == picker.list.index;
                match row {
                    PickerRow::Project(pi) => {
                        let p = &self.projects[*pi];
                        let detail = format!("{} · {}", p.machine_name, p.detail());
                        picker::row(&id, "", &p.cwd, &detail, "", selected)
                    }
                    PickerRow::AddFolder { machine_name, .. } => {
                        let label = format!("+ Add folder on {machine_name}…");
                        let mut r = picker::row(&id, "", &label, "", "", selected);
                        picker::set(&mut r, "accent", json!(true));
                        r
                    }
                }
            })
            .collect();
        let mut v = overlay("project-picker", "Choose project", "PROJECT", "accent");
        picker::set(&mut v, "glyph", json!("/"));
        picker::set(&mut v, "placeholder", json!("Filter folders…"));
        picker::set(&mut v, "query", json!(picker.list.query));
        let status = if rows.is_empty() {
            "No matching folders"
        } else {
            ""
        };
        picker::set(&mut v, "status", json!(status));
        picker::set(&mut v, "rows", json!(rows));
        picker::set(&mut v, "index", json!(picker.list.index));
        let hint = if picker.list.typing {
            "Enter choose · ↑ / ↓ move · Esc clear filter"
        } else {
            "j / k choose · Enter use folder · / filter · a add folder · Esc back"
        };
        picker::set(&mut v, "footer", json!(hint));
        picker::set(
            &mut v,
            "buttons",
            json!([picker::button("project-back", "Back", "Esc", false)]),
        );
        let focus_id = if picker.list.typing {
            "project-query"
        } else {
            ""
        };
        picker::set(&mut v, "focusId", json!(focus_id));
        v
    }

    fn add_folder_view(&self, adding: &AddFolder) -> Json {
        let d = &adding.directory;
        let suggestions: Vec<Json> = d
            .paths
            .iter()
            .take(8)
            .enumerate()
            .map(|(i, path)| picker::row(&format!("dir:{i}"), "", path, "", "", i == d.index))
            .collect();
        let (status, error) = if !self.error.is_empty() {
            (self.error.clone(), true)
        } else if d.loading {
            ("Looking up folders…".to_string(), false)
        } else if !d.error.is_empty() {
            (
                format!("Folders: {} · Enter keeps your path", d.error),
                true,
            )
        } else if d.paths.is_empty() {
            (
                "No matching folders · Enter keeps your path".to_string(),
                false,
            )
        } else if d.truncated {
            (
                "More folders available; keep typing to narrow results".to_string(),
                false,
            )
        } else {
            (String::new(), false)
        };
        let mut field = picker::field(
            "add-folder",
            "",
            &adding.path,
            "~/path/to/project",
            "path",
            true,
        );
        picker::set(&mut field, "suggestions", json!(suggestions));
        let mut v = overlay("add-folder", "Choose project", "PROJECT", "accent");
        picker::set(
            &mut v,
            "subtitle",
            json!(format!("New folder on {}", adding.machine_name)),
        );
        picker::set(&mut v, "fields", json!([field]));
        picker::set(&mut v, "status", json!(status));
        picker::set(&mut v, "statusError", json!(error));
        let hint = "↑ / ↓ choose · Tab complete · Enter use this folder · Esc back to the list";
        picker::set(&mut v, "footer", json!(hint));
        picker::set(&mut v, "focusId", json!("add-folder"));
        v
    }

    /// The `POPUP` over the dialog: the model list, the effort list, the
    /// account or permissions menu; `Null` when none is open.
    pub fn popup_view(&self, state: &State) -> Json {
        if let Some(picker) = &self.model_picker {
            return self.model_popup(picker);
        }
        if let Some(highlighted) = self.effort_picker {
            let current = self.effort_index();
            let items: Vec<Json> = self
                .effort_levels()
                .iter()
                .enumerate()
                .map(|(i, level)| {
                    let action = PopupAction::Command(String::new());
                    let item = PopupItem::choice(effort_label(level), i == current, action);
                    picker::item(&item, &format!("effort-row:{i}"), i == highlighted)
                })
                .collect();
            let title = format!("Select effort · {}", provider_label(&self.provider));
            return picker::popup("effort", "model-pill", None, &title, items, "");
        }
        let Some(popup) = self.popup else {
            return Json::Null;
        };
        let (kind, anchor) = match popup.kind {
            PopupKind::LaunchAccount => ("launch-account", "account-chip"),
            _ => ("launch-permissions", "permissions-chip"),
        };
        let items: Vec<Json> = self
            .popup_items(popup.kind, state)
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let value = match &item.action {
                    PopupAction::LaunchAccount(v) | PopupAction::LaunchPermissions(v) => v.as_str(),
                    _ => "",
                };
                picker::item(item, &format!("{kind}:{value}"), i == popup.index)
            })
            .collect();
        picker::popup(kind, anchor, popup.at, "", items, "")
    }

    fn model_popup(&self, picker: &PickerState) -> Json {
        let mut items = Vec::new();
        let mut section = String::new();
        for (i, row) in self.model_rows().iter().enumerate() {
            let group = if row.recent {
                "recent"
            } else {
                row.provider.as_str()
            };
            if group != section {
                section = group.to_string();
                let note = self
                    .catalogs
                    .get(&row.provider)
                    .filter(|_| !row.recent)
                    .map(|c| {
                        if c.loading {
                            format!("loading from {}…", c.machine)
                        } else if !c.error.is_empty() {
                            format!("unavailable: {}", c.error)
                        } else {
                            String::new()
                        }
                    })
                    .unwrap_or_default();
                let label = if row.recent {
                    "Recent".to_string()
                } else {
                    provider_label(&row.provider).to_string()
                };
                items.push(json!({
                    "id": format!("section:{group}"),
                    "label": label,
                    "icon": "",
                    "hint": note,
                    "checked": false,
                    "checkable": false,
                    "selected": false,
                    "disabled": true,
                }));
            }
            let current = row.provider == self.provider && row.id == self.model;
            let described = row.id.is_empty() || row.id == "default" || row.recent;
            let hint = if described {
                row.description.clone()
            } else {
                String::new()
            };
            let action = PopupAction::Command(String::new());
            let item = PopupItem::choice(row.name.clone(), current, action).with_hint(hint);
            items.push(picker::item(
                &item,
                &format!("model-row:{i}"),
                i == picker.index,
            ));
        }
        picker::popup("model", "model-pill", None, "Select model", items, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::tests::{fresh, state};
    use crate::launch::{DirectoryMatches, LaunchMemory};
    use crate::picker::Mods;

    #[test]
    fn the_composer_lists_its_chips_in_focus_order() {
        let st = state();
        let mut launch =
            Launch::open(&LaunchMemory::default(), &st, None, "Fix it".into()).unwrap();
        let v = launch.view(&st);
        assert_eq!(v["kind"], "launch");
        assert_eq!(v["width"], 680);
        assert_eq!(v["title"], "Launch session");
        assert_eq!(
            (v["pill"].as_str(), v["pillColor"].as_str()),
            (Some("INSERT"), Some("good"))
        );
        let ids: Vec<&str> = v["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["id"].as_str().unwrap())
            .collect();
        let expected = [
            "launch-project",
            "launch-machine",
            "launch-prompt",
            "model-pill",
            "account-chip",
            "permissions-chip",
        ];
        assert_eq!(ids, expected);
        assert_eq!(v["fields"][0]["value"], "~/Developer/fleet");
        assert_eq!(v["fields"][1]["value"], "LAPTOP");
        assert_eq!(v["fields"][2]["value"], "Fix it");
        assert_eq!(
            v["fields"][2]["placeholder"],
            "What would you like to work on?"
        );
        assert_eq!(v["fields"][2]["focused"], true);
        assert_eq!(v["fields"][2]["multiline"], true);
        assert_eq!(v["fields"][3]["label"], "Codex");
        assert_eq!(v["fields"][3]["value"], "Default");
        assert_eq!(v["fields"][3]["options"][0], "Auto");
        assert_eq!(v["fields"][4]["value"], "machine login");
        assert_eq!(v["fields"][5]["value"], "provider default");
        assert_eq!(v["focusId"], "launch-prompt");
        assert!(v["footer"].as_str().unwrap().starts_with("Enter launches"));
        assert_eq!(v["buttons"][1]["label"], "Launch");
        assert_eq!(v["buttons"][1]["primary"], true);
        assert_eq!(launch.popup_view(&st), Json::Null);
        // NAV mode, a machine default, a chosen mode, and a submit error.
        launch.key("Escape", &Mods::NONE, false, &st);
        launch.error = "Choose a project folder".into();
        let mut with_default = st.clone();
        with_default.machines[0]
            .permissions
            .insert("codex".into(), "yolo".into());
        let v = launch.view(&with_default);
        assert_eq!(
            (v["pill"].as_str(), v["pillColor"].as_str()),
            (Some("NAV"), Some("warn"))
        );
        assert_eq!(v["fields"][5]["value"], "yolo · machine default");
        assert_eq!(v["status"], "Choose a project folder");
        assert_eq!(v["statusError"], true);
        assert!(v["footer"]
            .as_str()
            .unwrap()
            .starts_with("Tab / j / k move"));
        launch.permissions = "yolo".into();
        launch.provider = "opencode".into();
        let v = launch.view(&with_default);
        assert_eq!(v["fields"][5]["value"], "YOLO · no approvals, no sandbox");
        assert_eq!(v["fields"][3]["options"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn pickers_and_popups_carry_their_rows_and_hints() {
        let st = state();
        let mut launch = fresh(&st);
        launch.open_machine_picker(&st);
        let v = launch.view(&st);
        assert_eq!(v["kind"], "machine-picker");
        assert_eq!(v["pill"], "MACHINE");
        assert_eq!(v["rows"][0]["label"], "LAPTOP");
        assert_eq!(v["rows"][0]["hint"], "✓");
        assert_eq!(v["rows"][1]["selected"], false);
        assert_eq!(v["buttons"][0]["id"], "machine-back");
        launch.machine_picker = None;

        launch.open_project_picker(&st);
        let v = launch.view(&st);
        assert_eq!(v["kind"], "project-picker");
        assert_eq!(v["rows"][0]["label"], "~/Developer/fleet");
        assert_eq!(v["rows"][0]["detail"], "LAPTOP · session");
        assert_eq!(v["rows"][0]["selected"], true);
        let last = v["rows"].as_array().unwrap().last().unwrap().clone();
        assert_eq!(last["label"], "+ Add folder on BLANK…");
        assert_eq!(last["accent"], true);
        assert!(v["footer"].as_str().unwrap().starts_with("j / k choose"));
        launch.input("project-query", "nothing-matches");
        assert_eq!(launch.view(&st)["status"], "");
        launch.projects.clear();
        assert_eq!(launch.view(&st)["status"], "No matching folders");

        let mut launch = fresh(&st);
        launch.open_project_picker(&st);
        launch.start_adding_folder("laptop".into(), &st);
        let v = launch.view(&st);
        assert_eq!(v["kind"], "add-folder");
        assert_eq!(v["subtitle"], "New folder on LAPTOP");
        assert_eq!(v["fields"][0]["value"], "~/Developer/");
        assert_eq!(v["status"], "Looking up folders…");
        let matches = DirectoryMatches {
            paths: (0..10).map(|i| format!("~/d{i}/")).collect(),
            truncated: true,
        };
        launch.set_directories("laptop", "~/Developer/", Ok(matches));
        let v = launch.view(&st);
        assert_eq!(v["fields"][0]["suggestions"].as_array().unwrap().len(), 8);
        assert_eq!(v["fields"][0]["suggestions"][0]["selected"], true);
        assert_eq!(
            v["status"],
            "More folders available; keep typing to narrow results"
        );
        launch.set_directories("laptop", "~/Developer/", Ok(DirectoryMatches::default()));
        assert_eq!(
            launch.view(&st)["status"],
            "No matching folders · Enter keeps your path"
        );

        launch.picker = None;
        launch.toggle_effort_picker();
        let p = launch.popup_view(&st);
        assert_eq!(p["kind"], "effort");
        assert_eq!(p["title"], "Select effort · Codex");
        let labels: Vec<&str> = p["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["label"].as_str().unwrap())
            .collect();
        assert_eq!(
            labels,
            ["Auto", "Minimal", "Low", "Medium", "High", "Extra High"]
        );
        assert_eq!(p["items"][0]["checked"], true);
        launch.open_model_picker(&st);
        let p = launch.popup_view(&st);
        assert_eq!(p["kind"], "model");
        assert_eq!(p["items"][0]["id"], "section:codex");
        assert_eq!(p["items"][0]["disabled"], true);
        assert_eq!(p["items"][1]["label"], "Default");
        assert_eq!(p["items"][1]["hint"], "Whatever Codex is set up with");
        assert_eq!(p["items"][1]["checked"], true);
        assert_eq!(p["items"][1]["selected"], true);
        launch.model_picker = None;
        launch.open_popup(PopupKind::LaunchAccount, &st);
        let p = launch.popup_view(&st);
        assert_eq!(p["kind"], "launch-account");
        assert_eq!(p["anchor"], "account-chip");
        assert_eq!(p["items"][0]["id"], "launch-account:");
        assert_eq!(p["items"][0]["label"], "Machine login");
    }
}
