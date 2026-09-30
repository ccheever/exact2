//! What desktop.json remembers about launches (settings.rs `LaunchDefaults`,
//! `RecentProject`, `RecentModel`, `model_efforts` and `remember_launch`):
//! the next dialog opens with the last choices, recent folders and models
//! lead their lists, and a model's last effort comes back with it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The last launch dialog choices; the next dialog opens with these filled in.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LaunchDefaults {
    /// The machine id.
    pub machine: String,
    /// The harness.
    pub provider: String,
    /// The account handle.
    pub account: String,
    /// The folder.
    pub cwd: String,
    /// The model id.
    pub model: String,
    /// The effort level.
    pub effort: String,
    /// A per-launch permission override; empty takes the machine default.
    pub permissions: String,
}

/// A folder that was launched into, on a specific machine. Most recent first.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RecentProject {
    /// The machine id.
    pub machine: String,
    /// The folder.
    pub cwd: String,
}

/// A model that was launched with, shown at the top of the model list.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RecentModel {
    /// The harness.
    pub provider: String,
    /// The model id.
    pub id: String,
    /// Its display name.
    pub name: String,
}

/// How many recent projects are kept.
pub const RECENT_PROJECT_LIMIT: usize = 20;
/// How many recent models are kept.
pub const RECENT_MODEL_LIMIT: usize = 6;

/// Key of the remembered effort for a harness and model.
pub fn effort_key(provider: &str, model: &str) -> String {
    format!("{provider}/{model}")
}

/// What desktop.json remembers about launches (settings.rs's launch fields).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LaunchMemory {
    /// The last launch.
    pub launch: Option<LaunchDefaults>,
    /// Folders launched into, most recent first.
    pub recent_projects: Vec<RecentProject>,
    /// Models launched with, most recent first.
    pub recent_models: Vec<RecentModel>,
    /// Last effort per harness/model, so picking a model brings its effort back.
    pub model_efforts: HashMap<String, String>,
}

impl LaunchMemory {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembering_launches_keeps_recent_lists_short() {
        let mut memory = LaunchMemory::default();
        for i in 0..8 {
            let d = LaunchDefaults {
                machine: "laptop".into(),
                provider: "claude".into(),
                cwd: format!("~/p{i}/"),
                model: format!("m{i}"),
                effort: "low".into(),
                ..Default::default()
            };
            memory.remember_launch(d, &format!("Model {i}"));
        }
        assert_eq!(memory.recent_models.len(), RECENT_MODEL_LIMIT);
        assert_eq!(memory.recent_models[0].name, "Model 7");
        assert_eq!(memory.recent_projects[0].cwd, "~/p7");
        assert_eq!(memory.model_efforts["claude/m3"], "low");
        // The same folder again moves to the front instead of repeating.
        let again = LaunchDefaults {
            machine: "laptop".into(),
            cwd: "~/p3".into(),
            ..Default::default()
        };
        memory.remember_launch(again, "Default");
        assert_eq!(memory.recent_projects.len(), 8);
        assert_eq!(memory.recent_projects[0].cwd, "~/p3");
        assert_eq!(memory.recent_models.len(), RECENT_MODEL_LIMIT);
    }
}
