//! Scheduling for progressive-acquisition-v1; coverage and the cursor belong to
//! the device. Mirrors Snapback's local-acquisition.ts at 13f30b2fbc.

use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) fn state(value: &Json) -> Option<&Json> {
    (value["acquired"].is_boolean()
        && value
            .get("acquisition_cursor")
            .is_some_and(|v| v.is_null() || v.is_string())
        && value["acquisition_exhausted"].is_boolean()
        && value["replacement_required"].is_boolean()
        && value["acquired_tables"]
            .as_array()
            .is_some_and(|v| v.iter().all(Json::is_string))
        && value["floored_groups"]
            .as_u64()
            .is_some_and(|v| v <= 9_007_199_254_740_991))
    .then_some(value)
}

#[derive(Default)]
pub(crate) struct Acquisition {
    /// None until this open has successfully fetched /schema.
    pub compatible: Option<bool>,
    pub failure: Option<Json>,
    stream_due: bool,
    drain_pages: usize,
    started: bool,
    served: VecDeque<Json>,
    hint: Option<Json>,
}

impl Acquisition {
    pub fn negotiate(&mut self, backend: &Json) {
        self.compatible = Some(
            backend["device_compatibility"]["advertised"]
                .as_array()
                .is_some_and(|caps| caps.iter().any(|c| c == "progressive-acquisition-v1")),
        );
    }

    pub fn reset(&mut self) {
        *self = Self {
            compatible: self.compatible,
            ..Self::default()
        };
    }

    pub fn hint(&mut self, hint: Option<Json>) {
        if hint.is_some() {
            self.hint = hint;
        }
    }

    pub fn refuse(&mut self, why: &Json) -> bool {
        if why["code"] != "E_BOUND" || why["family"] != "bound" || why["retryable"] == true {
            return false;
        }
        self.failure = Some(why.clone());
        true
    }

    pub fn replacement(&self, value: &Json) -> bool {
        state(value).is_some_and(|s| {
            s["replacement_required"] == true
                || s["acquired"] == false
                    && (!s["acquisition_cursor"].is_null() || s["acquisition_exhausted"] == true)
                    && self.compatible != Some(true)
        })
    }

    pub fn request(&mut self, value: &Json, snapshot: bool) -> Option<Json> {
        let s = state(value)?;
        if self.compatible != Some(true)
            || s["acquired"] == true && !self.stream_due
            || s["replacement_required"] == true
            || snapshot
        {
            return None;
        }
        let deferred = !s["adoption_capture"]["device:deferred:cursor"].is_null();
        if deferred
            || s["acquisition_exhausted"] == true
            || self.stream_due
            || !self.started && !s["acquisition_cursor"].is_null()
        {
            self.started = true;
            return Some(json!({"stream": true}));
        }
        self.started = true;
        let mut acquire = json!({"cursor": s["acquisition_cursor"], "skip": self.served});
        if let Some(hint) = self.hint.take().filter(|h| !self.served.contains(h)) {
            acquire["hint"] = hint;
        }
        Some(json!({"acquire": acquire}))
    }

    /// Only a published ordinary page can yield a turn. Deferred publication
    /// retains its drain even when its transfer watermark has not advanced.
    pub fn applied(&mut self, page: &Json, published: bool) -> bool {
        let progress = page.get("acquire").filter(|v| v.is_object());
        self.stream_due = progress.is_some() || !published || page["more"] == true;
        if !published {
            return false;
        }
        for event in page["events"].as_array().into_iter().flatten() {
            if event["kind"] == "scope" {
                self.served.retain(|group| group["table"] != event["table"]);
            }
        }
        if let Some(progress) = progress {
            let units = progress["units"].as_array().cloned().unwrap_or_else(|| {
                progress
                    .get("unit")
                    .filter(|u| u.is_object())
                    .cloned()
                    .into_iter()
                    .collect()
            });
            for unit in units {
                if unit["tier"] != 2 || !unit["group"].is_array() {
                    continue;
                }
                let group = json!({"table": unit["table"], "group": unit["group"]});
                if !self.served.contains(&group) {
                    self.served.push_back(group);
                }
                if self.served.len() > 32 {
                    self.served.pop_front();
                }
            }
            self.drain_pages = 0;
            return false;
        }
        self.drain_pages += 1;
        if page["more"] != true || self.drain_pages >= 4 {
            self.drain_pages = 0;
            self.stream_due = false;
            return true;
        }
        false
    }
}

/// Inspect a bound descending scan, never its answer. Unsupported query shapes
/// have no hint; this is scheduling, not query interpretation or a coverage proof.
pub(crate) fn foreground_hint(backend: &Json, name: &str, args: &Json) -> Option<Json> {
    let program = backend["programs"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)?;
    if program["native"] == true
        || program["kind"] != "Query"
        || program["held"] == true
        || program["search"] == true
    {
        return None;
    }
    let mut bindings = BTreeMap::new();
    let mut returned = &Json::Null;
    for statement in program["body"].as_array()? {
        if let Some(name) = statement["Let"]["name"].as_str() {
            bindings.insert(name, &statement["Let"]["value"]);
        } else if let Some(value) = statement.get("Return") {
            returned = value;
        } else {
            return None;
        }
    }
    let mut seen = BTreeSet::new();
    while let Some(name) = returned["Variable"].as_str() {
        let Some(value) = bindings.get(name).filter(|_| seen.insert(name)) else {
            break;
        };
        returned = value;
    }
    let scan = &returned["Scan"];
    let name = scan["table"].as_str()?;
    if scan["order"] != "Desc" || scan["existence"] == true {
        return None;
    }
    let prefix = scan["prefix"].as_array()?;
    let table = &backend["schema"]["tables"][name];
    let sync = &table["sync"];
    let index = sync["horizon"]["by"].as_str()?;
    if scan["index"] != index {
        return None;
    }
    let audience = &sync["audience"];
    let columns = audience["Target"]["prefix"]
        .as_array()
        .cloned()
        .or_else(|| audience["Column"].as_str().map(|c| vec![json!(c)]))?;
    if columns.is_empty() {
        return None;
    }
    let components = table["indexes"][index]["components"].as_array()?;
    let mut group = Vec::new();
    for (i, column) in columns.iter().enumerate() {
        if components.get(i)?["Column"] != *column {
            return None;
        }
        let scalar = &prefix.get(i)?["Scalar"];
        let value = if let Some(variable) = scalar["Variable"]
            .as_str()
            .filter(|v| program["args"].get(*v).is_some())
        {
            let alias = program["arg_aliases"].as_object().and_then(|aliases| {
                aliases
                    .iter()
                    .find(|(_, canonical)| *canonical == variable)
                    .map(|(alias, _)| alias)
            });
            args.get(variable)
                .filter(|v| !v.is_null())
                .or_else(|| alias.and_then(|alias| args.get(alias)))?
        } else {
            let literal = &scalar["Literal"];
            ["Id", "String", "Int"]
                .iter()
                .find_map(|key| literal.get(key).filter(|v| !v.is_null()))?
        };
        if !value.is_string()
            && !value
                .as_f64()
                .is_some_and(|n| n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0)
        {
            return None;
        }
        group.push(value.clone());
    }
    Some(json!({"table": name, "group": group}))
}
