//! Pair Phone (workspace.rs `pair_open`, `pair_toggle`; serve.rs `info`,
//! `info_on`, `peer_config`, `set_peer`; ui.rs `render_pair`): a QR code a
//! phone scans to reach Ocho through the relay, for this Mac (the server
//! the app started, else `fleet serve --describe`) or for another enrolled
//! machine (`fleet serve --on M`), and whether this Mac's server keeps a
//! mobile API on each machine.
//!
//! The `PhonePair` view:
//!
//! ```text
//! visible: bool
//! chips: list<{id, label, selected}>  // "pair-this-mac", "pair-machine-{id}"
//! toggle: string        // "" or "Keep a server on {name} so the phone can use it while this Mac is away"
//! toggleLabel: string   // "On" | "Off" (primary when on)
//! toggleOn: bool
//! error: string         // warn; with `help` under it
//! help: string
//! qr: list<QrRow>       // 4 px modules on white, p_3, rounded_md
//! hint: string          // "Open Ocho on the phone and tap Scan QR Code, or paste the pairing text."
//! address: string       // "{name} · {relay}", Menlo
//! note: string          // muted
//! ```

use super::*;
use serde::Deserialize;

/// What a phone needs, as `fleet serve --json|--describe` prints it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ServeInfo {
    /// The relay the phone dials.
    pub relay: String,
    /// The machine's name.
    pub name: String,
    /// QR rows of `0`/`1`, quiet zone included.
    pub qr: Vec<String>,
    /// Which enrolled machines this Mac's server keeps a mobile API on.
    pub peer_config: HashMap<String, bool>,
}

/// The open Pair Phone card.
#[derive(Clone, Debug, PartialEq)]
pub struct PhonePair {
    /// `None` for this Mac, else the machine chosen.
    pub machine: Option<String>,
    /// The pairing details, or what went wrong ("Loading…" first).
    pub info: Result<ServeInfo, String>,
    /// Per machine: keep a mobile API there (absent means on).
    pub peers: HashMap<String, bool>,
    /// The details came from the server this app started.
    pub started_here: bool,
    /// The open's number; older replies are dropped.
    pub request: u64,
}

const LOADING: &str = "Loading…";
const RELAY_HELP: &str = "Phones reach Ocho through the relay. Set its secret once with: fleet serve --relay-secret SECRET  (see relay/README.md), then reopen this dialog.";

fn parse(text: &str) -> Result<ServeInfo, String> {
    serde_json::from_str(text).map_err(|e| format!("invalid pairing details: {e}"))
}

impl Workspace {
    /// PairPhone, or a chip: open the card for this Mac or `machine`.
    pub fn pair_open(&mut self, machine: Option<String>) {
        let request = match &self.overlay {
            Overlay::PairPhone(p) => p.request + 1,
            _ => self.phone_requests + 1,
        };
        self.phone_requests = request;
        self.overlay = Overlay::PairPhone(Box::new(PhonePair {
            machine: machine.clone(),
            info: Err(LOADING.into()),
            peers: HashMap::new(),
            started_here: false,
            request,
        }));
        match &machine {
            None => {
                self.queue(
                    "host",
                    vec!["serve-running".into()],
                    String::new(),
                    Reply::ServeRunning(request),
                );
            }
            Some(id) => {
                let argv = vec![
                    "serve".into(),
                    "--on".into(),
                    id.clone(),
                    "--json".into(),
                    "--no-qr".into(),
                ];
                self.queue("serve", argv, String::new(), Reply::ServeInfo(request));
            }
        }
        let argv = vec!["serve".into(), "--describe".into(), "--no-qr".into()];
        self.queue("serve", argv, String::new(), Reply::ServePeers(request));
    }

    fn open_pair(&mut self, request: u64) -> Option<&mut PhonePair> {
        match &mut self.overlay {
            Overlay::PairPhone(p) if p.request == request => Some(p),
            _ => None,
        }
    }

    /// A pairing reply.
    pub(super) fn phone_reply(&mut self, what: Reply, result: Result<String, String>) {
        match what {
            Reply::ServeRunning(request) => {
                let running = result.ok().filter(|text| !text.trim().is_empty());
                match running {
                    Some(text) => {
                        if let Some(p) = self.open_pair(request) {
                            p.info = parse(&text);
                            p.started_here = true;
                        }
                    }
                    None => {
                        if self.open_pair(request).is_some() {
                            let argv = vec!["serve".into(), "--describe".into(), "--no-qr".into()];
                            self.queue("serve", argv, String::new(), Reply::ServeInfo(request));
                        }
                    }
                }
            }
            Reply::ServeInfo(request) => {
                if let Some(p) = self.open_pair(request) {
                    p.info = result.and_then(|text| parse(&text));
                }
            }
            Reply::ServePeers(request) => {
                let peers = result
                    .ok()
                    .and_then(|text| parse(&text).ok())
                    .map(|info| info.peer_config)
                    .unwrap_or_default();
                if let Some(p) = self.open_pair(request) {
                    p.peers = peers;
                }
            }
            Reply::ServePeer {
                request,
                machine,
                on,
            } => {
                let still_open = self.open_pair(request).is_some();
                match result {
                    Ok(_) if still_open => {
                        let selected = self.open_pair(request).and_then(|p| p.machine.clone());
                        self.pair_open(selected);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        if let Some(p) = self.open_pair(request) {
                            p.peers.insert(machine, !on);
                        }
                        self.set_error(error);
                    }
                }
            }
            _ => {}
        }
    }

    /// The machine chip's On/Off (`pair_toggle`): shown at once, the CLI
    /// confirms or reverts it.
    fn pair_toggle(&mut self, machine: String) {
        let Overlay::PairPhone(p) = &mut self.overlay else {
            return;
        };
        let on = !p.peers.get(&machine).copied().unwrap_or(true);
        p.peers.insert(machine.clone(), on);
        let request = p.request;
        let argv = vec![
            "serve".into(),
            "--peer".into(),
            format!("{machine}={}", if on { "on" } else { "off" }),
        ];
        self.queue(
            "serve",
            argv,
            String::new(),
            Reply::ServePeer {
                request,
                machine,
                on,
            },
        );
    }

    /// Presses on the card; `true` when `id` was one.
    pub(super) fn phone_press(&mut self, id: &str) -> bool {
        if !matches!(self.overlay, Overlay::PairPhone(_)) {
            return false;
        }
        if id == "pair-close" {
            self.overlay = Overlay::None;
        } else if id == "pair-this-mac" {
            self.pair_open(None);
        } else if let Some(machine) = id.strip_prefix("pair-machine-") {
            self.pair_open(Some(machine.to_string()));
        } else if id == "pair-toggle" {
            if let Overlay::PairPhone(p) = &self.overlay {
                if let Some(machine) = p.machine.clone() {
                    self.pair_toggle(machine);
                }
            }
        } else {
            return false;
        }
        true
    }

    /// The `PhonePair` view.
    pub fn phone_view(&self) -> serde_json::Value {
        let Overlay::PairPhone(p) = &self.overlay else {
            return serde_json::json!({ "visible": false });
        };
        let mut chips = vec![serde_json::json!({
            "id": "pair-this-mac", "label": "This Mac", "selected": p.machine.is_none(),
        })];
        let mut toggle = String::new();
        let mut on = true;
        for m in self.state.machines.iter().filter(|m| !m.local) {
            let selected = p.machine.as_deref() == Some(m.id.as_str());
            chips.push(serde_json::json!({
                "id": format!("pair-machine-{}", m.id), "label": m.name, "selected": selected,
            }));
            if selected {
                on = p.peers.get(&m.id).copied().unwrap_or(true);
                toggle = format!(
                    "Keep a server on {} so the phone can use it while this Mac is away",
                    m.name
                );
            }
        }
        let (error, help, qr, address, note) = match &p.info {
            Err(error) => (
                error.clone(),
                RELAY_HELP.to_string(),
                Vec::new(),
                String::new(),
                String::new(),
            ),
            Ok(info) => {
                let qr: Vec<serde_json::Value> = info
                    .qr
                    .iter()
                    .enumerate()
                    .map(|(i, row)| {
                        let cells: Vec<serde_json::Value> = row
                            .chars()
                            .enumerate()
                            .map(|(j, c)| serde_json::json!({ "id": format!("{i}-{j}"), "dark": c == '1' }))
                            .collect();
                        serde_json::json!({ "id": format!("qr-{i}"), "cells": cells })
                    })
                    .collect();
                let note = if p.machine.is_some() {
                    "This server runs on that machine until stopped there and shares Ocho's token, so a phone paired with this Mac can use it too."
                } else if p.started_here {
                    "The mobile API runs while Ocho is open and answers through the relay, so the phone works from anywhere. A phone paired here also learns the machines above and moves to one when this Mac is away."
                } else {
                    "The server this Mac started is not running; pairing uses the saved token and the relay."
                };
                (
                    String::new(),
                    String::new(),
                    qr,
                    format!("{} · {}", info.name, info.relay),
                    note.to_string(),
                )
            }
        };
        serde_json::json!({
            "visible": true,
            "chips": chips,
            "toggle": toggle,
            "toggleLabel": if on { "On" } else { "Off" },
            "toggleOn": on,
            "error": error,
            "help": help,
            "qr": qr,
            "hint": if p.info.is_ok() { "Open Ocho on the phone and tap Scan QR Code, or paste the pairing text." } else { "" },
            "address": address,
            "note": note,
        })
    }
}
