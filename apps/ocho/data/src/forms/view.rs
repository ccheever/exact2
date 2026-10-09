//! The form's `OVERLAY` JSON (ui.rs `render_form`, `directory_suggestions`,
//! `model_options_view`) and the dropdown's popup items.

use serde_json::{json, Value as Json};

use super::{choice_label, field_placeholder, FieldKind, Form};
use crate::launch::view::overlay;
use crate::picker::{self, PopupAction, PopupItem};
use crate::session::machine;
use crate::types::State;

/// Fields whose input takes several lines.
fn multiline(label: &str) -> bool {
    matches!(label, "Title prompt" | "Initial prompt")
}

impl Form {
    /// The `OVERLAY`: kind "form", the title, one `FIELD` per field (id = its
    /// index as text), the form error or the focused field's folder / model
    /// status, the kind's key hint as the footer, Cancel and the submit
    /// button, and the focused text field as `focusId` (`field-N`).
    pub fn view(&self, state: &State) -> Json {
        let mut fields: Vec<Json> = Vec::new();
        let mut status = (self.error.clone(), true);
        let mut focus_id = String::new();
        for (ix, f) in self.fields.iter().enumerate() {
            let focused = ix == self.focus;
            let kind = match f.kind {
                FieldKind::Text => "text",
                FieldKind::Choice => "choice",
                FieldKind::Directory => "path",
                FieldKind::Model => "model",
            };
            let (value, placeholder) = match f.kind {
                FieldKind::Choice => (choice_label(state, f.label, &f.value), ""),
                FieldKind::Model => (f.value.clone(), "Provider default"),
                _ => (f.value.clone(), field_placeholder(f.label)),
            };
            let mut v = picker::field(&ix.to_string(), f.label, &value, placeholder, kind, focused);
            if f.kind == FieldKind::Choice {
                let options: Vec<String> = self
                    .choices(state, f.label)
                    .iter()
                    .map(|c| choice_label(state, f.label, c))
                    .collect();
                picker::set(&mut v, "options", json!(options));
            }
            picker::set(&mut v, "multiline", json!(multiline(f.label)));
            if focused {
                if f.kind != FieldKind::Choice {
                    focus_id = format!("field-{ix}");
                }
                let hint = match f.kind {
                    FieldKind::Directory => {
                        "Ctrl+N / Ctrl+P choose folder · Tab complete · ↑ / ↓ fields · Enter next"
                    }
                    FieldKind::Model => {
                        "Click ‹ / › to choose · edit the model ID · Ctrl+R refresh · Tab next"
                    }
                    _ if multiline(f.label) => "Shift+Enter new line · Tab next field",
                    _ => "",
                };
                picker::set(&mut v, "hint", json!(hint));
                let (rows, field_status) = match f.kind {
                    FieldKind::Directory => self.directory_rows(),
                    FieldKind::Model => self.model_rows(state, &f.value),
                    _ => (Vec::new(), (String::new(), false)),
                };
                picker::set(&mut v, "suggestions", json!(rows));
                if self.error.is_empty() {
                    status = field_status;
                }
            }
            fields.push(v);
        }
        let mut v = overlay("form", self.title, "", "");
        picker::set(&mut v, "fields", json!(fields));
        picker::set(&mut v, "status", json!(status.0));
        picker::set(
            &mut v,
            "statusError",
            json!(status.1 && !status.0.is_empty()),
        );
        picker::set(&mut v, "footer", json!(self.hint()));
        let buttons = vec![
            picker::button("button:cancel", "Cancel", "Esc", false),
            picker::button("button:submit", self.submit_label(), "Enter", true),
        ];
        picker::set(&mut v, "buttons", json!(buttons));
        picker::set(&mut v, "focusId", json!(focus_id));
        v
    }

    /// Folder suggestions (`dir:N`, at most 8) and the line under them.
    fn directory_rows(&self) -> (Vec<Json>, (String, bool)) {
        let d = &self.directory;
        let rows: Vec<Json> = d
            .paths
            .iter()
            .take(8)
            .enumerate()
            .map(|(i, path)| picker::row(&format!("dir:{i}"), "", path, "", "", i == d.index))
            .collect();
        let status = if d.loading {
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
        (rows, status)
    }

    /// The catalog under the Model field (`model-row:N`, at most 10; the
    /// current one selected) and the line under it.
    fn model_rows(&self, state: &State, value: &str) -> (Vec<Json>, (String, bool)) {
        let m = &self.models;
        let machine_name = m
            .query
            .as_ref()
            .and_then(|k| machine(state, &k.machine))
            .map(|m| m.name.clone())
            .unwrap_or_default();
        if let Some(error) = self.catalog_unavailable(state) {
            return (
                Vec::new(),
                (
                    format!(
                        "Models: {error} · Use native default or type a model ID · Ctrl+R retry"
                    ),
                    true,
                ),
            );
        }
        if m.loading {
            return (
                Vec::new(),
                (format!("Loading models from {machine_name}…"), false),
            );
        }
        if !m.error.is_empty() {
            return (
                Vec::new(),
                (
                    format!(
                        "Models: {} · Use native default or type a model ID · Ctrl+R retry",
                        m.error
                    ),
                    true,
                ),
            );
        }
        let rows: Vec<Json> = crate::launch::model_options(&m.options, value)
            .iter()
            .take(10)
            .enumerate()
            .map(|(i, option)| {
                let name = if option.default {
                    format!("{} · provider default", option.name)
                } else {
                    option.name.clone()
                };
                picker::row(
                    &format!("model-row:{i}"),
                    "",
                    &name,
                    &option.description,
                    "",
                    option.id == value,
                )
            })
            .collect();
        let status = if machine_name.is_empty() {
            String::new()
        } else {
            format!("Models from {machine_name}")
        };
        (rows, (status, false))
    }

    /// The dropdown of the field at `index` (`PopupKind::FieldChoice`): every
    /// choice by its label, the current one checked.
    pub fn popup_items(&self, index: usize, state: &State) -> Vec<PopupItem> {
        let Some(field) = self.fields.get(index) else {
            return Vec::new();
        };
        self.choices(state, field.label)
            .into_iter()
            .map(|value| {
                PopupItem::choice(
                    choice_label(state, field.label, &value),
                    value == field.value,
                    PopupAction::Choice(index, value),
                )
            })
            .collect()
    }
}
