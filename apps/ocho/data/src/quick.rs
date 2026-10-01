//! Quick launch (⌘⇧N): persistent, append-only account and machine
//! assignments (quick_launch.rs), with the numbered-slot keys and card from
//! workspace.rs and ui.rs. Missing identities keep their slots; models
//! follow the selected CLI's current catalog instead, and can be searched
//! by name after `/`, chosen with the arrows and started with Enter.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

use crate::launch::{
    account, account_handle, account_label, effort_key, machine, overlay, provider_label,
};
use crate::launch::{Focus, Launch, LaunchMemory, ModelOption, Outcome};
use crate::picker::{self, canon, typed, Mods};
use crate::session::clean;
use crate::types::{Account, State};

/// The saved slots (`launch_shortcuts` in desktop.json).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Shortcuts {
    /// Account ids (`provider\nhandle`), slot 1 first.
    pub accounts: Vec<String>,
    /// Machine ids, slot 1 first.
    pub machines: Vec<String>,
}

/// Sort only new identities: inventory order and removals never change old slots.
pub fn enroll(slots: &mut Vec<String>, identities: impl IntoIterator<Item = String>) {
    let mut added: Vec<_> = identities
        .into_iter()
        .filter(|id| !slots.contains(id))
        .collect();
    added.sort();
    added.dedup();
    slots.extend(added.into_iter().take(99usize.saturating_sub(slots.len())));
}

/// The id an account keeps its slot under.
pub fn shortcut_id(a: &Account) -> String {
    format!("{}\n{}", a.provider, account_handle(a))
}

/// Freeze the fleet's shared accounts and enrolled machines into slots.
pub fn enroll_state(shortcuts: &mut Shortcuts, state: &State) {
    enroll(
        &mut shortcuts.accounts,
        state.accounts.iter().filter(|a| a.shared).map(shortcut_id),
    );
    enroll(
        &mut shortcuts.machines,
        state
            .machines
            .iter()
            .filter(|m| m.sprite.is_none())
            .map(|m| m.id.clone()),
    );
}

/// The model step mirrors the selected CLI catalog: same order and labels,
/// with the exact ID retained only as the launch value.
pub fn model_rows(options: &[ModelOption]) -> Vec<(usize, String, bool)> {
    options
        .iter()
        .enumerate()
        .map(|(index, model)| {
            let label = if model.name.is_empty() {
                model.id.clone()
            } else {
                model.name.clone()
            };
            (index + 1, label, true)
        })
        .collect()
}

/// The model id at a one-based slot.
pub fn model_id(options: &[ModelOption], slot: usize) -> Option<&str> {
    options
        .get(slot.checked_sub(1)?)
        .map(|model| model.id.as_str())
}

/// Filter display names and native IDs, keeping the original numbered slots.
/// An explicit model name wins; family queries prefer the newest version.
pub fn matching_model_rows(options: &[ModelOption], query: &str) -> Vec<(usize, String, bool)> {
    let query = query.trim().to_lowercase();
    let mut rows = model_rows(options);
    if query.is_empty() {
        return rows;
    }
    rows.retain(|(slot, _, _)| {
        let model = &options[slot - 1];
        let text = format!("{} {}", model.id, model.name).to_lowercase();
        query.split_whitespace().all(|term| text.contains(term))
    });
    rows.sort_by_cached_key(|(slot, _, _)| {
        let model = &options[slot - 1];
        let exact = model.id.to_lowercase() == query || model.name.to_lowercase() == query;
        let version = model_version(&model.name).or_else(|| model_version(&model.id));
        (std::cmp::Reverse(exact), std::cmp::Reverse(version))
    });
    rows
}

// Compare version components numerically: 6.10 is newer than 6.9. Ignore
// later numbers such as context sizes and release dates when ranking a family.
fn model_version(value: &str) -> Option<Vec<u64>> {
    let start = value.find(|c: char| c.is_ascii_digit())?;
    let version: String = value[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if value[start + version.len()..].starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(version.split('.').filter_map(|n| n.parse().ok()).collect())
}

/// A model choice entered before the current CLI catalog has loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingModel {
    /// A numbered slot.
    Slot(usize),
    /// Whatever the search highlights once the catalog lands.
    Search,
}

/// Account → Machine → Model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Choose account.
    Account,
    /// Choose machine.
    Machine,
    /// Choose model to start.
    Model,
}

/// The numbered chooser's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickLaunch {
    /// The step.
    pub step: Step,
    /// The digit typed so far (one at most).
    pub digits: String,
    /// A choice entered before the current CLI catalog has loaded.
    pub pending_model: Option<PendingModel>,
    /// The model search after `/`, when one is being typed.
    pub query: Option<String>,
    /// The highlighted model row (arrows and Enter on the model step).
    pub index: usize,
    /// The slots frozen when the chooser opened.
    pub shortcuts: Shortcuts,
}

impl QuickLaunch {
    /// Start on the account step.
    pub fn new(shortcuts: Shortcuts) -> Self {
        Self {
            step: Step::Account,
            digits: String::new(),
            pending_model: None,
            query: None,
            index: 0,
            shortcuts,
        }
    }

    /// Backspace: cancel a queued model, edit or leave the search, erase a
    /// digit, or go back a step.
    pub fn back(&mut self) {
        if self.pending_model.take().is_some() {
            return;
        }
        if let Some(query) = &mut self.query {
            if query.pop().is_none() {
                self.query = None;
            }
            self.index = 0;
            return;
        }
        if self.digits.pop().is_none() {
            self.step = match self.step {
                Step::Account | Step::Machine => Step::Account,
                Step::Model => Step::Machine,
            };
            self.index = 0;
        }
    }

    /// Slash explicitly starts search so digit-leading names cannot be
    /// confused with numbered shortcuts. All later characters are query
    /// text. False when the text was not search.
    pub fn search(&mut self, text: &str) -> bool {
        if self.query.is_none() && text != "/" {
            return false;
        }
        let text = if self.query.is_none() { "" } else { text };
        self.pending_model = None;
        self.digits.clear();
        self.query.get_or_insert_with(String::new).push_str(text);
        self.index = 0;
        true
    }

    /// Up / down over `count` model rows, clamped at the ends; cancels a
    /// queued choice and any digit typed so far.
    pub fn navigate(&mut self, down: bool, count: usize) {
        self.pending_model = None;
        self.digits.clear();
        let last = count.saturating_sub(1);
        self.index = self.index.min(last);
        self.index = if down {
            (self.index + 1).min(last)
        } else {
            self.index.saturating_sub(1)
        };
    }

    /// One digit; the slot once two are in.
    pub fn digit(&mut self, digit: char) -> Option<usize> {
        if !digit.is_ascii_digit() {
            return None;
        }
        self.digits.push(digit);
        if self.digits.len() != 2 {
            return None;
        }
        let slot = self.digits.parse().ok();
        self.digits.clear();
        slot
    }
}

/// ⌘⇧N (workspace.rs `open_quick_launch`): the launch dialog with the
/// frozen slots, a fresh prompt in the selected host's home; `draft` is
/// the composer's draft, held for Tab.
pub fn open(
    saved: &LaunchMemory,
    state: &State,
    selected: Option<&str>,
    shortcuts: &Shortcuts,
    draft: String,
) -> Result<Launch, &'static str> {
    let mut launch = Launch::open(saved, state, selected, String::new())?;
    launch.draft = draft;
    launch.model.clear();
    launch.effort.clear();
    launch.permissions.clear();
    launch.cwd = "~".into();
    launch.quick = Some(QuickLaunch::new(shortcuts.clone()));
    Ok(launch)
}

/// Accounts and machines keep stable slots. Models use the selected CLI's
/// current catalog; name searches rank matching versions newest first.
pub fn rows(launch: &Launch, state: &State) -> Vec<(usize, String, bool)> {
    let Some(q) = &launch.quick else {
        return Vec::new();
    };
    if q.step == Step::Model {
        return launch
            .catalogs
            .get(&launch.provider)
            .filter(|catalog| !catalog.loading && catalog.error.is_empty())
            .map(|catalog| {
                matching_model_rows(&catalog.options, q.query.as_deref().unwrap_or_default())
            })
            .unwrap_or_default();
    }
    let slots = if q.step == Step::Account {
        &q.shortcuts.accounts
    } else {
        &q.shortcuts.machines
    };
    let mut rows: Vec<_> = slots
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let label = if q.step == Step::Account {
                state
                    .accounts
                    .iter()
                    .find(|a| a.shared && shortcut_id(a) == *id)
                    .map(|a| format!("{} · {}", account_label(a), provider_label(&a.provider)))
            } else {
                machine(state, id).map(|m| m.name.clone())
            };
            let available = label.is_some();
            (
                i + 1,
                label.unwrap_or_else(|| "Unavailable · shortcut reserved".into()),
                available,
            )
        })
        .collect();
    if q.step == Step::Machine {
        rows.insert(
            0,
            (
                0,
                "Auto · choose after you describe the task".into(),
                state.machines.iter().any(|m| m.local),
            ),
        );
    }
    rows
}

/// One key (workspace.rs `handle_quick_launch_key`); held keys are ignored.
pub fn key(launch: &mut Launch, key: &str, mods: &Mods, held: bool, state: &State) -> Outcome {
    if held {
        return Outcome::None;
    }
    let key = canon(key);
    if key == "escape" {
        return Outcome::Close;
    }
    let rows = rows(launch, state);
    let Some(q) = launch.quick.as_mut() else {
        return Outcome::None;
    };
    if key == "tab" {
        launch.auto_machine = false;
        launch.quick = None;
        launch.prompt = std::mem::take(&mut launch.draft);
        launch.focus = Focus::Project;
        launch.error.clear();
        return Outcome::None;
    }
    if key == "backspace" {
        q.back();
        launch.error.clear();
        return Outcome::None;
    }
    if mods.ctrl || mods.meta || mods.alt {
        return Outcome::None;
    }
    let catalog = launch.catalogs.get(&launch.provider);
    let loading = q.step == Step::Model && catalog.is_none_or(|c| c.loading);
    let failed = catalog.is_some_and(|c| !c.error.is_empty());
    if q.step == Step::Model && matches!(key.as_str(), "up" | "down") {
        q.navigate(key == "down", rows.len());
    } else if q.step == Step::Model && key == "enter" {
        if q.pending_model.is_some() {
            return Outcome::None;
        }
        let numbered = q.digits.parse().ok();
        if loading {
            q.pending_model = Some(numbered.map_or(PendingModel::Search, PendingModel::Slot));
        } else if let Some(slot) = numbered.or_else(|| highlighted(q, &rows)) {
            return pick(launch, slot, state);
        }
    } else if key == "r" && q.query.is_none() && failed {
        launch.sync_catalogs(state, true);
    } else if let Some(text) = typed(&key, mods) {
        if q.step == Step::Model && q.search(&text) {
            launch.error.clear();
        } else if q.pending_model.is_none() {
            if let Some(slot) = text.chars().next().and_then(|digit| q.digit(digit)) {
                if loading {
                    q.pending_model = Some(PendingModel::Slot(slot));
                } else {
                    return pick(launch, slot, state);
                }
            }
        }
    }
    Outcome::None
}

/// The slot of the highlighted model row.
fn highlighted(q: &QuickLaunch, rows: &[(usize, String, bool)]) -> Option<usize> {
    rows.get(q.index.min(rows.len().saturating_sub(1)))
        .map(|row| row.0)
}

/// Choose the slot: the account, then the machine (0 for auto), then the
/// model, which submits.
pub fn pick(launch: &mut Launch, slot: usize, state: &State) -> Outcome {
    let available = rows(launch, state).iter().any(|r| r.0 == slot && r.2);
    let Some(q) = launch.quick.as_mut() else {
        return Outcome::None;
    };
    q.digits.clear();
    q.pending_model = None;
    if !available {
        launch.error =
            "Shortcut unavailable. Choose a listed number; reserved numbers never move.".into();
        return Outcome::None;
    }
    launch.error.clear();
    match q.step {
        Step::Account => {
            let id = &q.shortcuts.accounts[slot - 1];
            let Some(a) = state
                .accounts
                .iter()
                .find(|a| a.shared && shortcut_id(a) == *id)
            else {
                return Outcome::None;
            };
            launch.account = account_handle(a);
            launch.provider = a.provider.clone();
            launch.model.clear();
            launch.effort.clear();
            launch.catalogs.clear();
            launch.catalog_key.clear();
            q.step = Step::Machine;
        }
        Step::Machine => {
            launch.auto_machine = slot == 0;
            let machine_id = if launch.auto_machine {
                let provider = launch.provider.clone();
                state
                    .machines
                    .iter()
                    .filter(|m| m.sprite.is_none() && m.sprite_source.is_empty())
                    .filter(|m| {
                        m.last
                            .as_ref()
                            .and_then(|s| s.versions.get(&provider))
                            .is_some_and(|v| v != "not installed")
                    })
                    .min_by_key(|m| !m.local)
                    .or_else(|| state.machines.iter().find(|m| m.local))
                    .map(|m| m.id.clone())
                    .unwrap_or_else(|| launch.machine.clone())
            } else {
                q.shortcuts.machines[slot - 1].clone()
            };
            q.step = Step::Model;
            launch.select_quick_machine(&machine_id);
            launch.sync_catalogs(state, true);
        }
        Step::Model => {
            let model = launch
                .catalogs
                .get(&launch.provider)
                .filter(|catalog| !catalog.loading && catalog.error.is_empty())
                .and_then(|catalog| model_id(&catalog.options, slot))
                .map(str::to_string);
            let Some(model) = model else {
                launch.error =
                    "That model is no longer in the CLI catalog. Choose a listed number.".into();
                return Outcome::None;
            };
            launch.model = model;
            launch.effort = launch
                .efforts
                .get(&effort_key(&launch.provider, &launch.model))
                .cloned()
                .unwrap_or_default();
            // Revalidate the selected identities against the latest feed.
            let account_ok = state.accounts.iter().any(|a| {
                a.shared && a.provider == launch.provider && account_handle(a) == launch.account
            });
            if !account_ok {
                launch.error =
                    "The selected account is no longer available. Backspace to choose again."
                        .into();
            } else if !state.machines.iter().any(|m| m.id == launch.machine) {
                launch.error =
                    "The selected machine was removed. Backspace to choose again.".into();
            } else {
                return Outcome::Submit;
            }
        }
    }
    Outcome::None
}

/// Complete a model choice entered before the current CLI catalog arrived;
/// a catalog that failed drops the queued choice.
pub fn finish_pending(launch: &mut Launch, state: &State) -> Outcome {
    let rows = rows(launch, state);
    let settled = launch.catalogs.get(&launch.provider).filter(|c| !c.loading);
    let failed = settled.is_some_and(|c| !c.error.is_empty());
    let ready = settled.is_some();
    let slot = match launch.quick.as_mut() {
        Some(q) if q.step == Step::Model && ready => match q.pending_model.take() {
            _ if failed => None,
            Some(PendingModel::Slot(slot)) => Some(slot),
            Some(PendingModel::Search) => highlighted(q, &rows),
            None => None,
        },
        _ => None,
    };
    match slot {
        Some(slot) => pick(launch, slot, state),
        None => Outcome::None,
    }
}

/// The `OVERLAY` (kind "quick"): ui.rs `render_quick_launch`. The list's
/// own messages (loading, a failed catalog, nothing to show) and the error
/// share `status`; the search line `/query▏` is `hint`; the typed-number
/// indicator is `query`.
pub fn view(launch: &Launch, state: &State) -> Json {
    let Some(q) = &launch.quick else {
        return Json::Null;
    };
    let title = match q.step {
        Step::Account => "Choose account",
        Step::Machine => "Choose machine",
        Step::Model => "Choose model to start",
    };
    let catalog = launch.catalogs.get(&launch.provider);
    let loading = q.step == Step::Model && catalog.is_none_or(|c| c.loading);
    let failed = q.step == Step::Model && catalog.is_some_and(|c| !c.error.is_empty());
    let rows = rows(launch, state);
    let searching = q
        .query
        .as_ref()
        .is_some_and(|query| !query.trim().is_empty());
    let (status, error) = if !launch.error.is_empty() {
        (launch.error.clone(), true)
    } else if loading {
        let message = match q.pending_model {
            Some(PendingModel::Slot(slot)) => format!("Starting {slot:02} when models are ready…"),
            Some(PendingModel::Search) => {
                "Starting the matching model when models are ready…".into()
            }
            None if catalog.is_some_and(|c| !c.options.is_empty()) => "Refreshing models…".into(),
            None => "Loading models…".into(),
        };
        (message, false)
    } else if failed {
        let error = catalog.map(|c| c.error.as_str()).unwrap_or("");
        (format!("{error} · Clear search and press R to retry"), true)
    } else if rows.is_empty() {
        let message = if searching {
            "No matching models. Backspace to edit your search."
        } else {
            "No options available. Tab opens the full launcher."
        };
        (message.to_string(), false)
    } else {
        (String::new(), false)
    };
    let selected = q.index.min(rows.len().saturating_sub(1));
    let rows: Vec<Json> = rows
        .iter()
        .enumerate()
        .map(|(index, (slot, label, available))| {
            let prefix = format!("{slot:02}");
            let highlighted = if q.digits.is_empty() {
                q.step == Step::Model && index == selected
            } else {
                prefix.starts_with(&q.digits)
            };
            let id = format!("quick-slot:{slot}");
            let mut r = picker::row(&id, &prefix, &clean(label), "", "", highlighted);
            picker::set(&mut r, "accent", json!(available));
            r
        })
        .collect();
    let mut body = Vec::new();
    if q.step != Step::Account {
        let a = account(state, &launch.account)
            .map(account_label)
            .unwrap_or_else(|| launch.account.clone());
        body.push(format!(
            "{} · {}",
            clean(&a),
            provider_label(&launch.provider)
        ));
    }
    if q.step == Step::Model {
        let m = if launch.auto_machine {
            "Auto · chosen after you submit your task"
        } else {
            machine(state, &launch.machine)
                .map(|m| m.name.as_str())
                .unwrap_or("Removed machine")
        };
        body.push(format!("{} · {}", clean(m), clean(&launch.cwd)));
    }
    let input = match q.pending_model {
        Some(PendingModel::Slot(slot)) => format!("{slot:02} · queued"),
        Some(PendingModel::Search) => "Search queued".into(),
        None if q.query.is_some() => "↑↓ choose · Enter launch".into(),
        None => format!("{}{}", q.digits, "_".repeat(2 - q.digits.len().min(2))),
    };
    let hints = if q.pending_model.is_some() {
        "Backspace cancel · Esc close"
    } else if q.step == Step::Model {
        if q.query.is_some() {
            "Backspace edit · Esc close · Tab more"
        } else {
            "/ search · ↑↓ choose · Enter launch · Esc close · Tab more"
        }
    } else {
        "Esc close · Tab more"
    };
    let search = q
        .query
        .as_ref()
        .map(|query| format!("/{}▏", clean(query)))
        .unwrap_or_default();
    let mut v = overlay("quick", title, "CTRL N", "accent");
    picker::set(&mut v, "subtitle", json!("Account → Machine → Model"));
    picker::set(&mut v, "body", json!(body.join("\n")));
    picker::set(&mut v, "rows", json!(rows));
    picker::set(&mut v, "index", json!(selected));
    picker::set(&mut v, "status", json!(status));
    picker::set(&mut v, "statusError", json!(error));
    picker::set(&mut v, "query", json!(input));
    picker::set(&mut v, "hint", json!(search));
    picker::set(&mut v, "footer", json!(hints));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Machine, Snapshot};

    #[test]
    fn removed_slots_survive_restart_and_inventory_reordering() {
        let mut s = Shortcuts::default();
        enroll(&mut s.accounts, ["b".into(), "a".into()]);
        let mut s: Shortcuts = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        enroll(&mut s.accounts, ["c".into(), "b".into()]);
        assert_eq!(s.accounts, ["a", "b", "c"]);
        enroll(&mut s.accounts, ["a".into()]);
        assert_eq!(s.accounts, ["a", "b", "c"]);
    }

    #[test]
    fn two_digits_select_without_timeouts_and_backspace_edits_or_cancels() {
        let mut q = QuickLaunch::new(Shortcuts::default());
        assert_eq!(q.digit('0'), None);
        q.back();
        assert!(q.digits.is_empty());
        assert_eq!(q.digit('1'), None);
        assert_eq!(q.digit('2'), Some(12));
        q.step = Step::Model;
        q.pending_model = Some(PendingModel::Slot(4));
        q.back();
        assert_eq!(q.step, Step::Model);
        assert_eq!(q.pending_model, None);
        q.back();
        assert_eq!(q.step, Step::Machine);
        q.back();
        assert_eq!(q.step, Step::Account);
    }

    #[test]
    fn models_keep_the_cli_order_names_and_ids() {
        let options = vec![
            ModelOption {
                id: "opus[1m]".into(),
                name: "Opus 5.6 (1M)".into(),
                ..Default::default()
            },
            ModelOption {
                id: "sonnet".into(),
                name: "Sonnet 5.6".into(),
                ..Default::default()
            },
        ];
        assert_eq!(
            model_rows(&options),
            vec![
                (1, "Opus 5.6 (1M)".into(), true),
                (2, "Sonnet 5.6".into(), true)
            ]
        );
        assert_eq!(model_id(&options, 1), Some("opus[1m]"));
        assert_eq!(model_id(&options, 2), Some("sonnet"));
        assert_eq!(model_id(&options, 0), None);
        assert_eq!(model_id(&options, 3), None);
    }

    fn options(models: &[(&str, &str)]) -> Vec<ModelOption> {
        models
            .iter()
            .map(|(id, name)| ModelOption {
                id: (*id).into(),
                name: (*name).into(),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn family_search_ranks_versions_numerically_and_preserves_launch_ids() {
        let options = options(&[
            ("gpt-6-sol", "GPT-6 Sol"),
            ("gpt-6-astra", "GPT-6 Astra"),
            ("gpt-6.9-sol", "GPT-6.9 Sol"),
            ("gpt-6.10-sol", "GPT-6.10 Sol"),
            ("gpt-5.6-sol", "GPT-5.6 Sol"),
        ]);
        let rows = matching_model_rows(&options, " SOL ");
        assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), [4, 3, 1, 5]);
        assert_eq!(model_id(&options, rows[0].0), Some("gpt-6.10-sol"));
        assert_eq!(matching_model_rows(&options, "gpt-6-sol")[0].0, 1);
        assert_eq!(matching_model_rows(&options, "GPT-6 Sol")[0].0, 1);
        assert!(matching_model_rows(&options, "sonnet").is_empty());
        assert_eq!(matching_model_rows(&options, " "), model_rows(&options));
    }

    #[test]
    fn search_uses_display_versions_and_keeps_catalog_order_for_ties() {
        let options = options(&[
            ("sonnet[1m]", "Sonnet 4.5 (1M)"),
            ("current", "Sonnet 4.6"),
            ("current-long", "Sonnet 4.6 (1M)"),
        ]);
        let rows = matching_model_rows(&options, "sonnet");
        assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), [2, 3, 1]);
        assert_eq!(matching_model_rows(&options, "sonnet 1m")[0].0, 3);
    }

    #[test]
    fn search_edits_reset_selection_and_backspace_cancels_before_editing() {
        let mut q = QuickLaunch::new(Shortcuts::default());
        q.step = Step::Model;
        q.digit('0');
        q.search("/");
        q.search("sol");
        assert!(q.digits.is_empty());
        q.navigate(true, 3);
        assert_eq!(q.index, 1);
        q.search(" 6");
        assert_eq!(q.query.as_deref(), Some("sol 6"));
        assert_eq!(q.index, 0);
        q.pending_model = Some(PendingModel::Search);
        q.back();
        assert_eq!(q.pending_model, None);
        assert_eq!(q.query.as_deref(), Some("sol 6"));
        for _ in 0..5 {
            q.back();
        }
        assert_eq!(q.query.as_deref(), Some(""));
        assert_eq!(q.step, Step::Model);
        q.back();
        assert_eq!(q.query, None);
        assert_eq!(q.step, Step::Model);
        q.back();
        assert_eq!(q.step, Step::Machine);
    }

    #[test]
    fn slash_is_required_and_digit_leading_names_remain_search_text() {
        let mut q = QuickLaunch::new(Shortcuts::default());
        q.step = Step::Model;
        assert!(!q.search("s"));
        assert!(!q.search("6"));
        assert_eq!(q.query, None);
        assert_eq!(q.digit('0'), None);
        assert!(q.search("/"));
        assert!(q.digits.is_empty());
        for text in ["6", ".", "1", "-", "sol"] {
            assert!(q.search(text));
        }
        assert_eq!(q.query.as_deref(), Some("6.1-sol"));
        assert!(q.digits.is_empty());
        while q.query.is_some() {
            q.back();
        }
        assert!(!q.search("s"));
        assert_eq!(q.digit('0'), None);
        assert_eq!(q.digit('2'), Some(2));
    }

    #[test]
    fn arrow_navigation_stays_in_bounds_and_edits_cancel_queued_search() {
        let mut q = QuickLaunch::new(Shortcuts::default());
        q.search("/");
        q.search("sol");
        q.navigate(false, 3);
        assert_eq!(q.index, 0);
        for _ in 0..5 {
            q.navigate(true, 3);
        }
        assert_eq!(q.index, 2);
        q.pending_model = Some(PendingModel::Search);
        q.navigate(false, 3);
        assert_eq!(q.pending_model, None);
        assert_eq!(q.index, 1);
        q.navigate(true, 0);
        assert_eq!(q.index, 0);
        q.pending_model = Some(PendingModel::Search);
        q.search("jkl");
        assert_eq!(q.pending_model, None);
        assert_eq!(q.query.as_deref(), Some("soljkl"));
    }

    #[test]
    fn legacy_model_shortcuts_are_ignored() {
        let shortcuts: Shortcuts = serde_json::from_str(
            r#"{"accounts":["account"],"machines":["machine"],"models":{"claude":["old-opus"]},"model_names":{"claude":{"old-opus":"Opus 5.5"}}}"#,
        )
        .unwrap();
        assert_eq!(shortcuts.accounts, ["account"]);
        assert_eq!(shortcuts.machines, ["machine"]);
        let saved = serde_json::to_string(&shortcuts).unwrap();
        assert!(!saved.contains("old-opus"));
    }

    #[test]
    fn exhaustion_never_reuses_a_retired_slot() {
        let mut slots = Vec::new();
        enroll(&mut slots, (0..110).map(|i| format!("id-{i:03}")));
        assert_eq!(slots.len(), 99);
        enroll(&mut slots, ["new".into()]);
        assert_eq!(slots.len(), 99);
        assert!(!slots.contains(&"new".into()));
    }

    fn state() -> State {
        State {
            machines: vec![
                Machine {
                    id: "laptop".into(),
                    name: "Laptop".into(),
                    local: true,
                    last: Some(Snapshot::default()),
                    ..Default::default()
                },
                Machine {
                    id: "studio".into(),
                    name: "Studio".into(),
                    ..Default::default()
                },
            ],
            accounts: vec![Account {
                name: "me".into(),
                email: "me@example.com".into(),
                provider: "claude".into(),
                shared: true,
                status: "connected".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn digits_walk_the_steps_and_a_queued_model_submits_when_the_catalog_lands() {
        let st = state();
        let mut shortcuts = Shortcuts {
            accounts: vec!["gone\nx".into()],
            ..Default::default()
        };
        enroll_state(&mut shortcuts, &st);
        assert_eq!(
            shortcuts.accounts,
            ["gone\nx", "claude\nclaude:me@example.com"]
        );
        assert_eq!(shortcuts.machines, ["laptop", "studio"]);
        let mut launch = open(
            &LaunchMemory::default(),
            &st,
            None,
            &shortcuts,
            "draft".into(),
        )
        .unwrap();
        assert_eq!(launch.cwd, "~");
        let v = view(&launch, &st);
        assert_eq!(v["title"], "Choose account");
        assert_eq!(v["rows"][0]["label"], "Unavailable · shortcut reserved");
        assert_eq!(v["query"], "__");
        // A reserved slot says so; the live one moves on.
        launch.key("0", &Mods::NONE, false, &st);
        assert_eq!(view(&launch, &st)["query"], "0_");
        launch.key("1", &Mods::NONE, false, &st);
        assert!(launch.error.starts_with("Shortcut unavailable"));
        launch.key("0", &Mods::NONE, true, &st);
        launch.key("0", &Mods::NONE, false, &st);
        launch.key("2", &Mods::NONE, false, &st);
        assert_eq!(
            (launch.provider.as_str(), launch.account.as_str()),
            ("claude", "claude:me@example.com")
        );
        assert_eq!(launch.quick.as_ref().unwrap().step, Step::Machine);
        let v = view(&launch, &st);
        assert_eq!(
            v["rows"][0]["label"],
            "Auto · choose after you describe the task"
        );
        assert_eq!(v["rows"][2]["label"], "Studio");
        // Machine 02 (Studio): the catalog starts loading, a typed model waits.
        launch.key("0", &Mods::NONE, false, &st);
        launch.key("2", &Mods::NONE, false, &st);
        assert_eq!(launch.machine, "studio");
        let key = launch.wanted_catalog().unwrap();
        assert_eq!(view(&launch, &st)["status"], "Loading models…");
        launch.key("0", &Mods::NONE, false, &st);
        launch.key("1", &Mods::NONE, false, &st);
        let v = view(&launch, &st);
        assert_eq!(v["query"], "01 · queued");
        assert_eq!(v["status"], "Starting 01 when models are ready…");
        assert_eq!(v["footer"], "Backspace cancel · Esc close");
        let models = vec![ModelOption {
            id: "opus".into(),
            name: "Opus".into(),
            ..Default::default()
        }];
        assert_eq!(launch.set_models(&key, Ok(models), &st), Outcome::Submit);
        assert_eq!(launch.model, "opus");
        // Tab hands over to the full launcher with the earlier draft.
        launch.key("Tab", &Mods::NONE, false, &st);
        assert!(launch.quick.is_none());
        assert_eq!(
            (launch.prompt.as_str(), launch.focus),
            ("draft", Focus::Project)
        );
        assert_eq!(
            launch.key("Escape", &Mods::NONE, false, &st),
            Outcome::Close
        );
    }

    #[test]
    fn the_model_step_searches_by_name_and_enter_starts_the_highlight() {
        let st = state();
        let mut shortcuts = Shortcuts::default();
        enroll_state(&mut shortcuts, &st);
        let mut launch = open(
            &LaunchMemory::default(),
            &st,
            None,
            &shortcuts,
            String::new(),
        )
        .unwrap();
        for k in ["0", "1", "0", "1"] {
            launch.key(k, &Mods::NONE, false, &st);
        }
        let key = launch.wanted_catalog().unwrap();
        let v = view(&launch, &st);
        assert_eq!(
            v["footer"],
            "/ search · ↑↓ choose · Enter launch · Esc close · Tab more"
        );
        // Enter before the catalog lands queues whatever the search will match.
        launch.key("/", &Mods::NONE, false, &st);
        launch.key("s", &Mods::NONE, false, &st);
        launch.key("o", &Mods::NONE, false, &st);
        launch.key("Enter", &Mods::NONE, false, &st);
        let v = view(&launch, &st);
        assert_eq!(v["query"], "Search queued");
        assert_eq!(v["hint"], "/so▏");
        assert_eq!(
            v["status"],
            "Starting the matching model when models are ready…"
        );
        // Backspace cancels the queued launch before it edits the search.
        launch.key("Backspace", &Mods::NONE, false, &st);
        let v = view(&launch, &st);
        assert_eq!(v["query"], "↑↓ choose · Enter launch");
        assert_eq!(v["footer"], "Backspace edit · Esc close · Tab more");
        let models = options(&[
            ("opus", "Opus 5.6"),
            ("sonnet", "Sonnet 5.6"),
            ("sonnet-old", "Sonnet 4.5"),
        ]);
        assert_eq!(launch.set_models(&key, Ok(models), &st), Outcome::None);
        let v = view(&launch, &st);
        assert_eq!(v["rows"].as_array().unwrap().len(), 2);
        assert_eq!(v["rows"][0]["label"], "Sonnet 5.6");
        assert_eq!(v["rows"][0]["selected"], true);
        launch.key("ArrowDown", &Mods::NONE, false, &st);
        assert_eq!(view(&launch, &st)["rows"][1]["selected"], true);
        assert_eq!(
            launch.key("Enter", &Mods::NONE, false, &st),
            Outcome::Submit
        );
        assert_eq!(launch.model, "sonnet-old");
        // A search that matches nothing says how to fix it.
        launch.key("x", &Mods::NONE, false, &st);
        assert_eq!(
            view(&launch, &st)["status"],
            "No matching models. Backspace to edit your search."
        );
    }

    #[test]
    fn r_retries_only_a_failed_catalog_outside_a_search() {
        let st = state();
        let mut shortcuts = Shortcuts::default();
        enroll_state(&mut shortcuts, &st);
        let mut launch = open(
            &LaunchMemory::default(),
            &st,
            None,
            &shortcuts,
            String::new(),
        )
        .unwrap();
        for k in ["0", "1", "0", "1"] {
            launch.key(k, &Mods::NONE, false, &st);
        }
        let key = launch.wanted_catalog().unwrap();
        launch.catalog_requested(&key);
        // A queued slot is dropped when the catalog fails.
        launch.key("0", &Mods::NONE, false, &st);
        launch.key("1", &Mods::NONE, false, &st);
        launch.set_models(&key, Err("offline".into()), &st);
        assert_eq!(launch.quick.as_ref().unwrap().pending_model, None);
        assert_eq!(
            view(&launch, &st)["status"],
            "offline · Clear search and press R to retry"
        );
        launch.key("r", &Mods::NONE, false, &st);
        assert!(launch.catalogs["claude"].loading);
        assert_eq!(launch.wanted_catalog(), Some(key));
    }
}
