//! The watch entry: the app's own live plan for Apple Watch, rooted at a component the author
//! writes for the wrist, in its own file. Not a widget: it keeps state, resources, timers and
//! handlers. @ref LLP 1126.002 D4, LLP 1126 D10.
use super::Manifest;
use serde_json::Value;

/// Its key in the envelope's `plans` map and its file, `plans/watch.plan`.
pub const WATCH_KIND: &str = "watch";

/// A watch entry's declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchEntry {
    /// The entry's Contract file, beside `app.contract`.
    pub source: String,
    /// Root component of the watch plan: the first component of `source`.
    pub component: String,
}

impl Manifest {
    /// Read `watch`, whose kind is unique with every widget and activity.
    pub fn watch(&self) -> Result<Option<WatchEntry>, String> {
        let Some(value) = self.json.get("watch") else {
            return Ok(None);
        };
        let object = value.as_object().ok_or("watch must be an object")?;
        if let Some(key) = object
            .keys()
            .find(|k| !["source", "component"].contains(&k.as_str()))
        {
            return Err(format!("watch.{key} is unknown; remove it"));
        }
        let component = object
            .get("component")
            .and_then(Value::as_str)
            .filter(|v| exact_plan::entry_identifier(v))
            .ok_or("watch.component must be an identifier")?
            .to_owned();
        let source = object
            .get("source")
            .and_then(Value::as_str)
            .filter(|v| {
                v.strip_suffix(".contract").is_some_and(|stem| {
                    !stem.is_empty() && stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                })
            })
            .ok_or("watch.source must name a `.contract` file beside app.contract, such as `watch.contract`")?
            .to_owned();
        let taken = self
            .widgets()?
            .iter()
            .map(|w| w.kind.clone())
            .chain(self.live_activities()?.iter().map(|a| a.kind.clone()))
            .any(|kind| kind.eq_ignore_ascii_case(WATCH_KIND));
        if taken {
            return Err(format!("a widget or activity has the kind `{WATCH_KIND}`, which names the watch entry's plan; rename it"));
        }
        Ok(Some(WatchEntry { source, component }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(json: Value) -> Manifest {
        Manifest {
            json,
            id: "com.example.app".into(),
            name: "App".into(),
            declared: true,
        }
    }

    #[test]
    fn reads_the_entry() {
        let entry =
            manifest(json!({"watch": {"source": "watch.contract", "component": "AppWatch"}}))
                .watch();
        assert_eq!(
            entry,
            Ok(Some(WatchEntry {
                source: "watch.contract".into(),
                component: "AppWatch".into()
            }))
        );
        assert_eq!(manifest(json!({})).watch(), Ok(None));
    }

    #[test]
    fn refuses_what_it_cannot_compile() {
        for (watch, said) in [
            (
                json!({"source": "watch.contract", "component": "AppWatch", "families": []}),
                "watch.families is unknown",
            ),
            (
                json!({"source": "watch.contract"}),
                "watch.component must be an identifier",
            ),
            (
                json!({"source": "../watch.contract", "component": "AppWatch"}),
                "watch.source must name",
            ),
            (
                json!({"source": "watch.txt", "component": "AppWatch"}),
                "watch.source must name",
            ),
            (json!(["watch.contract"]), "watch must be an object"),
        ] {
            let err = manifest(json!({ "watch": watch })).watch().unwrap_err();
            assert!(err.contains(said), "{err}");
        }
    }

    #[test]
    fn its_kind_is_unique_with_widgets_and_activities() {
        let app = json!({
            "watch": {"source": "watch.contract", "component": "AppWatch"},
            "live_activities": [{"kind": "Watch", "component": "A", "state": "S", "preview": {}}],
        });
        assert!(manifest(app)
            .watch()
            .unwrap_err()
            .contains("names the watch entry's plan"));
    }
}
