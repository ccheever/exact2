//! What a submitted form asks the app to do (workspace.rs `submit_form`):
//! a terminal tab running `fleet …`, a background `fleet …` with a toast, a
//! saved profile, or a rail folder. Pair iMessage opens a pairing card
//! instead, which the app reads from [`Form::pairing_phone`].

use super::{normalize_phone, Form, FormKind, PHONE_ERROR};
use crate::model::Page;
use crate::session::{clean, machine, provider_label};
use crate::types::{LaunchProfile, State};

/// What the app executes for a submitted form.
#[derive(Clone, Debug, PartialEq)]
pub enum Submission {
    /// Open a terminal tab running `fleet argv` (`Workspace::open_terminal_tab`).
    Terminal {
        /// The tab key: `add:{request}`, `login:{request}` or
        /// `{machine}:{request}:false`.
        key: String,
        /// The tab title.
        title: String,
        /// The arguments after `fleet`.
        argv: Vec<String>,
        /// What a reopened tab runs.
        reconnect: Vec<String>,
        /// The session the tab attaches to: (machine, request, read-only).
        session: Option<(String, String, bool)>,
        /// The (machine, session) whose attached tabs close first: its
        /// terminal is about to stop, so they would only show a dead TUI.
        replaces: Option<(String, String)>,
    },
    /// Run `fleet argv` in the background and toast `done` (`Workspace::run_cli`).
    Cli {
        /// The arguments after `fleet`.
        argv: Vec<String>,
        /// The toast on success.
        done: String,
        /// The manager page to show meanwhile.
        page: Option<Page>,
        /// The (machine, session) whose tab to bring back: its pane respawns
        /// in place, so the open tab keeps its terminal.
        reveal: Option<(String, String)>,
    },
    /// `fleet profiles add …`: run it with the toast "Profile saved", and
    /// show the profile as saved ahead of the state event that confirms it.
    /// The page stays where it is (#267 removed the Profiles page).
    Profile {
        /// The arguments after `fleet`.
        argv: Vec<String>,
        /// The profile's name.
        name: String,
        /// The name it replaces, when renamed.
        previous: Option<String>,
        /// What was saved.
        saved: LaunchProfile,
    },
    /// Name a rail folder: rename the one with `key`, or a new one.
    Folder {
        /// The folder tab's key, `None` for a new folder.
        key: Option<String>,
        /// Its title.
        title: String,
    },
}

/// The `--permissions` list `fleet machine edit` takes for the machine
/// form's five modes, in `launch::PROVIDERS` order (permissions.rs
/// `machine_spec`): every provider named, a blank one as `default`, so a
/// cleared field removes that provider's default.
pub fn machine_spec(modes: &[String]) -> String {
    let mode = |i: usize| modes.get(i).map(String::as_str).unwrap_or("");
    crate::permissions::machine_spec(mode(0), mode(1), mode(2), mode(3), mode(4))
}

impl Form {
    /// The Pair iMessage form's number (workspace.rs `submit_form`'s
    /// `FormKind::PairIMessage`): `Some(Ok(phone))` normalized, to start
    /// `fleet machine pair-imessage LOCAL --phone PHONE` and show the
    /// pairing card; `Some(Err(PHONE_ERROR))` to show on the form, which
    /// stays open; `None` for every other form.
    pub fn pairing_phone(&self) -> Option<Result<String, String>> {
        if self.kind != FormKind::PairIMessage {
            return None;
        }
        Some(normalize_phone(&self.value(super::PHONE_FIELD)).ok_or_else(|| PHONE_ERROR.into()))
    }

    /// Enter on the last field (workspace.rs `submit_form`), with the request
    /// id `request` for what opens a terminal. An `Err` is the toast; it
    /// closes the form except where `retry_on_error` says otherwise.
    pub fn submit(&self, state: &State, request: &str) -> Result<Submission, String> {
        let values: Vec<String> = self.fields.iter().map(|f| f.value.clone()).collect();
        let machine_name = |id: &str| {
            machine(state, id)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| id.to_string())
        };
        match self.kind {
            FormKind::Folder => Ok(Submission::Folder {
                key: self.editing.clone(),
                title: values[0].clone(),
            }),
            FormKind::Machine => {
                let mut args = vec![
                    "add".to_string(),
                    values[0].clone(),
                    "--name".into(),
                    values[1].clone(),
                    "--install".into(),
                    "--key".into(),
                    "--ssh-args".into(),
                    values[3].clone(),
                ];
                if values[2] == "password" {
                    args.push("--password".into());
                }
                let title = format!(
                    "Add {}",
                    if values[1].is_empty() {
                        &values[0]
                    } else {
                        &values[1]
                    }
                );
                Ok(Submission::Terminal {
                    key: format!("add:{request}"),
                    title,
                    reconnect: args.clone(),
                    argv: args,
                    session: None,
                    replaces: None,
                })
            }
            FormKind::Account => {
                let mut args = vec![
                    "accounts".to_string(),
                    "add".into(),
                    "--provider".into(),
                    values[0].clone(),
                ];
                if matches!(values[0].as_str(), "codex" | "claude")
                    && self.value("Authentication") == "API key"
                {
                    args.push("--api-key".into());
                }
                Ok(Submission::Terminal {
                    key: format!("login:{request}"),
                    title: format!("Sign in · {}", provider_label(&values[0])),
                    reconnect: args.clone(),
                    argv: args,
                    session: None,
                    replaces: None,
                })
            }
            FormKind::RecoveryBackup => Ok(Submission::Cli {
                argv: vec!["cloud".into(), "export-recovery".into(), values[0].clone()],
                done: "Recovery key backed up. Keep the file somewhere safe.".into(),
                page: None,
                reveal: None,
            }),
            // The pairing card is the app's; it asks `pairing_phone` first.
            // Reaching here means it has none, so the form says so.
            FormKind::PairIMessage => match self.pairing_phone() {
                Some(Err(error)) => Err(error),
                _ => Err("Pairing iMessage is not available in this build yet.".into()),
            },
            FormKind::Label => {
                let Some(item) = &self.target else {
                    return Err("No session to label".into());
                };
                Ok(Submission::Cli {
                    argv: vec![
                        "label".into(),
                        item.machine.id.clone(),
                        item.session.id.clone(),
                        values[0].clone(),
                    ],
                    done: "Label saved".into(),
                    page: None,
                    reveal: None,
                })
            }
            FormKind::Migrate => {
                let Some(item) = &self.target else {
                    return Err("No session to move".into());
                };
                let target = values[0].clone();
                if target == item.machine.id {
                    return Err("Choose a different machine to move this session to.".into());
                }
                let mut args = vec![
                    "migrate".to_string(),
                    item.machine.id.clone(),
                    item.session.id.clone(),
                    target.clone(),
                    "--request-id".into(),
                    request.to_string(),
                    "--yes".into(),
                    "--attach".into(),
                ];
                if !values[1].trim().is_empty() {
                    args.push("--cwd".into());
                    args.push(values[1].trim().to_string());
                }
                // The tab carries the session's own name to its new machine, so
                // the move reads as the same session rather than a new one.
                let title = super::session_tab_title(&item.session.title, &machine_name(&target));
                // Reconnecting reuses the request id, which resumes the moved
                // session on its new machine instead of moving it again.
                Ok(Submission::Terminal {
                    key: format!("{target}:{request}:false"),
                    title,
                    argv: args,
                    reconnect: vec!["attach".into(), target.clone(), request.to_string()],
                    session: Some((target, request.to_string(), false)),
                    replaces: Some((item.machine.id.clone(), item.session.id.clone())),
                })
            }
            FormKind::SwitchAccount => {
                let Some(item) = &self.target else {
                    return Err("No session to switch".into());
                };
                let Some(account) = values.first().filter(|value| !value.is_empty()) else {
                    return Err("Choose another Codex account.".into());
                };
                let machine = item.machine.id.clone();
                if item.session.codex_socket.is_empty() {
                    // The pane respawns in place, so an open tab keeps its
                    // terminal; bring it back to watch the conversation resume.
                    return Ok(Submission::Cli {
                        argv: vec![
                            "switch-account".into(),
                            machine.clone(),
                            item.session.id.clone(),
                            "--account".into(),
                            account.clone(),
                            "--yes".into(),
                        ],
                        done: format!("Session resumed under {account}"),
                        page: None,
                        reveal: Some((machine, item.session.id.clone())),
                    });
                }
                // An app-server session is replaced by a new one resuming the
                // same thread; its tabs would only show a closed connection.
                let args = vec![
                    "switch-account".to_string(),
                    machine.clone(),
                    item.session.id.clone(),
                    "--account".into(),
                    account.clone(),
                    "--request-id".into(),
                    request.to_string(),
                    "--yes".into(),
                    "--attach".into(),
                ];
                Ok(Submission::Terminal {
                    key: format!("{machine}:{request}:false"),
                    title: super::session_tab_title(&item.session.title, &item.machine.name),
                    argv: args,
                    reconnect: vec!["attach".into(), machine.clone(), request.to_string()],
                    session: Some((machine.clone(), request.to_string(), false)),
                    replaces: Some((machine, item.session.id.clone())),
                })
            }
            FormKind::Handoff => {
                let Some(item) = &self.target else {
                    return Err("No session to hand off".into());
                };
                let Some(account) = values.first().filter(|value| !value.is_empty()) else {
                    return Err("Choose another account for the handoff.".into());
                };
                let machine = item.machine.id.clone();
                let args = vec![
                    "handoff".to_string(),
                    machine.clone(),
                    item.session.id.clone(),
                    "--account".into(),
                    account.clone(),
                    "--request-id".into(),
                    request.to_string(),
                    "--yes".into(),
                    "--attach".into(),
                ];
                let title = super::session_tab_title(
                    &format!("Take over: {}", clean(&item.session.title)),
                    &item.machine.name,
                );
                // The source is parked before its replacement starts, so its
                // attached tabs should not remain open on a stopped TUI.
                Ok(Submission::Terminal {
                    key: format!("{machine}:{request}:false"),
                    title,
                    argv: args,
                    reconnect: vec!["attach".into(), machine.clone(), request.to_string()],
                    session: Some((machine.clone(), request.to_string(), false)),
                    replaces: Some((machine, item.session.id.clone())),
                })
            }
            FormKind::Resume => {
                if values[4].is_empty() {
                    return Err("No native conversation identity is available.".into());
                }
                let machine = values[0].clone();
                let mut args = vec![
                    "run".to_string(),
                    machine.clone(),
                    "--request-id".into(),
                    request.to_string(),
                    "--provider".into(),
                    values[1].clone(),
                    "--account".into(),
                    values[2].clone(),
                    "--cwd".into(),
                    values[3].clone(),
                    "--resume".into(),
                    values[4].clone(),
                    "--attach".into(),
                ];
                if self
                    .target
                    .as_ref()
                    .is_some_and(|item| item.session.claude_remote)
                {
                    args.push("--claude-remote".into());
                }
                let title = format!(
                    "Resume · {} · {}",
                    provider_label(&values[1]),
                    machine_name(&machine)
                );
                Ok(Submission::Terminal {
                    key: format!("{machine}:{request}:false"),
                    title,
                    reconnect: args.clone(),
                    argv: args,
                    session: Some((machine, request.to_string(), false)),
                    replaces: None,
                })
            }
            FormKind::MachineEdit => {
                let Some(id) = self.editing.clone() else {
                    return Err("No machine to edit".into());
                };
                let name = values[0].trim().to_string();
                if name.is_empty() {
                    return Err("A machine needs a name".into());
                }
                Ok(Submission::Cli {
                    argv: vec![
                        "machine".to_string(),
                        "edit".into(),
                        id,
                        "--name".into(),
                        name,
                        "--notes".into(),
                        values[1].trim().to_string(),
                        "--tags".into(),
                        values[2].trim().to_string(),
                        "--permissions".into(),
                        machine_spec(&values[3..]),
                    ],
                    done: "Machine saved".into(),
                    page: Some(Page::Machines),
                    reveal: None,
                })
            }
            FormKind::Profile => {
                let name = values[0].trim().to_string();
                let mut args = vec![
                    "profiles".to_string(),
                    "add".into(),
                    name.clone(),
                    "--machine".into(),
                    values[2].clone(),
                    "--provider".into(),
                    values[3].clone(),
                    "--account".into(),
                    values[4].clone(),
                    "--cwd".into(),
                    values[5].clone(),
                    "--model".into(),
                    values[6].clone(),
                    "--effort".into(),
                    values[7].clone(),
                    "--prompt".into(),
                    values[9].clone(),
                    "--permissions".into(),
                    values[8].clone(),
                    "--shortcut".into(),
                    values[1].clone(),
                ];
                if let Some(previous) = &self.editing {
                    args.push("--replace".into());
                    args.push(previous.clone());
                }
                let saved = LaunchProfile {
                    provider: values[3].clone(),
                    account: values[4].clone(),
                    cwd: values[5].clone(),
                    model: values[6].clone(),
                    effort: values[7].clone(),
                    permissions: values[8].clone(),
                    prompt: values[9].clone(),
                    machine_id: values[2].clone(),
                    shortcut: values[1].trim().parse().unwrap_or(0),
                };
                let previous = self.editing.clone().filter(|p| p != &name);
                Ok(Submission::Profile {
                    argv: args,
                    name,
                    previous,
                    saved,
                })
            }
        }
    }

    /// A submit error keeps the form open for another try (the machine edit's
    /// missing name, a phone number without a country code); every other
    /// error closes it with the toast.
    pub fn retry_on_error(&self) -> bool {
        matches!(self.kind, FormKind::MachineEdit | FormKind::PairIMessage)
    }
}
