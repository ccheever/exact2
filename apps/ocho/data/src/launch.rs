//! The launch dialog: a composer with remembered defaults, a project picker,
//! and strictly modal keys (launch.rs, with the workspace.rs handlers and
//! the ui.rs views). Opening it and pressing Enter launches the last
//! configuration again; nothing is typed unless `i` (or a click on the
//! prompt) enters insert mode, so vim keys work everywhere else.
//!
//! This file is the model; `launch/catalog.rs` is the I/O the caller runs
//! for it (`fleet models`, `fleet directories`), `launch/keys.rs` its keys
//! and presses, `launch/view.rs` its `OVERLAY` and `POPUP` JSON.

pub mod catalog;
pub mod keys;
pub mod memory;
pub mod view;

use std::collections::HashMap;

use crate::picker::{self, PickerState, Popup};
use crate::quick::QuickLaunch;
use crate::types::{Account, Machine, State};

pub use catalog::{CatalogKey, DirectoryMatches, DirectoryState, ModelOption, ModelState};
pub use memory::{effort_key, LaunchDefaults, LaunchMemory, RecentModel, RecentProject};
pub use view::overlay;

/// The harnesses, in the order every list shows them.
pub const PROVIDERS: [&str; 3] = ["codex", "claude", "opencode"];

/// The toast when Esc keeps a prompt.
pub const DRAFT_KEPT: &str = "Prompt kept as a draft; ⌘N brings it back";

/// How a provider reads (backend.rs `provider_label`).
pub fn provider_label(p: &str) -> &str {
    match p {
        "claude" => "Claude",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        other => other,
    }
}

/// The handle `fleet run --account` takes: `provider:email`, or the name.
pub fn account_handle(a: &Account) -> String {
    if a.email.is_empty() {
        a.name.clone()
    } else {
        format!("{}:{}", a.provider, a.email)
    }
}

/// The email, or what stands in for one.
pub fn account_email(a: &Account) -> String {
    if !a.email.is_empty() {
        a.email.clone()
    } else if a.name.starts_with(&format!("{}-api-", a.provider)) {
        format!("API key · {}", a.name)
    } else {
        "Email unavailable".to_string()
    }
}

/// "Claude · me@example.com".
pub fn account_label(a: &Account) -> String {
    format!("{} · {}", provider_label(&a.provider), account_email(a))
}

/// The machine with this id.
pub fn machine<'a>(state: &'a State, id: &str) -> Option<&'a Machine> {
    state.machines.iter().find(|m| m.id == id)
}

/// The account by name or handle (backend.rs `State::account`).
pub fn account<'a>(state: &'a State, key: &str) -> Option<&'a Account> {
    if key.is_empty() {
        return None;
    }
    state
        .accounts
        .iter()
        .find(|a| a.name == key || account_handle(a) == key)
}

/// Prefer a connected cloud account; the machine login stays an explicit choice.
pub fn default_account(state: &State, provider: &str) -> String {
    let mut first = String::new();
    for a in &state.accounts {
        if !a.shared || a.provider != provider {
            continue;
        }
        if a.status == "connected" {
            return account_handle(a);
        }
        if first.is_empty() {
            first = account_handle(a);
        }
    }
    first
}

/// The machine's default mode for a provider, as `fleet run` will apply it.
pub fn machine_permission_default(state: &State, machine_id: &str, provider: &str) -> String {
    machine(state, machine_id)
        .and_then(|m| m.permissions.get(provider).cloned())
        .unwrap_or_default()
}

/// Native default first, discovered models next, then whatever custom ID is
/// typed (the profile form's model list).
pub fn model_options(loaded: &[ModelOption], value: &str) -> Vec<ModelOption> {
    let mut options = vec![ModelOption {
        id: String::new(),
        name: "Native default".into(),
        description: "Use the model configured by the provider on this machine".into(),
        default: false,
    }];
    options.extend(loaded.iter().cloned());
    if !options.iter().any(|o| o.id == value) {
        options.push(ModelOption {
            id: value.to_string(),
            name: format!("{value} (custom)"),
            description: "Use this exact model identifier".into(),
            default: false,
        });
    }
    options
}

/// Reasoning effort levels a provider accepts; empty keeps the native default.
/// OpenCode picks variants inside its own interface, so only the default applies.
pub fn effort_choices(provider: &str) -> &'static [&'static str] {
    match provider {
        "codex" => &["", "minimal", "low", "medium", "high", "xhigh"],
        "claude" => &["", "low", "medium", "high", "max"],
        _ => &[""],
    }
}

/// How an effort level reads in the chip and slider.
pub fn effort_label(effort: &str) -> &'static str {
    match effort {
        "" => "Auto",
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra High",
        "max" => "Max",
        _ => "Custom",
    }
}

/// Show a path under the machine's home directory as `~/…`.
pub fn tilde(path: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    if home.is_empty() {
        return path.to_string();
    }
    if path == home {
        return "~".into();
    }
    match path.strip_prefix(home) {
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// The dialog's controls. Model and Effort are the two halves of one pill,
/// like Codex's "GPT-5.6 Sol Extra High" chip; the account follows the harness.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Focus {
    /// The project chip.
    #[default]
    Project,
    /// The machine chip.
    Machine,
    /// The prompt box.
    Prompt,
    /// The model half of the pill.
    Model,
    /// The effort half of the pill.
    Effort,
    /// The account chip.
    Account,
    /// The permissions chip.
    Permissions,
}

impl Focus {
    /// Tab order.
    pub const ORDER: [Focus; 7] = [
        Focus::Project,
        Focus::Machine,
        Focus::Prompt,
        Focus::Model,
        Focus::Effort,
        Focus::Account,
        Focus::Permissions,
    ];

    /// `step` controls along, clamped at the ends.
    pub fn moved(self, step: i32) -> Focus {
        let at = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0) as i32;
        let last = Self::ORDER.len() as i32 - 1;
        Self::ORDER[(at + step).clamp(0, last) as usize]
    }

    /// Controls that accept free text in insert mode.
    pub fn editable(self) -> bool {
        matches!(self, Focus::Prompt | Focus::Model)
    }
}

/// Where a project row came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectSource {
    /// A remembered launch.
    Recent,
    /// A launch profile's folder.
    Profile(String),
    /// An observed session's folder.
    Session,
    /// The machine's home, when it has nothing else.
    Home,
}

/// A folder on a machine that has been worked in before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// The machine id.
    pub machine: String,
    /// The machine's name.
    pub machine_name: String,
    /// The folder.
    pub cwd: String,
    /// Where it came from.
    pub source: ProjectSource,
}

impl Project {
    /// "recent", "profile X", "session" or "home".
    pub fn detail(&self) -> String {
        match &self.source {
            ProjectSource::Recent => "recent".into(),
            ProjectSource::Profile(name) => format!("profile {name}"),
            ProjectSource::Session => "session".into(),
            ProjectSource::Home => "home".into(),
        }
    }
}

const SESSION_PROJECTS_PER_MACHINE: usize = 10;

/// Folders worth offering, current machine first: recent launches, profile
/// folders, then folders of observed sessions (newest first). Each machine
/// without any known folder offers its home directory.
pub fn collect_projects(state: &State, recent: &[RecentProject], current: &str) -> Vec<Project> {
    let mut machines: Vec<_> = state
        .machines
        .iter()
        .filter(|m| m.sprite.is_none())
        .collect();
    machines.sort_by_key(|m| m.id != current);
    let mut out: Vec<Project> = Vec::new();
    for m in machines {
        let start = out.len();
        let push = |cwd: &str, source: ProjectSource, out: &mut Vec<Project>| {
            let cwd = cwd.trim_end_matches('/');
            let cwd = if cwd.is_empty() { "/" } else { cwd };
            if out[start..].iter().any(|p| p.cwd == cwd) {
                return;
            }
            out.push(Project {
                machine: m.id.clone(),
                machine_name: m.name.clone(),
                cwd: cwd.to_string(),
                source,
            });
        };
        for r in recent
            .iter()
            .filter(|r| r.machine == m.id && !r.cwd.is_empty())
        {
            push(&r.cwd, ProjectSource::Recent, &mut out);
        }
        let mut profiles: Vec<_> = state
            .presets
            .iter()
            .filter(|(_, p)| p.machine_id == m.id && !p.cwd.is_empty())
            .collect();
        profiles.sort_by(|a, b| a.0.cmp(b.0));
        for (name, p) in profiles {
            push(&p.cwd, ProjectSource::Profile(name.clone()), &mut out);
        }
        if let Some(last) = &m.last {
            let mut sessions: Vec<_> = last.sessions.iter().filter(|s| !s.cwd.is_empty()).collect();
            sessions.sort_by(|a, b| b.started.cmp(&a.started));
            let before = out.len();
            for s in sessions {
                if out.len() - before >= SESSION_PROJECTS_PER_MACHINE {
                    break;
                }
                push(&tilde(&s.cwd, &last.home), ProjectSource::Session, &mut out);
            }
        }
        if out.len() == start {
            push("~", ProjectSource::Home, &mut out);
        }
    }
    out
}

/// One line of the project picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerRow {
    /// A known folder, by its index in `projects`.
    Project(usize),
    /// "+ Add folder on {machine}…".
    AddFolder {
        /// The machine id.
        machine: String,
        /// The machine's name.
        machine_name: String,
    },
}

/// Typing a path that is not known yet, with folder completion on that machine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AddFolder {
    /// The machine id.
    pub machine: String,
    /// The machine's name.
    pub machine_name: String,
    /// The typed path.
    pub path: String,
    /// Its completions.
    pub directory: DirectoryState,
}

/// The project picker: a filter, a highlight, and maybe a path being typed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectPicker {
    /// The filter and highlight; `typing` is the "/" filter mode.
    pub list: PickerState,
    /// The "add folder" input, when open.
    pub adding: Option<AddFolder>,
}

/// One selectable line of the model picker: a harness with a model ID
/// (empty for the native default) or the typed custom ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRow {
    /// The harness.
    pub provider: String,
    /// The model id, empty for the harness default.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The line under it.
    pub description: String,
    /// Listed under "Recent" rather than its harness.
    pub recent: bool,
}

/// What the caller does after a key or press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing beyond repainting.
    None,
    /// Close the dialog, keeping `close()`'s draft.
    Close,
    /// Call `submit`.
    Submit,
}

/// A launch to run: the tab and its `fleet` argv.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submission {
    /// The machine id (`auto:{request}` for an auto machine).
    pub machine: String,
    /// The tab's key: `{machine}:{request}:false`, or `auto:{request}`.
    pub tab_key: String,
    /// The tab's title: "{Provider} · {machine}".
    pub title: String,
    /// The arguments after `fleet`.
    pub argv: Vec<String>,
    /// The request id.
    pub request: String,
}

/// `fleet run` takes the machine default when no mode is named, so the flag
/// only travels with a choice; the frozen reconnect argv stays identical.
pub fn with_permissions(mut args: Vec<String>, permissions: &str) -> Vec<String> {
    if !permissions.is_empty() {
        let at = args.len() - 1;
        args.insert(at, "--permissions".into());
        args.insert(at + 1, permissions.to_string());
    }
    args
}

/// The dialog.
#[derive(Clone, Debug, Default)]
pub struct Launch {
    /// Quick launch's "Auto" machine: `fleet auto-machine` picks one.
    pub auto_machine: bool,
    /// The numbered quick launch, when that is what is open.
    pub quick: Option<QuickLaunch>,
    /// The earlier prompt draft, held while quick launch is open.
    pub draft: String,
    /// The machine id.
    pub machine: String,
    /// The harness.
    pub provider: String,
    /// The account handle, empty for the machine login.
    pub account: String,
    /// The folder.
    pub cwd: String,
    /// The model id, empty for the harness default.
    pub model: String,
    /// The effort level, empty for auto.
    pub effort: String,
    /// A permission mode for this launch; empty takes the machine default.
    pub permissions: String,
    /// The prompt.
    pub prompt: String,
    /// The focused control.
    pub focus: Focus,
    /// Typing edits the focused control instead of navigating.
    pub insert: bool,
    /// The project picker, when open.
    pub picker: Option<ProjectPicker>,
    /// The machine picker's highlight, when open.
    pub machine_picker: Option<PickerState>,
    /// The model popup's highlight, when open.
    pub model_picker: Option<PickerState>,
    /// The effort list under the effort half of the chip, with its highlight.
    pub effort_picker: Option<usize>,
    /// The account or permissions popup, when open.
    pub popup: Option<Popup>,
    /// Model catalogs by harness, loaded lazily while the model picker is open.
    pub catalogs: HashMap<String, ModelState>,
    /// The machine, account and folder the catalogs were loaded for.
    pub catalog_key: String,
    /// Known folders.
    pub projects: Vec<Project>,
    /// Models launched with before.
    pub recent_models: Vec<RecentModel>,
    /// Last effort per harness/model from earlier launches.
    pub efforts: HashMap<String, String>,
    /// The submit error, shown until the next key.
    pub error: String,
}

impl Launch {
    /// Start from the last launch, keeping only the parts that still exist:
    /// a removed machine or account falls back, and model and effort only
    /// carry over with the same machine and provider.
    pub fn new(
        saved: &LaunchMemory,
        state: &State,
        fallback_machine: &str,
        default_account: impl Fn(&str) -> String,
    ) -> Launch {
        let d = saved.launch.clone().unwrap_or_default();
        let machine = if state
            .machines
            .iter()
            .any(|m| m.id == d.machine && m.sprite.is_none())
        {
            d.machine.clone()
        } else {
            fallback_machine.to_string()
        };
        let provider = if PROVIDERS.contains(&d.provider.as_str()) {
            d.provider.clone()
        } else {
            "codex".to_string()
        };
        let account_ok = d.account.is_empty()
            || state
                .accounts
                .iter()
                .any(|a| a.shared && a.provider == provider && account_handle(a) == d.account);
        let account = if provider == d.provider && account_ok {
            d.account.clone()
        } else {
            default_account(&provider)
        };
        let projects = collect_projects(state, &saved.recent_projects, &machine);
        let cwd = if machine == d.machine && !d.cwd.is_empty() {
            d.cwd.clone()
        } else {
            projects
                .iter()
                .find(|p| p.machine == machine)
                .map(|p| p.cwd.clone())
                .unwrap_or_else(|| "~".into())
        };
        let same = machine == d.machine && provider == d.provider;
        let keep = |s: &String| if same { s.clone() } else { String::new() };
        Launch {
            machine,
            provider,
            account,
            cwd,
            model: keep(&d.model),
            effort: keep(&d.effort),
            permissions: keep(&d.permissions),
            projects,
            recent_models: saved.recent_models.clone(),
            efforts: saved.model_efforts.clone(),
            ..Default::default()
        }
    }

    /// ⌘N (workspace.rs `open_launch`): the selected machine or the first
    /// enrolled one, focus on the prompt in INSERT mode with the draft back.
    pub fn open(
        saved: &LaunchMemory,
        state: &State,
        selected: Option<&str>,
        draft: String,
    ) -> Result<Launch, &'static str> {
        let preselected = selected
            .and_then(|id| machine(state, id))
            .filter(|m| m.sprite.is_none());
        let Some(m) = preselected.or_else(|| state.machines.iter().find(|m| m.sprite.is_none()))
        else {
            return Err("Add a machine before launching a session");
        };
        let mut launch = Launch::new(saved, state, &m.id, |p| default_account(state, p));
        launch.prompt = draft;
        launch.focus = Focus::Prompt;
        launch.insert = true;
        Ok(launch)
    }

    /// Esc: the prompt to keep as a draft (the earlier draft, under quick
    /// launch). The caller toasts `DRAFT_KEPT` when it is not blank.
    pub fn close(&self) -> String {
        if self.quick.is_none() {
            self.prompt.clone()
        } else {
            self.draft.clone()
        }
    }

    /// Switching hosts chooses a folder known on that host, never a local
    /// absolute path carried over from the previous machine.
    pub fn select_machine(&mut self, machine: &str) {
        if self.machine == machine {
            return;
        }
        self.machine = machine.to_string();
        self.cwd = self
            .projects
            .iter()
            .find(|p| p.machine == machine)
            .map(|p| p.cwd.clone())
            .unwrap_or_else(|| "~".into());
        self.catalogs.clear();
        self.catalog_key.clear();
        self.error.clear();
    }

    /// Numbered quick launch always starts in the selected machine's home.
    pub fn select_quick_machine(&mut self, machine: &str) {
        self.select_machine(machine);
        self.cwd = "~".into();
    }

    /// What the next dialog opens with.
    pub fn defaults(&self) -> LaunchDefaults {
        LaunchDefaults {
            machine: self.machine.clone(),
            provider: self.provider.clone(),
            account: self.account.clone(),
            cwd: self.cwd.clone(),
            model: self.model.clone(),
            effort: self.effort.clone(),
            permissions: self.permissions.clone(),
        }
    }

    /// The permission choices for this launch: the machine default first,
    /// the provider's native behavior when the machine sets one, then each mode.
    pub fn permission_choices(&self, machine_default: &str) -> Vec<String> {
        let mut out = vec![String::new()];
        if !machine_default.is_empty() {
            out.push(crate::permissions::NATIVE.to_string());
        }
        out.extend(
            crate::permissions::choices(&self.provider)
                .iter()
                .map(|s| s.to_string()),
        );
        if !out.contains(&self.permissions) {
            out.push(self.permissions.clone());
        }
        out
    }

    /// The account list a launch offers: the machine's own login, then the
    /// shared accounts for the provider.
    pub fn account_choices(&self, state: &State) -> Vec<String> {
        let mut out = vec![String::new()];
        let shared = state
            .accounts
            .iter()
            .filter(|a| a.shared && a.provider == self.provider);
        out.extend(shared.map(account_handle));
        out
    }

    /// The control whose text insert mode edits.
    pub fn insert_target(&self) -> Focus {
        if self.focus.editable() {
            self.focus
        } else {
            Focus::Prompt
        }
    }

    /// Rows of the model popup: every harness in order, its default first,
    /// then the loaded catalog. A custom ID that was typed stays listed under
    /// its harness so it can be picked again.
    pub fn model_rows(&self) -> Vec<ModelRow> {
        let mut rows: Vec<ModelRow> = self
            .recent_models
            .iter()
            .filter(|m| PROVIDERS.contains(&m.provider.as_str()))
            .map(|m| ModelRow {
                provider: m.provider.clone(),
                id: m.id.clone(),
                name: if m.name.is_empty() {
                    m.id.clone()
                } else {
                    m.name.clone()
                },
                description: provider_label(&m.provider).to_string(),
                recent: true,
            })
            .collect();
        for provider in PROVIDERS {
            let mut options = vec![ModelOption {
                id: String::new(),
                name: "Default".into(),
                description: format!("Whatever {} is set up with", provider_label(provider)),
                default: false,
            }];
            if let Some(catalog) = self.catalogs.get(provider) {
                // A harness that lists its own default (Claude's "Default
                // (recommended)") replaces the synthetic one.
                if catalog.options.iter().any(|o| o.id == "default") {
                    options.clear();
                }
                options.extend(catalog.options.iter().cloned());
            }
            let custom = provider == self.provider
                && !self.model.is_empty()
                && !options.iter().any(|o| o.id == self.model);
            if custom {
                options.push(ModelOption {
                    id: self.model.clone(),
                    name: self.model.clone(),
                    description: "Custom model identifier".into(),
                    default: false,
                });
            }
            rows.extend(options.into_iter().map(|o| ModelRow {
                provider: provider.to_string(),
                id: o.id,
                name: o.name,
                description: o.description,
                recent: false,
            }));
        }
        rows
    }

    /// The row the picker should highlight first: the current harness and model.
    pub fn current_model_row(&self) -> Option<usize> {
        let rows = self.model_rows();
        rows.iter()
            .position(|r| r.provider == self.provider && r.id == self.model)
            .or_else(|| rows.iter().position(|r| r.provider == self.provider))
    }

    /// Pick a harness and model; the account follows a harness change, the
    /// model's last effort comes back, and an effort the harness does not
    /// offer is dropped.
    pub fn choose_model(&mut self, row: &ModelRow, default_account: impl Fn(&str) -> String) {
        if row.provider != self.provider {
            self.provider = row.provider.clone();
            self.account = default_account(&self.provider);
            // Modes are named per provider; the machine default takes over.
            self.permissions.clear();
        }
        self.model = row.id.clone();
        if let Some(effort) = self.efforts.get(&effort_key(&self.provider, &self.model)) {
            self.effort = effort.clone();
        }
        if !effort_choices(&self.provider).contains(&self.effort.as_str()) {
            self.effort.clear();
        }
    }

    /// The effort half of the pill only exists when the harness has levels.
    pub fn has_effort(&self) -> bool {
        effort_choices(&self.provider).len() > 1
    }

    /// The model half of the chip: the catalog name when known, else the ID,
    /// else "Default".
    pub fn model_name(&self) -> String {
        if self.model.is_empty() {
            return "Default".into();
        }
        self.catalogs
            .get(&self.provider)
            .and_then(|c| c.options.iter().find(|o| o.id == self.model))
            .map(|o| o.name.clone())
            .unwrap_or_else(|| self.model.clone())
    }

    /// Slider positions for the current harness: default first, then each
    /// level it accepts, plus a custom value if one was set.
    pub fn effort_levels(&self) -> Vec<String> {
        let mut out: Vec<String> = effort_choices(&self.provider)
            .iter()
            .map(|s| s.to_string())
            .collect();
        if !out.contains(&self.effort) {
            out.push(self.effort.clone());
        }
        out
    }

    /// Index of the current effort among the levels.
    pub fn effort_index(&self) -> usize {
        self.effort_levels()
            .iter()
            .position(|l| *l == self.effort)
            .unwrap_or(0)
    }

    /// Picker rows for the current filter: matching projects, then one
    /// "add folder" row per machine (current machine first).
    pub fn picker_rows(&self) -> Vec<PickerRow> {
        let query = self
            .picker
            .as_ref()
            .map(|p| p.list.query.trim().to_string())
            .unwrap_or_default();
        let mut scored: Vec<(usize, i32)> = self
            .projects
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                if query.is_empty() {
                    return Some((i, 0));
                }
                let text = format!("{} {}", p.cwd, p.machine_name);
                picker::fuzzy_score(&query, &text).map(|s| (i, s))
            })
            .collect();
        if !query.is_empty() {
            scored.sort_by_key(|(_, s)| *s);
        }
        let mut rows: Vec<PickerRow> = scored
            .into_iter()
            .map(|(i, _)| PickerRow::Project(i))
            .collect();
        let mut seen: Vec<&str> = Vec::new();
        for p in &self.projects {
            if seen.contains(&p.machine.as_str()) {
                continue;
            }
            seen.push(&p.machine);
            rows.push(PickerRow::AddFolder {
                machine: p.machine.clone(),
                machine_name: p.machine_name.clone(),
            });
        }
        rows
    }

    /// Index of the current project in the project list, if it is a known one.
    pub fn project_index(&self) -> Option<usize> {
        let cwd = self.cwd.trim_end_matches('/');
        self.projects
            .iter()
            .position(|p| p.machine == self.machine && p.cwd == cwd)
    }

    /// Enter (workspace.rs `submit_launch`): the `fleet run` to open in a
    /// new tab, remembering the choices. An error stays on the dialog.
    pub fn submit(
        &mut self,
        state: &State,
        memory: &mut LaunchMemory,
        request: &str,
    ) -> Result<Submission, String> {
        if self.auto_machine {
            let argv = [
                "auto-machine",
                "--request-id",
                request,
                "--provider",
                &self.provider,
                "--account",
                &self.account,
                "--model",
                &self.model,
                "--effort",
                &self.effort,
                "--permissions",
                &self.permissions,
            ];
            return Ok(Submission {
                machine: format!("auto:{request}"),
                tab_key: format!("auto:{request}"),
                title: "Auto Machine".into(),
                argv: argv.iter().map(|s| s.to_string()).collect(),
                request: request.into(),
            });
        }
        let Some(m) = machine(state, &self.machine) else {
            self.error = "Choose a project on an enrolled machine".into();
            return Err(self.error.clone());
        };
        let (machine_id, machine_name) = (m.id.clone(), m.name.clone());
        self.cwd = self.cwd.trim().to_string();
        if self.cwd.is_empty() {
            self.error = "Choose a project folder".into();
            return Err(self.error.clone());
        }
        // OpenCode picks reasoning variants inside its own interface.
        if self.provider == "opencode" {
            self.effort.clear();
        }
        let d = self.defaults();
        memory.remember_launch(d.clone(), &self.model_name());
        let argv = [
            "run",
            &machine_id,
            "--request-id",
            request,
            "--provider",
            &d.provider,
            "--account",
            &d.account,
            "--cwd",
            &d.cwd,
            "--model",
            &d.model,
            "--effort",
            &d.effort,
            "--prompt",
            &self.prompt,
            "--attach",
        ];
        let argv = argv.iter().map(|s| s.to_string()).collect();
        Ok(Submission {
            tab_key: format!("{machine_id}:{request}:false"),
            title: format!("{} · {machine_name}", provider_label(&d.provider)),
            argv: with_permissions(argv, &d.permissions),
            machine: machine_id,
            request: request.into(),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::types::{LaunchProfile, Session, Snapshot, SpriteBinding};

    fn machine(id: &str, home: &str, sessions: Vec<(&str, &str)>) -> Machine {
        let sessions = sessions
            .into_iter()
            .map(|(cwd, started)| Session {
                cwd: cwd.into(),
                started: started.into(),
                ..Default::default()
            })
            .collect();
        Machine {
            id: id.into(),
            name: id.to_uppercase(),
            last: Some(Snapshot {
                home: home.into(),
                sessions,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Three machines (laptop with sessions, studio with a profile, blank)
    /// and one connected Claude account.
    pub(crate) fn state() -> State {
        let mut presets = HashMap::new();
        presets.insert(
            "Research".to_string(),
            LaunchProfile {
                machine_id: "studio".into(),
                cwd: "~/research".into(),
                ..Default::default()
            },
        );
        let laptop = vec![
            ("/Users/me/old", "2026-09-01T10:00:00Z"),
            ("/Users/me/Developer/fleet", "2026-09-09T10:00:00Z"),
            ("/Users/me", "2026-09-05T10:00:00Z"),
        ];
        State {
            machines: vec![
                machine("laptop", "/Users/me", laptop),
                machine(
                    "studio",
                    "/home/me",
                    vec![("/srv/app", "2026-09-02T10:00:00Z")],
                ),
                machine("blank", "", vec![]),
            ],
            accounts: vec![Account {
                name: "me".into(),
                email: "me@example.com".into(),
                provider: "claude".into(),
                shared: true,
                status: "connected".into(),
                ..Default::default()
            }],
            presets,
        }
    }

    /// A dialog on the laptop with nothing remembered.
    pub(crate) fn fresh(st: &State) -> Launch {
        Launch::new(&LaunchMemory::default(), st, "laptop", |_| String::new())
    }

    fn triple(launch: &Launch) -> (String, String, String) {
        (
            launch.model.clone(),
            launch.effort.clone(),
            launch.permissions.clone(),
        )
    }

    #[test]
    fn home_folders_display_with_a_tilde() {
        assert_eq!(tilde("/Users/me/Developer", "/Users/me"), "~/Developer");
        assert_eq!(tilde("/Users/me", "/Users/me/"), "~");
        assert_eq!(tilde("/Users/meow/x", "/Users/me"), "/Users/meow/x");
        assert_eq!(tilde("/srv/app", ""), "/srv/app");
    }

    #[test]
    fn projects_put_the_current_machine_first_and_dedupe_folders() {
        let recent = vec![
            RecentProject {
                machine: "laptop".into(),
                cwd: "~/Developer/fleet/".into(),
            },
            RecentProject {
                machine: "studio".into(),
                cwd: "/srv/app".into(),
            },
            RecentProject {
                machine: "gone".into(),
                cwd: "/nowhere".into(),
            },
        ];
        let projects = collect_projects(&state(), &recent, "studio");
        let rows: Vec<String> = projects
            .iter()
            .map(|p| format!("{}:{}:{}", p.machine, p.cwd, p.detail()))
            .collect();
        assert_eq!(
            rows,
            vec![
                "studio:/srv/app:recent",
                "studio:~/research:profile Research",
                "laptop:~/Developer/fleet:recent",
                "laptop:~:session",
                "laptop:~/old:session",
                "blank:~:home",
            ]
        );
    }

    #[test]
    fn defaults_survive_only_when_their_parts_still_exist() {
        let st = state();
        let recent = vec![RecentProject {
            machine: "laptop".into(),
            cwd: "~/Developer/fleet".into(),
        }];
        let account = |provider: &str| format!("default-{provider}");
        let saved = |launch: Option<LaunchDefaults>| LaunchMemory {
            launch,
            recent_projects: recent.clone(),
            ..Default::default()
        };
        // Nothing remembered yet: a fresh dialog, first known folder on the machine.
        let launch = Launch::new(&saved(None), &st, "laptop", account);
        assert_eq!(launch.machine, "laptop");
        assert_eq!(launch.provider, "codex");
        assert_eq!(launch.account, "default-codex");
        assert_eq!(launch.cwd, "~/Developer/fleet");
        assert_eq!(launch.focus, Focus::Project);
        assert!(!launch.insert);

        let remembered = LaunchDefaults {
            machine: "studio".into(),
            provider: "claude".into(),
            account: "claude:me@example.com".into(),
            cwd: "/srv/app".into(),
            model: "opus".into(),
            effort: "high".into(),
            permissions: "plan".into(),
        };
        let launch = Launch::new(&saved(Some(remembered.clone())), &st, "laptop", account);
        assert_eq!(launch.machine, "studio");
        assert_eq!(launch.provider, "claude");
        assert_eq!(launch.account, "claude:me@example.com");
        assert_eq!(launch.cwd, "/srv/app");
        assert_eq!(
            triple(&launch),
            ("opus".into(), "high".into(), "plan".into())
        );
        assert_eq!(launch.defaults(), remembered);

        // The machine and account disappeared: fall back without the model.
        let stale = LaunchDefaults {
            machine: "retired".into(),
            account: "claude:someone@else".into(),
            ..remembered.clone()
        };
        let launch = Launch::new(&saved(Some(stale)), &st, "laptop", account);
        assert_eq!(launch.machine, "laptop");
        assert_eq!(launch.provider, "claude");
        assert_eq!(launch.account, "default-claude");
        assert_eq!(launch.cwd, "~/Developer/fleet");
        assert_eq!(
            triple(&launch),
            (String::new(), String::new(), String::new())
        );

        let unknown_provider = LaunchDefaults {
            provider: "mystery".into(),
            ..remembered
        };
        let launch = Launch::new(&saved(Some(unknown_provider)), &st, "laptop", account);
        assert_eq!(launch.provider, "codex");
        assert_eq!(launch.account, "default-codex");
    }

    #[test]
    fn new_launches_offer_the_fly_source_instead_of_an_existing_sessions_sprite() {
        let mut st = state();
        st.machines.push(Machine {
            id: "fly-acme".into(),
            name: "New Sprite · acme".into(),
            sprite_source: "acme".into(),
            ..Default::default()
        });
        st.machines.push(Machine {
            id: "sprite-old".into(),
            name: "Old session".into(),
            sprite: Some(SpriteBinding::default()),
            ..Default::default()
        });
        let projects = collect_projects(&st, &[], "fly-acme");
        assert!(projects.iter().any(|p| p.machine == "fly-acme"));
        assert!(!projects.iter().any(|p| p.machine == "sprite-old"));
        let remembered = LaunchDefaults {
            machine: "sprite-old".into(),
            ..Default::default()
        };
        let memory = LaunchMemory {
            launch: Some(remembered),
            ..Default::default()
        };
        let launch = Launch::new(&memory, &st, "fly-acme", |_| String::new());
        assert_eq!(launch.machine, "fly-acme");
    }

    #[test]
    fn switching_machines_without_profiles_uses_the_destination_folder() {
        let mut st = state();
        st.presets.clear();
        let mut launch = fresh(&st);
        launch.prompt = "Keep working".into();
        launch.catalog_key = "old machine catalog".into();
        launch
            .catalogs
            .insert("codex".into(), ModelState::default());

        launch.select_machine("studio");
        assert_eq!(launch.machine, "studio");
        assert_eq!(launch.cwd, "/srv/app");
        assert_eq!(launch.prompt, "Keep working");
        assert!(launch.catalogs.is_empty());
        assert!(launch.catalog_key.is_empty());

        // An enrolled host with no sessions or profiles is still launchable.
        launch.select_machine("blank");
        assert_eq!(launch.cwd, "~");
        let mut saved = LaunchMemory::default();
        saved.remember_launch(launch.defaults(), "Default");
        let reopened = Launch::new(&saved, &st, "laptop", |_| String::new());
        assert_eq!(reopened.machine, "blank");
        assert_eq!(reopened.cwd, "~");
    }

    #[test]
    fn selecting_the_current_machine_preserves_a_typed_folder() {
        let mut launch = fresh(&state());
        launch.cwd = "/custom/project".into();
        launch.select_machine("laptop");
        assert_eq!(launch.cwd, "/custom/project");
        // A machine enrolled after opening the dialog has no cached projects.
        launch.select_machine("new-host");
        assert_eq!(launch.machine, "new-host");
        assert_eq!(launch.cwd, "~");
    }

    #[test]
    fn quick_launch_uses_home_on_current_and_other_machines() {
        let mut launch = fresh(&state());
        launch.cwd = "~/research".into();
        launch.select_quick_machine("laptop");
        assert_eq!(launch.cwd, "~");
        launch.select_quick_machine("studio");
        assert_eq!(launch.machine, "studio");
        assert_eq!(launch.cwd, "~");
        launch.select_quick_machine("laptop");
        assert_eq!(launch.cwd, "~");
    }

    #[test]
    fn picker_rows_filter_projects_but_always_offer_adding_a_folder() {
        let st = state();
        let mut launch = fresh(&st);
        launch.picker = Some(ProjectPicker::default());
        let rows = launch.picker_rows();
        assert_eq!(rows.len(), launch.projects.len() + 3);
        let add = PickerRow::AddFolder {
            machine: "laptop".into(),
            machine_name: "LAPTOP".into(),
        };
        assert_eq!(rows[launch.projects.len()], add);

        launch.picker.as_mut().unwrap().list.query = "srv".into();
        let rows = launch.picker_rows();
        let projects: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                PickerRow::Project(i) => Some(launch.projects[*i].cwd.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(projects, vec!["/srv/app"]);
        let adds = rows
            .iter()
            .filter(|r| matches!(r, PickerRow::AddFolder { .. }))
            .count();
        assert_eq!(adds, 3);
    }

    #[test]
    fn focus_moves_in_order_and_insert_targets_text_controls() {
        assert_eq!(Focus::Project.moved(1), Focus::Machine);
        assert_eq!(Focus::Machine.moved(1), Focus::Prompt);
        assert_eq!(Focus::Project.moved(-1), Focus::Project);
        assert_eq!(Focus::Account.moved(1), Focus::Permissions);
        assert_eq!(Focus::Permissions.moved(1), Focus::Permissions);
        assert_eq!(Focus::Prompt.moved(3), Focus::Account);
        assert!(!Focus::Permissions.editable());
        let mut launch = fresh(&state());
        assert_eq!(launch.insert_target(), Focus::Prompt);
        launch.focus = Focus::Account;
        assert_eq!(launch.insert_target(), Focus::Prompt);
        launch.focus = Focus::Permissions;
        assert_eq!(launch.insert_target(), Focus::Prompt);
        launch.focus = Focus::Model;
        assert_eq!(launch.insert_target(), Focus::Model);
    }

    #[test]
    fn effort_choices_follow_the_harness() {
        assert_eq!(effort_choices("codex")[0], "");
        assert!(effort_choices("codex").contains(&"xhigh"));
        assert!(effort_choices("claude").contains(&"max"));
        assert_eq!(effort_choices("opencode"), &[""]);
    }

    #[test]
    fn model_rows_cover_every_harness_and_choosing_one_keeps_things_consistent() {
        let st = state();
        let account = |p: &str| format!("default-{p}");
        let mut launch = Launch::new(&LaunchMemory::default(), &st, "laptop", account);
        launch.effort = "xhigh".into();
        let opus = ModelOption {
            id: "opus".into(),
            name: "Opus".into(),
            description: "big".into(),
            default: true,
        };
        launch.catalogs.insert(
            "claude".into(),
            ModelState {
                options: vec![opus],
                ..Default::default()
            },
        );
        let rows = launch.model_rows();
        let ids: Vec<String> = rows
            .iter()
            .map(|r| format!("{}:{}", r.provider, r.id))
            .collect();
        assert_eq!(ids, vec!["codex:", "claude:", "claude:opus", "opencode:"]);
        assert_eq!(rows[0].name, "Default");
        assert_eq!(launch.current_model_row(), Some(0));
        assert_eq!(launch.model_name(), "Default");
        assert_eq!(effort_label(&launch.effort), "Extra High");

        // Switching harness resets the account and drops an effort Claude lacks.
        let opus = rows[2].clone();
        launch.choose_model(&opus, account);
        assert_eq!(launch.provider, "claude");
        assert_eq!(launch.model, "opus");
        assert_eq!(launch.account, "default-claude");
        assert_eq!(launch.effort, "");
        assert_eq!(launch.model_name(), "Opus");
        assert_eq!(launch.current_model_row(), Some(2));

        // A custom model shows in its harness's section so it can be re-picked.
        launch.model = "sonnet-x".into();
        let rows = launch.model_rows();
        assert!(rows
            .iter()
            .any(|r| r.provider == "claude" && r.id == "sonnet-x"));
        assert_eq!(launch.model_name(), "sonnet-x");

        // A harness that lists its own "default" replaces the synthetic row.
        let recommended = ModelOption {
            id: "default".into(),
            name: "Default (recommended)".into(),
            ..Default::default()
        };
        launch
            .catalogs
            .get_mut("claude")
            .unwrap()
            .options
            .insert(0, recommended);
        let rows = launch.model_rows();
        let claude: Vec<&str> = rows
            .iter()
            .filter(|r| r.provider == "claude")
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(claude, vec!["default", "opus", "sonnet-x"]);

        // Recent models lead the list, and picking one restores its last effort.
        launch.recent_models = vec![RecentModel {
            provider: "codex".into(),
            id: "gpt-6".into(),
            name: "GPT-6".into(),
        }];
        launch
            .efforts
            .insert(effort_key("codex", "gpt-6"), "xhigh".into());
        let rows = launch.model_rows();
        assert!(rows[0].recent);
        assert_eq!(rows[0].name, "GPT-6");
        let recent = rows[0].clone();
        launch.choose_model(&recent, account);
        assert_eq!(launch.provider, "codex");
        assert_eq!(launch.effort, "xhigh");
        assert!(launch.has_effort());
        launch.provider = "opencode".into();
        assert!(!launch.has_effort());
    }

    #[test]
    fn effort_levels_follow_the_harness_and_keep_a_custom_value() {
        let mut launch = fresh(&state());
        assert_eq!(
            launch.effort_levels(),
            ["", "minimal", "low", "medium", "high", "xhigh"]
        );
        assert_eq!(launch.effort_index(), 0);
        launch.effort = "low".into();
        assert_eq!(launch.effort_index(), 2);
        launch.provider = "claude".into();
        launch.effort = "ultra".into();
        assert_eq!(
            launch.effort_levels().last().map(String::as_str),
            Some("ultra")
        );
        assert_eq!(launch.effort_index(), 5);
        assert_eq!(effort_label("ultra"), "Custom");
        launch.provider = "opencode".into();
        launch.effort.clear();
        assert_eq!(launch.effort_levels(), [""]);
    }

    #[test]
    fn permission_choices_lead_with_the_machine_default_and_follow_the_provider() {
        let mut launch = fresh(&state());
        launch.provider = "codex".into();
        let codex = ["", "read-only", "auto", "full-access", "yolo"];
        assert_eq!(launch.permission_choices(""), codex);
        // A machine default makes "provider default" a distinct choice.
        let with_default = ["", "default", "read-only", "auto", "full-access", "yolo"];
        assert_eq!(launch.permission_choices("yolo"), with_default);
        // A remembered mode the provider no longer lists is still visible.
        launch.permissions = "custom".into();
        assert!(launch
            .permission_choices("")
            .contains(&"custom".to_string()));

        // Switching harness through the model list drops the override: modes
        // are named per provider, so the new machine default applies.
        launch.permissions = "auto".into();
        let rows = launch.model_rows();
        let row = rows
            .iter()
            .find(|r| r.provider == "claude")
            .expect("claude default row");
        launch.choose_model(row, |_| String::new());
        assert_eq!(launch.provider, "claude");
        assert_eq!(launch.permissions, "");
    }

    #[test]
    fn submit_builds_the_run_argv_and_remembers_the_launch() {
        let st = state();
        let mut launch = fresh(&st);
        launch.provider = "claude".into();
        launch.account = "claude:me@example.com".into();
        launch.cwd = "/srv/app ".into();
        launch.model = "opus".into();
        launch.effort = "high".into();
        launch.prompt = "Fix the build".into();
        let mut memory = LaunchMemory::default();
        let s = launch.submit(&st, &mut memory, "r1").unwrap();
        let expected = [
            "run",
            "laptop",
            "--request-id",
            "r1",
            "--provider",
            "claude",
            "--account",
            "claude:me@example.com",
            "--cwd",
            "/srv/app",
            "--model",
            "opus",
            "--effort",
            "high",
            "--prompt",
            "Fix the build",
            "--attach",
        ];
        assert_eq!(s.argv, expected);
        assert_eq!(s.title, "Claude · LAPTOP");
        assert_eq!(s.tab_key, "laptop:r1:false");
        assert_eq!(memory.launch.as_ref().unwrap().model, "opus");
        let recent = RecentProject {
            machine: "laptop".into(),
            cwd: "/srv/app".into(),
        };
        assert_eq!(memory.recent_projects[0], recent);
        assert_eq!(memory.recent_models[0].id, "opus");
        assert_eq!(memory.model_efforts["claude/opus"], "high");
        // A permission choice travels before --attach; OpenCode drops the effort.
        launch.permissions = "plan".into();
        launch.provider = "opencode".into();
        let s = launch.submit(&st, &mut memory, "r2").unwrap();
        assert_eq!(
            &s.argv[s.argv.len() - 3..],
            ["--permissions", "plan", "--attach"]
        );
        assert_eq!(launch.effort, "");
        launch.cwd = "  ".into();
        let err = launch.submit(&st, &mut memory, "r3").unwrap_err();
        assert_eq!(err, "Choose a project folder");
        launch.machine = "gone".into();
        let err = launch.submit(&st, &mut memory, "r4").unwrap_err();
        assert_eq!(err, "Choose a project on an enrolled machine");
        // The auto machine runs `fleet auto-machine` without a folder.
        launch.auto_machine = true;
        let s = launch.submit(&st, &mut memory, "r5").unwrap();
        assert_eq!(s.argv[0], "auto-machine");
        assert_eq!(s.tab_key, "auto:r5");
    }
}
