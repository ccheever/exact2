//! A new session from the phone: a machine, an account (its provider), a
//! model and how hard it thinks, and the first message. Fleet serve's
//! `GET …/machines/{m}/models` lists the models the account may use there;
//! `POST …/machines/{m}/launch` starts the session with the message as its
//! prompt, and the phone opens it.

use super::*;

/// Effort levels by provider, as the desktop offers them; "" is the
/// provider's own default.
fn efforts(provider: &str) -> &'static [&'static str] {
    match provider {
        "codex" => &["", "minimal", "low", "medium", "high", "xhigh"],
        "claude" => &["", "low", "medium", "high", "max"],
        _ => &[""],
    }
}

/// An effort level's name.
pub fn effort_name(effort: &str) -> &'static str {
    match effort {
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra High",
        "max" => "Max",
        _ => "Default",
    }
}

/// A provider's name.
pub fn provider_name(provider: &str) -> String {
    match provider {
        "codex" => "Codex".into(),
        "claude" => "Claude".into(),
        other => {
            let mut c = other.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        }
    }
}

/// A model a launch may ask for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelChoice {
    /// What the launch sends.
    pub id: String,
    /// What the picker says.
    pub name: String,
    /// The account's default.
    pub default: bool,
}

/// The new-session screen's choices and its launch.
#[derive(Clone, Debug, Default)]
pub struct Launcher {
    /// Where it runs.
    pub machine: String,
    /// Under which account ("" is the provider's default).
    pub account: String,
    /// The account's provider.
    pub provider: String,
    /// The model ("" is the account's default) and effort.
    pub model: String,
    /// How hard it thinks ("" is the default).
    pub effort: String,
    /// The models offered for `models_for`.
    pub models: Vec<ModelChoice>,
    models_for: (String, String, String),
    /// The models are being asked for.
    pub models_loading: bool,
    /// The launch in flight: its request id, kept for a retry.
    request_id: String,
    prompt: String,
    /// A launch is out.
    pub launching: bool,
    /// Why the last launch failed.
    pub error: String,
    /// The session to open once launched: (machine, session id).
    pub goto: Option<(String, String)>,
}

impl Model {
    /// The screen opened: choose where and as whom, keeping earlier choices
    /// that still stand.
    pub fn compose_open(&mut self) {
        let machines = self.launch_machines();
        if !machines.iter().any(|(id, _)| *id == self.launcher.machine) {
            // Home if it answers, else whichever machine answers for the phone.
            let home = self.home().to_string();
            let live = |id: &str| {
                self.fleet
                    .as_ref()
                    .and_then(|f| f.machines.get(id))
                    .is_some_and(|m| !m.stale)
            };
            self.launcher.machine = if live(&home) {
                home
            } else if machines.iter().any(|(id, _)| *id == self.via) {
                self.via.clone()
            } else {
                machines
                    .first()
                    .map(|(id, _)| id.clone())
                    .unwrap_or_default()
            };
        }
        let accounts = self.launch_accounts();
        if !accounts
            .iter()
            .any(|a| a.name == self.launcher.account && a.provider == self.launcher.provider)
        {
            let first = accounts
                .iter()
                .find(|a| a.provider == "codex")
                .or(accounts.first())
                .cloned()
                .unwrap_or_default();
            self.launcher.account = first.name;
            self.launcher.provider = first.provider;
            self.launcher.model.clear();
            self.launcher.effort.clear();
        }
        self.launcher.error.clear();
        self.want_models();
        self.version += 1;
    }

    /// The machines a session can start on: (id, name), the fleet's order.
    pub fn launch_machines(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .fleet
            .as_ref()
            .map(|f| {
                // Machines a person launches on: answering, and not one of
                // Fleet's own pool machines (`fleet-…`).
                f.machines
                    .iter()
                    .filter(|(id, m)| {
                        **id == self.launcher.machine || (!m.stale && !m.name.starts_with("fleet-"))
                    })
                    .map(|(id, m)| {
                        (
                            id.clone(),
                            if m.name.is_empty() {
                                id.clone()
                            } else {
                                m.name.clone()
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by_key(|a| a.1.to_lowercase());
        out
    }

    /// The accounts to choose from: the fleet's, or the providers' defaults
    /// from a server that names none.
    pub fn launch_accounts(&self) -> Vec<crate::fleet::Account> {
        let named = self
            .fleet
            .as_ref()
            .map(|f| f.accounts.clone())
            .unwrap_or_default();
        if !named.is_empty() {
            return named;
        }
        ["codex", "claude"]
            .iter()
            .map(|p| crate::fleet::Account {
                name: String::new(),
                provider: p.to_string(),
                email: String::new(),
            })
            .collect()
    }

    /// A choice made on the screen: `machine`, `account` (`provider/name`),
    /// `model`, or `effort`.
    pub fn compose_choose(&mut self, what: &str, value: &str) {
        match what {
            "machine" => {
                self.launcher.machine = value.to_string();
                self.launcher.model.clear();
            }
            "account" => {
                let (provider, name) = value.split_once('/').unwrap_or((value, ""));
                self.launcher.provider = provider.to_string();
                self.launcher.account = name.to_string();
                self.launcher.model.clear();
                if !efforts(provider).contains(&self.launcher.effort.as_str()) {
                    self.launcher.effort.clear();
                }
            }
            "model" => self.launcher.model = value.to_string(),
            "effort" => self.launcher.effort = value.to_string(),
            _ => return,
        }
        self.want_models();
        self.version += 1;
    }

    /// Ask for the models when the machine or account changed.
    fn want_models(&mut self) {
        let key = (
            self.launcher.machine.clone(),
            self.launcher.provider.clone(),
            self.launcher.account.clone(),
        );
        if key.0.is_empty() || key == self.launcher.models_for {
            return;
        }
        self.launcher.models_for = key;
        self.launcher.models.clear();
        self.launcher.models_loading = true;
        self.models.inflight = false;
        self.models.bump();
    }

    /// The models request: URL and bearer.
    pub fn models_request(&mut self) -> Option<(String, String)> {
        let conn = self.conn.as_ref()?;
        let (machine, provider, account) = self.launcher.models_for.clone();
        if machine.is_empty() {
            return None;
        }
        let mut path = format!(
            "/machines/{}/models?provider={}",
            crate::api::encode(&machine),
            crate::api::encode(&provider)
        );
        if !account.is_empty() {
            path.push_str(&format!("&account={}", crate::api::encode(&account)));
        }
        let url = conn.url(&self.route_of(&self.via), &path);
        self.models.start(self.now);
        Some((url, conn.bearer()))
    }

    /// The models answered (a list of `{id, name, default}`), or didn't:
    /// the picker then offers the default alone.
    pub fn models_done(&mut self, result: Result<serde_json::Value, String>) {
        self.models.inflight = false;
        self.launcher.models_loading = false;
        self.launcher.models = result
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(|m| {
                let text = |k: &str| m.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                ModelChoice {
                    name: if text("name").is_empty() {
                        text("id")
                    } else {
                        text("name")
                    },
                    id: text("id"),
                    default: m.get("default").and_then(|v| v.as_bool()).unwrap_or(false),
                }
            })
            .filter(|m| !m.id.is_empty())
            .collect();
        if !self
            .launcher
            .models
            .iter()
            .any(|m| m.id == self.launcher.model)
        {
            self.launcher.model.clear();
        }
        self.version += 1;
    }

    /// The model's name as the screen shows it.
    pub fn launch_model_name(&self) -> String {
        let pick = |m: &&ModelChoice| {
            if self.launcher.model.is_empty() {
                m.default
            } else {
                m.id == self.launcher.model
            }
        };
        self.launcher
            .models
            .iter()
            .find(pick)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| {
                if self.launcher.model.is_empty() {
                    "Default model".into()
                } else {
                    self.launcher.model.clone()
                }
            })
    }

    /// The effort levels the chosen provider takes.
    pub fn launch_efforts(&self) -> &'static [&'static str] {
        efforts(&self.launcher.provider)
    }

    /// Send pressed on the new-session screen: launch with `text` as the
    /// prompt.
    pub fn compose_send(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() || self.launcher.launching || self.launcher.machine.is_empty() {
            return;
        }
        self.sent_count += 1;
        self.launcher.request_id = format!("{:016x}{:016x}", self.now as u64, self.sent_count);
        self.launcher.prompt = text.to_string();
        self.launcher.launching = true;
        self.launcher.error.clear();
        self.launch.inflight = false;
        self.launch.bump();
        self.feel("light");
        self.version += 1;
    }

    /// Where the first message's files upload before there is a session:
    /// the chosen machine's paste folder (`…/machines/{m}/upload`).
    pub fn launch_upload(&self) -> Option<(String, String)> {
        let conn = self.conn.as_ref()?;
        if self.launcher.machine.is_empty() {
            return None;
        }
        let url = conn.url(
            &self.route_of(&self.via),
            &format!(
                "/machines/{}/upload",
                crate::api::encode(&self.launcher.machine)
            ),
        );
        Some((url, conn.bearer()))
    }

    /// The launch request: URL, bearer, JSON body.
    pub fn launch_request(&mut self) -> Option<(String, String, String)> {
        let conn = self.conn.as_ref()?;
        let l = &self.launcher;
        if !l.launching {
            return None;
        }
        let mut body = serde_json::json!({
            "request_id": l.request_id,
            "provider": l.provider,
            "cwd": "~",
            "prompt": l.prompt,
        });
        for (key, value) in [
            ("account", &l.account),
            ("model", &l.model),
            ("effort", &l.effort),
        ] {
            if !value.is_empty() {
                body[key] = value.clone().into();
            }
        }
        let url = conn.url(
            &self.route_of(&self.via),
            &format!("/machines/{}/launch", crate::api::encode(&l.machine)),
        );
        let bearer = conn.bearer();
        self.launch.start(self.now);
        Some((url, bearer, body.to_string()))
    }

    /// The launch answered `{request_id, session}`: open it. Otherwise say
    /// why, and keep the message to try again.
    pub fn launch_done(&mut self, result: Result<serde_json::Value, String>) {
        self.launch.inflight = false;
        self.launcher.launching = false;
        match result {
            Ok(reply) => match reply.pointer("/session/id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => {
                    self.launcher.goto = Some((self.launcher.machine.clone(), id.to_string()));
                    self.launcher.prompt.clear();
                    self.feel("success");
                    self.track(
                        "launch.result",
                        false,
                        vec![
                            ("ok", true.into()),
                            ("provider", self.launcher.provider.clone().into()),
                        ],
                    );
                    // The fleet answers with the new session soon after.
                    self.poll.next_at = 0.0;
                }
                _ => self.launch_failed("The machine didn't say which session it started.".into()),
            },
            Err(why) => self.launch_failed(why),
        }
        self.version += 1;
    }

    fn launch_failed(&mut self, why: String) {
        self.launcher.error = why;
        self.feel("error");
        self.track(
            "launch.result",
            true,
            vec![
                ("ok", false.into()),
                ("provider", self.launcher.provider.clone().into()),
            ],
        );
    }

    /// The navigation took the launched session: forget it.
    pub fn goto_done(&mut self) {
        self.launcher.goto = None;
        self.version += 1;
    }

    /// The saved drafts, at launch.
    pub fn load_drafts(&mut self, json: Option<&str>) {
        if let Some(drafts) = json.and_then(|j| serde_json::from_str(j).ok()) {
            self.drafts = drafts;
        }
    }

    /// A conversation's draft, kept per session (and across launches).
    pub fn draft_for(&self, key: &Key) -> String {
        self.drafts
            .get(&format!("{}/{}", key.0, key.1))
            .cloned()
            .unwrap_or_default()
    }

    /// The open conversation's draft changed (the composer lost focus, or a
    /// message went).
    pub fn draft_written(&mut self, text: &str) {
        let Some(key) = self.open.clone() else { return };
        let id = format!("{}/{}", key.0, key.1);
        if text.trim().is_empty() {
            if self.drafts.remove(&id).is_none() {
                return;
            }
        } else if self.drafts.get(&id).map(String::as_str) == Some(text) {
            return;
        } else {
            self.drafts.insert(id, text.to_string());
        }
        self.writes
            .push((KEY_DRAFTS, serde_json::to_string(&self.drafts).ok()));
    }
}
