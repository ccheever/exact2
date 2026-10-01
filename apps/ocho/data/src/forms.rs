//! The forms: a titled card of labelled fields with Tab order, dropdown
//! choices, folder completion on a Directory field and the model catalog
//! under a Model field (workspace.rs `Form`, `FormKind`, `open_*_form`,
//! `handle_form_key`, `submit_form`, `choices`, `choice_label`; ui.rs
//! `render_form`). Pure state: the app opens one as `Overlay::Form`, sends
//! keys, input and presses here, runs what `wanted_*` ask for, and executes
//! the `Submission` that `submit` builds.
//!
//! `forms/submit.rs` (`submit::Submission`) builds the `fleet` argv per kind, `forms/view.rs` the
//! `OVERLAY` JSON and the dropdown's popup items.

pub mod submit;
mod view;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::launch::{
    self, default_account, CatalogKey, DirectoryMatches, DirectoryState, ModelOption, ModelState,
};
use crate::model::SessionItem;
use crate::picker::{self, Mods};
use crate::session::{
    account, account_email, account_handle, machine, provider_label, session_state, NamedProfile,
};
use crate::types::{Account, LaunchProfile, Machine, Session, State};

/// The profile form's model field.
pub const MODEL_FIELD: &str = "Model (blank = native)";
/// The profile form's effort field.
pub const EFFORT_FIELD: &str = "Reasoning effort (blank = native)";
/// The profile form's permissions field.
pub const PERMISSIONS_FIELD: &str = "Permissions (blank = machine default)";
/// The machine form's Codex default mode.
pub const CODEX_PERMISSIONS_FIELD: &str = "Codex permissions";
/// The machine form's Claude default mode.
pub const CLAUDE_PERMISSIONS_FIELD: &str = "Claude permissions";
/// The machine form's OpenCode default mode.
pub const OPENCODE_PERMISSIONS_FIELD: &str = "OpenCode permissions";
/// The machine form's Antigravity default mode.
pub const ANTIGRAVITY_PERMISSIONS_FIELD: &str = "Antigravity permissions";
/// The machine form's Grok default mode.
pub const GROK_PERMISSIONS_FIELD: &str = "Grok permissions";
/// The Pair iMessage form's only field.
pub const PHONE_FIELD: &str = "Your iMessage number";
/// The Pair iMessage form's error for a number without a country code.
pub const PHONE_ERROR: &str =
    "Enter your phone number with country code, for example +14155552671.";

/// The toast when Esc keeps what was typed.
pub const DRAFT_KEPT: &str = "Draft kept; reopen the form to continue";

/// What a field is: free text, a dropdown, a folder with completion, or a
/// model ID with the catalog under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// Typed text.
    Text,
    /// One of `choices`.
    Choice,
    /// A folder on the form's machine, completed by `fleet directories`.
    Directory,
    /// A model ID, with `fleet models` listed under it.
    Model,
}

/// One labelled value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// The label, which also names the field in `choices` and `submit`.
    pub label: &'static str,
    /// The value: raw (a machine id, an account handle, a mode), never a label.
    pub value: String,
    /// What it is.
    pub kind: FieldKind,
}

/// A field; its kind follows the label (workspace.rs `field`).
pub fn field(label: &'static str, value: impl Into<String>) -> Field {
    let kind = match label {
        "Machine" | "Provider" | "Account" | "First connection" | "Shortcut" => FieldKind::Choice,
        PERMISSIONS_FIELD
        | CODEX_PERMISSIONS_FIELD
        | CLAUDE_PERMISSIONS_FIELD
        | OPENCODE_PERMISSIONS_FIELD
        | ANTIGRAVITY_PERMISSIONS_FIELD
        | GROK_PERMISSIONS_FIELD => FieldKind::Choice,
        "Directory" => FieldKind::Directory,
        MODEL_FIELD => FieldKind::Model,
        _ => FieldKind::Text,
    };
    Field {
        label,
        value: value.into(),
        kind,
    }
}

/// The placeholder under an empty text field (ui.rs `field_placeholder`).
pub fn field_placeholder(label: &str) -> &'static str {
    match label {
        "Directory" => "~/path/to/project",
        "Title prompt" => "Instructions for generating session titles…",
        EFFORT_FIELD => "Provider default",
        "Prompt" | "Initial prompt" | "Message" => "What would you like to work on?",
        "Name" | "Friendly name" | "Label" => "Enter a name…",
        "Notes" => "Where it is, what it is for…",
        "Tags" => "comma,separated",
        "Host" | "SSH host" | "SSH destination" => "user@hostname",
        "SSH arguments (optional)" => "Optional SSH arguments…",
        "Native conversation" => "Conversation ID…",
        _ => "Enter a value…",
    }
}

/// Which form.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FormKind {
    /// Ask for the phone number to pair iMessage with.
    PairIMessage,
    /// Name a rail folder.
    Folder,
    /// Enroll a machine.
    Machine,
    /// Edit a machine's name, notes, tags and default modes.
    MachineEdit,
    /// Sign an account in.
    Account,
    /// Save the fleet recovery key to a file.
    RecoveryBackup,
    /// Label a session.
    Label,
    /// Resume a native conversation.
    Resume,
    /// Hand a limited session to another account.
    Handoff,
    /// Resume a Codex session under another account.
    SwitchAccount,
    /// Move a session to another machine.
    Migrate,
    /// Save a launch profile.
    Profile,
}

/// What the caller does after a key or press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormOutcome {
    /// Handled; repaint.
    None,
    /// Not a form key: the focused text input keeps it.
    Ignored,
    /// Close the form, keeping `draft()`.
    Close,
    /// Call `submit`.
    Submit,
    /// Open the dropdown of the field at this index (`PopupKind::FieldChoice`).
    OpenChoice(usize),
}

/// A form.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    /// Which one.
    pub kind: FormKind,
    /// The card's title.
    pub title: &'static str,
    /// The fields, in Tab order.
    pub fields: Vec<Field>,
    /// The focused field.
    pub focus: usize,
    /// The session a Label, Resume, Handoff, SwitchAccount or Migrate form acts on.
    pub target: Option<SessionItem>,
    /// What an edit form edits: the folder key, machine id or profile name.
    pub editing: Option<String>,
    /// Folder completion for the focused Directory field.
    pub directory: DirectoryState,
    /// The model catalog for the Model field.
    pub models: ModelState,
    /// Catalogs loaded while this form was open, by what they were loaded for.
    pub catalogs: HashMap<CatalogKey, Vec<ModelOption>>,
    /// The submit error, shown on the card.
    pub error: String,
}

/// A session moves to another enrolled machine, never onto the one it runs on.
pub fn migrate_target(machines: &[Machine], source: &str) -> Option<String> {
    machines
        .iter()
        .find(|m| m.id != source)
        .map(|m| m.id.clone())
}

/// A handoff stays with the provider but must cross account boundaries. Accept
/// either the stable account name or its user-facing handle on older sessions.
pub fn handoff_account_choices(accounts: &[Account], source: &Session) -> Vec<String> {
    accounts
        .iter()
        .filter(|account| {
            account.shared
                && account.status == "connected"
                && account.provider == source.provider
                && account.name != source.account
                && account_handle(account) != source.account
        })
        .map(account_handle)
        .collect()
}

/// Resuming a conversation under another account is offered for Codex only.
/// Claude's terms do not allow switching accounts to keep going past a usage
/// limit, so Claude sessions never see the action.
pub fn offers_account_switch(provider: &str) -> bool {
    provider == "codex"
}

/// The machine default first, then each mode the provider names.
fn permission_choices(provider: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    out.extend(
        crate::permissions::choices(provider)
            .iter()
            .map(|s| s.to_string()),
    );
    out
}

/// imessage_pair.rs `normalize_phone`: pasted spaces, parentheses and
/// dashes dropped; a `+`, a country code that does not start with 0, and 7
/// to 15 digits in all.
pub fn normalize_phone(input: &str) -> Option<String> {
    let phone: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '(' | ')' | '-'))
        .collect();
    let digits = phone.strip_prefix('+')?;
    (digits.len() >= 7
        && digits.len() <= 15
        && !digits.starts_with('0')
        && digits.bytes().all(|c| c.is_ascii_digit()))
    .then_some(phone)
}

/// "{label} · {machine}", the session tab title a submission opens with.
fn session_tab_title(label: &str, machine: &str) -> String {
    format!("{label} · {machine}")
}

/// The label part of a session tab's title (workspace.rs `tab_session_label`).
fn tab_session_label(title: &str) -> String {
    title
        .rsplit_once(" · ")
        .map(|(label, _)| label)
        .unwrap_or(title)
        .trim_end_matches(" [view]")
        .to_string()
}

/// How a choice reads (workspace.rs `choice_label`): a shortcut as ⌘⌥N, a
/// machine by name, an account by email, a mode by its label.
pub fn choice_label(state: &State, label: &str, value: &str) -> String {
    match label {
        "Shortcut" => {
            if value == "0" {
                "None".into()
            } else {
                format!("⌘⌥{value}")
            }
        }
        "Machine" => machine(state, value)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| value.to_string()),
        "Provider" => provider_label(value).to_string(),
        PERMISSIONS_FIELD => crate::permissions::label(value),
        CODEX_PERMISSIONS_FIELD
        | CLAUDE_PERMISSIONS_FIELD
        | OPENCODE_PERMISSIONS_FIELD
        | ANTIGRAVITY_PERMISSIONS_FIELD
        | GROK_PERMISSIONS_FIELD => {
            if value.is_empty() {
                "Provider default".into()
            } else {
                crate::permissions::label(value)
            }
        }
        "Account" => {
            if value.is_empty() {
                "Use machine’s existing login".into()
            } else {
                account(state, value)
                    .map(account_email)
                    .unwrap_or_else(|| value.to_string())
            }
        }
        _ => value.to_string(),
    }
}

impl Form {
    /// A form on its first field, nothing loaded.
    pub fn new(kind: FormKind, title: &'static str, fields: Vec<Field>) -> Form {
        Form {
            kind,
            title,
            fields,
            focus: 0,
            target: None,
            editing: None,
            directory: DirectoryState::default(),
            models: ModelState::default(),
            catalogs: HashMap::new(),
            error: String::new(),
        }
    }

    // ----- constructors (workspace.rs `open_*_form`, `Command::Add*`) -------

    /// "Add machine": `fleet add` in a terminal tab.
    pub fn machine() -> Form {
        Form::new(
            FormKind::Machine,
            "Add machine",
            vec![
                field("SSH destination", ""),
                field("Friendly name", ""),
                field("First connection", "existing SSH"),
                field("SSH arguments (optional)", ""),
            ],
        )
    }

    /// "Edit machine": name, notes, tags and the default mode per provider.
    pub fn machine_edit(machine: &Machine) -> Form {
        let mode = |provider: &str| {
            machine
                .permissions
                .get(provider)
                .cloned()
                .unwrap_or_default()
        };
        let mut form = Form::new(
            FormKind::MachineEdit,
            "Edit machine",
            vec![
                field("Friendly name", machine.name.clone()),
                field("Notes", machine.notes.clone()),
                field("Tags", machine.tags.join(",")),
                field(CODEX_PERMISSIONS_FIELD, mode("codex")),
                field(CLAUDE_PERMISSIONS_FIELD, mode("claude")),
                field(OPENCODE_PERMISSIONS_FIELD, mode("opencode")),
                field(ANTIGRAVITY_PERMISSIONS_FIELD, mode("antigravity")),
                field(GROK_PERMISSIONS_FIELD, mode("grok")),
            ],
        );
        form.editing = Some(machine.id.clone());
        form
    }

    /// "Add account": a provider sign-in in a terminal tab.
    pub fn account() -> Form {
        Form::new(
            FormKind::Account,
            "Add account",
            vec![
                field("Provider", "codex"),
                field("Authentication", "Provider sign-in"),
            ],
        )
    }

    /// "Back up recovery key" (`Command::BackupRecovery`): where
    /// `fleet cloud export-recovery` writes the key.
    pub fn recovery_backup() -> Form {
        Form::new(
            FormKind::RecoveryBackup,
            "Back up recovery key",
            vec![field(
                "Save outside Ocho state",
                "~/Desktop/ocho-recovery.json",
            )],
        )
    }

    /// "Pair iMessage" (`Command::PairIMessage`): the number to pair; the
    /// pairing code it leads to is the app's (`pairing_phone`).
    pub fn pair_imessage() -> Form {
        Form::new(
            FormKind::PairIMessage,
            "Pair iMessage",
            vec![field(PHONE_FIELD, "")],
        )
    }

    /// "Label session" for a session row (workspace.rs `open_session_label`).
    pub fn label(item: SessionItem) -> Form {
        let label = field("Label", item.session.title.clone());
        let mut form = Form::new(FormKind::Label, "Label session", vec![label]);
        form.target = Some(item);
        form
    }

    /// "Label session" for a terminal tab backed by a session (workspace.rs
    /// `label_tab_session`): the fleet's record when it has one, else a stand-in
    /// carrying the tab's own label.
    pub fn label_tab(state: &State, machine_id: &str, session_id: &str, tab_title: &str) -> Form {
        let known = machine(state, machine_id).and_then(|m| {
            m.last
                .as_ref()
                .and_then(|last| last.sessions.iter().find(|s| s.id == session_id))
                .map(|s| SessionItem {
                    machine: m.clone(),
                    session: s.clone(),
                })
        });
        let item = known.unwrap_or_else(|| SessionItem {
            session: Session {
                id: session_id.to_string(),
                title: tab_session_label(tab_title),
                ..Default::default()
            },
            machine: machine(state, machine_id).cloned().unwrap_or(Machine {
                id: machine_id.to_string(),
                name: machine_id.to_string(),
                ..Default::default()
            }),
        });
        Form::label(item)
    }

    /// "New folder", or "Rename folder" for `existing` (its key and title).
    pub fn folder(existing: Option<(&str, &str)>) -> Form {
        let name = field("Name", existing.map_or("New folder", |(_, title)| title));
        let title = if existing.is_some() {
            "Rename folder"
        } else {
            "New folder"
        };
        let mut form = Form::new(FormKind::Folder, title, vec![name]);
        form.editing = existing.map(|(key, _)| key.to_string());
        form
    }

    /// "Resume native conversation".
    pub fn resume(item: SessionItem) -> Form {
        let mut form = Form::new(
            FormKind::Resume,
            "Resume native conversation",
            vec![
                field("Machine", item.machine.id.clone()),
                field("Provider", item.session.provider.clone()),
                field("Account", item.session.account.clone()),
                field("Directory", item.session.cwd.clone()),
                field("Native conversation", item.session.native_id.clone()),
            ],
        );
        form.target = Some(item);
        form
    }

    /// "Resume with a different account", for an idle or limited Codex session
    /// with another connected Codex account to pick; the error otherwise.
    pub fn switch_account(item: SessionItem, state: &State) -> Result<Form, String> {
        // Claude's terms forbid rotating accounts to continue past a limit; the
        // CLI refuses too, but the action is never offered for it here.
        if !offers_account_switch(&item.session.provider) {
            return Err("Only Codex sessions can resume under a different account.".into());
        }
        if !matches!(session_state(&item.session), "IDLE" | "LIMITED") {
            return Err("Wait for the current turn to finish before switching accounts.".into());
        }
        let accounts = handoff_account_choices(&state.accounts, &item.session);
        let Some(account) = accounts.first() else {
            return Err("Connect another Codex account before switching this session.".into());
        };
        let mut form = Form::new(
            FormKind::SwitchAccount,
            "Resume with a different account",
            vec![field("Account", account.clone())],
        );
        form.target = Some(item);
        Ok(form)
    }

    /// "Hand off to another agent", for a limited session with another
    /// connected account of its provider; the error otherwise.
    pub fn handoff(item: SessionItem, state: &State) -> Result<Form, String> {
        if session_state(&item.session) != "LIMITED" {
            return Err("Only a session stopped by a usage limit can be handed off.".into());
        }
        let accounts = handoff_account_choices(&state.accounts, &item.session);
        let Some(account) = accounts.first() else {
            return Err(format!(
                "Connect another {} account before handing this session off.",
                provider_label(&item.session.provider)
            ));
        };
        let mut form = Form::new(
            FormKind::Handoff,
            "Hand off to another agent",
            vec![field("Account", account.clone())],
        );
        form.target = Some(item);
        Ok(form)
    }

    /// "Move session to another machine". Moving keeps the conversation: the
    /// session stops here and reopens there with the same working directory
    /// and native history.
    pub fn migrate(item: SessionItem, state: &State) -> Result<Form, String> {
        if item.session.native_id.is_empty() {
            return Err("This session has no native conversation to move yet.".into());
        }
        let Some(target) = migrate_target(&state.machines, &item.machine.id) else {
            return Err("Add a second machine before moving a session.".into());
        };
        let mut form = Form::new(
            FormKind::Migrate,
            "Move session to another machine",
            // A blank directory lands the session in an Ocho-managed workspace,
            // so the same path does not have to exist on the target machine.
            vec![field("Machine", target), field("Directory", String::new())],
        );
        form.target = Some(item);
        Ok(form)
    }

    /// "New profile" on the first machine with the first free shortcut, or
    /// "Edit profile" for `existing`; an error without a machine.
    pub fn profile(existing: Option<NamedProfile>, state: &State) -> Result<Form, String> {
        let Some(machine) = state.machines.first() else {
            return Err("Add a machine before creating a profile".into());
        };
        let mut p = LaunchProfile {
            machine_id: machine.id.clone(),
            provider: "codex".into(),
            cwd: "~".into(),
            account: default_account(state, "codex"),
            ..Default::default()
        };
        let mut name = String::new();
        let used: Vec<i64> = state.presets.values().map(|p| p.shortcut).collect();
        if let Some(slot) = (1..=9).find(|s| !used.contains(s)) {
            p.shortcut = slot;
        }
        let mut editing = None;
        if let Some(existing) = existing {
            p = existing.profile;
            name = existing.name.clone();
            editing = Some(existing.name);
        }
        let mut form = Form::new(
            FormKind::Profile,
            if editing.is_some() {
                "Edit profile"
            } else {
                "New profile"
            },
            vec![
                field("Name", name),
                field("Shortcut", p.shortcut.to_string()),
                field("Machine", p.machine_id),
                field("Provider", p.provider),
                field("Account", p.account),
                field("Directory", p.cwd),
                field(MODEL_FIELD, p.model),
                field(EFFORT_FIELD, p.effort),
                field(PERMISSIONS_FIELD, p.permissions),
                field("Initial prompt", p.prompt),
            ],
        );
        form.editing = editing;
        Ok(form)
    }

    // ----- reading ----------------------------------------------------------

    /// The value of the field with this label, empty when there is none.
    pub fn value(&self, label: &str) -> String {
        self.fields
            .iter()
            .find(|f| f.label == label)
            .map(|f| f.value.clone())
            .unwrap_or_default()
    }

    /// The focused field.
    pub fn focused(&self) -> &Field {
        &self.fields[self.focus.min(self.fields.len() - 1)]
    }

    /// The focused field is the Directory.
    pub fn directory_focused(&self) -> bool {
        self.focused().kind == FieldKind::Directory
    }

    /// The focused field is the Model.
    pub fn model_focused(&self) -> bool {
        self.focused().kind == FieldKind::Model
    }

    /// Native default first, the loaded catalog, then the typed custom ID.
    pub fn model_options(&self) -> Vec<ModelOption> {
        launch::model_options(&self.models.options, &self.focused().value)
    }

    /// The primary button's text.
    pub fn submit_label(&self) -> &'static str {
        match self.kind {
            FormKind::PairIMessage => "Get pairing code",
            FormKind::Resume => "Launch",
            FormKind::Machine => "Add machine",
            FormKind::MachineEdit => "Save machine",
            FormKind::Account => "Sign in",
            FormKind::RecoveryBackup => "Save recovery key",
            FormKind::Label => "Save label",
            FormKind::Folder => "Save folder",
            FormKind::Migrate => "Move session",
            FormKind::Handoff => "Hand off",
            FormKind::SwitchAccount => "Switch account",
            FormKind::Profile => "Save profile",
        }
    }

    /// The key hint line under the fields.
    pub fn hint(&self) -> &'static str {
        match self.kind {
            FormKind::PairIMessage => {
                "Use the number you send iMessages from, including country code (for example +14155552671). We'll show you a QR code to connect it to your Fleet."
            }
            FormKind::Account => {
                "Space picks the provider · Enter opens the sign-in terminal. Ocho reads your email from the provider and saves the account to Cloudflare."
            }
            FormKind::Label => "Enter saves the label and tracks the session · Esc cancel",
            FormKind::Profile => {
                "Space opens a dropdown · Tab next · Enter saves on the last field · Esc keeps a draft"
            }
            FormKind::MachineEdit => {
                "Permissions apply to new sessions on this machine that do not choose their own · Space opens a dropdown · Enter saves on the last field · Esc cancel"
            }
            FormKind::Migrate => {
                "Space picks the machine to move to · leave the directory blank for an Ocho workspace · Enter stops this session and reopens it there with its history and Git work · Esc cancel"
            }
            FormKind::SwitchAccount => {
                "Space picks another Codex account · Enter resumes this conversation under that account (an app-server session reopens as a new session) · Esc cancel"
            }
            FormKind::Handoff => {
                "Space picks another account · Enter parks the limited session and starts a fresh agent in the same directory. Native conversation history is not copied."
            }
            _ => {
                "Space opens a dropdown · ← / → change it · Tab next · Enter submits on the last field · Esc keeps a draft"
            }
        }
    }

    /// The values a choice field offers (workspace.rs `choices`); empty for
    /// a text field.
    pub fn choices(&self, state: &State, label: &str) -> Vec<String> {
        match label {
            "Provider" => launch::PROVIDERS.iter().map(|p| (*p).into()).collect(),
            "Authentication"
                if matches!(
                    self.value("Provider").as_str(),
                    "opencode" | "antigravity" | "grok"
                ) =>
            {
                vec!["Provider sign-in".into()]
            }
            "Authentication" => vec!["Provider sign-in".into(), "API key".into()],
            "First connection" => vec!["existing SSH".into(), "password".into()],
            "Shortcut" => (0..=9).map(|i| i.to_string()).collect(),
            "Machine" => state.machines.iter().map(|m| m.id.clone()).collect(),
            PERMISSIONS_FIELD => permission_choices(&self.value("Provider")),
            CODEX_PERMISSIONS_FIELD => permission_choices("codex"),
            CLAUDE_PERMISSIONS_FIELD => permission_choices("claude"),
            OPENCODE_PERMISSIONS_FIELD => permission_choices("opencode"),
            ANTIGRAVITY_PERMISSIONS_FIELD => permission_choices("antigravity"),
            GROK_PERMISSIONS_FIELD => permission_choices("grok"),
            "Account" if matches!(self.kind, FormKind::Handoff | FormKind::SwitchAccount) => self
                .target
                .as_ref()
                .map(|item| handoff_account_choices(&state.accounts, &item.session))
                .unwrap_or_default(),
            "Account" => {
                let provider = self.value("Provider");
                let mut out = vec![String::new()];
                out.extend(
                    state
                        .accounts
                        .iter()
                        .filter(|a| a.shared && a.provider == provider)
                        .map(account_handle),
                );
                out
            }
            _ => Vec::new(),
        }
    }

    // ----- drafts (workspace.rs `open_form`, `close_form`) ------------------

    /// Only forms opened fresh keep a draft: the Machine, Account, Profile and
    /// Folder forms when they edit or target nothing.
    pub fn keeps_draft(&self) -> bool {
        matches!(
            self.kind,
            FormKind::Machine | FormKind::Account | FormKind::Profile | FormKind::Folder
        ) && self.editing.is_none()
            && self.target.is_none()
    }

    /// Whether any text field holds something the user typed.
    pub fn has_input(&self) -> bool {
        self.fields.iter().any(|f| {
            matches!(f.kind, FieldKind::Text | FieldKind::Directory) && !f.value.trim().is_empty()
        })
    }

    /// Esc: the values to keep under this kind, when the form keeps drafts and
    /// something was typed (the caller toasts `DRAFT_KEPT`); `None` means
    /// forget any draft kept for the kind.
    pub fn draft(&self) -> Option<(FormKind, Vec<String>)> {
        if self.keeps_draft() && self.has_input() {
            Some((
                self.kind,
                self.fields.iter().map(|f| f.value.clone()).collect(),
            ))
        } else {
            None
        }
    }

    /// A form closed without submitting comes back the way it was left.
    pub fn with_draft(mut self, draft: &[String]) -> Form {
        if self.keeps_draft() && draft.len() == self.fields.len() {
            for (field, value) in self.fields.iter_mut().zip(draft) {
                field.value = value.clone();
            }
        }
        self
    }

    // ----- changing --------------------------------------------------------

    /// The host's text input changed the field at `index`.
    pub fn input(&mut self, index: usize, value: &str) {
        if let Some(field) = self.fields.get_mut(index) {
            field.value = value.to_string();
            self.field_changed();
        }
    }

    /// Focus the field at `index` (a click on it).
    pub fn focus_field(&mut self, index: usize) {
        if index < self.fields.len() && index != self.focus {
            self.focus = index;
            self.field_changed();
        }
    }

    /// `step` fields along, clamped at the ends.
    pub fn move_focus(&mut self, step: i32) {
        let last = self.fields.len() as i32 - 1;
        let next = (self.focus as i32 + step).clamp(0, last) as usize;
        if next != self.focus {
            self.focus = next;
            self.field_changed();
        }
    }

    /// Set a choice field from its dropdown (`PopupAction::Choice`); a new
    /// provider resets what depends on it.
    pub fn apply_choice(&mut self, index: usize, value: &str, state: &State) {
        let Some(field) = self.fields.get_mut(index) else {
            return;
        };
        let provider_changed = field.label == "Provider" && field.value != value;
        field.value = value.to_string();
        if provider_changed {
            self.provider_changed(state);
        }
        self.field_changed();
    }

    /// ← / → on the focused choice field: the next value round.
    pub fn cycle_choice(&mut self, step: i32, state: &State) {
        let label = self.focused().label;
        let choices = self.choices(state, label);
        if choices.is_empty() {
            return;
        }
        let focus = self.focus;
        let current = choices
            .iter()
            .position(|c| *c == self.fields[focus].value)
            .unwrap_or(0) as i32;
        let next = (current + step).rem_euclid(choices.len() as i32) as usize;
        self.fields[focus].value = choices[next].clone();
        if label == "Provider" {
            self.provider_changed(state);
        }
        self.field_changed();
    }

    /// Authentication back to the provider's sign-in, the account to the
    /// provider's default, model, effort and permissions cleared.
    fn provider_changed(&mut self, state: &State) {
        let provider = self.value("Provider");
        let default = default_account(state, &provider);
        for f in &mut self.fields {
            if f.label == "Authentication" {
                f.value = "Provider sign-in".into();
            }
            if f.label == "Account" {
                f.value = default.clone();
            }
            if f.label == MODEL_FIELD || f.label == EFFORT_FIELD || f.label == PERMISSIONS_FIELD {
                f.value.clear();
            }
        }
    }

    /// ‹ / › on the focused model field: the next catalog entry round.
    pub fn cycle_model(&mut self, step: i32) {
        let options = self.model_options();
        let focus = self.focus;
        let current = options
            .iter()
            .position(|o| o.id == self.fields[focus].value)
            .unwrap_or(0) as i32;
        let next = (current + step).rem_euclid(options.len() as i32) as usize;
        self.fields[focus].value = options[next].id.clone();
    }

    /// A click on the directory suggestion at `index`.
    pub fn pick_directory(&mut self, index: usize) {
        if let Some(path) = self.directory.paths.get(index).cloned() {
            let focus = self.focus;
            self.fields[focus].value = path;
            self.field_changed();
        }
    }

    /// A click on the model catalog row at `index`.
    pub fn pick_model(&mut self, index: usize) {
        let options = self.model_options();
        if let Some(option) = options.get(index) {
            let focus = self.focus;
            self.fields[focus].value = option.id.clone();
        }
    }

    /// A value or the focus changed: folders and models follow.
    pub fn field_changed(&mut self) {
        self.schedule_directory();
        self.schedule_models(false);
    }

    /// One key (workspace.rs `handle_form_key`): Tab / ⇧Tab / ↑ / ↓ (and
    /// j / k on a choice) move, ← / → (and h / l on a choice) change a value,
    /// Space opens the dropdown, Enter goes on or submits on the last field,
    /// Esc closes; a Directory field completes with Tab and picks a
    /// suggestion with ⌃N / ⌃P; a Model field reloads with ⌃R.
    pub fn key(&mut self, key: &str, mods: &Mods, state: &State) -> FormOutcome {
        let key = picker::canon(key);
        let key = key.as_str();
        let text = picker::typed(key, mods);
        let kind = self.focused().kind;
        let last = self.fields.len().saturating_sub(1);
        // Left/right (plus h/l where typing is impossible) change a value; up/down
        // and j/k always move between fields.
        if kind == FieldKind::Choice && key == "space" {
            return FormOutcome::OpenChoice(self.focus);
        }
        let step = match kind {
            FieldKind::Choice => match key {
                "left" => Some(-1),
                "right" => Some(1),
                _ => match text.as_deref() {
                    Some("h") => Some(-1),
                    Some("l") => Some(1),
                    _ => None,
                },
            },
            FieldKind::Model => match key {
                "left" => Some(-1),
                "right" => Some(1),
                _ => None,
            },
            _ => None,
        };
        if let Some(step) = step {
            if kind == FieldKind::Model {
                self.cycle_model(step);
            } else {
                self.cycle_choice(step, state);
            }
            return FormOutcome::None;
        }
        if kind == FieldKind::Model && key == "r" && mods.ctrl {
            self.schedule_models(true);
            return FormOutcome::None;
        }
        if kind == FieldKind::Directory {
            let n = self.directory.paths.len();
            match key {
                "n" | "p" if mods.ctrl && n > 0 => {
                    let step = if key == "p" { n - 1 } else { 1 };
                    self.directory.index = (self.directory.index + step) % n;
                    return FormOutcome::None;
                }
                "tab" if !mods.shift && n > 0 => {
                    let focus = self.focus;
                    self.fields[focus].value = self.directory.paths[self.directory.index].clone();
                    self.field_changed();
                    return FormOutcome::None;
                }
                "tab" if !mods.shift && self.directory.loading => return FormOutcome::None,
                _ => {}
            }
        }
        let vertical = if kind == FieldKind::Choice {
            text.as_deref()
        } else {
            None
        };
        let on_last = self.focus >= last;
        match (key, vertical) {
            ("escape", _) => FormOutcome::Close,
            ("enter", _) if !mods.shift => {
                if on_last {
                    FormOutcome::Submit
                } else {
                    self.move_focus(1);
                    FormOutcome::None
                }
            }
            ("tab", _) if !mods.shift => {
                self.move_focus(1);
                FormOutcome::None
            }
            ("tab", _) => {
                self.move_focus(-1);
                FormOutcome::None
            }
            ("down", _) | (_, Some("j")) => {
                self.move_focus(1);
                FormOutcome::None
            }
            ("up", _) | (_, Some("k")) => {
                self.move_focus(-1);
                FormOutcome::None
            }
            // Choice fields swallow stray typing instead of leaking it to the manager.
            _ if kind == FieldKind::Choice && (key == "backspace" || text.is_some()) => {
                FormOutcome::None
            }
            _ => FormOutcome::Ignored,
        }
    }

    /// A click, by the ids the view names: `field:N` focuses, `choice:N`
    /// opens the dropdown, `choice:N:VALUE` (raw value or its label) sets it,
    /// `dir:N` picks a folder suggestion, `model-prev` / `model-next` /
    /// `model-row:N` choose a model, `button:submit` / `button:cancel`.
    pub fn press(&mut self, id: &str, state: &State) -> FormOutcome {
        let (head, arg) = id.split_once(':').unwrap_or((id, ""));
        match head {
            "field" => {
                if let Ok(n) = arg.parse::<usize>() {
                    self.focus_field(n);
                }
                FormOutcome::None
            }
            "choice" => {
                let (n, value) = arg.split_once(':').unwrap_or((arg, ""));
                let Ok(n) = n.parse::<usize>() else {
                    return FormOutcome::Ignored;
                };
                if n >= self.fields.len() {
                    return FormOutcome::Ignored;
                }
                self.focus_field(n);
                if value.is_empty() && !arg.contains(':') {
                    return FormOutcome::OpenChoice(n);
                }
                let label = self.fields[n].label;
                let choices = self.choices(state, label);
                let raw = choices
                    .iter()
                    .find(|c| *c == value || choice_label(state, label, c) == value)
                    .cloned()
                    .unwrap_or_else(|| value.to_string());
                self.apply_choice(n, &raw, state);
                FormOutcome::None
            }
            "dir" => {
                if let Ok(n) = arg.parse::<usize>() {
                    self.pick_directory(n);
                }
                FormOutcome::None
            }
            "model-prev" | "model-next" => {
                if let Some(ix) = self.fields.iter().position(|f| f.kind == FieldKind::Model) {
                    self.focus_field(ix);
                    self.cycle_model(if head == "model-prev" { -1 } else { 1 });
                }
                FormOutcome::None
            }
            "model-row" => {
                if let Ok(n) = arg.parse::<usize>() {
                    self.pick_model(n);
                }
                FormOutcome::None
            }
            "button" => match arg {
                "submit" => FormOutcome::Submit,
                "cancel" => FormOutcome::Close,
                _ => FormOutcome::Ignored,
            },
            _ => FormOutcome::Ignored,
        }
    }

    // ----- folders (workspace.rs `schedule_directory`) ----------------------

    /// Stale matches clear on every change; a focused Directory field asks
    /// for its folders again.
    fn schedule_directory(&mut self) {
        self.directory = DirectoryState::default();
        if self.directory_focused() {
            self.directory.loading = true;
        }
    }

    /// The `fleet directories MACHINE PATH` to run, if the focused Directory
    /// field is waiting for its completions; `directories_requested` marks it
    /// taken.
    pub fn wanted_directories(&self) -> Option<(String, String)> {
        if self.directory_focused() && self.directory.loading && !self.directory.requested {
            Some((self.value("Machine"), self.value("Directory")))
        } else {
            None
        }
    }

    /// The caller started `fleet directories` for the path.
    pub fn directories_requested(&mut self) {
        self.directory.requested = true;
    }

    /// `fleet directories` answered for `machine` and `path`; an answer to an
    /// earlier path, or to a field no longer focused, is dropped.
    pub fn set_directories(
        &mut self,
        machine: &str,
        path: &str,
        result: Result<DirectoryMatches, String>,
    ) {
        if !self.directory_focused()
            || self.value("Machine") != machine
            || self.value("Directory") != path
        {
            return;
        }
        let d = &mut self.directory;
        d.loading = false;
        d.requested = false;
        match result {
            Ok(m) => {
                d.paths = m.paths;
                d.truncated = m.truncated;
            }
            Err(e) => d.error = e,
        }
    }

    // ----- models (workspace.rs `schedule_models`) --------------------------

    /// What the Model field would load for: the form's machine, provider,
    /// account and folder.
    pub fn catalog_key(&self) -> CatalogKey {
        CatalogKey {
            machine: self.value("Machine"),
            provider: self.value("Provider"),
            account: self.value("Account"),
            cwd: self.value("Directory"),
        }
    }

    /// Fetch the model catalog once the model control is focused; other
    /// changes only clear stale options so nothing loads until it is needed.
    /// A catalog loaded for the same key stays visible while `force`
    /// refreshes it.
    fn schedule_models(&mut self, force: bool) {
        let focused = self.model_focused();
        let key = self.catalog_key();
        let fresh = !force && self.catalogs.contains_key(&key);
        if focused && self.models.active && self.models.query.as_ref() == Some(&key) && fresh {
            return;
        }
        let options = self.catalogs.get(&key).cloned().unwrap_or_default();
        self.models = ModelState {
            query: Some(key),
            options,
            ..Default::default()
        };
        if !focused {
            return;
        }
        self.models.active = true;
        self.models.loading = !fresh;
    }

    /// Why the catalog cannot be asked for: the machine or account is not
    /// in the fleet.
    fn catalog_unavailable(&self, state: &State) -> Option<String> {
        let key = self.models.query.as_ref()?;
        if machine(state, &key.machine).is_none() {
            return Some(format!("machine {} not found", key.machine));
        }
        if !key.account.is_empty() && account(state, &key.account).is_none() {
            return Some(format!("account {} not found", key.account));
        }
        None
    }

    /// The `fleet models` to run, if the Model field is waiting for its
    /// catalog on a known machine and account; `catalog_requested` marks it
    /// taken.
    pub fn wanted_catalog(&self, state: &State) -> Option<CatalogKey> {
        if !self.models.loading
            || self.models.requested
            || self.catalog_unavailable(state).is_some()
        {
            return None;
        }
        self.models.query.clone()
    }

    /// The caller started `fleet models` for this key.
    pub fn catalog_requested(&mut self, key: &CatalogKey) {
        if self.models.query.as_ref() == Some(key) {
            self.models.requested = true;
        }
    }

    /// `fleet models` answered; a catalog is remembered for its key so
    /// refocusing the field does not ask again, and a stale answer (another
    /// key) is only remembered.
    pub fn set_models(&mut self, key: &CatalogKey, result: Result<Vec<ModelOption>, String>) {
        if let Ok(options) = &result {
            self.catalogs.insert(key.clone(), options.clone());
        }
        if self.models.query.as_ref() != Some(key) {
            return;
        }
        self.models.loading = false;
        self.models.requested = false;
        match result {
            Ok(options) => {
                self.models.options = options;
                self.models.error.clear();
            }
            Err(error) => self.models.error = error,
        }
    }
}
