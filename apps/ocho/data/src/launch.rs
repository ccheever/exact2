//! The launch composer: a prompt, and one line under it naming where the
//! session runs and what runs it (launch.rs, with the workspace.rs handlers
//! and the ui.rs views). Typing always types — there is no navigation mode —
//! and Enter launches the remembered configuration again.
//!
//! What the line shows is decided by what launches actually set: of 165 real
//! launches, none set a permission mode and ten set an effort, so those live
//! behind the "everything else" menu (⌘E) rather than on screen.
//!
//! This file is the model; `launch/menu.rs` the one menu that opens over
//! the composer and its rows, `launch/catalog.rs` the I/O the caller runs
//! for it (`fleet models`, `fleet directories`), `launch/keys.rs` its keys
//! and presses, `launch/view.rs` its `OVERLAY` JSON.

pub mod catalog;
pub mod keys;
pub mod memory;
pub mod menu;
pub mod view;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;

use crate::quick::QuickLaunch;
use crate::types::{Account, Machine, State};

pub use catalog::{CatalogKey, DirectoryMatches, DirectoryState, ModelOption, ModelState};
pub use memory::{effort_key, LaunchDefaults, LaunchMemory, RecentModel, RecentProject};
pub use menu::{Menu, MenuKind, ModelRow};
pub use view::overlay;

/// The harnesses, in the order every list shows them.
pub const PROVIDERS: [&str; 5] = ["codex", "claude", "opencode", "antigravity", "grok"];

/// The toast when Esc keeps a prompt.
pub const DRAFT_KEPT: &str = "Prompt kept as a draft; ⌘N brings it back";

/// How a provider reads (backend.rs `provider_label`).
pub fn provider_label(p: &str) -> &str {
    match p {
        "claude" => "Claude",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        "antigravity" => "Antigravity",
        "grok" => "Grok",
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

/// The machine's default mode for a provider, as `fleet run` will apply it;
/// empty when the machine sets none.
pub fn machine_permission_default(state: &State, machine_id: &str, provider: &str) -> String {
    machine(state, machine_id)
        .and_then(|m| m.permissions.get(provider).cloned())
        .unwrap_or_default()
}

/// Shared account handles for a harness, in inventory order (workspace.rs
/// `account_choices`). The machine login is not among them.
pub fn account_choices(state: &State, provider: &str) -> Vec<String> {
    state
        .accounts
        .iter()
        .filter(|a| a.shared && a.provider == provider)
        .map(account_handle)
        .collect()
}

/// Whether the machine is bound to an EAS project, which runs Codex only.
pub fn is_eas(state: &State, machine_id: &str) -> bool {
    machine(state, machine_id).is_some_and(|m| m.eas.is_some())
}

/// What a machine is called on the line (ui.rs `machine_label`): the one
/// under your hands is not worth spelling out as a hostname.
pub fn machine_label(m: &Machine) -> String {
    if m.local {
        return "this Mac".into();
    }
    m.name
        .split('.')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(&m.name)
        .to_string()
}

/// The harness's dot colour, as a theme colour name (ui.rs `provider_dot`).
/// Colour is the only thing a dot can say, in less room than the word.
pub fn provider_dot(provider: &str) -> &'static str {
    match provider {
        "claude" => "warn",
        "codex" => "good",
        _ => "accent",
    }
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
        "claude" | "antigravity" => &["", "low", "medium", "high", "max"],
        _ => &[""],
    }
}

/// How an effort level reads on the line and in the menu.
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

/// The controls on the line under the prompt, in tab order. The prompt is
/// one of them, so Tab walks out of the text and back into it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Focus {
    /// The prompt; typing always reaches it.
    #[default]
    Prompt,
    /// Where it runs: the project, with its machine.
    Project,
    /// What runs it: the harness and model.
    Model,
    /// Who runs it: the account, when the harness has more than one.
    Account,
}

impl Focus {
    /// Tab order.
    pub const ORDER: [Focus; 4] = [Focus::Prompt, Focus::Project, Focus::Model, Focus::Account];

    /// Tab wraps: the line is a loop, not a form to fall off the end of.
    pub fn moved(self, step: i32) -> Focus {
        let len = Self::ORDER.len() as i32;
        let at = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0) as i32;
        Self::ORDER[(((at + step) % len + len) % len) as usize]
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
    /// The name to say out loud: the folder's own name, or "home" for `~`.
    pub fn name(&self) -> String {
        let cwd = self.cwd.trim_end_matches('/');
        if cwd.is_empty() || cwd == "~" {
            return "home".into();
        }
        cwd.rsplit('/').next().unwrap_or(cwd).to_string()
    }

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

/// Folders Fleet made for its own plumbing — viewer copies, managed
/// workspaces, preview checkouts — are not projects anyone chose to work in.
fn fleet_owned(cwd: &str) -> bool {
    cwd.contains("/.local/share/fleet/") || cwd.contains("/Library/Application Support/fleet/")
}

/// Folders worth offering, current machine first: recent launches, profile
/// folders, then folders of observed sessions (newest first). An EAS machine
/// offers this Mac's folders instead of its ephemeral worker paths. Each
/// machine without any known folder offers its home directory.
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
            if fleet_owned(cwd) || out[start..].iter().any(|p| p.cwd == cwd) {
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
            let mut sessions: Vec<_> = last
                .sessions
                .iter()
                .filter(|s| !s.cwd.is_empty() && !s.eas)
                .collect();
            sessions.sort_by(|a, b| b.started.cmp(&a.started));
            let before = out.len();
            for s in sessions {
                if out.len() - before >= SESSION_PROJECTS_PER_MACHINE {
                    break;
                }
                push(&tilde(&s.cwd, &last.home), ProjectSource::Session, &mut out);
            }
        }
        if m.eas.is_some() {
            for local in state.machines.iter().filter(|local| local.local) {
                for r in recent
                    .iter()
                    .filter(|r| r.machine == local.id && !r.cwd.is_empty())
                {
                    push(&r.cwd, ProjectSource::Recent, &mut out);
                }
                if let Some(last) = &local.last {
                    let sessions = last.sessions.iter().filter(|s| !s.cwd.is_empty());
                    for s in sessions.take(SESSION_PROJECTS_PER_MACHINE) {
                        push(&tilde(&s.cwd, &last.home), ProjectSource::Session, &mut out);
                    }
                }
            }
        }
        if out.len() == start {
            push("~", ProjectSource::Home, &mut out);
        }
    }
    out
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

/// The composer.
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
    /// The folder; `~` is "no project".
    pub cwd: String,
    /// The model id, empty for the harness default.
    pub model: String,
    /// The effort level, empty for auto.
    pub effort: String,
    /// A permission mode for this launch; empty takes the machine default.
    pub permissions: String,
    /// The prompt.
    pub prompt: String,
    /// Which control the keyboard is on. Typing always reaches the prompt.
    pub focus: Focus,
    /// The one menu that can be open over the composer.
    pub menu: Option<Menu>,
    /// Model catalogs by harness, loaded while the model menu is open.
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
            focus: Focus::Prompt,
            projects,
            recent_models: saved.recent_models.clone(),
            efforts: saved.model_efforts.clone(),
            ..Default::default()
        }
    }

    /// ⌘N (workspace.rs `open_launch`): the selected machine or the first
    /// enrolled one. The dialog is a place to type: it opens on the prompt
    /// with the draft back, and Enter still launches the remembered choices.
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
        Ok(launch)
    }

    /// Esc: the prompt to keep as a draft (the earlier draft, under quick
    /// launch). The caller toasts `DRAFT_KEPT` when it is not blank
    /// (`trim()` non-empty), quick launch included.
    pub fn close(&self) -> String {
        if self.quick.is_none() {
            self.prompt.clone()
        } else {
            self.draft.clone()
        }
    }

    /// Switching hosts chooses a folder known on that host, never a local
    /// absolute path carried over from the previous machine: the same repo
    /// on the new machine if it has one, else the newest folder it does
    /// know, else its home directory.
    pub fn select_machine(&mut self, machine: &str) {
        if self.machine == machine {
            return;
        }
        let leaving = self
            .projects
            .iter()
            .find(|p| p.machine == self.machine && p.cwd == self.cwd)
            .map(|p| p.name());
        self.machine = machine.to_string();
        let here = |pick: &dyn Fn(&Project) -> bool| {
            self.projects
                .iter()
                .find(|p| p.machine == self.machine && pick(p))
                .map(|p| p.cwd.clone())
        };
        self.cwd = leaving
            .and_then(|name| here(&|p: &Project| p.name() == name))
            .or_else(|| here(&|_: &Project| true))
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

    /// Choose a machine from the composer (workspace.rs `set_launch_machine`):
    /// an EAS machine runs Codex only, so the harness follows, and an open
    /// menu starts over on the new machine.
    pub fn set_machine(&mut self, machine_id: &str, state: &State) {
        self.select_machine(machine_id);
        if is_eas(state, machine_id) && self.provider != "codex" {
            self.provider = "codex".into();
            self.model.clear();
            self.account = state
                .accounts
                .iter()
                .find(|a| a.shared && a.provider == "codex")
                .map(account_handle)
                .unwrap_or_default();
        }
        if let Some(menu) = self.menu.as_mut() {
            menu.index = 0;
            menu.query.clear();
        }
        self.sync_catalogs(state, false);
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

    /// The effort rows only exist when the harness has levels.
    pub fn has_effort(&self) -> bool {
        effort_choices(&self.provider).len() > 1
    }

    /// The model's name: the catalog name when known, else the ID, else
    /// "Default".
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

    /// Effort levels for the current harness: default first, then each
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

    /// Enter / ⌘Enter / the send button (workspace.rs `submit_launch`): the
    /// `fleet run` to open in a new tab, remembering the choices. An error
    /// stays on the dialog.
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
        let codex_account = state
            .accounts
            .iter()
            .any(|a| a.shared && a.provider == "codex" && account_handle(a) == self.account);
        if m.eas.is_some() && (self.provider != "codex" || !codex_account) {
            self.error = "EAS requires Codex and a connected Codex account".into();
            return Err(self.error.clone());
        }
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
