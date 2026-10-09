//! Per-viewer desktop settings in `$FLEET_HOME/desktop.json`: theme choice,
//! the windows and terminal tabs to restore on the next launch, and the last
//! launch dialog choices plus recently used project folders. A port of the
//! GPUI desktop's `settings.rs` (origin/main e6adfa8) without the file I/O:
//! the app's module reads and writes the file, this parses and serializes
//! its text.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One row of the sidebar's TERMINALS tree as saved.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SavedTab {
    /// `"{machine}:{session|request}:{read_only}"`, `"shell:{id}"`,
    /// `"login:{…}"`, `"auto:{req}"`, `"fly:{…}"` or `"folder:{…}"`.
    pub key: String,
    /// The rail row's title.
    pub title: String,
    /// The `fleet` argv that reopens the terminal.
    pub reconnect: Vec<String>,
    /// (machine id, session id, read-only) for tabs that follow a session.
    pub session: Option<(String, String, bool)>,
    /// Machine the terminal runs on, when known.
    pub machine: Option<String>,
    /// Nesting level in the sidebar tree; 0 is a top-level tab.
    pub depth: usize,
    /// Whether this tab's children are hidden in the sidebar.
    pub collapsed: bool,
    /// A sidebar folder: no terminal, only a title and nested rows.
    pub folder: bool,
}

/// One window's tabs and selection.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SavedWindow {
    /// The rail rows, pre-order.
    pub tabs: Vec<SavedTab>,
    /// 0 for the manager, `i + 1` for `tabs[i]`.
    pub active: usize,
    /// The rail's width in px, when it was resized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rail_width: Option<f64>,
}

/// The last launch dialog choices; the next dialog opens with these filled in.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LaunchDefaults {
    /// Machine id.
    pub machine: String,
    /// Provider (`claude`, `codex`, …).
    pub provider: String,
    /// Account email.
    pub account: String,
    /// Working directory.
    pub cwd: String,
    /// Model id.
    pub model: String,
    /// Effort level.
    pub effort: String,
    /// A per-launch permission override; empty takes the machine default.
    pub permissions: String,
}

/// A folder that was launched into, on a specific machine. Most recent first.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RecentProject {
    /// Machine id.
    pub machine: String,
    /// The folder, without a trailing slash.
    pub cwd: String,
}

/// How many recent projects are kept.
pub const RECENT_PROJECT_LIMIT: usize = 20;
/// How many recent models are kept.
pub const RECENT_MODEL_LIMIT: usize = 6;
/// How many model catalogs are kept.
pub const MODEL_CATALOG_LIMIT: usize = 32;

/// A model that was launched with, shown at the top of the model list.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RecentModel {
    /// Provider.
    pub provider: String,
    /// Model id, the launch value.
    pub id: String,
    /// Display name.
    pub name: String,
}

/// Key of the remembered effort for a harness and model.
pub fn effort_key(provider: &str, model: &str) -> String {
    format!("{provider}/{model}")
}

/// One model a provider's CLI offers (backend.rs `ModelOption`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ModelOption {
    /// Model id, the launch value.
    #[serde(default)]
    pub id: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// The provider's blurb.
    #[serde(default)]
    pub description: String,
    /// The provider's default.
    #[serde(default)]
    pub default: bool,
}

/// Last successful catalogs survive window/app restarts. These are suggestions,
/// not permission checks; the provider remains authoritative at launch.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SavedModelCatalog {
    /// Machine id.
    pub machine: String,
    /// Provider.
    pub provider: String,
    /// Account email.
    pub account: String,
    /// Working directory the catalog was fetched for.
    pub cwd: String,
    /// The models.
    pub options: Vec<ModelOption>,
    /// Unix seconds.
    pub fetched_at: u64,
    /// CLI version observed on this machine when the catalog was fetched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_version: Option<String>,
}

/// Quick launch (⌘⇧N) numbering: identities keep their slot for good
/// (quick_launch.rs `Shortcuts`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Shortcuts {
    /// Account emails, slot order.
    pub accounts: Vec<String>,
    /// Machine ids, slot order.
    pub machines: Vec<String>,
}

/// The whole of `desktop.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DesktopSettings {
    /// Quick launch slots.
    pub launch_shortcuts: Shortcuts,
    /// Cached model catalogs, newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_catalogs: Vec<SavedModelCatalog>,
    /// The chosen theme's name; None is the bundled one for the appearance.
    pub theme: Option<String>,
    /// UI font size in points (default 13); padding and widths scale with it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_font_size: Option<f64>,
    /// Terminal font size in points (default 13).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_font_size: Option<f64>,
    /// Missing means on, preserving notifications for existing installations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_turn_complete: Option<bool>,
    /// Missing means on, preserving notifications for existing installations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_input_required: Option<bool>,
    /// Force SSH for remote terminals, including explicit mosh requests.
    pub disable_mosh: bool,
    /// Missing defaults to on unless the environment explicitly disables it;
    /// an explicit UI choice always wins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_codex_app_server: Option<bool>,
    /// Same for the local Claude client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_claude_native: Option<bool>,
    /// Every window's tabs; None until the first multi-window save.
    pub windows: Option<Vec<SavedWindow>>,
    /// The last launch dialog choices.
    pub launch: Option<LaunchDefaults>,
    /// Folders launched into, most recent first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_projects: Vec<RecentProject>,
    /// Models launched with, most recent first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_models: Vec<RecentModel>,
    /// Last effort per harness/model, so picking a model brings its effort back.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub model_efforts: HashMap<String, String>,
    /// Legacy single-window state, read only until the first multi-window save.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<SavedTab>,
    /// Legacy single-window selection.
    #[serde(skip_serializing_if = "is_zero")]
    pub active: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

impl DesktopSettings {
    /// The settings in `text`, JSONC tolerated (comments, trailing commas);
    /// unreadable text is the defaults, as the desktop treats a missing file.
    pub fn parse(text: &str) -> DesktopSettings {
        serde_json::from_str(&strip_jsonc(text)).unwrap_or_default()
    }

    /// The settings in `text`, or why they could not be read.
    /// Nothing has been saved yet: a fresh install, not an upgrade
    /// (settings.rs `is_first_launch`).
    pub fn is_first_launch(&self) -> bool {
        self.windows.is_none() && self.tabs.is_empty()
    }

    pub fn try_parse(text: &str) -> Result<DesktopSettings, String> {
        serde_json::from_str(&strip_jsonc(text)).map_err(|e| e.to_string())
    }

    /// The file's text, pretty-printed as the desktop writes it.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Keep a catalog, replacing the one for the same query; at most 32.
    pub fn remember_model_catalog(&mut self, catalog: SavedModelCatalog) {
        self.model_catalogs.retain(|c| {
            c.machine != catalog.machine
                || c.provider != catalog.provider
                || c.account != catalog.account
                || c.cwd != catalog.cwd
        });
        self.model_catalogs.insert(0, catalog);
        self.model_catalogs.truncate(MODEL_CATALOG_LIMIT);
    }

    /// The cached catalog for a query, if any.
    pub fn model_catalog(
        &self,
        machine: &str,
        provider: &str,
        account: &str,
        cwd: &str,
    ) -> Option<&SavedModelCatalog> {
        self.model_catalogs.iter().find(|c| {
            c.machine == machine && c.provider == provider && c.account == account && c.cwd == cwd
        })
    }

    /// Whether remote Codex sessions use a local client: the explicit choice,
    /// else on unless the environment says `FLEET_CODEX_APP_SERVER=0`.
    pub fn remote_codex_app_server(&self) -> bool {
        self.remote_codex_app_server
            .unwrap_or_else(|| std::env::var("FLEET_CODEX_APP_SERVER").as_deref() != Ok("0"))
    }

    /// Whether remote Claude sessions use a local client: the explicit
    /// choice, else the `FLEET_CLAUDE_REMOTE=1` environment opt-in.
    pub fn remote_claude_native(&self) -> bool {
        self.remote_claude_native
            .unwrap_or_else(|| std::env::var("FLEET_CLAUDE_REMOTE").as_deref() == Ok("1"))
    }

    /// Completed-turn notifications, on unless switched off.
    pub fn notify_turn_complete(&self) -> bool {
        self.notify_turn_complete.unwrap_or(true)
    }

    /// Input-required notifications, on unless switched off.
    pub fn notify_input_required(&self) -> bool {
        self.notify_input_required.unwrap_or(true)
    }

    /// The windows to open at launch: the saved ones, one empty window when
    /// every window was closed, or the legacy single window.
    pub fn restored_windows(&self) -> Vec<SavedWindow> {
        match &self.windows {
            Some(windows) if !windows.is_empty() => windows.clone(),
            Some(_) => vec![SavedWindow::default()],
            None => vec![SavedWindow {
                tabs: self.tabs.clone(),
                active: self.active,
                rail_width: None,
            }],
        }
    }

    /// Record a launch: its choices become the next dialog's defaults, its
    /// folder moves to the front of the recent projects, a named model moves
    /// to the front of the recent models, and its effort is kept for that model.
    pub fn remember_launch(&mut self, defaults: LaunchDefaults, model_name: &str) {
        if !defaults.model.is_empty() {
            self.recent_models
                .retain(|m| !(m.provider == defaults.provider && m.id == defaults.model));
            self.recent_models.insert(
                0,
                RecentModel {
                    provider: defaults.provider.clone(),
                    id: defaults.model.clone(),
                    name: model_name.to_string(),
                },
            );
            self.recent_models.truncate(RECENT_MODEL_LIMIT);
        }
        self.model_efforts.insert(
            effort_key(&defaults.provider, &defaults.model),
            defaults.effort.clone(),
        );
        let project = RecentProject {
            machine: defaults.machine.clone(),
            cwd: defaults.cwd.trim_end_matches('/').to_string(),
        };
        if !project.cwd.is_empty() {
            self.recent_projects.retain(|p| {
                !(p.machine == project.machine && p.cwd.trim_end_matches('/') == project.cwd)
            });
            self.recent_projects.insert(0, project);
            self.recent_projects.truncate(RECENT_PROJECT_LIMIT);
        }
        self.launch = Some(defaults);
    }

    /// The remembered effort for a harness and model, if any.
    pub fn remembered_effort(&self, provider: &str, model: &str) -> Option<&str> {
        self.model_efforts
            .get(&effort_key(provider, model))
            .map(String::as_str)
    }

    /// Replace the saved windows. Closing the last window retains its tabs
    /// for the next launch.
    pub fn set_windows(&mut self, windows: Vec<SavedWindow>) {
        if windows.is_empty() {
            return;
        }
        self.windows = Some(windows);
        self.tabs.clear();
        self.active = 0;
    }
}

/// JSON with `//` and `/* */` comments and trailing commas removed, so a
/// hand-edited file still reads (theme.rs `strip_jsonc`).
pub fn strip_jsonc(text: &str) -> String {
    remove_trailing_commas(&remove_comments(text))
}

fn remove_comments(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
        } else if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn remove_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
        } else if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if !(j < chars.len() && (chars[j] == '}' || chars[j] == ']')) {
                out.push(c);
            }
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_replace_the_same_query_and_stay_bounded() {
        let catalog = |models: &[&str]| SavedModelCatalog {
            machine: "remote".into(),
            provider: "codex".into(),
            account: "work".into(),
            cwd: "~".into(),
            options: models
                .iter()
                .map(|id| ModelOption {
                    id: (*id).into(),
                    ..Default::default()
                })
                .collect(),
            fetched_at: 1,
            provider_version: Some("codex-cli 0.154.0".into()),
        };
        let mut settings = DesktopSettings::default();
        settings.remember_model_catalog(catalog(&["old", "kept"]));
        settings.remember_model_catalog(catalog(&["kept", "added"]));
        let saved = serde_json::to_string(&settings).unwrap();
        let mut restored: DesktopSettings = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            restored.model_catalogs[0].provider_version.as_deref(),
            Some("codex-cli 0.154.0")
        );
        assert_eq!(restored.model_catalogs.len(), 1);
        assert_eq!(restored.model_catalogs[0].options.len(), 2);
        assert!(restored
            .model_catalog("remote", "codex", "work", "~")
            .is_some());
        assert!(restored
            .model_catalog("remote", "codex", "work", "/")
            .is_none());
        let mut legacy: serde_json::Value = serde_json::from_str(&saved).unwrap();
        legacy["model_catalogs"][0]
            .as_object_mut()
            .unwrap()
            .remove("provider_version");
        let legacy: DesktopSettings = serde_json::from_value(legacy).unwrap();
        assert_eq!(legacy.model_catalogs[0].provider_version, None);
        for index in 0..40 {
            let mut next = catalog(&["kept"]);
            next.account = format!("account-{index}");
            restored.remember_model_catalog(next);
        }
        assert_eq!(restored.model_catalogs.len(), 32);
    }

    fn window(key: &str) -> SavedWindow {
        SavedWindow {
            tabs: vec![SavedTab {
                key: key.into(),
                reconnect: vec!["attach".into(), "machine".into(), key.into()],
                ..Default::default()
            }],
            active: 1,
            rail_width: None,
        }
    }

    #[test]
    fn legacy_tabs_migrate_into_one_window_without_changing_theme() {
        let mut settings: DesktopSettings = serde_json::from_str(
            r#"{"theme":"Ocho Dark","tabs":[{"key":"machine:session:false","reconnect":["attach","machine","session"]}],"active":1}"#,
        ).unwrap();
        let restored = settings.restored_windows();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].tabs, settings.tabs);
        assert_eq!(restored[0].active, 1);
        settings.set_windows(restored.clone());
        assert_eq!(settings.theme.as_deref(), Some("Ocho Dark"));
        assert!(settings.tabs.is_empty());
        assert_eq!(settings.active, 0);
        let saved = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<DesktopSettings>(&saved)
                .unwrap()
                .restored_windows(),
            restored
        );
    }

    #[test]
    fn multiple_windows_keep_separate_tabs_and_selections() {
        let mut first = window("first");
        first.active = 0;
        first.rail_width = Some(312.0);
        let second = window("second");
        let empty = SavedWindow::default();
        let mut settings = DesktopSettings::default();
        settings.set_windows(vec![first.clone(), second.clone(), empty.clone()]);
        let encoded = serde_json::to_string(&settings).unwrap();
        let mut restored: DesktopSettings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            restored.restored_windows(),
            vec![first, second.clone(), empty]
        );

        // Saving the remaining windows drops a closed window, without carrying
        // its tabs into its neighbor. Last-window close retains the last state.
        restored.set_windows(vec![second.clone()]);
        restored.set_windows(vec![]);
        assert_eq!(restored.restored_windows(), vec![second]);
    }

    #[test]
    fn window_state_takes_precedence_over_legacy_tabs() {
        let mut settings = DesktopSettings {
            tabs: window("legacy").tabs,
            active: 1,
            windows: Some(vec![window("current")]),
            ..Default::default()
        };
        assert_eq!(settings.restored_windows(), vec![window("current")]);
        settings.windows = Some(vec![]);
        assert_eq!(settings.restored_windows(), vec![SavedWindow::default()]);
        assert_eq!(
            DesktopSettings::default().restored_windows(),
            vec![SavedWindow::default()]
        );
    }

    #[test]
    fn disable_mosh_defaults_off_and_survives_restart() {
        assert!(!DesktopSettings::default().disable_mosh);
        let legacy: DesktopSettings = serde_json::from_str(r#"{"theme":"Ocho Dark"}"#).unwrap();
        assert!(!legacy.disable_mosh);
        for enabled in [true, false] {
            let settings = DesktopSettings {
                disable_mosh: enabled,
                ..Default::default()
            };
            let saved = serde_json::to_string(&settings).unwrap();
            let restored: DesktopSettings = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.disable_mosh, enabled);
        }
    }

    #[test]
    fn notifications_default_on_and_round_trip_explicitly_off() {
        let defaults = DesktopSettings::default();
        assert!(defaults.notify_turn_complete());
        assert!(defaults.notify_input_required());

        let settings: DesktopSettings =
            serde_json::from_str(r#"{"notify_turn_complete":false,"notify_input_required":false}"#)
                .unwrap();
        assert!(!settings.notify_turn_complete());
        assert!(!settings.notify_input_required());
    }

    #[test]
    fn remote_codex_setting_defaults_on_and_preserves_explicit_choices() {
        let legacy: DesktopSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.remote_codex_app_server, None);
        assert_eq!(
            legacy.remote_codex_app_server(),
            std::env::var("FLEET_CODEX_APP_SERVER").as_deref() != Ok("0")
        );
        assert!(!serde_json::to_string(&legacy)
            .unwrap()
            .contains("remote_codex_app_server"));
        for enabled in [true, false] {
            let settings = DesktopSettings {
                remote_codex_app_server: Some(enabled),
                ..Default::default()
            };
            let saved = serde_json::to_string(&settings).unwrap();
            let restored: DesktopSettings = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.remote_codex_app_server, Some(enabled));
            assert_eq!(restored.remote_codex_app_server(), enabled);
        }
    }

    #[test]
    fn remote_claude_setting_roundtrips_explicit_choices() {
        let legacy: DesktopSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.remote_claude_native, None);
        assert_eq!(
            legacy.remote_claude_native(),
            std::env::var("FLEET_CLAUDE_REMOTE").as_deref() == Ok("1")
        );
        for enabled in [true, false] {
            let settings = DesktopSettings {
                remote_claude_native: Some(enabled),
                ..Default::default()
            };
            let saved = serde_json::to_string(&settings).unwrap();
            let restored: DesktopSettings = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.remote_claude_native(), enabled);
        }
    }

    #[test]
    fn remembered_launches_track_recent_models_and_their_effort() {
        let mut settings = DesktopSettings::default();
        let launch = |model: &str, effort: &str| LaunchDefaults {
            provider: "claude".into(),
            model: model.into(),
            effort: effort.into(),
            cwd: "~".into(),
            ..Default::default()
        };
        settings.remember_launch(launch("", "high"), "Default");
        assert!(settings.recent_models.is_empty());
        assert_eq!(settings.model_efforts["claude/"], "high");
        settings.remember_launch(launch("opus", "max"), "Opus");
        settings.remember_launch(launch("sonnet", ""), "Sonnet");
        settings.remember_launch(launch("opus", "low"), "Opus");
        let names: Vec<&str> = settings
            .recent_models
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        assert_eq!(names, vec!["Opus", "Sonnet"]);
        assert_eq!(settings.model_efforts["claude/opus"], "low");
        assert_eq!(settings.model_efforts["claude/sonnet"], "");
        assert_eq!(settings.remembered_effort("claude", "opus"), Some("low"));
        assert_eq!(settings.remembered_effort("codex", "opus"), None);
        for i in 0..(RECENT_MODEL_LIMIT + 2) {
            settings.remember_launch(launch(&format!("m{i}"), ""), "m");
        }
        assert_eq!(settings.recent_models.len(), RECENT_MODEL_LIMIT);
    }

    #[test]
    fn remembered_launches_dedupe_and_cap_recent_projects() {
        let mut settings = DesktopSettings::default();
        let launch = |machine: &str, cwd: &str| LaunchDefaults {
            machine: machine.into(),
            provider: "claude".into(),
            cwd: cwd.into(),
            ..Default::default()
        };
        settings.remember_launch(launch("studio", "~/a/"), "");
        settings.remember_launch(launch("laptop", "~/a"), "");
        settings.remember_launch(launch("studio", "~/a"), "");
        assert_eq!(
            settings
                .recent_projects
                .iter()
                .map(|p| format!("{}:{}", p.machine, p.cwd))
                .collect::<Vec<_>>(),
            vec!["studio:~/a", "laptop:~/a"]
        );
        assert_eq!(settings.launch.as_ref().unwrap().provider, "claude");

        for i in 0..(RECENT_PROJECT_LIMIT + 5) {
            settings.remember_launch(launch("studio", &format!("~/p{i}")), "");
        }
        assert_eq!(settings.recent_projects.len(), RECENT_PROJECT_LIMIT);
        assert_eq!(settings.recent_projects[0].cwd, "~/p24");

        // An empty folder still updates the defaults but is not a project.
        settings.remember_launch(launch("studio", ""), "");
        assert_eq!(settings.recent_projects.len(), RECENT_PROJECT_LIMIT);
        assert_eq!(settings.launch.as_ref().unwrap().cwd, "");

        let encoded = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<DesktopSettings>(&encoded).unwrap(),
            settings
        );
        assert!(!serde_json::to_string(&DesktopSettings::default())
            .unwrap()
            .contains("recent_projects"));
    }

    #[test]
    fn settings_round_trip_and_tolerate_unknown_keys() {
        let mut s = DesktopSettings::parse("{\"theme\": \"Ayu Mirage\", \"future\": 1,}");
        assert_eq!(s.theme.as_deref(), Some("Ayu Mirage"));
        s.remote_codex_app_server = Some(true);
        s.disable_mosh = true;
        s.tabs.push(SavedTab {
            key: "m:s:false".into(),
            title: "t".into(),
            reconnect: vec!["attach".into(), "m".into(), "s".into()],
            session: Some(("m".into(), "s".into(), false)),
            machine: Some("m".into()),
            depth: 1,
            collapsed: true,
            folder: false,
        });
        s.active = 1;
        let text = s.to_json_pretty();
        assert_eq!(DesktopSettings::parse(&text), s);
        assert_eq!(DesktopSettings::try_parse(&text).unwrap(), s);
    }

    #[test]
    fn jsonc_comments_and_trailing_commas_are_tolerated() {
        let text = r#"{
            // the theme
            "theme": "Ocho Light", /* inline */
            "windows": [
                {"tabs": [], "active": 0,},
            ],
            "recent_projects": [{"machine": "m", "cwd": "~/x // not a comment",},],
        }"#;
        let s = DesktopSettings::parse(text);
        assert_eq!(s.theme.as_deref(), Some("Ocho Light"));
        assert_eq!(s.windows.as_ref().map(Vec::len), Some(1));
        assert_eq!(s.recent_projects[0].cwd, "~/x // not a comment");
        assert_eq!(strip_jsonc(r#"{"a": "b,}" ,}"#), r#"{"a": "b,}" }"#);
        assert_eq!(strip_jsonc("[1, 2, /* x */ 3,\n]"), "[1, 2,  3\n]");
        // Broken text reads as the defaults, as a missing file does.
        assert_eq!(DesktopSettings::parse("{nope"), DesktopSettings::default());
        assert!(DesktopSettings::try_parse("{nope").is_err());
        assert_eq!(DesktopSettings::parse(""), DesktopSettings::default());
    }

    #[test]
    fn saved_tabs_round_trip_as_the_desktop_writes_them() {
        let tab = SavedTab {
            key: "studio:original:true".into(),
            title: "Agent · studio".into(),
            reconnect: vec![
                "attach".into(),
                "studio".into(),
                "original".into(),
                "--read-only".into(),
            ],
            session: Some(("studio".into(), "original".into(), true)),
            machine: Some("studio".into()),
            depth: 2,
            collapsed: false,
            folder: false,
        };
        let json = serde_json::to_value(&tab).unwrap();
        assert_eq!(
            json["session"],
            serde_json::json!(["studio", "original", true])
        );
        assert_eq!(json["reconnect"][3], "--read-only");
        let back: SavedTab = serde_json::from_value(json).unwrap();
        assert_eq!(back, tab);
        // A folder needs no terminal fields.
        let folder: SavedTab = serde_json::from_str(
            r#"{"key":"folder:1","title":"Work","folder":true,"collapsed":true}"#,
        )
        .unwrap();
        assert!(folder.folder && folder.collapsed && folder.reconnect.is_empty());
        assert_eq!(folder.session, None);
    }
}
