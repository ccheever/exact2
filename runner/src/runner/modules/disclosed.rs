//! What a bound implementation discloses of its own objects under the agent
//! (LLP 1133.008 D8 item 9): a camera's tracks and photos, a model's
//! sessions. A native implementation says them through its module's
//! `changed` ([`STATED`]), since the runner cannot ask a host synchronously
//! when `state.extensions` is read; each call's fields replace the ones of
//! the same name it said before. They are listed as the extension's
//! `implementation`, as the web runtime lists the object its
//! implementation's `state()` returns.

use super::super::{DataSource, Runner};
use super::Json;
use std::collections::BTreeMap;

/// The prefix of a `changed` topic that is an implementation's disclosure:
/// `{"extension", "state": {…}}` after it.
pub(in crate::runner) const STATED: &str = "exact-state:";

/// Each extension's disclosed fields.
#[derive(Default)]
pub(in crate::runner) struct Disclosed {
    by: BTreeMap<String, serde_json::Map<String, Json>>,
}

impl<D: DataSource> Runner<D> {
    /// A disclosure an implementation said: its fields kept.
    pub(in crate::runner) fn disclosed_told(&mut self, body: &str) {
        let v: Json = serde_json::from_str(body).unwrap_or_default();
        let (Some(extension), Some(fields)) = (v["extension"].as_str(), v["state"].as_object())
        else {
            self.log(format!("an implementation's state is unreadable: {body}"));
            return;
        };
        let kept = self
            .extensions
            .disclosed
            .by
            .entry(extension.to_owned())
            .or_default();
        for (k, v) in fields {
            kept.insert(k.clone(), v.clone());
        }
    }

    /// `extension`'s disclosed fields, if it said any.
    pub(in crate::runner) fn disclosed_json(&self, extension: &str) -> Option<Json> {
        (self.extensions.disclosed.by.get(extension)).map(|m| Json::Object(m.clone()))
    }

    /// The extensions that disclosed anything.
    pub(in crate::runner) fn disclosed_extensions(&self) -> Vec<String> {
        self.extensions.disclosed.by.keys().cloned().collect()
    }
}
