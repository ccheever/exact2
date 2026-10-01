//! The pure session, machine, account and profile helpers of the GPUI
//! desktop (workspace.rs `session_state`, `session_badge`,
//! `session_summary`, the Sessions filter and sort, `machine_status`,
//! `provider_versions`; backend.rs `provider_label`, `account_email`,
//! `machine_rows`; permissions.rs `summary`), verbatim in behavior. Times
//! are host-clock milliseconds (`now_ms`) or Unix seconds
//! (`now_epoch_s`), never `Instant`.

use crate::types::{epoch_seconds, Account, LaunchProfile, Machine, Session, State};
use std::cmp::Ordering;
use std::collections::HashMap;

// ----- text ------------------------------------------------------------------

/// workspace.rs `clean`: drop control characters, keeping newlines.
pub fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect()
}

/// workspace.rs `one_line`: `clean`, then every run of whitespace as one space.
pub fn one_line(s: &str) -> String {
    clean(s).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// workspace.rs `gib`: bytes as "N.N GB" (GiB).
pub fn gib(v: u64) -> String {
    format!("{:.1} GB", v as f64 / (1u64 << 30) as f64)
}

// ----- providers and accounts (backend.rs:538-570) -----------------------------

/// The provider's display name.
pub fn provider_label(p: &str) -> &str {
    match p {
        "claude" => "Claude",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        "antigravity" => "Antigravity",
        "grok" => "Grok",
        other => other,
    }
}

/// `provider:email`, or the name when there is no email (backend.rs `account_handle`).
pub fn account_handle(a: &Account) -> String {
    if a.email.is_empty() {
        a.name.clone()
    } else {
        format!("{}:{}", a.provider, a.email)
    }
}

/// The email, "API key · {name}" for API-key accounts, else "Email unavailable".
pub fn account_email(a: &Account) -> String {
    if a.email.is_empty() {
        if a.name.starts_with(&format!("{}-api-", a.provider)) {
            format!("API key · {}", a.name)
        } else {
            "Email unavailable".to_string()
        }
    } else {
        a.email.clone()
    }
}

/// "{Provider} · {email}".
pub fn account_label(a: &Account) -> String {
    format!("{} · {}", provider_label(&a.provider), account_email(a))
}

/// backend.rs `State::machine`: by id or name.
pub fn machine<'a>(state: &'a State, key: &str) -> Option<&'a Machine> {
    state.machines.iter().find(|m| m.id == key || m.name == key)
}

/// backend.rs `State::account`, mirroring Go's FindAccount: exact name,
/// provider:email handle, or a bare email that identifies exactly one account.
pub fn account<'a>(state: &'a State, key: &str) -> Option<&'a Account> {
    if key.is_empty() {
        return None;
    }
    if let Some(a) = state
        .accounts
        .iter()
        .find(|a| a.name == key || account_handle(a) == key)
    {
        return Some(a);
    }
    let mut by_email = state
        .accounts
        .iter()
        .filter(|a| !a.email.is_empty() && a.email == key);
    let first = by_email.next()?;
    if by_email.next().is_some() {
        None
    } else {
        Some(first)
    }
}

/// backend.rs `State::machine_rows`: session Sprites stay in inventory for
/// attachment and observation but are not reusable machines in the manager;
/// Fly organization sources are.
pub fn machine_rows(state: &State) -> impl Iterator<Item = &Machine> {
    state
        .machines
        .iter()
        .filter(|machine| machine.sprite.is_none())
}

// ----- session state (workspace.rs:10054-10144) --------------------------------

/// The manager's state word for a session: TERMINAL, PAUSED, IDLE, CLOSED,
/// RUNNING, BLOCKED, LIMITED or UNAVAILABLE.
pub fn session_state(s: &Session) -> &'static str {
    if s.provider == "shell" {
        return "TERMINAL";
    }
    // A paused session has no process, but its terminal is kept and opening it
    // resumes the conversation, so it is neither closed nor idle.
    if s.state == "paused" && !s.historical {
        return "PAUSED";
    }
    // A launch record has no process yet: the desktop's optimistic row and
    // the core's own "starting" session both carry pid 0 until the agent is
    // bound. That is a session on its way, not one that closed.
    if s.state == "starting" && !s.historical {
        return "IDLE";
    }
    if s.historical || s.pid == 0 || matches!(s.state.as_str(), "closed" | "exited") {
        return "CLOSED";
    }
    // Older helpers use working/awaiting; keep that compatibility at the boundary.
    match s.state.as_str() {
        "working" | "running" => "RUNNING",
        "awaiting approval" | "awaiting input" | "blocked" => "BLOCKED",
        // The provider refused the last turn for a usage window; the agent is
        // at its composer but cannot continue until the window resets.
        "limited" => "LIMITED",
        "idle" | "starting" => "IDLE",
        _ => "UNAVAILABLE",
    }
}

/// The badge text, with the `⌖  ` prefix when pinned.
pub fn session_badge(s: &Session) -> String {
    let mut badge = match session_state(s) {
        "RUNNING" => "✳  WORKING",
        "BLOCKED" => "◆  INPUT",
        "LIMITED" => "◷  LIMITED",
        "IDLE" => "○  READY",
        "PAUSED" => "Ⅱ  PAUSED",
        "CLOSED" => "—  CLOSED",
        "TERMINAL" => "›_ TERMINAL",
        _ => "?  UNKNOWN",
    }
    .to_string();
    if s.pinned {
        badge = format!("⌖  {badge}");
    }
    badge
}

/// The second line of a session row.
pub fn session_summary(s: &Session) -> String {
    match session_state(s) {
        "PAUSED" => {
            return if s.status_text.is_empty() {
                "Paused; opens where it left off".into()
            } else {
                one_line(&s.status_text)
            };
        }
        "RUNNING" => {
            return if s.status_text.is_empty() {
                "Working…".into()
            } else {
                one_line(&s.status_text)
            };
        }
        "BLOCKED" => {
            return if s.status_text.is_empty() {
                "Waiting for your approval or answer".into()
            } else {
                one_line(&s.status_text)
            };
        }
        "LIMITED" => {
            return if s.status_text.is_empty() {
                "Usage limit reached".into()
            } else {
                one_line(&s.status_text)
            };
        }
        "UNAVAILABLE" => return "Unable to observe the current turn".into(),
        _ => {}
    }
    if !s.last_message.is_empty() {
        return one_line(&s.last_message);
    }
    if !s.hook_warning.is_empty() {
        return s.hook_warning.clone();
    }
    if session_state(s) == "IDLE" {
        "Ready for a message".into()
    } else {
        "Agent process is closed".into()
    }
}

/// True when every word is a duration like `1m` `25s` `2h`.
fn status_elapsed(text: &str) -> bool {
    let mut found = false;
    for part in text.split_whitespace() {
        let Some(unit) = part.chars().last() else {
            return false;
        };
        let number = &part[..part.len() - unit.len_utf8()];
        if !matches!(unit, 'h' | 'm' | 's')
            || number.is_empty()
            || !number.bytes().all(|byte| byte.is_ascii_digit())
        {
            return false;
        }
        found = true;
    }
    found
}

/// A running status without its provider timer: "Thinking… (1s · ↓ 134
/// tokens)" becomes "Thinking… (↓ 134 tokens)"; a retry keeps only its reason.
fn stale_running_summary(text: &str) -> String {
    if let Some((reason, _)) = text.split_once(" · Retrying in ") {
        return reason.to_string();
    }
    let Some(header_end) = text.rfind(" (") else {
        return text.to_string();
    };
    let Some(detail) = text[header_end + 2..].strip_suffix(')') else {
        return text.to_string();
    };
    let (elapsed, remainder) = detail
        .split_once(" · ")
        .map_or((detail, None), |(elapsed, remainder)| {
            (elapsed, Some(remainder))
        });
    if !status_elapsed(elapsed) {
        return text.to_string();
    }
    match remainder {
        Some(remainder) => format!("{} ({remainder})", &text[..header_end]),
        None => text[..header_end].to_string(),
    }
}

/// A retained observation is useful context, but its provider timer is not
/// the age of the observation. Mark freshness visually and keep only durable
/// activity detail once the observation expires.
pub fn stale_session_summary(s: &Session) -> String {
    let summary = session_summary(s);
    let summary = if session_state(s) == "RUNNING" {
        stale_running_summary(&summary)
    } else {
        summary
    };
    format!("◷  {summary}")
}

/// A transport blip is not an agent state transition. Retain a recently
/// observed status for a bounded grace period; failed/replayed polls do not
/// renew it.
pub const SESSION_STATUS_MAX_AGE_MS: f64 = 30_000.0;

/// Whether the session's observation is fresh enough to show as live, at
/// `now_ms` on the host clock (`observation_received_at` is on the same clock).
pub fn session_observation_available(machine: &Machine, session: &Session, now_ms: f64) -> bool {
    if machine.last.is_none() {
        return false;
    }
    // History, explicit pause and a launch still starting are durable states,
    // not a live working claim that could go stale.
    if session.historical || session.state == "paused" || session.state == "starting" {
        return true;
    }
    session
        .observation_received_at
        .is_some_and(|observed| (now_ms - observed).max(0.0) < SESSION_STATUS_MAX_AGE_MS)
}

// ----- the Sessions list (workspace.rs:1798-1852, 10094-10104) -----------------

/// The Sessions page's filter: hidden sessions never show; history only
/// with `history`; by default only unarchived managed-or-tracked sessions,
/// every session with `all_sessions`; `hide_non_running` keeps RUNNING only.
pub fn session_matches_filters(
    session: &Session,
    history: bool,
    all_sessions: bool,
    hide_non_running: bool,
) -> bool {
    !session.hidden
        && (history || !session.historical)
        && (all_sessions || (!session.archived && (session.managed || session.tracked)))
        && (!hide_non_running || session_state(session) == "RUNNING")
}

/// The search haystack: `title cwd account provider id machine`, lowercased,
/// where `account` is the account's label when the fleet knows it.
pub fn session_haystack(machine: &Machine, session: &Session, account: &str) -> String {
    format!(
        "{} {} {} {} {} {}",
        session.title, session.cwd, account, session.provider, session.id, machine.name
    )
    .to_lowercase()
}

/// The list order: pinned first, then unarchived, then newest `started` first.
pub fn session_order(a: &Session, b: &Session) -> Ordering {
    b.pinned
        .cmp(&a.pinned)
        .then_with(|| a.archived.cmp(&b.archived))
        .then_with(|| {
            b.started_epoch
                .partial_cmp(&a.started_epoch)
                .unwrap_or(Ordering::Equal)
        })
}

/// The visible sessions in list order, borrowed (workspace.rs `session_refs`).
pub fn session_refs<'a>(
    state: &'a State,
    query: &str,
    history: bool,
    all_sessions: bool,
    hide_non_running: bool,
) -> Vec<(&'a Machine, &'a Session)> {
    let query = query.to_lowercase();
    let accounts: HashMap<&str, String> = if query.is_empty() {
        HashMap::new()
    } else {
        state
            .accounts
            .iter()
            .map(|account| (account.name.as_str(), account_label(account)))
            .collect()
    };
    let mut out = Vec::new();
    for machine in &state.machines {
        let Some(last) = &machine.last else { continue };
        for s in &last.sessions {
            if !session_matches_filters(s, history, all_sessions, hide_non_running) {
                continue;
            }
            if !query.is_empty() {
                let account = accounts
                    .get(s.account.as_str())
                    .cloned()
                    .unwrap_or_else(|| s.account.clone());
                if !session_haystack(machine, s, &account).contains(&query) {
                    continue;
                }
            }
            out.push((machine, s));
        }
    }
    out.sort_by(|(_, a), (_, b)| session_order(a, b));
    out
}

// ----- machines (workspace.rs:10146-10282) -------------------------------------

/// The status and spec lines of a machine row.
pub fn machine_status(m: &Machine) -> (String, String) {
    if m.sprite.as_ref().is_some_and(|s| s.id.is_empty()) {
        return ("Creating session Sprite…".into(), "Fly.io".into());
    }
    if !m.sprite_source.is_empty() {
        return (
            "Creates a new Sprite for each session".into(),
            format!("Fly.io · {}", m.sprite_source),
        );
    }
    let mut status = "not contacted".to_string();
    let mut spec = String::new();
    if let Some(s) = &m.last {
        let mut active = 0;
        let mut working = 0;
        for ss in &s.sessions {
            if matches!(
                session_state(ss),
                "RUNNING" | "BLOCKED" | "LIMITED" | "IDLE"
            ) {
                active += 1;
                if session_state(ss) == "RUNNING" {
                    working += 1;
                }
            }
        }
        status = format!(
            "{active} sessions · {working} working · CPU {:.0}% · memory {} / {}",
            s.cpu_percent,
            gib(s.memory_used),
            gib(s.memory_total)
        );
        if !s.live_inventory {
            status = format!(
                "Live inventory unavailable · memory {} / {}",
                gib(s.memory_used),
                gib(s.memory_total)
            );
        }
        spec = format!("{} · {} cores · {}", s.platform, s.cores, s.cpu);
    }
    if !m.error.is_empty() {
        status = format!("UNREACHABLE · {}", m.error);
    }
    if !m.helper_status.is_empty() {
        if !spec.is_empty() {
            spec.push_str(" · ");
        }
        spec.push_str(&m.helper_status);
    }
    (status, spec)
}

/// The installed version of a provider on a machine, "not installed" when
/// the helper reported it unavailable, None when unknown.
pub fn provider_version(m: &Machine, provider: &str) -> Option<String> {
    let raw = m.last.as_ref()?.versions.get(provider)?.trim();
    if raw.is_empty() {
        return None;
    }
    if raw == "unavailable" {
        return Some("not installed".into());
    }
    let version = match provider {
        "codex" => raw.strip_prefix("codex-cli ").unwrap_or(raw),
        "grok" => raw
            .strip_prefix("grok ")
            .unwrap_or(raw)
            .split_whitespace()
            .next()
            .unwrap_or(raw),
        "claude" => raw.strip_suffix(" (Claude Code)").unwrap_or(raw),
        _ => raw,
    };
    Some(version.to_string())
}

fn version_parts(version: &str) -> Option<Vec<u64>> {
    let core = version
        .trim()
        .trim_start_matches('v')
        .split(['-', '+', ' '])
        .next()?;
    let parts: Option<Vec<u64>> = core.split('.').map(|part| part.parse().ok()).collect();
    parts.filter(|parts| parts.len() >= 3)
}

/// Some(true) when a newer version is known (or the tool is not installed),
/// Some(false) when current, None when either side is unknown.
pub fn provider_update_available(
    machine: &Machine,
    latest_versions: &HashMap<String, String>,
    provider: &str,
) -> Option<bool> {
    let installed = provider_version(machine, provider)?;
    if installed == "not installed" {
        return Some(true);
    }
    let latest = latest_versions.get(provider)?;
    if installed == *latest {
        return Some(false);
    }
    Some(version_parts(&installed)? < version_parts(latest)?)
}

/// Whether the row menu offers "Update {provider}…": anything but known-current.
pub fn provider_update_action_available(
    machine: &Machine,
    latest_versions: &HashMap<String, String>,
    provider: &str,
) -> bool {
    provider_update_available(machine, latest_versions, provider) != Some(false)
}

/// Installed but behind the latest known version.
pub fn provider_outdated(
    machine: &Machine,
    latest_versions: &HashMap<String, String>,
    provider: &str,
) -> bool {
    provider_version(machine, provider).is_some_and(|installed| {
        installed != "not installed"
            && provider_update_available(machine, latest_versions, provider) == Some(true)
    })
}

fn provider_version_status(
    machine: &Machine,
    latest_versions: &HashMap<String, String>,
    provider: &str,
) -> String {
    let installed = provider_version(machine, provider).unwrap_or_else(|| "unknown".into());
    if provider_update_available(machine, latest_versions, provider) == Some(true) {
        if let Some(latest) = latest_versions.get(provider) {
            return format!("{installed} → {latest} available");
        }
    }
    installed
}

/// "Codex {v} · Claude {v} · OpenCode {v}", then " · Grok {v}" when the
/// machine reported a Grok version and " · Antigravity {v}" when it reported
/// Antigravity at all; empty before the first snapshot.
pub fn provider_versions(m: &Machine, latest_versions: &HashMap<String, String>) -> String {
    if m.last.is_none() {
        return String::new();
    }
    let codex = provider_version_status(m, latest_versions, "codex");
    let claude = provider_version_status(m, latest_versions, "claude");
    let opencode = provider_version_status(m, latest_versions, "opencode");
    let mut text = format!("Codex {codex} · Claude {claude} · OpenCode {opencode}");
    if provider_version(m, "grok").is_some() {
        text.push_str(&format!(
            " · Grok {}",
            provider_version_status(m, latest_versions, "grok")
        ));
    }
    if m.last
        .as_ref()
        .is_some_and(|snap| snap.versions.contains_key("antigravity"))
    {
        text.push_str(&format!(
            " · Antigravity {}",
            provider_version_status(m, latest_versions, "antigravity")
        ));
    }
    text
}

/// permissions.rs `summary`: "Codex yolo · Claude auto", providers in
/// `launch::PROVIDERS` order, empty when nothing is set.
pub fn permissions_summary(defaults: &HashMap<String, String>) -> String {
    crate::launch::PROVIDERS
        .into_iter()
        .filter_map(|provider| {
            defaults
                .get(provider)
                .filter(|mode| !mode.is_empty())
                .map(|mode| format!("{} {mode}", provider_label(provider)))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

// ----- profiles (workspace.rs:1878-1899) ---------------------------------------

/// A launch profile with the name it is saved under.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedProfile {
    /// The preset's key in `State::presets`.
    pub name: String,
    /// The profile.
    pub profile: LaunchProfile,
}

/// Launch profiles in shortcut order (⌘⌥1–9 first, unassigned last, then
/// name); they launch from ⌘⌥N and the palette now that the Profiles page
/// is gone (#267).
pub fn profiles(state: &State) -> Vec<NamedProfile> {
    let mut out: Vec<NamedProfile> = state
        .presets
        .iter()
        .map(|(name, p)| NamedProfile {
            name: name.clone(),
            profile: p.clone(),
        })
        .collect();
    let slot = |p: &NamedProfile| {
        if p.profile.shortcut == 0 {
            10
        } else {
            p.profile.shortcut
        }
    };
    out.sort_by(|a, b| slot(a).cmp(&slot(b)).then_with(|| a.name.cmp(&b.name)));
    out
}

// ----- usage (workspace.rs:9921-9947) ------------------------------------------

/// "resets in 30 min" / "resets in 2 hours" / "resets in 3 days" / "resets
/// now" / "reset time unknown", for a window's `resets_at` at `now_epoch_s`
/// Unix seconds.
pub fn format_usage_reset(stamp: &str, now_epoch_s: f64) -> String {
    let reset = epoch_seconds(stamp);
    if reset <= 0.0 {
        return "reset time unknown".into();
    }
    let seconds = reset - now_epoch_s;
    if seconds <= 0.0 {
        return "resets now".into();
    }
    let minutes = (seconds / 60.0).ceil() as u64;
    if minutes < 60 {
        return format!("resets in {minutes} min");
    }
    let hours = (seconds / (60.0 * 60.0)).ceil() as u64;
    if hours < 24 {
        return format!(
            "resets in {hours} {}",
            if hours == 1 { "hour" } else { "hours" }
        );
    }
    let days = (seconds / (24.0 * 60.0 * 60.0)).ceil() as u64;
    format!(
        "resets in {days} {}",
        if days == 1 { "day" } else { "days" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Snapshot, SpriteBinding};

    #[test]
    fn session_status_uses_turn_evidence_and_preserves_idle_message() {
        let mut s = Session {
            state: "unknown".into(),
            id: "abc".into(),
            pid: 123,
            ..Default::default()
        };
        assert_eq!(session_badge(&s), "?  UNKNOWN");
        assert_eq!(session_summary(&s), "Unable to observe the current turn");
        s.state = "exited".into();
        s.pinned = true;
        assert_eq!(session_badge(&s), "⌖  —  CLOSED");
        s.last_message = "  **hello**\n world ".into();
        assert_eq!(session_summary(&s), "**hello** world");
        s.state = "idle".into();
        assert_eq!(session_badge(&s), "⌖  ○  READY");
        assert_eq!(session_summary(&s), "**hello** world");
        s.state = "running".into();
        s.status_text = "Working (25s)".into();
        assert_eq!(session_badge(&s), "⌖  ✳  WORKING");
        assert_eq!(session_summary(&s), "Working (25s)");
        s.state = "blocked".into();
        s.status_text = "Waiting for your answer".into();
        assert_eq!(session_badge(&s), "⌖  ◆  INPUT");
        assert_eq!(session_summary(&s), "Waiting for your answer");
        s.state = "limited".into();
        s.status_text = "Session limit reached · resets 3pm".into();
        assert_eq!(session_badge(&s), "⌖  ◷  LIMITED");
        assert_eq!(session_summary(&s), "Session limit reached · resets 3pm");
        s.status_text.clear();
        assert_eq!(session_summary(&s), "Usage limit reached");
        s.pid = 0;
        assert_eq!(session_badge(&s), "⌖  —  CLOSED");
        assert_eq!(session_summary(&s), "**hello** world");
    }

    #[test]
    fn session_state_table() {
        let mk = |provider: &str, state: &str, pid: i32, historical: bool| Session {
            provider: provider.into(),
            state: state.into(),
            pid,
            historical,
            ..Default::default()
        };
        for (session, expected) in [
            (mk("shell", "running", 1, false), "TERMINAL"),
            (mk("shell", "closed", 0, true), "TERMINAL"),
            (mk("claude", "paused", 0, false), "PAUSED"),
            (mk("claude", "paused", 0, true), "CLOSED"),
            (mk("claude", "starting", 0, false), "IDLE"),
            (mk("claude", "starting", 0, true), "CLOSED"),
            (mk("claude", "running", 1, true), "CLOSED"),
            (mk("claude", "running", 0, false), "CLOSED"),
            (mk("claude", "closed", 9, false), "CLOSED"),
            (mk("claude", "exited", 9, false), "CLOSED"),
            (mk("claude", "working", 9, false), "RUNNING"),
            (mk("claude", "running", 9, false), "RUNNING"),
            (mk("claude", "awaiting approval", 9, false), "BLOCKED"),
            (mk("claude", "awaiting input", 9, false), "BLOCKED"),
            (mk("claude", "blocked", 9, false), "BLOCKED"),
            (mk("claude", "limited", 9, false), "LIMITED"),
            (mk("claude", "idle", 9, false), "IDLE"),
            (mk("claude", "gone", 9, false), "UNAVAILABLE"),
            (mk("claude", "", 9, false), "UNAVAILABLE"),
        ] {
            assert_eq!(session_state(&session), expected, "{session:?}");
        }
    }

    #[test]
    fn badge_table_and_pinned_prefix() {
        let mk = |state: &str, pinned: bool| Session {
            state: state.into(),
            pid: 7,
            pinned,
            ..Default::default()
        };
        assert_eq!(session_badge(&mk("running", false)), "✳  WORKING");
        assert_eq!(session_badge(&mk("blocked", false)), "◆  INPUT");
        assert_eq!(session_badge(&mk("limited", false)), "◷  LIMITED");
        assert_eq!(session_badge(&mk("idle", false)), "○  READY");
        assert_eq!(session_badge(&mk("paused", false)), "Ⅱ  PAUSED");
        assert_eq!(session_badge(&mk("closed", false)), "—  CLOSED");
        assert_eq!(session_badge(&mk("other", false)), "?  UNKNOWN");
        assert_eq!(session_badge(&mk("idle", true)), "⌖  ○  READY");
        let shell = Session {
            provider: "shell".into(),
            ..Default::default()
        };
        assert_eq!(session_badge(&shell), "›_ TERMINAL");
    }

    #[test]
    fn summary_table() {
        let mk = |state: &str, status: &str, last: &str, hook: &str| Session {
            state: state.into(),
            pid: 7,
            status_text: status.into(),
            last_message: last.into(),
            hook_warning: hook.into(),
            ..Default::default()
        };
        assert_eq!(
            session_summary(&mk("paused", "", "", "")),
            "Paused; opens where it left off"
        );
        assert_eq!(session_summary(&mk("running", "", "", "")), "Working…");
        assert_eq!(
            session_summary(&mk("blocked", "", "", "")),
            "Waiting for your approval or answer"
        );
        assert_eq!(
            session_summary(&mk("limited", "", "", "")),
            "Usage limit reached"
        );
        assert_eq!(
            session_summary(&mk("weird", "", "msg", "")),
            "Unable to observe the current turn"
        );
        assert_eq!(session_summary(&mk("idle", "", "a\n\nb", "")), "a b");
        assert_eq!(
            session_summary(&mk("idle", "", "", "hook broken")),
            "hook broken"
        );
        assert_eq!(
            session_summary(&mk("idle", "", "", "")),
            "Ready for a message"
        );
        assert_eq!(
            session_summary(&mk("closed", "", "", "")),
            "Agent process is closed"
        );
        let shell = Session {
            provider: "shell".into(),
            ..Default::default()
        };
        assert_eq!(session_summary(&shell), "Agent process is closed");
    }

    #[test]
    fn stale_running_status_marks_freshness_without_an_expired_timer() {
        let mut s = Session {
            state: "running".into(),
            pid: 123,
            status_text: "Working (0s)".into(),
            ..Default::default()
        };
        assert_eq!(stale_session_summary(&s), "◷  Working");

        s.status_text = "Thinking… (1s · ↓ 134 tokens)".into();
        assert_eq!(stale_session_summary(&s), "◷  Thinking… (↓ 134 tokens)");

        s.status_text = "API error · Retrying in 36s · attempt 7/10".into();
        assert_eq!(stale_session_summary(&s), "◷  API error");

        s.status_text = "Reading (a file)".into();
        assert_eq!(stale_session_summary(&s), "◷  Reading (a file)");

        s.state = "idle".into();
        s.last_message = "done".into();
        assert_eq!(stale_session_summary(&s), "◷  done");
    }

    #[test]
    fn paused_sessions_keep_their_own_state() {
        let mut s = Session {
            state: "paused".into(),
            id: "abc".into(),
            pid: 0,
            managed: true,
            tmux_pane: "%3".into(),
            ..Default::default()
        };
        assert_eq!(session_badge(&s), "Ⅱ  PAUSED");
        assert_eq!(session_summary(&s), "Paused; opens where it left off");
        s.status_text = "Paused 3h ago; opens where it left off".into();
        assert_eq!(
            session_summary(&s),
            "Paused 3h ago; opens where it left off"
        );
        assert!(session_matches_filters(&s, false, false, false));
        assert!(!session_matches_filters(&s, false, false, true));
        s.historical = true;
        assert_eq!(session_badge(&s), "—  CLOSED");
    }

    #[test]
    fn hide_non_running_only_shows_running_sessions() {
        let mut session = Session {
            state: "idle".into(),
            pid: 123,
            managed: true,
            ..Default::default()
        };
        assert!(session_matches_filters(&session, false, false, false));
        assert!(!session_matches_filters(&session, false, false, true));

        session.state = "running".into();
        assert!(session_matches_filters(&session, false, false, true));
        session.state = "blocked".into();
        assert!(!session_matches_filters(&session, false, false, true));
        session.state = "limited".into();
        assert!(!session_matches_filters(&session, false, false, true));

        session.state = "paused".into();
        assert!(!session_matches_filters(&session, false, false, true));
        session.state = "closed".into();
        assert!(!session_matches_filters(&session, false, false, true));
        session.state = "starting".into();
        assert!(!session_matches_filters(&session, false, false, true));
        session.state = "unknown".into();
        assert!(!session_matches_filters(&session, false, false, true));
        session.state = "working".into();
        assert!(session_matches_filters(&session, false, false, true));
        session.provider = "shell".into();
        assert!(!session_matches_filters(&session, false, false, true));
    }

    #[test]
    fn filter_table() {
        let mk = |hidden: bool, historical: bool, archived: bool, managed: bool, tracked: bool| {
            Session {
                state: "idle".into(),
                pid: 1,
                hidden,
                historical,
                archived,
                managed,
                tracked,
                ..Default::default()
            }
        };
        // (session, history, all, hide_non_running) → visible
        let cases = [
            (
                mk(true, false, false, true, false),
                false,
                true,
                false,
                false,
            ),
            (
                mk(false, true, false, true, false),
                false,
                false,
                false,
                false,
            ),
            (
                mk(false, true, false, true, false),
                true,
                false,
                false,
                true,
            ),
            (
                mk(false, false, true, true, false),
                false,
                false,
                false,
                false,
            ),
            (
                mk(false, false, true, true, false),
                false,
                true,
                false,
                true,
            ),
            (
                mk(false, false, false, false, false),
                false,
                false,
                false,
                false,
            ),
            (
                mk(false, false, false, false, false),
                false,
                true,
                false,
                true,
            ),
            (
                mk(false, false, false, false, true),
                false,
                false,
                false,
                true,
            ),
            (
                mk(false, false, false, true, false),
                false,
                false,
                false,
                true,
            ),
            (
                mk(false, false, false, true, false),
                false,
                false,
                true,
                false,
            ),
        ];
        for (i, (s, history, all, hide, expected)) in cases.iter().enumerate() {
            assert_eq!(
                session_matches_filters(s, *history, *all, *hide),
                *expected,
                "case {i}"
            );
        }
    }

    #[test]
    fn machine_errors_preserve_recent_status_but_do_not_extend_freshness() {
        let mut m = Machine::default();
        let observed = 100_000.0;
        let s = Session {
            observation_received_at: Some(observed),
            ..Default::default()
        };
        assert!(!session_observation_available(&m, &s, observed));
        m.last = Some(Snapshot {
            live_inventory: true,
            ..Default::default()
        });
        assert!(session_observation_available(&m, &s, observed));
        m.error = "SSH failed".into();
        m.observation_failed_at = Some(observed + 2_000.0);
        assert!(session_observation_available(&m, &s, observed + 29_000.0));
        m.observation_failed_at = Some(observed + 29_000.0);
        assert!(!session_observation_available(
            &m,
            &s,
            observed + SESSION_STATUS_MAX_AGE_MS
        ));
        // Durable states never go stale; a session never observed is not live.
        let paused = Session {
            state: "paused".into(),
            ..Default::default()
        };
        assert!(session_observation_available(&m, &paused, observed + 1e9));
        let never = Session::default();
        assert!(!session_observation_available(&m, &never, observed));
    }

    fn state_with_sessions(sessions: Vec<Session>) -> State {
        State {
            machines: vec![Machine {
                id: "m1".into(),
                name: "mac".into(),
                last: Some(Snapshot {
                    sessions,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            accounts: vec![Account {
                name: "acct".into(),
                email: "me@example.com".into(),
                provider: "claude".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn sessions_sort_pinned_then_unarchived_then_newest() {
        let mk = |id: &str, pinned: bool, archived: bool, started: f64| Session {
            id: id.into(),
            state: "idle".into(),
            pid: 1,
            managed: true,
            pinned,
            archived,
            started_epoch: started,
            ..Default::default()
        };
        let state = state_with_sessions(vec![
            mk("old", false, false, 10.0),
            mk("archived-new", false, true, 50.0),
            mk("new", false, false, 30.0),
            mk("pinned-old", true, false, 5.0),
            mk("pinned-archived", true, true, 40.0),
        ]);
        let ids: Vec<&str> = session_refs(&state, "", false, true, false)
            .iter()
            .map(|(_, s)| s.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "pinned-old",
                "pinned-archived",
                "new",
                "old",
                "archived-new"
            ]
        );
        let ids: Vec<&str> = session_refs(&state, "", false, false, false)
            .iter()
            .map(|(_, s)| s.id.as_str())
            .collect();
        assert_eq!(ids, ["pinned-old", "new", "old"]);
    }

    #[test]
    fn search_matches_title_cwd_account_provider_id_and_machine() {
        let state = state_with_sessions(vec![Session {
            id: "sess-42".into(),
            title: "Fix Login".into(),
            cwd: "/home/x/Repo".into(),
            account: "acct".into(),
            provider: "claude".into(),
            state: "idle".into(),
            pid: 1,
            managed: true,
            ..Default::default()
        }]);
        for query in [
            "login",
            "REPO",
            "me@example",
            "Claude",
            "sess-42",
            "MAC",
            "claude · me@",
        ] {
            assert_eq!(
                session_refs(&state, query, false, false, false).len(),
                1,
                "{query}"
            );
        }
        assert!(session_refs(&state, "nothing", false, false, false).is_empty());
    }

    #[test]
    fn machine_provider_versions_are_compact_and_explicit() {
        let mut m = Machine::default();
        let mut latest = HashMap::new();
        assert_eq!(provider_versions(&m, &latest), "");

        let mut versions = HashMap::new();
        versions.insert("codex".into(), "codex-cli 0.153.4".into());
        versions.insert("claude".into(), "2.1.266 (Claude Code)".into());
        versions.insert("opencode".into(), "1.18.4".into());
        m.last = Some(Snapshot {
            versions,
            ..Default::default()
        });
        assert_eq!(
            provider_versions(&m, &latest),
            "Codex 0.153.4 · Claude 2.1.266 · OpenCode 1.18.4"
        );
        assert!(provider_update_action_available(&m, &latest, "codex"));

        latest.insert("codex".into(), "0.154.0".into());
        latest.insert("claude".into(), "2.1.266".into());
        assert_eq!(
            provider_versions(&m, &latest),
            "Codex 0.153.4 → 0.154.0 available · Claude 2.1.266 · OpenCode 1.18.4"
        );
        assert_eq!(provider_update_available(&m, &latest, "codex"), Some(true));
        assert!(provider_outdated(&m, &latest, "codex"));
        assert!(provider_update_action_available(&m, &latest, "codex"));
        assert_eq!(
            provider_update_available(&m, &latest, "claude"),
            Some(false)
        );
        assert!(!provider_outdated(&m, &latest, "claude"));
        assert!(!provider_update_action_available(&m, &latest, "claude"));

        m.last
            .as_mut()
            .unwrap()
            .versions
            .insert("claude".into(), "unavailable".into());
        assert_eq!(
            provider_versions(&m, &latest),
            "Codex 0.153.4 → 0.154.0 available · Claude not installed → 2.1.266 available · OpenCode 1.18.4"
        );
        assert!(!provider_outdated(&m, &latest, "claude"));
        assert!(provider_update_action_available(&m, &latest, "claude"));

        // Grok appears once it reports a version; its banner keeps only the
        // version word. Antigravity appears whenever it is reported at all.
        let versions = &mut m.last.as_mut().unwrap().versions;
        versions.insert("claude".into(), "2.1.266 (Claude Code)".into());
        versions.insert("grok".into(), "grok 0.4.2 (build abc)".into());
        versions.insert("antigravity".into(), "unavailable".into());
        latest.insert("grok".into(), "0.5.0".into());
        assert_eq!(provider_version(&m, "grok").as_deref(), Some("0.4.2"));
        assert_eq!(
            provider_versions(&m, &latest),
            "Codex 0.153.4 → 0.154.0 available · Claude 2.1.266 · OpenCode 1.18.4 \
             · Grok 0.4.2 → 0.5.0 available · Antigravity not installed"
        );
        let versions = &mut m.last.as_mut().unwrap().versions;
        versions.insert("grok".into(), "".into());
        versions.insert("antigravity".into(), "".into());
        assert_eq!(
            provider_versions(&m, &latest),
            "Codex 0.153.4 → 0.154.0 available · Claude 2.1.266 · OpenCode 1.18.4 \
             · Antigravity unknown"
        );
    }

    #[test]
    fn machine_status_lines() {
        let mut m = Machine::default();
        assert_eq!(machine_status(&m), ("not contacted".into(), String::new()));
        m.last = Some(Snapshot {
            platform: "darwin".into(),
            cores: 8,
            cpu: "Apple M2".into(),
            cpu_percent: 12.4,
            memory_used: 1 << 30,
            memory_total: 16 << 30,
            live_inventory: true,
            sessions: vec![
                Session {
                    state: "running".into(),
                    pid: 1,
                    ..Default::default()
                },
                Session {
                    state: "idle".into(),
                    pid: 2,
                    ..Default::default()
                },
                Session {
                    state: "closed".into(),
                    pid: 3,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        assert_eq!(
            machine_status(&m),
            (
                "2 sessions · 1 working · CPU 12% · memory 1.0 GB / 16.0 GB".into(),
                "darwin · 8 cores · Apple M2".into()
            )
        );
        m.last.as_mut().unwrap().live_inventory = false;
        m.helper_status = "helper outdated".into();
        assert_eq!(
            machine_status(&m),
            (
                "Live inventory unavailable · memory 1.0 GB / 16.0 GB".into(),
                "darwin · 8 cores · Apple M2 · helper outdated".into()
            )
        );
        m.error = "ssh: timed out".into();
        assert_eq!(machine_status(&m).0, "UNREACHABLE · ssh: timed out");
        m.sprite_source = "acme".into();
        assert_eq!(
            machine_status(&m),
            (
                "Creates a new Sprite for each session".into(),
                "Fly.io · acme".into()
            )
        );
        m.sprite = Some(SpriteBinding::default());
        assert_eq!(
            machine_status(&m),
            ("Creating session Sprite…".into(), "Fly.io".into())
        );
    }

    #[test]
    fn machine_rows_hide_session_sprites_without_losing_inventory() {
        let state = State {
            machines: vec![
                Machine {
                    id: "mac".into(),
                    local: true,
                    ..Default::default()
                },
                Machine {
                    id: "pending".into(),
                    sprite: Some(SpriteBinding::default()),
                    ..Default::default()
                },
                Machine {
                    id: "linux".into(),
                    ..Default::default()
                },
                Machine {
                    id: "running".into(),
                    sprite: Some(SpriteBinding {
                        id: "cloud-id".into(),
                        request_id: "session-id".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Machine {
                    id: "fly-org".into(),
                    sprite_source: "acme".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            machine_rows(&state)
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            ["mac", "linux", "fly-org"]
        );
    }

    #[test]
    fn summary_keeps_provider_order_and_skips_unset() {
        let mut defaults = HashMap::new();
        assert_eq!(permissions_summary(&defaults), "");
        defaults.insert("claude".to_string(), "auto".to_string());
        defaults.insert("codex".to_string(), "yolo".to_string());
        defaults.insert("opencode".to_string(), String::new());
        assert_eq!(permissions_summary(&defaults), "Codex yolo · Claude auto");
    }

    #[test]
    fn profiles_sort_by_shortcut_then_name() {
        let mut presets = HashMap::new();
        let mk = |shortcut: i64| LaunchProfile {
            shortcut,
            ..Default::default()
        };
        presets.insert("zeta".to_string(), mk(0));
        presets.insert("alpha".to_string(), mk(0));
        presets.insert("nine".to_string(), mk(9));
        presets.insert("two-b".to_string(), mk(2));
        presets.insert("two-a".to_string(), mk(2));
        let state = State {
            presets,
            ..Default::default()
        };
        let names: Vec<String> = profiles(&state).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["two-a", "two-b", "nine", "alpha", "zeta"]);
    }

    #[test]
    fn accounts_and_labels() {
        let a = Account {
            email: "x@y.z".into(),
            name: "n".into(),
            provider: "codex".into(),
            ..Default::default()
        };
        assert_eq!(account_email(&a), "x@y.z");
        assert_eq!(account_label(&a), "Codex · x@y.z");
        assert_eq!(account_handle(&a), "codex:x@y.z");
        let key = Account {
            name: "claude-api-work".into(),
            provider: "claude".into(),
            ..Default::default()
        };
        assert_eq!(account_email(&key), "API key · claude-api-work");
        let none = Account {
            name: "n".into(),
            provider: "claude".into(),
            ..Default::default()
        };
        assert_eq!(account_email(&none), "Email unavailable");
        assert_eq!(provider_label("opencode"), "OpenCode");
        assert_eq!(provider_label("antigravity"), "Antigravity");
        assert_eq!(provider_label("grok"), "Grok");
        assert_eq!(provider_label("other"), "other");

        let state = State {
            accounts: vec![
                a.clone(),
                Account {
                    email: "dup@y.z".into(),
                    name: "d1".into(),
                    provider: "claude".into(),
                    ..Default::default()
                },
                Account {
                    email: "dup@y.z".into(),
                    name: "d2".into(),
                    provider: "codex".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(account(&state, "n").map(|a| a.name.as_str()), Some("n"));
        assert_eq!(
            account(&state, "codex:x@y.z").map(|a| a.name.as_str()),
            Some("n")
        );
        assert_eq!(account(&state, "x@y.z").map(|a| a.name.as_str()), Some("n"));
        assert_eq!(account(&state, "d2").map(|a| a.name.as_str()), Some("d2"));
        assert!(account(&state, "dup@y.z").is_none());
        assert!(account(&state, "").is_none());
    }

    #[test]
    fn usage_reset_is_relative() {
        assert_eq!(
            format_usage_reset("1970-01-01T00:30:00Z", 0.0),
            "resets in 30 min"
        );
        assert_eq!(
            format_usage_reset("1970-01-01T01:00:00Z", 0.0),
            "resets in 1 hour"
        );
        assert_eq!(
            format_usage_reset("1970-01-01T05:00:00Z", 0.0),
            "resets in 5 hours"
        );
        assert_eq!(
            format_usage_reset("1970-01-02T00:00:00Z", 0.0),
            "resets in 1 day"
        );
        assert_eq!(
            format_usage_reset("1970-01-03T00:00:00Z", 0.0),
            "resets in 2 days"
        );
        assert_eq!(
            format_usage_reset("1970-01-01T00:00:01Z", 5.0),
            "resets now"
        );
        assert_eq!(format_usage_reset("", 0.0), "reset time unknown");
    }

    #[test]
    fn text_helpers() {
        assert_eq!(clean("a\u{7}b\nc"), "ab\nc");
        assert_eq!(one_line("  a \n\n b\t c "), "a b c");
        assert_eq!(gib(1 << 30), "1.0 GB");
        assert_eq!(gib(3 << 29), "1.5 GB");
    }
}
