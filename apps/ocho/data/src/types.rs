//! The fleet's records, as the `fleet` CLI prints them: a port of the GPUI
//! desktop's `backend.rs` types (origin/main e6adfa8), without the process
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
    /// The shell the configuration file belongs to.
    pub shell: String,
}

/// A machine in the fleet and its latest observation.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Machine {
    /// The EAS project a cloud machine is bound to, if any.
    pub eas: Option<EasBinding>,
    /// Where the sprite binding came from.
    pub sprite_source: String,
    /// The Fly.io sprite this machine runs on, if any.
    pub sprite: Option<SpriteBinding>,
    /// Local receipt time of the latest failed machine observation. A later
    /// successful session patch can recover that session independently.
    #[serde(skip)]
    pub observation_failed_at: Option<f64>,
    /// Stable machine id.
    #[serde(default)]
    pub id: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Addresses the machine is reached at.
    #[serde(default, deserialize_with = "null_default")]
    pub addresses: Vec<String>,
    /// Free-form notes.
    #[serde(default)]
    pub notes: String,
    /// User tags.
    #[serde(default, deserialize_with = "null_default")]
    pub tags: Vec<String>,
    /// Default permission mode per provider for sessions launched here.
    #[serde(default, deserialize_with = "null_default")]
    pub permissions: HashMap<String, String>,
    /// This machine is the one Ocho runs on.
    #[serde(default)]
    pub local: bool,
    /// The latest successful observation.
    #[serde(default)]
    pub last: Option<Snapshot>,
    /// Why the latest observation failed, if it did.
    #[serde(default)]
    pub error: String,
    /// Set by `fleet snapshot` when the machine's helper does not match this viewer.
    #[serde(default)]
    pub helper_status: String,
}

/// The Expo Application Services (EAS) project a machine is bound to.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct EasBinding {
    /// The EAS connection's id.
    pub connection_id: String,
    /// The project's URL.
    pub url: String,
    /// The Expo account name.
    pub account_name: String,
    /// The Expo project name.
    pub project_name: String,
}

/// The Fly.io sprite a machine runs on.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct SpriteBinding {
    /// Fly.io organization.
    pub organization: String,
    /// Sprite name.
    pub name: String,
    /// Sprite id.
    pub id: String,
    /// The request that created the sprite.
    pub request_id: String,
}

/// One observation of a machine: hardware, provider versions and sessions.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Snapshot {
    /// Host name.
    #[serde(default)]
    pub hostname: String,
    /// The helper user's home directory, so session folders can be shown as `~/…`.
    #[serde(default)]
    pub home: String,
    /// Operating system and architecture.
    #[serde(default)]
    pub platform: String,
    /// CPU model.
    #[serde(default)]
    pub cpu: String,
    /// CPU core count.
    #[serde(default)]
    pub cores: i64,
    /// CPU load, percent.
    #[serde(default)]
    pub cpu_percent: f64,
    /// Total memory, bytes.
    #[serde(default)]
    pub memory_total: u64,
    /// Used memory, bytes.
    #[serde(default)]
    pub memory_used: u64,
    /// Installed version per provider (and the helper).
    #[serde(default, deserialize_with = "null_default")]
    pub versions: HashMap<String, String>,
    /// The machine's sessions.
    #[serde(default, deserialize_with = "null_default")]
    pub sessions: Vec<Session>,
    /// Observation warnings.
    #[serde(default, deserialize_with = "null_default")]
    pub warnings: Vec<String>,
    /// Sessions come from a live inventory rather than history only.
    #[serde(default)]
    pub live_inventory: bool,
    /// When the observation was taken (RFC 3339).
    #[serde(default)]
    pub at: String,
}

/// One agent session on a machine, live or historical.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Session {
    /// An EAS Codex session, attached by its native id rather than a pane.
    #[serde(default)]
    pub eas: bool,
    /// The local socket a Grok session's client talks to.
    #[serde(default)]
    pub grok_socket: String,
    /// A Claude Remote Control session.
    #[serde(default)]
    pub claude_remote: bool,
    /// Local, monotonic freshness clock; never deserialize a helper's clock.
    /// Only advancing observations renew it, not replayed inventory metadata.
    #[serde(skip)]
    pub observation_received_at: Option<f64>,
    /// Fleet session id.
    #[serde(default)]
    pub id: String,
    /// The provider's own conversation id.
    #[serde(default)]
    pub native_id: String,
    /// Provider (`claude`, `codex`, `opencode`, `antigravity`, `grok`).
    #[serde(default)]
    pub provider: String,
    /// A remote Codex app-server uses a local TUI, so pasted images must first
    /// be readable on this machine before its client sends them to the server.
    #[serde(default)]
    pub codex_socket: String,
    /// A Messages client is paired; desktop access remains concurrent.
    #[serde(default)]
    pub imessage_attached: bool,
    /// Account email.
    #[serde(default)]
    pub account: String,
    /// Working directory.
    #[serde(default)]
    pub cwd: String,
    /// Session title.
    #[serde(default)]
    pub title: String,
    /// Where the title came from.
    #[serde(default)]
    pub title_source: String,
    /// Model id.
    #[serde(default)]
    pub model: String,
    /// The session's state as the helper reports it.
    #[serde(default)]
    pub state: String,
    /// Agent process id.
    #[serde(default)]
    pub pid: i32,
    /// The status line text.
    #[serde(default)]
    pub status_text: String,
    /// The status line colours.
    #[serde(default)]
    pub status_style: Option<StatusStyle>,
    /// When the status was read, host clock.
    #[serde(default)]
    pub status_observed_at: i64,
    /// The tmux pane the session runs in; empty when none.
    #[serde(default)]
    pub tmux_pane: String,
    /// When the session started (RFC 3339).
    #[serde(default)]
    pub started: String,
    /// `started` as Unix seconds, parsed once when the event arrives so a sort
    /// of the session list does not parse every timestamp per comparison.
    #[serde(skip)]
    pub started_epoch: f64,
    /// The last hook event or transcript turn the helper observed.
    #[serde(default)]
    pub updated: String,
    /// When the session last did anything, as Unix seconds: the newer of
    /// `updated` and `started`. Parsed with `started_epoch`.
    #[serde(skip)]
    pub active_epoch: f64,
    /// Ocho tracks the session.
    #[serde(default)]
    pub tracked: bool,
    /// Ocho launched the session.
    #[serde(default)]
    pub managed: bool,
    /// A past conversation with no live process.
    #[serde(default)]
    pub historical: bool,
    /// Pinned to the top.
    #[serde(default)]
    pub pinned: bool,
    /// Hidden from the list.
    #[serde(default)]
    pub hidden: bool,
    /// Archived.
    #[serde(default)]
    pub archived: bool,
    /// The last assistant message.
    #[serde(default)]
    pub last_message: String,
    /// The last turn was interrupted.
    #[serde(default)]
    pub last_turn_interrupted: bool,
    /// Where the status was read from.
    #[serde(default)]
    pub status_source: String,
    /// Why the provider's hooks are not reporting, if they are not.
    #[serde(default)]
    pub hook_warning: String,
    /// A viewer is attached to the session's terminal right now.
    #[serde(default)]
    pub attached: bool,
    /// Why the helper would refuse to pause this session: background work,
    /// scheduled wakeups, an active goal, queued input.
    #[serde(default, deserialize_with = "null_default")]
    pub pending_work: Vec<String>,
    /// When the session was paused; 0 when it is not.
    #[serde(default)]
    pub paused_at: i64,
    /// Why the session was paused.
    #[serde(default)]
    pub pause_reason: String,
    /// Pull requests the agent opened or reported.
    #[serde(default, deserialize_with = "null_default")]
    pub pull_requests: Vec<PullRequest>,
}

/// How a session that cannot be recovered in place would be relaunched.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct RecoveryLaunch {
    /// Relaunch as Claude Remote Control.
    #[serde(default)]
    pub claude_remote: bool,
    /// Provider.
    #[serde(default)]
    pub provider: String,
    /// Account email.
    #[serde(default)]
    pub account: String,
    /// Working directory.
    #[serde(default)]
    pub cwd: String,
    /// Model id.
    #[serde(default)]
    pub model: String,
    /// Effort level.
    #[serde(default)]
    pub effort: String,
    /// Permission mode.
    #[serde(default)]
    pub permissions: String,
    /// The conversation to resume.
    #[serde(default)]
    pub resume: String,
}

/// What `fleet` reports when a terminal's session went away.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct LaunchRecovery {
    /// Recovery status.
    #[serde(default)]
    pub status: String,
    /// Why the session needs recovery.
    #[serde(default)]
    pub reason: String,
    /// The session as last seen.
    #[serde(default)]
    pub session: Option<Session>,
    /// How it would be relaunched.
    #[serde(default)]
    pub launch: RecoveryLaunch,
}

/// A GitHub pull request the session's agent opened or reported. The Go side
/// scans agent output and tool results; user prompts never count.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct PullRequest {
    /// The pull request's URL.
    #[serde(default)]
    pub url: String,
    /// `owner/name`.
    #[serde(default)]
    pub repo: String,
    /// The pull request number.
    #[serde(default)]
    pub number: i64,
}

impl PullRequest {
    /// "#9": the number is how people refer to a request out loud.
    pub fn label(&self) -> String {
        format!("#{}", self.number)
    }
}

/// One foreground style in a status line.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct StatusInk {
    /// A 24-bit colour.
    #[serde(default)]
    pub rgb: Option<[u8; 3]>,
    /// A palette colour index.
    #[serde(default)]
    pub index: Option<u8>,
    /// Bold.
    #[serde(default)]
    pub bold: bool,
    /// Dim.
    #[serde(default)]
    pub dim: bool,
}

impl Session {
    /// Whether a terminal can attach: a tmux pane, or a live EAS Codex
    /// session reached by its native id.
    pub fn can_attach(&self) -> bool {
        !self.tmux_pane.is_empty() || (self.eas && !self.native_id.is_empty() && !self.historical)
    }

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

/// The colours of a session's status glyph and text.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct StatusStyle {
    /// The status glyph's style.
    #[serde(default)]
    pub glyph: StatusInk,
    /// One foreground style per Unicode code point in status_text.
    #[serde(default, deserialize_with = "null_default")]
    pub text: Vec<StatusInk>,
}

/// A provider account the fleet can launch sessions under.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Account {
    /// Account email (the account's id).
    #[serde(default)]
    pub email: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Provider.
    #[serde(default)]
    pub provider: String,
    /// Free-form notes.
    #[serde(default)]
    pub notes: String,
    /// Shared across machines.
    #[serde(default)]
    pub shared: bool,
    /// Sign-in status.
    #[serde(default)]
    pub status: String,
}

/// One usage-limit window of an account.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct UsageWindow {
    /// Percent of the window used.
    #[serde(default)]
    pub used_percent: f64,
    /// When the window resets (RFC 3339).
    #[serde(default)]
    pub resets_at: String,
}

/// An account's usage-limit windows.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ProviderUsage {
    /// The five-hour window.
    #[serde(default)]
    pub five_hour: Option<UsageWindow>,
    /// The weekly window.
    #[serde(default)]
    pub weekly: Option<UsageWindow>,
    /// The weekly window for Fable models.
    #[serde(default)]
    pub fable_weekly: Option<UsageWindow>,
}

/// A saved launch: provider, account, folder, model and prompt.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct LaunchProfile {
    /// Provider.
    #[serde(default)]
    pub provider: String,
    /// Account email.
    #[serde(default)]
    pub account: String,
    /// Working directory.
    #[serde(default)]
    pub cwd: String,
    /// Model id.
    #[serde(default)]
    pub model: String,
    /// Effort level.
    #[serde(default)]
    pub effort: String,
    /// Initial prompt.
    #[serde(default)]
    pub prompt: String,
    /// Empty takes the machine default at launch.
    #[serde(default)]
    pub permissions: String,
    /// Machine to launch on.
    #[serde(default)]
    pub machine_id: String,
    /// ⌘⌥ digit, 0 for none.
    #[serde(default)]
    pub shortcut: i64,
}

/// The saved fleet: machines, accounts and launch profiles.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct State {
    /// Machines.
    #[serde(default, deserialize_with = "null_default")]
    pub machines: Vec<Machine>,
    /// Accounts.
    #[serde(default, deserialize_with = "null_default")]
    pub accounts: Vec<Account>,
    /// Launch profiles by name.
    #[serde(default, deserialize_with = "null_default")]
    pub presets: HashMap<String, LaunchProfile>,
}

/// One line of `fleet snapshot --watch`: `state` (the saved fleet, closing a
/// round), `machine` (a fresh observation), `session-indicators` (live pane
/// patches), or `error`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FleetEvent {
    /// `state`, `machine`, `session-indicators` or `error`.
    #[serde(default, rename = "type")]
    pub kind: String,
    /// The saved fleet, on `state`.
    #[serde(default)]
    pub state: Option<State>,
    /// A fresh observation, on `machine`.
    #[serde(default)]
    pub machine: Option<Machine>,
    /// Live pane patches, on `session-indicators`.
    #[serde(default)]
    pub indicators: Option<SessionIndicators>,
    /// The newest release per provider.
    #[serde(default, deserialize_with = "null_default")]
    pub latest_provider_versions: HashMap<String, String>,
    /// Why accounts could not be read.
    #[serde(default)]
    pub account_error: String,
    /// The error, on `error`.
    #[serde(default)]
    pub error: String,
    /// A menu bar item's command id (`Command::id`), on `menu`: the host
    /// sends the menu through the feed.
    #[serde(default)]
    pub command: String,
    /// A clicked notification's thread (`machine:session`), on `notification`.
    #[serde(default)]
    pub thread: String,
    /// A machine event whose sessions are the same as last time: the
    /// snapshot arrives without them and the ones shown count as observed.
    #[serde(default)]
    pub unchanged: bool,
}

/// Live status patches for one machine's sessions.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct SessionIndicators {
    /// The machine.
    pub machine_id: String,
    /// Sessions whose readings changed.
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
            session.active_epoch = epoch_seconds(&session.updated).max(session.started_epoch);
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eas_machines_and_sessions_parse() {
        let machine: Machine = serde_json::from_str(
            r#"{"id":"eas-1","name":"EAS","eas":{"connection_id":"c1","url":"https://expo.dev/x","account_name":"me","project_name":"app"}}"#,
        )
        .unwrap();
        let eas = machine.eas.unwrap();
        assert_eq!(eas.connection_id, "c1");
        assert_eq!(eas.project_name, "app");
        let plain: Machine = serde_json::from_str(r#"{"id":"m"}"#).unwrap();
        assert_eq!(plain.eas, None);

        let session: Session = serde_json::from_str(
            r#"{"id":"s","provider":"grok","grok_socket":"/tmp/grok.sock","eas":true,"native_id":"n"}"#,
        )
        .unwrap();
        assert!(session.eas);
        assert_eq!(session.grok_socket, "/tmp/grok.sock");
    }

    #[test]
    fn eas_sessions_attach_without_a_pane() {
        let mut session = Session {
            eas: true,
            native_id: "thread".into(),
            ..Default::default()
        };
        assert!(session.can_attach());
        session.historical = true;
        assert!(!session.can_attach());
        session.historical = false;
        session.native_id.clear();
        assert!(!session.can_attach());
        session.eas = false;
        session.tmux_pane = "%3".into();
        assert!(session.can_attach());
        assert!(!Session::default().can_attach());
    }
}
