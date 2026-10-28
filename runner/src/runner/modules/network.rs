//! The local network under the runner (LLP 1133.036): the reach each request
//! carries for the hop rule (`crate::local_network`), and the declared
//! substitute's simulated network (D19).
//!
//! A permission whose fact is the host's (`"fact": "host"`) reads `unknown`
//! as "a reach may ask", so `unknown` holds a request under a ticket as
//! `prompt` does, where for other permissions it admits as `granted`.

use super::super::{DataSource, Runner};
use super::{CallOrigin, Json, ModulesRow, Route};
use crate::local_network::{Reach, State};
use serde_json::json;

/// The permission whose state the hop rule reads.
pub(super) const PERMISSION: &str = "local-network";

impl<D: DataSource> Runner<D> {
    /// What a request asked under `origin` carries: under the agent, with the
    /// app's local-network permission answered by the substitute, that
    /// permission's state stands for the host's fact.
    pub(in crate::runner) fn reach_of(&self, origin: CallOrigin) -> Reach {
        Reach {
            origin,
            substitute: self.local_network_substitute(),
        }
    }

    /// Whether permission `name`'s fact is the host's (`"fact": "host"`), so
    /// `unknown` is a state the agent may prefer.
    pub(super) fn host_keeps(&self, name: &str) -> bool {
        (self.plan.modules.iter()).any(|m| {
            let p = &self.substitute(m)["permission"];
            p["name"] == name && p["fact"] == "host"
        })
    }

    fn local_network_substitute(&self) -> Option<State> {
        let row = (self.plan.modules.iter())
            .find(|m| self.substitute(m)["permission"]["name"] == PERMISSION)?;
        if self.route(row) == Route::Real {
            return None;
        }
        Some(State::parse(&self.permission_state(PERMISSION)).unwrap_or(State::Prompt))
    }
}

/// Whether permission `permission` in state `state` holds a request that
/// asks under a ticket: `prompt`, and `unknown` where the host keeps the fact.
pub(super) fn asks_in(permission: &Json, state: &str) -> bool {
    state == "prompt" || state == "unknown" && permission["fact"] == "host"
}

impl<D: DataSource> Runner<D> {
    /// The declared substitute's simulated network (D19, the keyed fixture
    /// of LLP 1133.022 D19 item 2): the agent's preferred value for the
    /// `browse` source is the whole network, a list of `{"name", "type",
    /// "host", "port", "hostName"?, "addresses"?, "txt"?: {key: value}}`.
    /// A browse answers the entries of its `type`, by name; a resolve the
    /// entry its `name` and `type` select, read from the source its `from`
    /// names, or `NotFoundError`. `None` for a source with no `network`.
    pub(super) fn network_value(&self, row: &ModulesRow, sub: &Json, args: &Json) -> Option<Json> {
        let browse = match sub["network"].as_str()? {
            "browse" => row.clone(),
            "resolve" => self.sibling(row, sub["from"].as_str()?)?,
            _ => return None,
        };
        let network = self.extensions.preferred.get(&self.key(&browse));
        if let Some(code) = network.filter(|n| super::error_code(n).is_some()) {
            return Some(code.clone());
        }
        let entries = network
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default();
        let of_type = entries.iter().filter(|e| e["type"] == args["type"]);
        if sub["network"] == "browse" {
            let mut services: Vec<Json> = of_type
                .map(|e| json!({ "name": e["name"], "type": e["type"] }))
                .collect();
            let name = |s: &Json| s["name"].as_str().unwrap_or_default().to_lowercase();
            services.sort_by_key(name);
            return Some(json!({ "services": services }));
        }
        let found = of_type.clone().find(|e| e["name"] == args["name"]);
        Some(found.map_or_else(|| json!({ "error": "NotFoundError" }), service_info))
    }

    /// Whether a preferred value for a `browse` source is a network: each
    /// entry names its instance, type, host and port.
    pub(super) fn network_checked(&self, value: &Json) -> Result<(), String> {
        let entries = value
            .as_array()
            .ok_or("the network is a list of services")?;
        for e in entries {
            let fields = e["name"].is_string()
                && e["type"]
                    .as_str()
                    .is_some_and(exact_grants::is_service_type)
                && e["host"].is_string()
                && e["port"].as_u64().is_some_and(|p| (1..=65535).contains(&p));
            if !fields {
                return Err(format!(
                    "each service has a `name`, a `type` (`_<name>._tcp`), a `host` and a `port`: {e}"
                ));
            }
        }
        Ok(())
    }

    /// The keys a change to `key`'s preferred value asks again: its own, and
    /// each source that reads its network (`"from"`).
    pub(super) fn network_dependents(&self, key: &str) -> Vec<String> {
        (self.plan.modules.iter())
            .filter(|m| {
                let sub = self.substitute(m);
                sub["network"] == "resolve"
                    && self
                        .sibling(m, sub["from"].as_str().unwrap_or_default())
                        .is_some_and(|b| self.key(&b) == key)
            })
            .map(|m| self.key(m))
            .collect()
    }

    /// Whether a reader of `row` with `args` that may not start it can join
    /// it (D3): a resource reads the same group under the app's origin.
    pub(super) fn joins_running(&self, row: &ModulesRow, args: &Json) -> bool {
        if self.substitute(row)["join"] != true {
            return false;
        }
        let groups = [super::samples::group(args), args.to_string()];
        let readers = self.extensions.readers.get(&self.key(row));
        readers.into_iter().flatten().any(|(i, group)| {
            groups.contains(group)
                && self.resources.get(*i).is_some_and(Option::is_some)
                && self.origin_asked(*i).is_none_or(|o| o == CallOrigin::App)
        })
    }
}

/// A network entry as `LocalServiceInfo`: `hostName` defaults to `host`,
/// `addresses` to `[host]` when the host is an address, and `txt` is a list
/// of lower-cased keys.
fn service_info(e: &Json) -> Json {
    let host = e["host"].as_str().unwrap_or_default();
    let address = host
        .trim_matches(['[', ']'])
        .parse::<std::net::IpAddr>()
        .is_ok();
    let mut txt: Vec<Json> = (e["txt"].as_object().into_iter().flatten())
        .map(|(k, v)| json!({ "key": k.to_lowercase(), "value": v.as_str() }))
        .collect();
    txt.sort_by_key(|t| t["key"].as_str().unwrap_or_default().to_owned());
    json!({
        "name": e["name"],
        "type": e["type"],
        "host": host,
        "hostName": if e["hostName"].is_string() { e["hostName"].clone() } else { json!(host) },
        "port": e["port"],
        "addresses": match &e["addresses"] {
            Json::Array(a) => json!(a),
            _ if address => json!([host]),
            _ => json!([]),
        },
        "txt": txt,
    })
}
