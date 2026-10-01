//! What's New and iMessage pairing (workspace.rs `Overlay::WhatsNew`,
//! `Overlay::PairIMessage`, `pair_imessage_open`, `pair_imessage_generate`;
//! ui.rs `render_whats_new`, `render_pair_imessage`).

use super::{Overlay, Reply, Workspace};
use crate::palette::Command;
use crate::picker::Mods;
use crate::whats_new::WhatsNewKey;

impl Workspace {
    /// What's New: Settings closes first (workspace.rs `Command::WhatsNew`).
    pub fn open_whats_new(&mut self) {
        if let Overlay::Settings(page) = &mut self.overlay {
            let effect = page.close();
            self.apply_settings_effect(effect);
        }
        self.overlay = Overlay::WhatsNew;
        self.whats_new.show();
        if !self.whats_new_asked {
            // The history this build bundles: the module reads it (upstream's
            // build script embeds `git log --first-parent -60`).
            self.whats_new_asked = true;
            self.queue(
                "host",
                vec!["whats-new-history".into()],
                String::new(),
                Reply::WhatsNewHistory,
            );
        }
    }

    /// Welcome to Ocho (workspace.rs `show_welcome`): Settings closes
    /// first, as for What's New.
    pub fn show_welcome(&mut self) {
        if let Overlay::Settings(page) = &mut self.overlay {
            let effect = page.close();
            self.apply_settings_effect(effect);
        }
        self.overlay = Overlay::Welcome;
    }

    /// The Welcome panel's fields (ui/welcome_ui.rs): the step buttons' key
    /// hints and the local Claude client's state.
    pub fn welcome_view(&self) -> serde_json::Value {
        use crate::preferences::{setting_bool, SettingItem};
        serde_json::json!({
            "visible": matches!(self.overlay, Overlay::Welcome),
            "claudeOn": setting_bool(&self.settings, SettingItem::RemoteClaudeNative),
            "accountHint": self.hint(Command::AddAccount),
            "machineHint": self.hint(Command::AddMachine),
            "launchHint": self.hint(Command::New),
            "quickHint": self.hint(Command::QuickLaunch),
        })
    }

    /// The history text arrived.
    pub fn whats_new_arrived(&mut self, result: Result<String, String>) {
        let open = self.whats_new.open;
        self.whats_new = crate::whats_new::WhatsNew::new(&result.unwrap_or_default());
        if open {
            self.whats_new.show();
        }
    }

    /// "Pair iMessage…": the phone-number form.
    pub fn open_pair_imessage(&mut self) {
        self.open_form(crate::forms::Form::pair_imessage());
    }

    /// Ask for a pairing code for `phone` on this Mac (workspace.rs `pair_imessage_generate`).
    pub fn pair_imessage_generate(&mut self, phone: String) {
        let local = self
            .state
            .machines
            .iter()
            .find(|m| m.local)
            .map(|m| m.id.clone());
        let (request, argv) = self.imessage.generate(&phone, local.as_deref());
        let reply = if local.is_some() {
            Reply::Pairing(request)
        } else {
            Reply::PairDescribe(request)
        };
        self.overlay = Overlay::PairIMessage;
        self.queue("pair", argv, String::new(), reply);
    }

    /// `serve --describe` answered: now ask for the code.
    pub fn pair_describe_arrived(&mut self, request: u64, result: Result<String, String>) {
        if let Some(argv) = self.imessage.set_describe(request, result) {
            self.queue("pair", argv, String::new(), Reply::Pairing(request));
        }
    }

    /// A key on What's New or the pairing card; `true` when it was one.
    pub(super) fn extras_key(&mut self, name: &str, mods: &Mods) -> bool {
        match self.overlay {
            Overlay::WhatsNew => {
                if let WhatsNewKey::Close = self.whats_new.key(name, self.window.1) {
                    self.overlay = Overlay::None;
                }
                true
            }
            Overlay::PairIMessage => {
                if self.imessage.key(name, mods) {
                    self.overlay = Overlay::None;
                }
                true
            }
            // workspace.rs: Esc, Enter or q closes the pairing and Welcome cards.
            Overlay::PairPhone(_) | Overlay::Welcome => {
                let key = crate::picker::canon(name);
                if matches!(key.as_str(), "escape" | "enter") || (key == "q" && !mods.meta) {
                    self.overlay = Overlay::None;
                }
                true
            }
            _ => false,
        }
    }

    /// A press on What's New or the pairing card; `true` when it was one.
    pub(super) fn extras_press(&mut self, id: &str) -> bool {
        match id {
            "welcome-close" => {
                self.overlay = Overlay::None;
                true
            }
            "welcome-account" | "welcome-machine" | "welcome-launch" | "welcome-quick" => {
                self.overlay = Overlay::None;
                self.execute(match id {
                    "welcome-account" => Command::AddAccount,
                    "welcome-machine" => Command::AddMachine,
                    "welcome-launch" => Command::New,
                    _ => Command::QuickLaunch,
                });
                true
            }
            "welcome-claude-toggle" => {
                use crate::preferences::{set_bool, setting_bool, SettingItem};
                let on = setting_bool(&self.settings, SettingItem::RemoteClaudeNative);
                set_bool(&mut self.settings, SettingItem::RemoteClaudeNative, !on);
                let text = self.settings.to_json_pretty();
                self.host(vec!["save-desktop".into()], text);
                true
            }
            "whats-new-close" | "imessage-close" => {
                self.whats_new.close();
                self.imessage.close();
                self.overlay = Overlay::None;
                true
            }
            "imessage-retry" => {
                let phone = self.imessage.phone.clone();
                self.pair_imessage_generate(phone);
                true
            }
            "imessage-copy" => {
                if let Some(Ok(info)) = &self.imessage.info {
                    let code = info.code.clone();
                    self.host(vec!["clipboard-write".into()], code);
                    self.set_message("Code copied");
                }
                true
            }
            "imessage-open" => {
                if let Some(Ok(info)) = &self.imessage.info {
                    let url = info.url.clone();
                    self.host(vec!["open-url".into(), url], String::new());
                }
                true
            }
            other => {
                if let Some(n) = other
                    .strip_prefix("whats-new-change-")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    if let Some(change) = self.whats_new.changes.get(n) {
                        let url = change.url();
                        self.host(vec!["open-url".into(), url], String::new());
                    }
                    return true;
                }
                false
            }
        }
    }
}
