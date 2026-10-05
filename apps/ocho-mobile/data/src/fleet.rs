//! `GET /api/fleet`'s answer, the part the phone reads: every machine's
//! sessions, the desktop's windows and tabs (`desktop.json` as `fleet serve`
//! publishes it), and the peers that can answer when the Mac cannot.

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::HashMap;

/// One session, as much of `internal/core/model.go`'s `Session` as is drawn.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Session {
    /// The execution id.
    #[serde(default)]
    pub id: String,
    /// `claude`, `codex`, …
    #[serde(default)]
    pub provider: String,
    /// The title Fleet keeps.
    #[serde(default)]
    pub title: String,
    /// `running`, `blocked`, `idle`, `starting`, `closed`, `exited`, …
    #[serde(default)]
    pub state: String,
    /// What it is doing, while it works.
    #[serde(default)]
    pub status_text: String,
    /// Its last reply, at most 600 characters.
    #[serde(default)]
    pub last_message: String,
    /// The tmux pane a terminal session lives in.
    #[serde(default)]
    pub tmux_pane: String,
    /// A Codex app-server session's socket.
    #[serde(default)]
    pub codex_socket: String,
    /// The provider's own id: a Codex session's thread.
    #[serde(default)]
    pub native_id: String,
    /// A Claude Remote Control session: a worker Claude in a tmux pane,
    /// bridged to claude.ai.
    #[serde(default)]
    pub claude_remote: bool,
    /// An EAS session (the cloud worker's).
    #[serde(default)]
    pub eas: bool,
    /// What it can do (`attach`, `interrupt`, …).
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl Session {
    /// Codex's voice can join it: a live Codex app-server thread (Fleet's
    /// voice bridge needs its connection; an EAS worker exposes none).
    pub fn can_talk(&self) -> bool {
        self.provider == "codex"
            && !self.codex_socket.is_empty()
            && !self.native_id.is_empty()
            && !self.eas
            && !self.ended()
    }

    /// Working: the spinner.
    pub fn working(&self) -> bool {
        matches!(self.state.as_str(), "running" | "starting")
    }

    /// Waiting on an approval or a question.
    pub fn blocked(&self) -> bool {
        self.state == "blocked" || self.state == "awaiting approval"
    }

    /// Gone: nothing will answer a message.
    pub fn ended(&self) -> bool {
        matches!(self.state.as_str(), "closed" | "exited")
    }

    /// How a message reaches it: a Codex app-server session queues it behind
    /// any running turn; an EAS session takes a message; anything in a
    /// terminal takes the text as typed input. A
    /// Claude Remote Control worker is the latter: an interactive Claude Code
    /// in a tmux pane on its machine, whose composer takes typing as any
    /// Claude terminal's does (the old phone app sent these to claude.ai).
    pub fn send_route(&self) -> SendRoute {
        if self.eas {
            SendRoute::Message
        } else if !self.codex_socket.is_empty() {
            if self.native_id.is_empty() {
                SendRoute::None(
                    "Open this conversation in Codex once before sending from the phone.",
                )
            } else {
                SendRoute::Queue
            }
        } else if !self.tmux_pane.is_empty() && self.capabilities.iter().any(|c| c == "attach") {
            SendRoute::Input
        } else {
            SendRoute::None(if self.claude_remote {
                "This Remote Control session has no terminal — reply in Claude."
            } else {
                "This session has no terminal to type into."
            })
        }
    }

    /// The row's second line: what it is doing, else the start of its last
    /// reply.
    pub fn status_line(&self) -> String {
        let text = if self.working() || self.blocked() {
            if self.status_text.trim().is_empty() {
                if self.blocked() {
                    "Needs your input"
                } else {
                    "Working…"
                }
                .to_string()
            } else {
                self.status_text.clone()
            }
        } else {
            self.last_message.clone()
        };
        let line = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("");
        ocho_data::markdown_doc::runs_text(&ocho_data::markdown_doc::inline(line))
    }
}

/// How the composer reaches a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendRoute {
    /// `POST …/input {text, enter}`.
    Input,
    /// `POST …/message {request_id, text}`.
    Message,
    /// `POST …/codex-client`: attach to the Codex thread, `send` (Fleet holds
    /// the message until the running turn ends), detach.
    Queue,
    /// Not from here; why.
    None(&'static str),
}

/// A machine and what was last observed on it.
#[derive(Clone, Debug, Default)]
pub struct Machine {
    /// Its name.
    pub name: String,
    /// Why the last observation failed, if it did.
    pub error: String,
    /// Its sessions can't be trusted: the last observation failed and the
    /// last good one is older than [`OBSERVATION_GRACE_S`].
    pub stale: bool,
    /// Its sessions by id.
    pub sessions: HashMap<String, Session>,
}

/// How long a machine's last good observation stands through failures. One
/// failed SSH observe already takes its 25 s deadline; the desktop holds a
/// session's status 30 s past its observation (`SESSION_STATUS_MAX_AGE_MS`),
/// which a single timeout nearly spends, so a transient failure greyed every
/// session on the machine.
pub const OBSERVATION_GRACE_S: f64 = 60.0;

/// One desktop tab, as `fleet serve` publishes `desktop.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    /// `machine:session:readonly`, `folder:…`, `shell:…`, …
    #[serde(default)]
    pub key: String,
    /// The rail's title, `<title> · <machine name>` for a session.
    #[serde(default)]
    pub title: String,
    /// The session's machine.
    #[serde(default)]
    pub machine: String,
    /// The session's id.
    #[serde(default)]
    pub session: String,
    /// Nesting under folders, when the server publishes it.
    #[serde(default)]
    pub depth: u32,
    /// A folder row, when the server publishes it (else read from the key).
    #[serde(default)]
    pub folder: bool,
    /// When the tab's agent finished a turn nobody has read (Unix ms), on
    /// the desktop or a phone; zero when read.
    #[serde(default)]
    pub unread_at: u64,
}

impl Tab {
    /// A folder: a heading over the tabs after it.
    pub fn is_folder(&self) -> bool {
        self.folder || self.key.starts_with("folder:")
    }
}

/// One desktop window: its tabs in rail order, and which is open (1-based;
/// 0 is the manager).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// The tabs.
    #[serde(default)]
    pub tabs: Vec<Tab>,
    /// The open tab, 1-based.
    #[serde(default)]
    pub active: usize,
}

/// A peer: another machine running the same server.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    /// Its machine id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Where it answers on the relay, when it connects with its own key.
    #[serde(default)]
    pub route: String,
}

/// One `/api/fleet` answer.
#[derive(Clone, Debug, Default)]
pub struct Fleet {
    /// The answering server's run; a new one restarts the cursor.
    pub instance: String,
    /// Fingerprint of the phone's view (`view=phone`), empty from an older
    /// server.
    pub tag: String,
    /// The cursor.
    pub version: u64,
    /// Every machine by id.
    pub machines: HashMap<String, Machine>,
    /// The desktop's windows; empty when the answering machine runs none.
    pub windows: Vec<Window>,
    /// Peers that may answer instead.
    pub peers: Vec<Peer>,
    /// The answering machine.
    pub machine: String,
    /// Its name.
    pub name: String,
}

impl Fleet {
    /// Read an answer; `None` when it is not one.
    pub fn read(json: &Json) -> Option<Fleet> {
        let instance = json.get("instance")?.as_str()?.to_string();
        let text = |v: &Json, k: &str| v.get(k).and_then(Json::as_str).unwrap_or("").to_string();
        let mut machines = HashMap::new();
        for m in json
            .pointer("/fleet/machines")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let sessions = m
                .pointer("/last/sessions")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .filter_map(|s| serde_json::from_value::<Session>(s.clone()).ok())
                .map(|s| (s.id.clone(), s))
                .collect();
            let error = text(m, "error");
            // Both times are the server's: the answer's, and the last good
            // observation's (kept, with its sessions, through a failure).
            let age = match (
                rfc3339(&text(json, "updated_at")),
                rfc3339(&text(m.get("last").unwrap_or(&Json::Null), "at")),
            ) {
                (Some(now), Some(at)) => Some(now - at),
                _ => None,
            };
            let stale = !error.is_empty() && age.is_none_or(|age| age >= OBSERVATION_GRACE_S);
            machines.insert(
                text(m, "id"),
                Machine {
                    name: text(m, "name"),
                    error,
                    stale,
                    sessions,
                },
            );
        }
        let windows = json
            .pointer("/desktop/windows")
            .and_then(|w| serde_json::from_value::<Vec<Window>>(w.clone()).ok())
            .unwrap_or_default();
        let peers = json
            .get("peers")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter(|p| p.get("enabled").and_then(Json::as_bool).unwrap_or(false))
            .map(|p| Peer {
                id: text(p, "machine_id"),
                name: text(p, "name"),
                route: Some(text(p, "route"))
                    .filter(|r| crate::api::is_route(r))
                    .unwrap_or_default(),
            })
            .filter(|p| !p.id.is_empty())
            .collect();
        Some(Fleet {
            instance,
            tag: text(json, "tag"),
            version: json.get("version").and_then(Json::as_u64).unwrap_or(0),
            machines,
            windows,
            peers,
            machine: text(json, "machine"),
            name: text(json, "name"),
        })
    }

    /// A session wherever it is observed.
    pub fn session(&self, machine: &str, id: &str) -> Option<&Session> {
        self.machines.get(machine)?.sessions.get(id)
    }

    /// A machine's name, else a short id.
    pub fn machine_name(&self, machine: &str) -> String {
        match self.machines.get(machine) {
            Some(m) if !m.name.is_empty() => m.name.clone(),
            _ => machine.chars().take(8).collect(),
        }
    }
}

/// An RFC 3339 time (`2026-10-02T02:43:38.6Z`, `…-07:00`) as Unix seconds.
pub fn rfc3339(text: &str) -> Option<f64> {
    let b = text.as_bytes();
    let num = |r: std::ops::Range<usize>| text.get(r)?.parse::<i64>().ok();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ') {
        return None;
    }
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let mut i = 19;
    let mut frac = 0.0;
    if b.get(i) == Some(&b'.') {
        let start = i;
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        frac = text[start..i].parse::<f64>().ok()?;
    }
    let offset = match b.get(i)? {
        b'Z' | b'z' => 0,
        sign @ (b'+' | b'-') => {
            let minutes = num(i + 1..i + 3)? * 60 + num(i + 4..i + 6)?;
            if *sign == b'-' {
                -minutes
            } else {
                minutes
            }
        }
        _ => return None,
    };
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + h * 3600 + mi * 60 + sec - offset * 60) as f64 + frac)
}

/// `"arborist / fizz · Eliots-MacBook-Pro.local"` → its two halves.
pub fn split_title(title: &str) -> (String, String) {
    match title.rsplit_once(" · ") {
        Some((name, machine)) => (name.to_string(), machine.to_string()),
        None => (title.to_string(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub fn sample() -> Json {
        json!({
            "instance": "i1", "version": 7, "machine": "mac", "name": "Mac",
            "fleet": {"machines": [
                {"id": "mac", "name": "Mac", "error": "", "last": {"sessions": [
                    {"id": "s1", "provider": "claude", "title": "Fix login", "state": "running",
                     "status_text": "Reading files…", "tmux_pane": "%1", "capabilities": ["attach"]},
                    {"id": "s2", "provider": "codex", "title": "Plan", "state": "idle",
                     "last_message": "**Done** — shipped.\nMore", "codex_socket": "/tmp/s", "native_id": "t1"}
                ]}},
                {"id": "redwood", "name": "redwood", "last": {"sessions": [
                    {"id": "r1", "provider": "claude", "state": "idle", "claude_remote": true,
                     "tmux_pane": "%14", "capabilities": ["attach", "terminate", "interrupt"]},
                    {"id": "r2", "provider": "claude", "state": "idle", "claude_remote": true}
                ]}},
                {"id": "sprite", "name": "Sprite", "error": "Sprite is sleeping", "last": {"sessions": [
                    {"id": "s3", "state": "idle"}
                ]}}
            ]},
            "desktop": {"windows": [{"tabs": [
                {"key": "folder:a", "title": "work"},
                {"key": "mac:s1:false", "title": "Fix login · Mac", "machine": "mac", "session": "s1"},
                {"key": "sprite:s3:false", "title": "Nap · Sprite", "machine": "sprite", "session": "s3"},
                {"key": "shell:x", "title": "zsh"}
            ], "active": 2}]},
            "peers": [{"machine_id": "redwood", "name": "redwood", "enabled": true},
                      {"machine_id": "off", "name": "off", "enabled": false}]
        })
    }

    #[test]
    fn reads_sessions_tabs_and_peers() {
        let f = Fleet::read(&sample()).unwrap();
        assert_eq!(f.version, 7);
        assert!(f.session("mac", "s1").unwrap().working());
        assert_eq!(
            f.session("mac", "s2").unwrap().status_line(),
            "Done — shipped."
        );
        assert_eq!(f.windows[0].tabs.len(), 4);
        assert!(f.windows[0].tabs[0].is_folder());
        assert_eq!(
            f.peers,
            vec![Peer {
                id: "redwood".into(),
                name: "redwood".into(),
                route: String::new(),
            }]
        );
        assert_eq!(split_title("a / b · Mac"), ("a / b".into(), "Mac".into()));
    }

    #[test]
    fn times_read_in_any_offset() {
        assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(0.0));
        let z = rfc3339("2026-10-02T02:44:08.154427Z").unwrap();
        let pdt = rfc3339("2026-10-01T19:44:08.154427-07:00").unwrap();
        assert!((z - pdt).abs() < 1e-6);
        assert_eq!(
            rfc3339("2026-10-02T02:43:38Z")
                .map(|t| z - t)
                .map(f64::round),
            Some(30.0)
        );
        assert_eq!(rfc3339("yesterday"), None);
    }

    #[test]
    fn a_failed_observation_keeps_recent_sessions_live() {
        let answer = |last_at: &str| {
            json!({"instance": "i", "version": 1, "updated_at": "2026-10-01T19:44:08-07:00",
                   "fleet": {"machines": [{"id": "r", "name": "redwood",
                     "error": "SSH redwood (observe): context deadline exceeded",
                     "last": {"at": last_at, "sessions": [{"id": "s", "state": "running"}]}}]}})
        };
        let recent = Fleet::read(&answer("2026-10-02T02:43:38.6Z")).unwrap();
        assert!(!recent.machines["r"].stale, "30 s: one timed-out observe");
        let old = Fleet::read(&answer("2026-10-02T02:42:00Z")).unwrap();
        assert!(
            old.machines["r"].stale,
            "two minutes without a good observation"
        );
    }

    #[test]
    fn send_routes_follow_the_session_kind() {
        let f = Fleet::read(&sample()).unwrap();
        assert_eq!(
            f.session("mac", "s1").unwrap().send_route(),
            SendRoute::Input
        );
        assert_eq!(
            f.session("mac", "s2").unwrap().send_route(),
            SendRoute::Queue
        );
        assert!(matches!(
            f.session("sprite", "s3").unwrap().send_route(),
            SendRoute::None(_)
        ));
    }
}
