//! A host's own implementation of a module (LLP 1133.001 D4, LLP 1133.002
//! D5): outside the agent the host binds the extensions it carries, and
//! their names go to the native slot as `{"extension", "source"|"send",
//! "args"}`, which the host's composed module routes to the implementation.
//! Under the agent the host binds those a drive names (`--bind`) as in
//! production, and the rest told they run under the agent: those answer an
//! interface that is its own substitute, behind the runner's tickets
//! (`admit.rs`), and the declared substitute answers every other (LLP 1124
//! D7). The runner keeps each bound live source's readers, and tells the
//! module when the last one goes (`{"extension", "release": name}`), so its
//! acquisition stops.
//!
//! A synchronous form (`geolocationStatusSync`, LLP 1133.002 D14) is a
//! `native.call` that data code makes inside an answer, which the runner
//! cannot be asked during. So the runner keeps each one's answer in the
//! native slot: none when bound as in production (the host's module
//! answers), else the substitute's value now, never held (a request it
//! would hold answers its `cancel`).

use super::super::{DataSource, Runner};
use super::{Decision, Json, ModulesRow, Route};
use crate::request::{Reply, Request};
use serde_json::json;
use std::collections::BTreeSet;

impl<D: DataSource> Runner<D> {
    /// The host's implementations answer `real` as in production, and
    /// `agent`, told they run under the agent, answer an interface that is
    /// its own substitute: their names' readers are asked again, of the
    /// module now, and each `agent` implementation is given the preferred
    /// values the agent set.
    pub fn bind_extensions(&mut self, real: &[&str], agent: &[&str]) {
        self.extensions.bound = real.iter().map(|e| e.to_string()).collect();
        self.extensions.agent = (agent.iter())
            .filter(|e| !real.contains(e))
            .map(|e| e.to_string())
            .collect();
        self.presence_rebound();
        if real.is_empty() && agent.is_empty() {
            return;
        }
        self.retell_views(&[real, agent].concat());
        if !real.is_empty() {
            self.log(format!("modules bound here: {}", real.join(", ")));
        }
        let answering: std::collections::BTreeSet<String> = (self.plan.modules.iter())
            .filter(|m| self.route(m) == Route::Agent)
            .map(|m| self.plan.str(m.extension).to_owned())
            .collect();
        if !answering.is_empty() {
            let names: Vec<&str> = answering.iter().map(String::as_str).collect();
            self.log(format!(
                "modules bound under the agent: {}",
                names.join(", ")
            ));
        }
        for extension in &answering {
            self.tell_preferred(extension);
        }
        if self.page.hidden {
            self.tell_foreground();
        }
        self.publish_sync();
        let keys: Vec<String> = (self.plan.modules.iter())
            .filter(|m| self.route(m) != Route::Substitute)
            .map(|m| self.key(m))
            .collect();
        for key in keys {
            self.announce(&key);
        }
    }

    /// Each synchronous form's answer, by `extension.syncName`, into the
    /// native slot.
    pub(in crate::runner) fn publish_sync(&self) {
        let mut table = std::collections::BTreeMap::new();
        for row in self
            .plan
            .modules
            .iter()
            .filter(|m| !self.plan.str(m.sync).is_empty())
        {
            let key = format!(
                "{}.{}",
                self.plan.str(row.extension),
                self.plan.str(row.sync)
            );
            let answer = (self.route(row) != Route::Real).then(|| {
                let answer = match self.decide(row, row.send, &json!({})) {
                    Decision::Now(answer) => answer,
                    Decision::Hold(_) | Decision::Forward => {
                        match &self.substitute(row)["cancel"] {
                            Json::Null => json!({ "error": "NotSupportedError" }),
                            cancel => cancel.clone(),
                        }
                    }
                };
                self.completed(row, answer).to_string().into_bytes()
            });
            table.insert(key, answer);
        }
        self.native_slot().sync(table);
    }

    /// A bound name's read or send, for the native slot.
    pub(in crate::runner) fn bound_request(
        &mut self,
        row: &ModulesRow,
        args: Json,
        reader: Option<usize>,
    ) -> Request {
        Request::native(self.forward_body(row, args, reader))
    }

    /// The body of a call of `row` for its implementation, `{"extension",
    /// "source"|"send", "args"}`; under the agent with the preferred value
    /// the agent set for it, as `preferred` (LLP 1133.009 D10 item 11); with
    /// its `origin` when a tool or a wake made it, so an implementation that
    /// starts a device on a read (a track's first lease, D6) can refuse. A
    /// live source's reader is counted, so its release can be said, and a
    /// Contract reader (`reader`, its resource) with its arguments, so each
    /// argument group's can.
    pub(in crate::runner) fn forward_body(
        &mut self,
        row: &ModulesRow,
        args: Json,
        reader: Option<usize>,
    ) -> Vec<u8> {
        let verb = if row.send { "send" } else { "source" };
        // An option the call omits is left out, so it stays the platform's (D2).
        let mut args = args;
        written(&mut args);
        // The options its declaration names strings keys are sent as their
        // text, which the platform shows (a sheet's reason, LLP 1133.010 D1).
        let strings = self.substitute(row)["strings"].clone();
        for path in strings
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
        {
            let pointer = format!("/{}", path.replace('.', "/"));
            let key = args
                .pointer(&pointer)
                .and_then(Json::as_str)
                .map(str::to_owned);
            if let (Some(key), Some(at)) = (key, args.pointer_mut(&pointer)) {
                *at = Json::String(self.strings_text(&key));
            }
        }
        let key = self.key(row);
        let mut body = json!({ "extension": self.plan.str(row.extension), verb: self.plan.str(row.declared), "args": args });
        if self.route(row) == Route::Agent {
            if let Some(preferred) = self.extensions.preferred.get(&key) {
                body["preferred"] = preferred.clone();
            }
        }
        let origin = match reader {
            Some(i) => self.origin_for(Some(crate::runner::Target::Resource(i))),
            None => self.call_origin(),
        };
        if origin != super::CallOrigin::App {
            body["origin"] = json!(origin.word());
            // A call not the app's that would ask (LLP 1133.002 D12): the
            // host reads the status first and refuses it while `prompt`,
            // since only the implementation knows the platform's state.
            let sub = self.substitute(row);
            if self.asks(row, row.send)
                && sub["admission"] != "none"
                && sub["permission"]["status"].is_string()
            {
                body["admit"] = json!({
                    "status": sub["permission"]["status"],
                    "field": sub["permission"]["field"],
                    "denied": sub["permission"]["denied"],
                });
            }
        }
        // A send's number, which `cancel` names if it is let go (LLP 1133.016 D15 item 2).
        if row.send {
            self.extensions.requests += 1;
            body["request"] = json!(self.extensions.requests);
        }
        if row.live && !row.send {
            self.note_poll(row);
            if let Some(i) = reader {
                let group = body["args"].to_string();
                let readers = self.extensions.readers.entry(key.clone()).or_default();
                readers.insert(i, group.clone());
                self.extensions
                    .groups
                    .entry(key.clone())
                    .or_default()
                    .insert(group);
            }
            self.extensions.live.insert(key);
        }
        // A sampled source's reader carries its cursor (LLP 1133.013 D5),
        // outside the group its arguments make.
        if let Some(after) = self.cursor(row) {
            body["args"]["after"] = json!(after);
        }
        body.to_string().into_bytes()
    }

    /// Request `ticket` went out with `body`: a send to an implementation
    /// is kept by its number until its reply lands.
    pub(in crate::runner) fn note_send(&mut self, ticket: u64, body: &[u8]) {
        let Ok(v) = serde_json::from_slice::<Json>(body) else {
            return;
        };
        if let (Some(extension), Some(send), Some(n)) = (
            v["extension"].as_str(),
            v["send"].as_str(),
            v["request"].as_u64(),
        ) {
            (self.extensions.sent).insert(ticket, (format!("{extension}.{send}"), n));
        }
    }

    /// Request `ticket`'s reply landed: its send is no longer cancelled.
    pub(in crate::runner) fn landed_send(&mut self, ticket: u64) {
        self.extensions.sent.remove(&ticket);
    }

    /// Each send let go before its reply landed (superseded, its commit
    /// refused): its implementation is told `{"extension", "cancel": n}`,
    /// so the work no one reads stops (LLP 1133.016 D15 item 2).
    pub(in crate::runner) fn cancel_let_go(&mut self) {
        let gone: Vec<u64> = (self.extensions.sent.keys())
            .filter(|t| !self.pending.iter().any(|p| p.ticket == **t))
            .copied()
            .collect();
        let handler = self.native_slot().handler();
        for ticket in gone {
            let Some((key, n)) = self.extensions.sent.remove(&ticket) else {
                continue;
            };
            self.log(format!("cancel {key} request {n}: its call was let go"));
            let extension = key.split_once('.').map_or(key.as_str(), |(e, _)| e);
            if let Some(handler) = &handler {
                let body = json!({ "extension": extension, "cancel": n });
                handler(body.to_string().into_bytes(), Reply::new(|_| {}));
            }
        }
    }

    /// The preferred values the agent set for `extension`'s names, for its
    /// implementation bound under the agent: given when it is bound and
    /// whenever one changes, whether or not anything reads the name (LLP
    /// 1133.008 D8 item 9), as `{"extension", "preferred": {name: value}}`.
    pub(in crate::runner) fn tell_preferred(&self, extension: &str) {
        let Some(handler) = self.native_slot().handler() else {
            return;
        };
        let prefix = format!("{extension}.");
        let values: serde_json::Map<String, Json> = (self.extensions.preferred.iter())
            .filter_map(|(k, v)| Some((k.strip_prefix(&prefix)?.to_owned(), v.clone())))
            .collect();
        // And the state of each permission it declares, which its answers
        // follow (LLP 1133.019 §10: under `limited`, only the selection).
        let permissions: serde_json::Map<String, Json> = (self.plan.modules.iter())
            .filter(|m| self.plan.str(m.extension) == extension)
            .flat_map(|m| {
                let list = self.substitute(m)["permissionList"].clone();
                list.as_array().cloned().unwrap_or_default()
            })
            .filter_map(|p| {
                let name = p["name"].as_str()?.to_owned();
                // A `<prefix>.*` permission stands for each one set under it.
                let under: Vec<String> = match name.strip_suffix('*') {
                    Some(pre) => (self.extensions.permission.keys())
                        .filter(|k| k.starts_with(pre))
                        .cloned()
                        .collect(),
                    None => vec![name],
                };
                Some(
                    under
                        .into_iter()
                        .map(|n| (n.clone(), json!(self.permission_state(&n)))),
                )
            })
            .flatten()
            .collect();
        // The facts a person's answer kept beside a state the platform
        // hides (`keyed.rs`), as the `facts` word.
        let mut values = values;
        if let Some(facts) = self.facts_of(extension) {
            values.insert("facts".into(), facts);
        }
        let body =
            json!({ "extension": extension, "preferred": values, "permissions": permissions });
        handler(body.to_string().into_bytes(), Reply::new(|_| {}));
    }

    /// The app went to the foreground or left it: each implementation that
    /// answers here is told, `{"extension", "foreground": bool}`, as the
    /// fact admission reads (LLP 1133.009 D10 item 3). An implementation
    /// begins in the foreground.
    pub(in crate::runner) fn tell_foreground(&self) {
        let Some(handler) = self.native_slot().handler() else {
            return;
        };
        let told: std::collections::BTreeSet<&str> = (self.plan.modules.iter())
            .filter(|m| self.route(m) != Route::Substitute)
            .map(|m| self.plan.str(m.extension))
            .collect();
        for extension in told {
            let body = json!({ "extension": extension, "foreground": !self.page.hidden });
            handler(body.to_string().into_bytes(), Reply::new(|_| {}));
        }
    }

    /// A data-code read of a bound live source, counted as a reader too.
    pub(in crate::runner) fn bound_read(&mut self, key: &str) {
        let live = (self.plan.modules.iter())
            .any(|m| m.live && !m.send && self.route(m) != Route::Substitute && self.key(m) == key);
        if live {
            self.extensions.live.insert(key.to_owned());
        }
    }

    /// Each bound live source no answer reads any longer: released, once
    /// (after a commit), `{"extension", "release": name}`. Before that, each
    /// argument group no Contract reader asks with any longer, `{"extension",
    /// "release": name, "args"}` (LLP 1133.009 D10 item 12): only once no
    /// request of the source is in flight, so the implementation has a
    /// reader's new arguments before it hears the old ones are gone. A
    /// group data code asks for is released only with the whole name.
    pub(in crate::runner) fn release_idle(&mut self) {
        let live: Vec<String> = self.extensions.live.iter().cloned().collect();
        let handler = self.native_slot().handler();
        let release = |body: Json| {
            if let Some(handler) = &handler {
                handler(body.to_string().into_bytes(), Reply::new(|_| {}));
            }
        };
        for key in live {
            let Some((extension, name)) = key.split_once('.') else {
                continue;
            };
            if !self.read_by_any(&key) {
                self.extensions.live.remove(&key);
                self.extensions.readers.remove(&key);
                self.extensions.groups.remove(&key);
                self.log(format!("release {key}: no reader is left"));
                release(json!({ "extension": extension, "release": name }));
                continue;
            }
            if self.asking(&key) {
                continue;
            }
            let resources = &self.resources;
            let readers = self.extensions.readers.entry(key.clone()).or_default();
            readers.retain(|i, _| resources.get(*i).is_some_and(Option::is_some));
            let current: BTreeSet<String> = readers.values().cloned().collect();
            let told = self.extensions.groups.entry(key.clone()).or_default();
            let gone: Vec<String> = told.difference(&current).cloned().collect();
            told.retain(|g| current.contains(g));
            for group in gone {
                self.log(format!("release {key} {group}: no reader asks with it"));
                let args: Json = serde_json::from_str(&group).unwrap_or_default();
                release(json!({ "extension": extension, "release": name, "args": args }));
            }
        }
    }

    /// Whether a request of `key` is in flight.
    pub(super) fn asking(&self, key: &str) -> bool {
        (self.pending.iter()).any(|p| {
            (p.keepable.as_ref())
                .and_then(|r| self.module_topic(&r.body))
                .as_deref()
                == Some(key)
        })
    }

    /// Whether any resource reads `key`: its source's own reader, an answer
    /// whose data code read it, or one asked again that is reading it now.
    /// A fresh answer's topics replace the last one's before its module
    /// request lands (`settlement.rs`), so that request is a reader too.
    fn read_by_any(&self, key: &str) -> bool {
        let Some(row) = self.plan.modules.iter().find(|m| self.key(m) == key) else {
            return false;
        };
        let source = self.plan.sources[row.source.0 as usize].name;
        (0..self.plan.resources.len()).any(|i| {
            self.plan.resources[i].source == source || self.watching[i].iter().any(|t| t == key)
        }) || self.pending.iter().any(|p| {
            (p.keepable.as_ref())
                .and_then(|r| self.module_topic(&r.body))
                .as_deref()
                == Some(key)
        })
    }
}

/// `value` without its `null`s, at every depth: what the call wrote.
fn written(value: &mut Json) {
    if let Json::Object(map) = value {
        map.retain(|_, v| !v.is_null());
        map.values_mut().for_each(written);
    }
}
