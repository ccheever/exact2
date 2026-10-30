//! Members (LLP 1133.014 D10 item 1): an option another package declares on
//! a ceremony's name, as `@exact/sign-in` adds FedCM's `identity` to
//! `@exact/passkeys`' `get`. Its row in the plan names its owner
//! (`memberOf`, `passkeys.get`) and its option (`member`); the owner's
//! call carries it, and the runner reads the row's substitute:
//!
//! - **Admission.** An option that is neither the owner's (`takes`), a
//!   platform's words, nor a bound member's fails `NotSupportedError`
//!   naming it. An identity member (one with `providers`) is held to
//!   FedCM's rules as D1 and D3 keep them: one provider in `"active"` mode
//!   and none beside `publicKey`; `mediation` `"silent"` and
//!   `"conditional"` refused; each `configURL` a provider the member names;
//!   each `clientId` (and on Apple Google's `apple.clientId`) granted by the
//!   provider's line, else `SecurityError`; on the web a `web.redirectURI`
//!   within a `net.fetch` line; and what this target can offer, by the
//!   member's `status` source. A passive request leaves out a provider the
//!   target cannot offer passively, and fails `NetworkError` with none left.
//! - **The ticket.** The owner's ticket (`credential`) names the member
//!   under its option: the mode, and each provider's `configURL`,
//!   `clientId`, the audience this target's token carries (D8), the nonce,
//!   the fields and the token's form (`relay` on the web): what
//!   `packages/sign-in/test/idp.mjs` signs over.
//! - **Answers.** A typed answer of the member's `answer` type must name a
//!   provider the request did; `cancel` and the member's choices fail with
//!   the member's errors when the call carries no option of the owner's
//!   ceremony (`NetworkError` for a sign-in alone; with a passkey,
//!   passkeys' `NotAllowedError`). A dropped answer's handles go to the
//!   member's package.

use super::super::{DataSource, Runner};
use super::ceremony::{at, target};
use exact_plan::ModulesRow;
use serde_json::{json, Value as Json};

/// The words a platform adds under its own key (LLP 1133.002 D13).
const PLATFORMS: [&str; 5] = ["apple", "android", "web", "windows", "linux"];

/// FedCM's default disclosure (D4).
const FIELDS: [&str; 3] = ["name", "email", "picture"];

/// An identity request's refusal: the error, and why.
type Refused = (&'static str, String);

impl<D: DataSource> Runner<D> {
    /// Every member bound to `row`'s name: its option and its row.
    pub(super) fn members_of(&self, row: &ModulesRow) -> Vec<(String, ModulesRow)> {
        let owner = self.key(row);
        (self.plan.modules.iter())
            .filter_map(|m| {
                let sub: Json = serde_json::from_str(self.plan.str(m.substitute)).ok()?;
                (sub["memberOf"] == owner.as_str())
                    .then(|| Some((sub["member"].as_str()?.to_owned(), m.clone())))?
            })
            .collect()
    }

    /// The members `args` carries.
    pub(super) fn members_in(&self, row: &ModulesRow, args: &Json) -> Vec<(String, ModulesRow)> {
        (self.members_of(row).into_iter())
            .filter(|(option, _)| args.get(option).is_some_and(|v| !v.is_null()))
            .collect()
    }

    /// Whether a ceremony's call carries an option of the owner's own
    /// ceremony (`publicKey`).
    pub(super) fn owns_ceremony(&self, sub: &Json, args: &Json) -> bool {
        (sub["ceremony"]["options"].as_str()).is_none_or(|o| !args[o].is_null())
    }

    /// The member answering `answer`, by its `answer` fields.
    pub(super) fn member_answering(
        &self,
        row: &ModulesRow,
        answer: &Json,
    ) -> Option<(String, Json)> {
        (self.members_of(row).into_iter()).find_map(|(_, m)| {
            let sub = self.substitute(&m);
            let wants = sub["answer"].as_object()?;
            (wants.iter().all(|(k, v)| answer.get(k) == Some(v)))
                .then(|| (self.plan.str(m.extension).to_owned(), (*sub).clone()))
        })
    }

    /// The value of a member's `status` source (`getIdentityCapabilities`).
    fn member_status(&self, member: &ModulesRow, sub: &Json) -> Json {
        (sub["status"].as_str())
            .and_then(|s| self.sibling(member, s))
            .and_then(|s| self.value_of(&s))
            .unwrap_or_default()
    }

    /// Why a call of `row` with `args` is refused by its members' rules, if
    /// it is (module doc).
    pub(super) fn members_refused(&self, row: &ModulesRow, args: &Json) -> Option<Json> {
        let sub = self.substitute(row);
        let takes = sub["takes"].as_array()?;
        if args["mediation"] == "silent" {
            let why = "mediation \"silent\": no target can promise no UI (LLP 1133.014 D1)";
            return Some(json!({ "error": "NotSupportedError", "message": why }));
        }
        let members = self.members_of(row);
        let keys: Vec<String> = args
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        for key in keys {
            let ours = takes.iter().any(|t| t == key.as_str()) || PLATFORMS.contains(&key.as_str());
            if ours || args[&key].is_null() {
                continue;
            }
            let Some((_, member)) = members.iter().find(|(o, _)| *o == key) else {
                let why = format!(
                    "`{key}` is no option of {}, and no package bound here declares it as a member",
                    self.key(row)
                );
                return Some(json!({ "error": "NotSupportedError", "message": why }));
            };
            let member_sub = self.substitute(member);
            if !member_sub["providers"].is_object() {
                continue;
            }
            if let Err((code, why)) = self.identity_admitted(&sub, member, &member_sub, &key, args)
            {
                return Some(json!({ "error": code, "message": why }));
            }
        }
        None
    }

    /// An identity request's rules (module doc).
    fn identity_admitted(
        &self,
        owner: &Json,
        member: &ModulesRow,
        sub: &Json,
        option: &str,
        args: &Json,
    ) -> Result<(), Refused> {
        let unsupported = |why: String| ("NotSupportedError", why);
        let request = &args[option];
        let mode = request["mode"].as_str().unwrap_or("passive");
        let mediation = args["mediation"].as_str().unwrap_or("optional");
        let with_key = self.owns_ceremony(owner, args) && owner["ceremony"].is_object();
        let providers = request["providers"].as_array().cloned().unwrap_or_default();
        if providers.is_empty() {
            return Err((
                "TypeError",
                format!("{option}.providers names at least one provider"),
            ));
        }
        if !matches!(mode, "active" | "passive") {
            return Err((
                "TypeError",
                format!("{option}.mode is \"active\" or \"passive\", not {mode:?}"),
            ));
        }
        if matches!(mediation, "silent" | "conditional") {
            return Err(unsupported(format!(
                "mediation {mediation:?} beside {option}: no target can keep it"
            )));
        }
        if mode == "active" && providers.len() != 1 {
            return Err(unsupported(format!(
                "an active {option} request names one provider"
            )));
        }
        if mode == "active" && with_key {
            return Err(unsupported(format!(
                "an active {option} request is a press's own, never beside publicKey"
            )));
        }
        let status = self.member_status(member, sub);
        if status["available"] == false {
            return Err(unsupported(format!(
                "no {option} provider is offered on {}",
                target()
            )));
        }
        let grants = self.data.grants();
        let granted = |line: &str, id: &str| {
            grants.lines().any(|l| {
                let mut words = l.split_whitespace();
                words.next() == Some(line) && words.next() == Some(id) && words.next().is_none()
            })
        };
        let mut kept = 0;
        for (i, provider) in providers.iter().enumerate() {
            let config = provider["configURL"].as_str().unwrap_or_default();
            let Some(known) = sub["providers"][config].as_object() else {
                return Err(unsupported(format!(
                    "{option}.providers[{i}].configURL {config:?} is no provider here"
                )));
            };
            let name = known["name"].as_str().unwrap_or_default();
            let line = known["line"].as_str().unwrap_or_default();
            let client = provider["clientId"].as_str().unwrap_or_default();
            if !granted(line, client) {
                return Err((
                    "SecurityError",
                    format!("grant-missing ({name}'s clientId={client} needs `{line} {client}`)"),
                ));
            }
            if known["audience"][target()] == "apple.clientId" {
                let Some(ios) = provider["apple"]["clientId"].as_str() else {
                    return Err(unsupported(format!(
                        "{name} on {} needs apple.clientId, the iOS client's id",
                        target()
                    )));
                };
                if !granted(line, ios) {
                    return Err((
                        "SecurityError",
                        format!(
                            "grant-missing ({name}'s apple.clientId={ios} needs `{line} {ios}`)"
                        ),
                    ));
                }
            }
            if target() == "web" {
                let Some(back) = provider["web"]["redirectURI"].as_str() else {
                    return Err(unsupported(format!(
                        "{name} on the web needs web.redirectURI, the app server's return route"
                    )));
                };
                if !fetches(grants, back) {
                    return Err(("SecurityError", format!("grant-missing (web.redirectURI={back} is outside every `net.fetch` line)")));
                }
            }
            let can = &status["providers"][name];
            // Beside a passkey, a provider counts only where one sheet holds both (D1).
            let shares = !with_key || can["withPublicKey"] == true;
            match (mode, can[mode] == true && shares) {
                (_, true) => kept += 1,
                ("active", false) => {
                    return Err(unsupported(format!(
                        "{name} offers no active request on {}",
                        target()
                    )))
                }
                // A passive request leaves out what this target cannot offer (D3).
                _ => {}
            }
        }
        if kept == 0 && with_key {
            return Err(unsupported(format!(
                "no provider of this {option} request shares one sheet with a passkey on {}",
                target()
            )));
        }
        if kept == 0 {
            return Err((
                "NetworkError",
                format!(
                    "no provider of this passive {option} request is offered on {}",
                    target()
                ),
            ));
        }
        Ok(())
    }

    /// The members `args` carries, as the owner's ticket names them.
    pub(super) fn members_args(&self, row: &ModulesRow, args: &Json) -> Vec<(String, Json)> {
        (self.members_in(row, args).into_iter())
            .map(|(option, m)| {
                let sub = self.substitute(&m);
                let request = &args[&option];
                if !sub["providers"].is_object() {
                    return (option, request.clone());
                }
                let relay = (sub["relay"].as_array()).is_some_and(|r| r.iter().any(|t| t == target()));
                let mode = request["mode"].as_str().unwrap_or("passive");
                let status = self.member_status(&m, &sub);
                let owner = self.substitute(row);
                let with_key = owner["ceremony"].is_object() && self.owns_ceremony(&owner, args);
                // A passive request leaves out what this target cannot offer (D3).
                let offered = |p: &&Json| {
                    let config = p["configURL"].as_str().unwrap_or_default();
                    let name = sub["providers"][config]["name"].as_str().unwrap_or_default();
                    let can = &status["providers"][name];
                    let shares = !with_key || can["withPublicKey"] == true;
                    mode == "active" || (can["passive"] == true && shares)
                };
                let providers: Vec<Json> = (request["providers"].as_array().into_iter().flatten())
                    .filter(offered)
                    .map(|p| {
                        let config = p["configURL"].as_str().unwrap_or_default();
                        let known = &sub["providers"][config];
                        let audience = match known["audience"][target()].as_str() {
                            Some("app") => json!(self.data.app_id()),
                            Some(path) => at(p, path).cloned().unwrap_or(Json::Null),
                            None => Json::Null,
                        };
                        let fields = match &p["fields"] {
                            Json::Null => json!(FIELDS),
                            given => given.clone(),
                        };
                        json!({
                            "provider": known["name"],
                            "configURL": config,
                            "clientId": p["clientId"],
                            "audience": audience,
                            "nonce": p["params"]["nonce"],
                            "fields": fields,
                            "loginHint": p["loginHint"],
                            "domainHint": p["domainHint"],
                            "token": if relay { "relay" } else { "native" },
                            "redirectURI": if relay { p["web"]["redirectURI"].clone() } else { Json::Null },
                        })
                    })
                    .collect();
                let described = json!({
                    "mode": mode,
                    "context": request["context"].as_str().unwrap_or("signin"),
                    "providers": providers,
                });
                (option, described)
            })
            .collect()
    }

    /// A typed answer of a member's type: one of the providers the request named.
    pub(super) fn member_typed(
        &self,
        row: &ModulesRow,
        args: &Json,
        value: &Json,
    ) -> Option<Result<(), String>> {
        let (_, sub) = self.member_answering(row, value)?;
        let option = sub["member"].as_str().unwrap_or_default();
        let named: Vec<&Json> = (args[option]["providers"].as_array().into_iter().flatten())
            .map(|p| &p["configURL"])
            .collect();
        Some(match named.contains(&&value["configURL"]) {
            true => Ok(()),
            false => Err(format!(
                "{}: the answer's configURL is {}, and the request named {}",
                self.key(row),
                value["configURL"],
                Json::Array(named.into_iter().cloned().collect())
            )),
        })
    }

    /// A tap on a ticket whose call carries no option of the owner's
    /// ceremony: the first member's `cancel` or choice.
    pub(super) fn member_tapped(
        &self,
        row: &ModulesRow,
        args: &Json,
        choice: &str,
    ) -> Option<Json> {
        if self.owns_ceremony(&self.substitute(row), args) {
            return None;
        }
        let (_, m) = self.members_in(row, args).into_iter().next()?;
        let sub = self.substitute(&m);
        let answer = match choice {
            "cancel" => &sub["cancel"],
            other => &sub["choices"][other],
        };
        (!answer.is_null()).then(|| answer.clone())
    }

    /// The taps every member bound to `row` adds besides `cancel`.
    pub(super) fn members_choices(&self, row: &ModulesRow) -> Vec<String> {
        (self.members_of(row).iter())
            .flat_map(|(_, m)| {
                let sub = self.substitute(m);
                (sub["choices"].as_object().into_iter().flatten())
                    .map(|(c, _)| c.clone())
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

/// Whether a `net.fetch` line of `grants` covers `url`: the line's target
/// is the URL's origin, or a prefix of it ending at a path boundary.
fn fetches(grants: &str, url: &str) -> bool {
    grants.lines().any(|l| {
        let mut words = l.split_whitespace();
        let (Some("net.fetch"), Some(target)) = (words.next(), words.next()) else {
            return false;
        };
        let target = target.trim_end_matches('/');
        url == target || url.starts_with(&format!("{target}/"))
    })
}
