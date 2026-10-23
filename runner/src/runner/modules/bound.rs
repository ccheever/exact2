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
        if real.is_empty() && agent.is_empty() {
            return;
        }
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
                let answer = match self.decide(row, row.send) {
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

    /// A bound name's read or send, for the native slot: a live source's
    /// reader is counted, so its release can be said.
    pub(in crate::runner) fn bound_request(&mut self, row: &ModulesRow, args: Json) -> Request {
        Request::native(self.forward_body(row, args))
    }

    /// The body of a call of `row` for its implementation, `{"extension",
    /// "source"|"send", "args"}`; under the agent with the preferred value
    /// the agent set for it, as `preferred` (LLP 1133.009 D10 item 11). A
    /// live source's reader is counted, so its release can be said.
    pub(in crate::runner) fn forward_body(&mut self, row: &ModulesRow, args: Json) -> Vec<u8> {
        let verb = if row.send { "send" } else { "source" };
        // An option the call omits is left out, so it stays the platform's (D2).
        let mut args = args;
        written(&mut args);
        let key = self.key(row);
        let mut body = json!({ "extension": self.plan.str(row.extension), verb: self.plan.str(row.declared), "args": args });
        if self.route(row) == Route::Agent {
            if let Some(preferred) = self.extensions.preferred.get(&key) {
                body["preferred"] = preferred.clone();
            }
        }
        if row.live && !row.send {
            self.extensions.live.insert(key);
        }
        body.to_string().into_bytes()
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
        let body = json!({ "extension": extension, "preferred": values });
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
    /// (after a commit).
    pub(in crate::runner) fn release_idle(&mut self) {
        let idle: Vec<String> = (self.extensions.live.iter())
            .filter(|key| !self.read_by_any(key))
            .cloned()
            .collect();
        for key in idle {
            self.extensions.live.remove(&key);
            let Some((extension, name)) = key.split_once('.') else {
                continue;
            };
            self.log(format!("release {key}: no reader is left"));
            if let Some(handler) = self.native_slot().handler() {
                let body = json!({ "extension": extension, "release": name });
                handler(body.to_string().into_bytes(), Reply::new(|_| {}));
            }
        }
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
