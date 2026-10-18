//! Activity entries share the app's cohort, never a generated Swift state type.
//! @ref LLP 1126.001 WP8, LLP 1126 D9.
use super::Manifest;
use serde_json::Value;
use std::collections::HashSet;

/// A Live Activity's declaration. Placements belong to its component's view.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    /// Stable entry identifier, unique across widgets and activities.
    pub kind: String,
    /// Literal platform label, defaulting to the humanised kind.
    pub title: Option<String>,
    /// Literal platform description, defaulting to the humanised kind.
    pub description: Option<String>,
    /// Root component, whose props match the state shape.
    pub component: String,
    /// Contract shape for the JSON state.
    pub state: String,
    /// Placeholder state, validated at bake.
    pub preview: Value,
    /// The app's backend advances this activity through APNs.
    pub push: bool,
    /// Request ActivityKit's frequent-update budget.
    pub frequent: bool,
}

impl Manifest {
    /// Read activities with the same identifier and literal-label rules as widgets.
    pub fn live_activities(&self) -> Result<Vec<Activity>, String> {
        let Some(value) = self.json.get("live_activities") else {
            return Ok(Vec::new());
        };
        let rows = value.as_array().ok_or("live_activities must be an array")?;
        let mut kinds: HashSet<_> = self
            .widgets()?
            .iter()
            .map(|w| w.kind.to_ascii_lowercase())
            .collect();
        rows.iter().enumerate().map(|(i, row)| {
            let at = format!("live_activities[{i}]");
            let object = row.as_object().ok_or_else(|| format!("{at} must be an object"))?;
            for key in object.keys() {
                if !["kind", "component", "state", "preview", "title", "description", "push", "frequent"].contains(&key.as_str()) {
                    return Err(format!("{at}.{key} is unknown; remove it"));
                }
            }
            let name = |key: &str| object.get(key).and_then(Value::as_str)
                .filter(|v| exact_plan::entry_identifier(v)).map(str::to_owned)
                .ok_or_else(|| format!("{at}.{key} must be an identifier"));
            let label = |key: &str| object.get(key).map(|v| v.as_str()
                .filter(|s| !s.trim().is_empty() && !s.chars().any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')))
                .map(str::to_owned).ok_or_else(|| format!("{at}.{key} must be a non-empty string without control characters"))).transpose();
            let flag = |key: &str| object.get(key).map(|v| v.as_bool()
                .ok_or_else(|| format!("{at}.{key} must be a Boolean"))).transpose().map(|v| v.unwrap_or(false));
            let kind = name("kind")?;
            if !kinds.insert(kind.to_ascii_lowercase()) {
                return Err(format!("{at}.kind repeats {kind}; give each widget and activity a unique kind"));
            }
            Ok(Activity {
                kind, title: label("title")?, description: label("description")?,
                component: name("component")?, state: name("state")?,
                preview: object.get("preview").filter(|v| v.is_object())
                    .ok_or_else(|| format!("{at}.preview must be an object of state fields"))?.clone(),
                push: flag("push")?, frequent: flag("frequent")?,
            })
        }).collect()
    }
}
