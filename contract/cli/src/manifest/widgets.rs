//! Widget entry declarations (LLP 1126 D2). The compiler checks the named
//! component and props; refresh names a function below the data seam.

use super::Manifest;
use serde_json::Value;
use std::collections::HashSet;

/// A WidgetKit entry, independent of any one Apple host.
#[derive(Debug, Clone, PartialEq)]
pub struct Widget {
    /// Stable identifier, also the key of the envelope's `plans` map.
    pub kind: String,
    /// Gallery display name; defaults to the humanised kind.
    pub title: Option<String>,
    /// Gallery description; defaults to the humanised kind.
    pub description: Option<String>,
    /// WidgetKit family names.
    pub families: Vec<String>,
    /// Component declared or imported by `app.contract`.
    pub component: String,
    /// Contract shape whose fields are the component's props.
    pub props: String,
    /// Data-source function: `(epoch_ms: number) -> {date: number, props: Props}[]`.
    pub refresh: String,
    /// Placeholder props, checked against the shape at bake.
    pub preview: Value,
}

/// A Live Activity entry as the packaging reads it (LLP 1126.001 WP13):
/// the kind and the two flags that decide the app's entitlements and plist
/// keys. The entry's root, state shape and preview are WP8's.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveActivity {
    /// Stable identifier, unique among the app's activities.
    pub kind: String,
    /// The app's server advances it: `aps-environment` is signed in.
    pub push: bool,
    /// It needs the frequent-update push budget (`NSSupportsLiveActivitiesFrequentUpdates`).
    pub frequent: bool,
}

use exact_plan::entry_identifier;

impl Manifest {
    /// Read the activity entries' packaging fields. A row is an object with an
    /// identifier `kind`, unique across rows, and Boolean `push` and `frequent`
    /// when present; its other keys are checked by the bake (WP8).
    pub fn live_activities(&self) -> Result<Vec<LiveActivity>, String> {
        let Some(value) = self.json.get("live_activities") else {
            return Ok(Vec::new());
        };
        let rows = value.as_array().ok_or("live_activities must be an array")?;
        let mut kinds = HashSet::new();
        rows.iter()
            .enumerate()
            .map(|(i, row)| {
                let at = format!("live_activities[{i}]");
                let object = row
                    .as_object()
                    .ok_or_else(|| format!("{at} must be an object"))?;
                let kind = object
                    .get("kind")
                    .and_then(Value::as_str)
                    .filter(|v| entry_identifier(v))
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{at}.kind must be an identifier"))?;
                if !kinds.insert(kind.to_ascii_lowercase()) {
                    return Err(format!(
                        "{at}.kind repeats {kind}; give each activity a unique kind"
                    ));
                }
                let flag = |key: &str| -> Result<bool, String> {
                    match object.get(key) {
                        None => Ok(false),
                        Some(Value::Bool(b)) => Ok(*b),
                        Some(_) => Err(format!("{at}.{key} must be true or false")),
                    }
                };
                Ok(LiveActivity {
                    kind,
                    push: flag("push")?,
                    frequent: flag("frequent")?,
                })
            })
            .collect()
    }

    /// Read entries, refusing malformed declarations.
    pub fn widgets(&self) -> Result<Vec<Widget>, String> {
        let Some(value) = self.json.get("widgets") else {
            return Ok(Vec::new());
        };
        let rows = value.as_array().ok_or("widgets must be an array")?;
        let mut kinds = HashSet::new();
        rows.iter()
            .enumerate()
            .map(|(i, row)| {
                let at = format!("widgets[{i}]");
                let object = row
                    .as_object()
                    .ok_or_else(|| format!("{at} must be an object"))?;
                for key in object.keys() {
                    if ![
                        "kind",
                        "title",
                        "description",
                        "families",
                        "component",
                        "props",
                        "refresh",
                        "preview",
                    ]
                    .contains(&key.as_str())
                    {
                        return Err(format!("{at}.{key} is unknown; remove it"));
                    }
                }
                let name = |key: &str| -> Result<String, String> {
                    object
                        .get(key)
                        .and_then(Value::as_str)
                        .filter(|v| entry_identifier(v))
                        .map(str::to_owned)
                        .ok_or_else(|| format!("{at}.{key} must be an identifier"))
                };
                let label = |key: &str| -> Result<Option<String>, String> {
                    object
                        .get(key)
                        .map(|value| {
                            // The label is a Swift string literal in the generated widget bundle.
                            value
                                .as_str()
                                .filter(|s| !s.trim().is_empty() && !s.chars().any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')))
                                .map(str::to_owned)
                                .ok_or_else(|| format!("{at}.{key} must be a non-empty string without control characters"))
                        })
                        .transpose()
                };
                let kind = name("kind")?;
                if !kinds.insert(kind.to_ascii_lowercase()) {
                    return Err(format!(
                        "{at}.kind repeats {kind}; give each widget a unique kind"
                    ));
                }
                let families = object
                    .get("families")
                    .and_then(Value::as_array)
                    .filter(|a| !a.is_empty())
                    .ok_or_else(|| format!("{at}.families must be a nonempty array"))?;
                let mut seen = HashSet::new();
                let families = families
                    .iter()
                    .map(|v| {
                        let family = v
                            .as_str()
                            .filter(|f| {
                                matches!(
                                    *f,
                                    "systemSmall"
                                        | "systemMedium"
                                        | "systemLarge"
                                        | "systemExtraLarge"
                                        | "accessoryCircular"
                                        | "accessoryRectangular"
                                        | "accessoryInline"
                                )
                            })
                            .ok_or_else(|| format!("{at}.families: use a WidgetKit family name"))?;
                        if !seen.insert(family) {
                            return Err(format!(
                                "{at}.families repeats {family}; remove the duplicate"
                            ));
                        }
                        Ok(family.to_string())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let preview = object
                    .get("preview")
                    .filter(|v| v.is_object())
                    .ok_or_else(|| format!("{at}.preview must be an object of props"))?
                    .clone();
                Ok(Widget {
                    kind,
                    title: label("title")?,
                    description: label("description")?,
                    families,
                    component: name("component")?,
                    props: name("props")?,
                    refresh: name("refresh")?,
                    preview,
                })
            })
            .collect()
    }
}
