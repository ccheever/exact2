//! Received acquisition images are counts, independent of durable coverage.

use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Progress {
    tables: BTreeMap<String, u64>,
    deferred: BTreeMap<String, u64>,
}

impl Progress {
    pub fn page(&mut self, page: &Json, first: bool, published: bool) {
        let snapshot = page["snapshot"] == true;
        if snapshot {
            self.deferred.clear();
            if first {
                self.tables.clear();
            }
        }
        if snapshot || page.get("acquire").is_some_and(Json::is_object) {
            let counts = if snapshot {
                &mut self.tables
            } else {
                &mut self.deferred
            };
            for event in page["events"].as_array().into_iter().flatten() {
                if event["kind"] == "put" {
                    if let Some(table) = event["table"].as_str() {
                        *counts.entry(table.to_owned()).or_default() += 1;
                    }
                }
            }
        }
        if !snapshot && published {
            for (table, count) in std::mem::take(&mut self.deferred) {
                *self.tables.entry(table).or_default() += count;
            }
        }
    }

    pub fn status(&self, state: &Json, failure: &Option<Json>) -> Json {
        let mut status = json!({
            "acquired": state["acquired"] == true,
            "exhausted": state["acquisition_exhausted"] == true,
            "cursor": state.get("acquisition_cursor").cloned().unwrap_or(Json::Null),
            "acquiredTables": state.get("acquired_tables").cloned().unwrap_or_else(|| json!([])),
            "flooredGroups": state["floored_groups"].as_u64().unwrap_or(0),
            "rows": self.tables.values().sum::<u64>(), "tables": self.tables,
        });
        if let Some(why) = failure {
            status["failure"] = why.clone();
        }
        status
    }
}
