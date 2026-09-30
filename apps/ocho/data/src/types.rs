//! The fleet's records, as the `fleet` CLI prints them: a port of the GPUI
//! desktop's `backend.rs` types (origin/main e577272), without the process
//! plumbing. Instants are milliseconds on the host clock (`f64`), so the
//! same crate runs in the web's wasm.

use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

// Go encodes an uninitialized slice or map as null. Treat that the same as an
// omitted collection so partial events remain compatible across helper builds.
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// The shell change that would put an updated provider ahead of a stale copy
/// on PATH, as printed by `fleet machine fix-path`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct PathFix {
    /// The shell configuration file that would be appended to.
    pub file: String,
    /// Exactly the line that would be added.
    pub line: String,
    /// The directory being moved ahead on PATH.
    pub directory: String,
    /// True when the line is already present, so only a new shell is needed.
    pub applied: bool,
    pub shell: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Machine {
    pub sprite_source: String,
    pub sprite: Option<SpriteBinding>,
    /// Local receipt time of the latest failed machine observation. A later
    /// successful session patch can recover that session independently.
    #[serde(skip)]
    pub observation_failed_at: Option<f64>,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default, deserialize_with = "null_default")]
    pub tags: Vec<String>,
    /// Default permission mode per provider for sessions launched here.
    #[serde(default, deserialize_with = "null_default")]
    pub permissions: HashMap<String, String>,
    #[serde(default)]
    pub local: bool,
    #[serde(default)]
    pub last: Option<Snapshot>,
    #[serde(default)]
    pub error: String,
    /// Set by `fleet snapshot` when the machine's helper does not match this viewer.
    #[serde(default)]
    pub helper_status: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct SpriteBinding {
    pub organization: String,
    pub name: String,
    pub id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Snapshot {
    #[serde(default)]
    pub hostname: String,
    /// The helper user's home directory, so session folders can be shown as `~/…`.
    #[serde(default)]
    pub home: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub cpu: String,
    #[serde(default)]
    pub cores: i64,
    #[serde(default)]
    pub cpu_percent: f64,
    #[serde(default)]
    pub memory_total: u64,
    #[serde(default)]
    pub memory_used: u64,
    #[serde(default, deserialize_with = "null_default")]
    pub versions: HashMap<String, String>,
    #[serde(default, deserialize_with = "null_default")]
    pub sessions: Vec<Session>,
    #[serde(default, deserialize_with = "null_default")]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub live_inventory: bool,
    #[serde(default)]
    pub at: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Session {
    #[serde(default)]
    pub claude_remote: bool,
    /// Local, monotonic freshness clock; never deserialize a helper's clock.
    /// Only advancing observations renew it, not replayed inventory metadata.
    #[serde(skip)]
    pub observation_received_at: Option<f64>,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub native_id: String,
    #[serde(default)]
    pub provider: String,
    /// A remote Codex app-server uses a local TUI, so pasted images must first
    /// be readable on this machine before its client sends them to the server.
    #[serde(default)]
    pub codex_socket: String,
    /// A Messages client is paired; desktop access remains concurrent.
    #[serde(default)]
    pub imessage_attached: bool,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub title_source: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub pid: i32,
    #[serde(default)]
    pub status_text: String,
    #[serde(default)]
    pub status_style: Option<StatusStyle>,
    #[serde(default)]
    pub status_observed_at: i64,
    #[serde(default)]
    pub tmux_pane: String,
    #[serde(default)]
    pub started: String,
    /// `started` as Unix seconds, parsed once when the event arrives so a sort
    /// of the session list does not parse every timestamp per comparison.
    #[serde(skip)]
    pub started_epoch: f64,
    #[serde(default)]
    pub tracked: bool,
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub historical: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub last_message: String,
    #[serde(default)]
    pub last_turn_interrupted: bool,
    #[serde(default)]
    pub status_source: String,
    #[serde(default)]
    pub hook_warning: String,
    /// A viewer is attached to the session's terminal right now.
    #[serde(default)]
    pub attached: bool,
    /// Why the helper would refuse to pause this session: background work,
    /// scheduled wakeups, an active goal, queued input.
    #[serde(default, deserialize_with = "null_default")]
    pub pending_work: Vec<String>,
    #[serde(default)]
    pub paused_at: i64,
    #[serde(default)]
    pub pause_reason: String,
    #[serde(default, deserialize_with = "null_default")]
    pub pull_requests: Vec<PullRequest>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct RecoveryLaunch {
    #[serde(default)]
    pub claude_remote: bool,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub effort: String,
    #[serde(default)]
    pub permissions: String,
    #[serde(default)]
    pub resume: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct LaunchRecovery {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub session: Option<Session>,
    #[serde(default)]
    pub launch: RecoveryLaunch,
}

/// A GitHub pull request the session's agent opened or reported. The Go side
/// scans agent output and tool results; user prompts never count.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct PullRequest {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub number: i64,
}

impl PullRequest {
    /// "#9": the number is how people refer to a request out loud.
    pub fn label(&self) -> String {
        format!("#{}", self.number)
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct StatusInk {
    #[serde(default)]
    pub rgb: Option<[u8; 3]>,
    #[serde(default)]
    pub index: Option<u8>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub dim: bool,
}

impl Session {
    /// A pane update changes turn information, preserving discovery metadata.
    /// Both timestamps come from the same host, so remote clock skew is harmless.
    pub fn apply_indicator(&mut self, update: &Session) -> bool {
        if self.id != update.id || update.status_observed_at <= self.status_observed_at {
            return false;
        }
        self.status_observed_at = update.status_observed_at;
        self.observation_received_at = update.observation_received_at;
        self.state = update.state.clone();
        self.pid = update.pid;
        self.status_text = update.status_text.clone();
        self.status_style = update.status_style.clone();
        self.status_source = update.status_source.clone();
        self.last_message = update.last_message.clone();
        self.last_turn_interrupted = update.last_turn_interrupted;
        for pr in &update.pull_requests {
            if !self.pull_requests.iter().any(|known| known.url == pr.url) {
                self.pull_requests.push(pr.clone());
            }
        }
        true
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct StatusStyle {
    #[serde(default)]
    pub glyph: StatusInk,
    /// One foreground style per Unicode code point in status_text.
    #[serde(default, deserialize_with = "null_default")]
    pub text: Vec<StatusInk>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Account {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub status: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct UsageWindow {
    #[serde(default)]
    pub used_percent: f64,
    #[serde(default)]
    pub resets_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ProviderUsage {
    #[serde(default)]
    pub five_hour: Option<UsageWindow>,
    #[serde(default)]
    pub weekly: Option<UsageWindow>,
    #[serde(default)]
    pub fable_weekly: Option<UsageWindow>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct LaunchProfile {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub effort: String,
    #[serde(default)]
    pub prompt: String,
    /// Empty takes the machine default at launch.
    #[serde(default)]
    pub permissions: String,
    #[serde(default)]
    pub machine_id: String,
    #[serde(default)]
    pub shortcut: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct State {
    #[serde(default, deserialize_with = "null_default")]
    pub machines: Vec<Machine>,
    #[serde(default, deserialize_with = "null_default")]
    pub accounts: Vec<Account>,
    #[serde(default, deserialize_with = "null_default")]
    pub presets: HashMap<String, LaunchProfile>,
}

/// One line of `fleet snapshot --watch`: `state` (the saved fleet, closing a
/// round), `machine` (a fresh observation), `session-indicators` (live pane
/// patches), or `error`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FleetEvent {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub state: Option<State>,
    #[serde(default)]
    pub machine: Option<Machine>,
    #[serde(default)]
    pub indicators: Option<SessionIndicators>,
    #[serde(default, deserialize_with = "null_default")]
    pub latest_provider_versions: HashMap<String, String>,
    #[serde(default)]
    pub account_error: String,
    #[serde(default)]
    pub error: String,
    /// A machine event whose sessions are the same as last time: the
    /// snapshot arrives without them and the ones shown count as observed.
    #[serde(default)]
    pub unchanged: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct SessionIndicators {
    pub machine_id: String,
    #[serde(default, deserialize_with = "null_default")]
    pub sessions: Vec<Session>,
    /// Sessions read this tick whose readings matched the last report; the
    /// watcher leaves their payload out but they were observed all the same.
    #[serde(default, deserialize_with = "null_default")]
    pub unchanged: Vec<String>,
}

impl FleetEvent {
    /// Derive what the UI sorts and compares by, once, off the UI thread.
    pub fn index(&mut self) {
        let mut sessions: Vec<&mut Session> = Vec::new();
        if let Some(state) = &mut self.state {
            for machine in &mut state.machines {
                if let Some(last) = &mut machine.last {
                    sessions.extend(last.sessions.iter_mut());
                }
            }
        }
        if let Some(last) = self.machine.as_mut().and_then(|m| m.last.as_mut()) {
            sessions.extend(last.sessions.iter_mut());
        }
        if let Some(update) = &mut self.indicators {
            sessions.extend(update.sessions.iter_mut());
        }
        for session in sessions {
            session.started_epoch = epoch_seconds(&session.started);
        }
    }
}

/// What the watcher's reader thread hands to the workspace.
// One message per event crosses a channel; the variant size gap is moot.
#[allow(clippy::large_enum_variant)]
/// Seconds since the Unix epoch for an RFC 3339 stamp, 0 when unreadable.
pub fn epoch_seconds(stamp: &str) -> f64 {
    let b = stamp.as_bytes();
    let num = |from: usize, to: usize| -> Option<i64> { stamp.get(from..to)?.parse::<i64>().ok() };
    let (Some(y), Some(mo), Some(d), Some(h), Some(mi), Some(s)) = (
        num(0, 4),
        num(5, 7),
        num(8, 10),
        num(11, 13),
        num(14, 16),
        num(17, 19),
    ) else {
        return 0.0;
    };
    let mut i = 19;
    let mut frac = 0.0;
    if b.get(i) == Some(&b'.') {
        let start = i + 1;
        i = start;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if let Ok(f) = format!("0.{}", &stamp[start..i]).parse::<f64>() {
            frac = f;
        }
    }
    let mut offset = 0;
    if let Some(sign) = b.get(i).filter(|sign| **sign == b'+' || **sign == b'-') {
        let oh = num(i + 1, i + 3).unwrap_or(0);
        let om = num(i + 4, i + 6).unwrap_or(0);
        offset = (oh * 3600 + om * 60) * if *sign == b'+' { 1 } else { -1 };
    }
    let (y, m) = if mo <= 2 {
        (y - 1, mo + 9)
    } else {
        (y, mo - 3)
    };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86400 + h * 3600 + mi * 60 + s - offset) as f64 + frac
}
