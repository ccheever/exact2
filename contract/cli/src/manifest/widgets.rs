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

use exact_plan::entry_identifier;

impl Manifest {
    /// Read entries, refusing malformed declarations and stage 2's reserved key.
    pub fn widgets(&self) -> Result<Vec<Widget>, String> {
        if self.json.get("live_activities").is_some() {
            return Err("live_activities is reserved for stage 2; remove it for this bake".into());
        }
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
