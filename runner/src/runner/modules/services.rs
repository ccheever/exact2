//! Foreground services under the agent (LLP 1133.033 D2, D21): Android's
//! reader starts none while its implementations run under the agent, and
//! says each claim and release through the claimant's `changed`
//! ([`CLAIMED`]). The runner journals it (`foreground service microphone:
//! claimed`) and lists the claims held in the claimant's
//! `state.extensions` entry as `foregroundService`.

use super::super::{DataSource, Runner};
use super::Json;
use serde_json::json;

/// The prefix of a `changed` topic that is a foreground-service claim, made
/// or released: `{"extension", "kind", "types", "claimed", "text",
/// "refused"?}` after it.
pub(in crate::runner) const CLAIMED: &str = "exact-foreground-service:";

/// The claims held, by extension, each `{type, kind, package, since, text}`.
#[derive(Default)]
pub(in crate::runner) struct Services {
    held: Vec<(String, Json)>,
}

impl<D: DataSource> Runner<D> {
    /// A claim an implementation told: journaled, and listed while held.
    pub(in crate::runner) fn foreground_service_told(&mut self, body: &str) {
        let v: Json = serde_json::from_str(body).unwrap_or_default();
        let (Some(extension), Some(kind)) = (v["extension"].as_str(), v["kind"].as_str()) else {
            self.log(format!("a foreground-service claim is unreadable: {body}"));
            return;
        };
        let types: Vec<&str> = (v["types"].as_array().into_iter().flatten())
            .filter_map(Json::as_str)
            .collect();
        let named = types.join("|");
        let held = &mut self.extensions.services.held;
        let line = if let Some(why) = v["refused"].as_str() {
            format!("refused: {why}")
        } else if v["claimed"] == true {
            let entry = json!({
                "type": named,
                "kind": kind,
                "package": extension,
                "since": self.now_ms,
                "text": v["text"],
            });
            held.push((extension.to_owned(), entry));
            "claimed".to_owned()
        } else {
            let mine = |(e, c): &(String, Json)| {
                e == extension && c["kind"] == kind && c["type"] == named.as_str()
            };
            if let Some(at) = held.iter().position(mine) {
                held.remove(at);
            }
            "released".to_owned()
        };
        self.log(format!("foreground service {named}: {line}"));
    }

    /// `state.extensions`' `foregroundService` for `extension`: the claims
    /// it holds, oldest first; `None` for none.
    pub(in crate::runner) fn services_json(&self, extension: &str) -> Option<Json> {
        let held: Vec<&Json> = (self.extensions.services.held.iter())
            .filter(|(e, _)| e == extension)
            .map(|(_, c)| c)
            .collect();
        (!held.is_empty()).then(|| json!(held))
    }
}
