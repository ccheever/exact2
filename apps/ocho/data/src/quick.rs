//! Quick launch (⌘⇧N): persistent, append-only account and machine
//! assignments (quick_launch.rs), with the numbered-slot keys and card from
//! workspace.rs and ui.rs. Missing identities keep their slots; models
//! follow the selected CLI's current catalog instead.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

use crate::launch::{
    account, account_handle, account_label, effort_key, machine, overlay, provider_label,
};
use crate::launch::{Focus, Launch, LaunchMemory, ModelOption, Outcome};
use crate::picker::{self, canon, typed, Mods};
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
    /// A model number typed before the current CLI catalog has loaded.
    pub pending_model: Option<usize>,
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
            shortcuts,
        }
    }

    /// Backspace: cancel a queued model, erase a digit, or go back a step.
    pub fn back(&mut self) {
        if self.pending_model.take().is_some() {
            return;
        }
        if self.digits.pop().is_none() {
            self.step = match self.step {
                Step::Account | Step::Machine => Step::Account,
                Step::Model => Step::Machine,
            };
        }
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
    launch.insert = false;
    launch.draft = draft;
    launch.model.clear();
    launch.effort.clear();
    launch.permissions.clear();
    launch.cwd = "~".into();
    launch.quick = Some(QuickLaunch::new(shortcuts.clone()));
    Ok(launch)
}

/// Accounts and machines keep stable slots. Models use the selected CLI's
/// current catalog verbatim, including its order and display names.
pub fn rows(launch: &Launch, state: &State) -> Vec<(usize, String, bool)> {
    let Some(q) = &launch.quick else {
        return Vec::new();
    };
    if q.step == Step::Model {
        return launch
            .catalogs
            .get(&launch.provider)
            .filter(|catalog| !catalog.loading && catalog.error.is_empty())
            .map(|catalog| model_rows(&catalog.options))
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
    let Some(q) = launch.quick.as_mut() else {
        return Outcome::None;
    };
    if key == "tab" {
        launch.auto_machine = false;
        launch.quick = None;
        launch.prompt = std::mem::take(&mut launch.draft);
        launch.focus = Focus::Project;
        launch.error.clear();
    } else if key == "backspace" {
        q.back();
        launch.error.clear();
    } else if key == "r" && !mods.ctrl && !mods.meta {
        launch.sync_catalogs(state, true);
    } else if !mods.ctrl && !mods.meta && !mods.alt {
        let digit = typed(&key, mods)
            .and_then(|s| s.chars().next())
            .filter(|c| c.is_ascii_digit());
        let Some(digit) = digit else {
            return Outcome::None;
        };
        if q.pending_model.is_some() {
            return Outcome::None;
        }
        if let Some(slot) = q.digit(digit) {
            let loading = q.step == Step::Model
                && launch
                    .catalogs
                    .get(&launch.provider)
                    .is_none_or(|c| c.loading);
            if loading {
                q.pending_model = Some(slot);
            } else {
                return pick(launch, slot, state);
            }
        }
    }
    Outcome::None
}

/// Choose the slot: the account, then the machine (0 for auto), then the
/// model, which submits.
pub fn pick(launch: &mut Launch, slot: usize, state: &State) -> Outcome {
    let available = rows(launch, state).iter().any(|r| r.0 == slot && r.2);
    let Some(q) = launch.quick.as_mut() else {
        return Outcome::None;
    };
    q.digits.clear();
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

/// Complete a model choice entered before the current CLI catalog arrived.
pub fn finish_pending(launch: &mut Launch, state: &State) -> Outcome {
    let ready = launch
        .catalogs
        .get(&launch.provider)
        .is_some_and(|c| !c.loading && c.error.is_empty());
    let slot = match launch.quick.as_mut() {
        Some(q) if q.step == Step::Model && ready => q.pending_model.take(),
        _ => None,
    };
    match slot {
        Some(slot) => pick(launch, slot, state),
        None => Outcome::None,
    }
}

/// The `OVERLAY` (kind "quick"): ui.rs `render_quick_launch`.
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
    let (status, error) = if !launch.error.is_empty() {
        (launch.error.clone(), true)
    } else if loading {
        let message = match q.pending_model {
            Some(slot) => format!("Starting {slot:02} when models are ready…"),
            None if catalog.is_some_and(|c| !c.options.is_empty()) => "Refreshing models…".into(),
            None => "Loading models…".into(),
        };
        (message, false)
    } else if failed {
        (
            format!(
                "{} · Press R to retry",
                catalog.map(|c| c.error.as_str()).unwrap_or("")
            ),
            true,
        )
    } else if rows.is_empty() {
        (
            "No options available. Tab opens the full launcher.".to_string(),
            false,
        )
    } else {
        (String::new(), false)
    };
    let rows: Vec<Json> = rows
        .iter()
        .map(|(slot, label, available)| {
            let prefix = format!("{slot:02}");
            let highlighted = !q.digits.is_empty() && prefix.starts_with(&q.digits);
            let mut r = picker::row(
                &format!("quick-slot:{slot}"),
                &prefix,
                label,
                "",
                "",
                highlighted,
            );
            picker::set(&mut r, "accent", json!(available));
            r
        })
        .collect();
    let mut body = Vec::new();
    if q.step != Step::Account {
        let a = account(state, &launch.account)
            .map(account_label)
            .unwrap_or_else(|| launch.account.clone());
        body.push(format!("{a} · {}", provider_label(&launch.provider)));
    }
    if q.step == Step::Model {
        let m = if launch.auto_machine {
            "Auto · chosen after you submit your task"
        } else {
            machine(state, &launch.machine)
                .map(|m| m.name.as_str())
                .unwrap_or("Removed machine")
        };
        body.push(format!("{m} · {}", launch.cwd));
    }
    let input = match q.pending_model {
        Some(slot) => format!("{slot:02} · queued"),
        None => format!("{}{}", q.digits, "_".repeat(2 - q.digits.len().min(2))),
    };
    let hints = if q.pending_model.is_some() {
        "Backspace cancel · Esc close"
    } else {
        "Esc close · Tab more"
    };
    let mut v = overlay("quick", title, "CTRL N", "accent");
    picker::set(&mut v, "subtitle", json!("Account → Machine → Model"));
    picker::set(&mut v, "body", json!(body.join("\n")));
    picker::set(&mut v, "rows", json!(rows));
    picker::set(&mut v, "status", json!(status));
    picker::set(&mut v, "statusError", json!(error));
    picker::set(&mut v, "query", json!(input));
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
        q.pending_model = Some(4);
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
        assert_eq!((launch.insert, launch.cwd.as_str()), (false, "~"));
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
}
