//! Held device requests (LLP 1069.007 D3): under the agent a capability's
//! arm never reaches the OS. It holds the request under a ticket from the
//! runner's own counter, so `state.pending` lists every hold beside the
//! requests in flight and no host number can alias a fetch. A hold is not
//! I/O: it stays out of [`Runner::has_pending`], so no host's clock waits
//! on it (`clock settle` returns `reason: "device"` instead).
//!
//! A hold ends exactly once: answered or cancelled by the agent (`tap @t`,
//! `type @t`, [`Runner::answer_hold`]), or retired when its node goes. A
//! second answer, or one to a retired hold, is refused by name
//! (`not pending: @7`). A reload drops every hold, as it drops every
//! request (LLP 1016).

use super::{DataSource, Runner};
use exact_kernel::ViewId;

/// A device request a host holds for the agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Hold {
    /// The runner's ticket: the agent's `@N`.
    pub ticket: u64,
    /// The capability's name in `pending` (`pick`, `share`, `auth`, …).
    pub capability: String,
    /// The requesting node's `testId` (its id when it has none), or the
    /// resource an HTTP-shaped request answers.
    pub name: String,
    /// The requesting node: the hold is retired when it is removed.
    pub node: Option<ViewId>,
    /// The capability's inspection summary as a JSON object (D7): what a
    /// test checks the app asked for, never a credential.
    pub args: String,
    /// What `tap @t <choice>` accepts besides `cancel`.
    pub choices: Vec<String>,
    /// Whether `type @t <value>` answers it.
    pub takes_value: bool,
    /// An HTTP-shaped request held under its own ticket (an auth session,
    /// LLP 1069.006 D7): retired when the runner forgets that ticket.
    pub request: bool,
}

/// How the agent answered a hold.
#[derive(Clone, Debug, PartialEq)]
pub enum HoldAnswer {
    /// `tap @t <choice>`: `cancel`, or one of the hold's choices.
    Choice(String),
    /// `type @t <value>`: the value, which the capability validates and
    /// delivers; the runner never journals it.
    Value(String),
}

impl<D: DataSource> Runner<D> {
    /// Hold a device request for the agent: a fresh ticket, and a journal
    /// line (`device pick 7 held (agent)`). `args` is the capability's
    /// inspection summary, a JSON object.
    pub fn hold(
        &mut self,
        capability: &str,
        node: Option<ViewId>,
        args: &str,
        choices: &[&str],
        takes_value: bool,
    ) -> u64 {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let name = node
            .and_then(|id| {
                let n = self.kernel.node(id)?;
                Some(
                    n.props
                        .str(exact_kernel::PropId::TestId)
                        .map_or_else(|| id.to_string(), str::to_owned),
                )
            })
            .unwrap_or_else(|| capability.to_owned());
        self.log(format!("device {capability} {ticket} held (agent)"));
        self.device_holds.push(Hold {
            ticket,
            capability: capability.to_owned(),
            name,
            node,
            args: args.to_owned(),
            choices: choices.iter().map(|c| c.to_string()).collect(),
            takes_value,
            request: false,
        });
        ticket
    }

    /// Hold an HTTP-shaped request the runner already ticketed (an auth
    /// session): the agent answers it by that ticket, with a value or
    /// `cancel`; it leaves the clock's wait set, and is retired with its
    /// ticket (superseded, or its resource gone).
    pub fn hold_request(&mut self, ticket: u64, capability: &str, name: &str, args: &str) {
        self.log(format!("device {capability} {ticket} held (agent)"));
        self.device_holds.push(Hold {
            ticket,
            capability: capability.to_owned(),
            name: name.to_owned(),
            node: None,
            args: args.to_owned(),
            choices: Vec::new(),
            takes_value: true,
            request: true,
        });
    }

    /// Every device request held for the agent, oldest first.
    pub fn device_holds(&self) -> &[Hold] {
        &self.device_holds
    }

    /// End hold `ticket` with the agent's answer, validated against what
    /// the hold accepts, in one step: the hold is gone before the host
    /// delivers the answer. Refused by name when no such hold is live.
    pub fn answer_hold(&mut self, ticket: u64, answer: &HoldAnswer) -> Result<Hold, String> {
        let Some(pos) = self.device_holds.iter().position(|h| h.ticket == ticket) else {
            return Err(format!("not pending: @{ticket}"));
        };
        let hold = &self.device_holds[pos];
        let line = match answer {
            HoldAnswer::Choice(c) if c == "cancel" => "cancelled".to_owned(),
            HoldAnswer::Choice(c) if hold.choices.contains(c) => format!("answered: {c}"),
            HoldAnswer::Choice(c) => {
                let mut takes = vec!["cancel".to_owned()];
                takes.extend(hold.choices.iter().cloned());
                return Err(format!(
                    "@{ticket} ({}): tap takes {}, not {c}",
                    hold.capability,
                    takes.join(" | ")
                ));
            }
            // A picker's files are checked against its input before the
            // hold is spent (LLP 1069.002 D9), so a refused answer can be
            // corrected.
            HoldAnswer::Value(v) if hold.capability == "pick" => {
                if let Err(e) = self.check_pick(hold.node, v) {
                    return Err(format!("@{ticket} (pick): {e}"));
                }
                let n = super::picker::answer_paths(v).len();
                format!("answered: {n} {}", if n == 1 { "item" } else { "items" })
            }
            // An export's answer is where the copy goes (LLP 1069.010 D3):
            // an absolute path on the driver's machine, never journalled.
            HoldAnswer::Value(v) if hold.capability == "export" => {
                let v = v.trim();
                if !(v.starts_with('/') || v.get(1..3) == Some(":\\")) || v.ends_with(['/', '\\']) {
                    return Err(format!(
                        "@{ticket} (export): type an absolute file path to save to"
                    ));
                }
                "answered: a file".to_owned()
            }
            // A picker's answer is what the person would have chosen (LLP
            // 1069.010 D2): absolute paths, one unless `multiple`.
            HoldAnswer::Value(v)
                if matches!(
                    hold.capability.as_str(),
                    "open-file" | "open-directory" | "save-file"
                ) =>
            {
                let paths = crate::file_pickers::answer_paths(v);
                let multiple = hold.args.contains("\"multiple\":true");
                if paths.is_empty() || paths.iter().any(|p| !p.starts_with('/')) {
                    return Err(format!(
                        "@{ticket} ({}): type an absolute path",
                        hold.capability
                    ));
                }
                if paths.len() > 1 && !multiple {
                    return Err(format!(
                        "@{ticket} ({}): one path, not {}",
                        hold.capability,
                        paths.len()
                    ));
                }
                let n = paths.len();
                format!("answered: {n} {}", if n == 1 { "item" } else { "items" })
            }
            // An auth callback is checked as a host checks a completion
            // (LLP 1069.006 D3, D7), before the hold is spent.
            HoldAnswer::Value(v) if hold.capability == "auth" => {
                let checked = self.device_links.auth.map_or(
                    Err("openAuthSession is not linked into this artifact".into()),
                    |auth| (auth.check_answer)(self, ticket, v),
                );
                if let Err(e) = checked {
                    return Err(format!("@{ticket} (auth): {e}"));
                }
                "answered: a callback".to_owned()
            }
            HoldAnswer::Value(_) if hold.takes_value => "answered: a value".to_owned(),
            HoldAnswer::Value(_) => {
                return Err(format!(
                    "@{ticket} ({}): answered by tap, not type",
                    hold.capability
                ))
            }
        };
        let hold = self.device_holds.remove(pos);
        self.log(format!("device {} {ticket} {line}", hold.capability));
        if hold.capability == "auth" {
            match answer {
                HoldAnswer::Value(url) => crate::auth::settle(self, ticket, 200, url),
                HoldAnswer::Choice(_) => crate::auth::settle(self, ticket, 499, "cancelled"),
            }
        }
        Ok(hold)
    }

    /// Retire every hold whose node is gone (after a commit).
    pub(super) fn retire_holds(&mut self) {
        let kernel = &self.kernel;
        let pending = &self.pending;
        let (live, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut self.device_holds)
            .into_iter()
            .partition(|h| {
                h.node.is_none_or(|id| kernel.node(id).is_some())
                    && (!h.request || pending.iter().any(|p| p.ticket == h.ticket))
            });
        self.device_holds = live;
        for hold in gone {
            self.log(format!(
                "device {} {} retired",
                hold.capability, hold.ticket
            ));
        }
    }
}
