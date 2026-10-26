//! The agent's side of the module seam (LLP 1133.002 §6, LLP 1124 D7):
//! `state.extensions`, and `prefer extension` and `prefer permission`.

use super::super::{CommitReceipt, DataSource, Runner};
use super::{error_code, Json, ModulesRow, Route};
use serde_json::{json, Map};

impl<D: DataSource> Runner<D> {
    /// `state.extensions` (LLP 1133.002 §6, Disclosure): each module the plan
    /// names, by its extension: whether a host's implementation answers it
    /// (`bound`, and `agent` where it answers under the agent), its
    /// permission's state, the preferred values, each source's readers (a
    /// resource that reads it, or whose data code did), the tickets held
    /// with their calls' origins, and its ceremonies and signals, each by
    /// name, RP ID, origin and outcome, never a response (LLP 1133.011 §9).
    /// `None` for a plan that names none.
    pub fn extensions_json(&self) -> Option<String> {
        if self.plan.modules.is_empty() && self.plan.views.is_empty() {
            return None;
        }
        let mut out = Map::new();
        for row in &self.plan.modules {
            let extension = self.plan.str(row.extension).to_owned();
            let declared = self.plan.str(row.declared).to_owned();
            let key = self.key(row);
            let bound = self.extensions.bound.contains(&extension);
            let entry = out.entry(extension.clone()).or_insert_with(
                || json!({ "bound": bound, "preferred": {}, "readers": {}, "held": [] }),
            );
            if self.route(row) == Route::Agent {
                entry["agent"] = json!(true);
            }
            // Each permission by name; `permission` too where it has one.
            let sub = self.substitute(row);
            let by = sub["permissionBy"].as_object().into_iter().flatten();
            let list = sub["permissionList"].as_array().into_iter().flatten();
            for p in std::iter::once(&sub["permission"])
                .chain(by.map(|(_, p)| p))
                .chain(list)
            {
                // A `<prefix>.*` permission is shown as each one set under it.
                match p["name"].as_str().map(|n| (n, n.strip_suffix('*'))) {
                    Some((_, Some(pre))) => {
                        for name in self
                            .extensions
                            .permission
                            .keys()
                            .filter(|k| k.starts_with(pre))
                        {
                            entry["permissions"][name] = json!(self.permission_state(name));
                        }
                    }
                    Some((name, None)) => {
                        entry["permissions"][name] = json!(self.permission_state(name));
                    }
                    None => {}
                }
            }
            if let Some(v) = self.extensions.preferred.get(&key) {
                entry["preferred"][&declared] = v.clone();
            }
            if !row.send {
                let readers: Vec<&str> = (0..self.plan.resources.len())
                    .filter(|&i| {
                        self.plan.resources[i].source
                            == self.plan.sources[row.source.0 as usize].name
                            || self.watching[i].contains(&key)
                    })
                    .map(|i| self.plan.str(self.plan.resources[i].name))
                    .collect();
                entry["readers"][&declared] = json!(readers);
            }
            if let Some(facts) = self.facts_of(&extension) {
                entry["facts"] = facts;
            }
            let ceremonies = self.ceremonies_of(&extension);
            if !ceremonies.is_empty() {
                entry["ceremonies"] = json!(ceremonies);
            }
            for h in self.extensions.held.iter().filter(|h| {
                h.source == self.plan.str(self.plan.sources[row.source.0 as usize].name)
                    && !h.queued
            }) {
                if let Some(held) = entry["held"].as_array_mut() {
                    held.push(json!({ "ticket": h.hold, "name": declared, "origin": h.origin.word(), "waits": h.waits.word() }));
                }
            }
        }
        for (extension, entry) in out.iter_mut() {
            // Its streams, sampled readers and canvas inputs (LLP 1133.013 §9).
            if let Some(samples) = self.samples_json(extension) {
                for (k, v) in samples.as_object().into_iter().flatten() {
                    entry[k] = v.clone();
                }
            }
            if let Some(pictures) = self.pictures_json(extension) {
                entry["pictures"] = pictures;
            }
            // The canvases its inputs feed, and their leases (D6).
            if let Some(surfaces) = self.fed_json(extension) {
                entry["surfaces"] = surfaces;
            }
            self.reach_json(extension, entry);
            let one = (entry["permissions"].as_object())
                .filter(|p| p.len() == 1)
                .and_then(|p| p.values().next().cloned());
            if let Some(state) = one {
                entry["permission"] = state;
            }
        }
        // Each module event not yet heard (LLP 1133.004 D6).
        for (extension, event) in self.queued_events() {
            let entry = out.entry(extension).or_insert_with(
                || json!({ "bound": false, "preferred": {}, "readers": {}, "held": [] }),
            );
            match entry["queued"].as_array_mut() {
                Some(queued) => queued.push(event),
                None => entry["queued"] = json!([event]),
            }
        }
        // Each package view, and who made it displayed (LLP 1133.008 D8 item 2).
        for row in &self.plan.views {
            let extension = self.plan.str(row.extension);
            let views = self.views_json(extension);
            let bound = self.extensions.bound.contains(extension);
            let entry = (out.entry(extension.to_owned())).or_insert_with(
                || json!({ "bound": bound, "preferred": {}, "readers": {}, "held": [] }),
            );
            entry["views"] = json!(views);
        }
        Some(Json::Object(out).to_string())
    }

    /// The agent's `prefer extension` and `prefer permission` (LLP 1124 D7,
    /// LLP 1069.007 D2): `{"op":"prefer","extension":{"geolocation.watchPosition":
    /// {…}|"clear"}}` sets a source's answer; `{"op":"prefer","permission":
    /// {"geolocation":"granted"}}` a permission's state. Readers are asked
    /// again. `None` for any other request.
    pub fn extension_agent(
        &mut self,
        request: &str,
    ) -> Option<(Vec<CommitReceipt>, Result<String, String>)> {
        let v: Json = serde_json::from_str(request).ok()?;
        if v.get("op").and_then(|o| o.as_str()) != Some("prefer") {
            return None;
        }
        let refuse = |e: String| Some((Vec::new(), Err(e)));
        if let Some(group) = v.get("permission") {
            let Some(group) = group.as_object() else {
                return refuse(
                    "prefer permission: an object of `<name>`: granted|denied|prompt".into(),
                );
            };
            for (name, value) in group {
                // A state, or a fact of one the platform hides (`keyed.rs`);
                // a name may stand for each permission under it (`health`).
                let Some(value) = value.as_str() else {
                    return refuse(format!(
                        "prefer permission.{name}: granted, denied or prompt"
                    ));
                };
                if let Err(e) = self.prefer_permission(name, value) {
                    return refuse(e);
                }
            }
            if v.get("extension").is_none() {
                let (receipts, error) = self.apply_announced();
                if let Some(e) = error {
                    return refuse(format!("prefer permission: {e:?}"));
                }
                let reply = json!({ "permission": self.extensions.permission });
                return Some((receipts, Ok(reply.to_string())));
            }
        }
        let Some(group) = v.get("extension")?.as_object() else {
            return refuse("prefer extension: an object of `extension.source`".into());
        };
        for (key, value) in group {
            if let Some(ty) = self.agent_word(key) {
                if value != "clear" && !word_fits(&ty, value) {
                    return refuse(format!("prefer extension: `{key}` is a {ty}"));
                }
                continue;
            }
            let Some(row) = self
                .plan
                .modules
                .iter()
                .find(|m| self.key(m) == *key)
                .cloned()
            else {
                let known: Vec<String> = (self.plan.modules.iter())
                    .filter(|m| !m.send)
                    .map(|m| self.key(m))
                    .collect();
                return refuse(format!(
                    "prefer extension: no source `{key}` in this app; its module sources are {}",
                    if known.is_empty() {
                        "none".into()
                    } else {
                        known.join(", ")
                    }
                ));
            };
            if row.send {
                return refuse(format!(
                    "prefer extension: `{key}` is a send, which the module answers"
                ));
            }
            let sub = self.substitute(&row);
            let statuses = self.statuses_of(&row);
            for permission in &statuses {
                let field = permission["field"].as_str().unwrap_or_default();
                if value.get(field).is_some() {
                    // One authority per fact (§6): the state is the permission's.
                    let name = permission["name"].as_str().unwrap_or_default();
                    return refuse(format!(
                        "prefer extension: `{key}`'s `{field}` is set by `prefer permission.{name}`"
                    ));
                }
            }
            if value != "clear" && error_code(value).is_none() {
                // Checked with the state the permission has, which is
                // in the field's enum where `prompt` may not be (a
                // permission no platform asks, LLP 1133.017 D14).
                let mut checked = value.clone();
                for permission in &statuses {
                    let (Some(field), Some(name)) =
                        (permission["field"].as_str(), permission["name"].as_str())
                    else {
                        continue;
                    };
                    let state = self.permission_state(name);
                    if let Some(object) = checked.as_object_mut() {
                        object.insert(field.into(), json!(state));
                    }
                }
                // An implementation's own substitute takes a partial
                // overlay (LLP 1133.009 D10 item 11), as a `partial` source
                // does over its default; a declared one, a value. A sampled
                // source's takes a reading or a track of steps (§9).
                let checked = if sub["track"] == true {
                    self.track_checked(&row, &checked)
                } else if sub["implementation"] == true || sub["partial"] == true {
                    self.partial(&row, &checked)
                } else {
                    self.typed(&row, &checked.to_string()).map(|_| ())
                };
                if let Err(e) = checked {
                    return refuse(format!("prefer extension: {e}"));
                }
            }
        }
        let mut told = std::collections::BTreeSet::new();
        for (key, value) in group {
            if value == "clear" {
                self.extensions.preferred.remove(key);
            } else {
                self.extensions.preferred.insert(key.clone(), value.clone());
            }
            self.note_track(key, value);
            self.log(format!("prefer extension {key}"));
            self.announce(key);
            // An agent word is its extension's: any of its names stands for it.
            let extension = key.split_once('.').map_or("", |(e, _)| e);
            let row = (self.plan.modules.iter()).find(|m| {
                self.key(m) == *key
                    || (self.plan.str(m.extension) == extension && self.agent_word(key).is_some())
            });
            if let Some(row) = row.filter(|r| self.route(r) == Route::Agent) {
                told.insert(self.plan.str(row.extension).to_owned());
            }
        }
        for extension in told {
            self.tell_preferred(&extension);
        }
        self.settle_values();
        let (receipts, error) = self.apply_announced();
        if let Some(e) = error {
            return refuse(format!("prefer extension: {e:?}"));
        }
        let reply = json!({ "extension": self.extensions.preferred, "permission": self.extensions.permission });
        Some((receipts, Ok(reply.to_string())))
    }

    /// `value`, a partial overlay of `row`'s answer: each field it writes
    /// checked against the answer's shape, record by record.
    fn partial(&self, row: &ModulesRow, value: &Json) -> Result<(), String> {
        let ty = self.plan.sources[row.source.0 as usize].ty;
        let shape = exact_js_value::Shape::from_plan(&self.plan, ty)?;
        partial(&self.key(row), value, &shape)
    }

    /// The type of `key` (`speech-recognition.heard`) where it is one of the
    /// words its interface declares for the agent path alone (`agent`, LLP
    /// 1133.012 §9) and is no name of the interface.
    fn agent_word(&self, key: &str) -> Option<String> {
        let (extension, word) = key.split_once('.')?;
        if (self.plan.modules.iter()).any(|m| self.key(m) == key) {
            return None;
        }
        let row = (self.plan.modules.iter()).find(|m| self.plan.str(m.extension) == extension)?;
        self.substitute(row)["agent"][word]
            .as_str()
            .map(str::to_owned)
    }
}

/// Whether `value` is one of `ty`'s: a string, number or bool by its kind,
/// a list of them, and anything else as JSON.
fn word_fits(ty: &str, value: &Json) -> bool {
    match ty.trim() {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "bool" => value.is_boolean(),
        list if list.ends_with("[]") => (value.as_array())
            .is_some_and(|a| a.iter().all(|v| word_fits(&list[..list.len() - 2], v))),
        _ => true,
    }
}

fn partial(at: &str, value: &Json, shape: &exact_js_value::Shape) -> Result<(), String> {
    use exact_js_value::Shape;
    match (value, shape) {
        (Json::Object(map), Shape::Record(fields)) => {
            for (name, v) in map {
                let Some((_, field)) = fields.iter().find(|(f, _)| f == name) else {
                    return Err(format!("{at}: field `{name}` is not in the shape"));
                };
                partial(&format!("{at}.{name}"), v, field)?;
            }
            Ok(())
        }
        (Json::Null, Shape::Option(_)) => Ok(()),
        (value, Shape::Option(inner)) => partial(at, value, inner),
        (value, shape) => exact_js_value::from_json(value, shape)
            .map(|_| ())
            .map_err(|e| format!("{at}: {e}")),
    }
}
