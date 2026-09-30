//! The manager's list rows (ui.rs `rows`, 494-700), the row actions and
//! "⋯" menus (ui.rs 347-445, workspace.rs `row_menu_items`) and the empty
//! states (ui.rs 1611-1633), as the `ROW`, `ROW_ACTION` and `METER` shapes.

use crate::indicator;
use crate::model::Page;
use crate::session::{
    account, account_email, clean, format_usage_reset, machine, machine_rows, machine_status,
    permissions_summary, profiles, provider_label, provider_outdated,
    provider_update_action_available, provider_versions, session_badge,
    session_observation_available, session_refs, session_state, session_summary,
    stale_session_summary,
};
use crate::theme::{provider_usage_color, Theme};
use crate::types::{Machine, ProviderUsage, State};
use serde::Serialize;
use std::collections::HashMap;

/// A button on a row: `shapes::ROW_ACTION`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowAction {
    /// The command's snake name (`open`, `archive`, `update_open_code`, …).
    pub id: String,
    /// The Lucide icon's name.
    pub icon: String,
    /// The tooltip's label.
    pub label: String,
    /// The key hint after the label ("" until the keymap is wired).
    pub hint: String,
}

/// One usage bar: `shapes::METER`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meter {
    /// "5 hour", "Weekly", "Weekly (Fable)".
    pub label: String,
    /// 0–100.
    pub percent: f64,
    /// "NN%".
    pub text: String,
    /// "resets in 30 min", ….
    pub reset: String,
    /// The provider's bar color, CSS.
    pub color: String,
}

/// A row of the manager's list: `shapes::ROW`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// Machines: the machine id; Sessions: `{machine id}/{session id}`;
    /// Accounts: the account name; Profiles: the profile name.
    pub id: String,
    /// A group header above the row (Accounts: the provider), else "".
    pub header: String,
    /// The badge's text.
    pub badge: String,
    /// The badge's color, CSS.
    pub badge_color: String,
    /// The chip's background: the badge color at 0.15 alpha (ui.rs:1690).
    pub badge_bg: String,
    /// The title.
    pub title: String,
    /// The iMessage icon follows the title.
    pub imessage: bool,
    /// "#9" chips after the title.
    pub prs: Vec<String>,
    /// The lines under the title, empties dropped.
    pub lines: Vec<String>,
    /// `lines[0]` (or ""): the line `attention` paints warn.
    pub first_line: String,
    /// The index in `lines` rendered as inline markdown, -1 for none.
    pub markdown_line: i64,
    /// The first line is painted `warn` (Machines: Codex or Claude outdated).
    pub attention: bool,
    /// A working indicator row follows the lines.
    pub working: bool,
    /// The indicator's glyph ("" for non-Claude).
    pub working_glyph: String,
    /// The indicator's header text.
    pub working_head: String,
    /// The header's color, CSS.
    pub working_head_color: String,
    /// The text after the header.
    pub working_rest: String,
    /// Usage meters (Accounts, connected and loaded).
    pub usage: Vec<Meter>,
    /// "Loading usage limits…", "Usage limits unavailable" or "".
    pub usage_note: String,
    /// The session is archived (the Archive action reads Unarchive).
    pub archived: bool,
    /// The row is the selected one.
    pub selected: bool,
    /// The buttons at the row's right.
    pub actions: Vec<RowAction>,
}

/// An item of a row's "⋯" menu (`shapes::MENU_ITEM`'s id, label and icon).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuItem {
    /// The command's snake name.
    pub id: String,
    /// The label.
    pub label: String,
    /// The Lucide icon's name.
    pub icon: String,
}

/// What the model knows about an account's usage limits. Absent from the
/// map: nothing is shown. The 5-minute cache is the caller's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UsageLoad {
    /// When the fetch was asked for or answered, host milliseconds.
    pub at_ms: f64,
    /// The usage, once fetched; None while loading.
    pub usage: Option<ProviderUsage>,
    /// The fetch failed.
    pub error: bool,
}

/// What the rows are built from.
#[derive(Clone, Copy, Debug)]
pub struct Rows<'a> {
    /// The page.
    pub page: Page,
    /// The fleet.
    pub state: &'a State,
    /// Newest provider versions the feed reported.
    pub latest_provider_versions: &'a HashMap<String, String>,
    /// Usage by account name.
    pub usage: &'a HashMap<String, UsageLoad>,
    /// The search text.
    pub query: &'a str,
    /// View ▾ History.
    pub history: bool,
    /// View ▾ Untracked & archived.
    pub all_sessions: bool,
    /// View ▾ Hide non-running.
    pub hide_non_running: bool,
    /// The selected row.
    pub selected: usize,
    /// The host clock, milliseconds (observation freshness, the indicator).
    pub now_ms: f64,
    /// Unix seconds (usage reset text).
    pub now_epoch_s: f64,
    /// The theme, for colors.
    pub theme: &'a Theme,
}

/// (id, icon, label): a row button or menu item before its per-row swap.
pub type Action = (&'static str, &'static str, &'static str);

/// Buttons shown on every row (ui.rs `row_actions`). Keep these to the
/// essentials; everything else lives in the row's "⋯" menu (`row_menu`).
pub fn row_actions(page: Page) -> &'static [Action] {
    match page {
        Page::Machines => &[
            ("connect_fly", "plus", "Connect Fly.io…"),
            ("open", "list", "Show sessions"),
            ("shell", "terminal", "Open shell"),
            ("new", "play", "Launch session here"),
        ],
        Page::Sessions => &[
            ("open", "play", "Attach"),
            ("message", "message-square", "Last assistant message"),
            ("handoff", "arrow-right-left", "Hand off…"),
            ("archive", "archive", "Archive"),
        ],
        Page::Accounts => &[
            ("open", "log-in", "Sign in / open shell"),
            ("edit", "key-round", "Reconnect"),
        ],
        Page::Profiles => &[
            ("open", "play", "Launch"),
            ("edit", "pencil", "Edit"),
            ("delete", "trash", "Remove…"),
        ],
    }
}

/// Less common row actions, shown in a dropdown behind the "⋯" button and
/// on right-click (ui.rs `row_menu`).
pub fn row_menu(page: Page) -> &'static [Action] {
    match page {
        Page::Machines => &[
            ("edit", "pencil", "Edit machine…"),
            ("update_codex", "rotate-ccw", "Update Codex…"),
            ("update_claude", "rotate-ccw", "Update Claude…"),
            ("update_open_code", "rotate-ccw", "Update OpenCode…"),
            ("update_machine", "rotate-ccw", "Update Ocho helper"),
            ("delete", "trash", "Remove from Ocho…"),
        ],
        Page::Sessions => &[
            ("view_read_only", "eye", "Attach read-only"),
            ("edit", "tag", "Label"),
            ("pin", "pin", "Pin / unpin"),
            ("interrupt", "pause", "Interrupt turn…"),
            ("pause", "pause", "Pause / unpause"),
            ("resume", "rotate-ccw", "Resume native conversation"),
            ("move", "arrow-right-left", "Move to another machine…"),
            ("stop", "square", "Stop…"),
            ("delete", "trash", "Stop and remove…"),
        ],
        Page::Profiles => &[("edit", "pencil", "Edit…"), ("delete", "trash", "Remove…")],
        Page::Accounts => &[("edit", "key-round", "Reconnect")],
    }
}

/// Archive / unarchive swap icon and label depending on the row.
pub fn row_action_for(action: &Action, archived: bool) -> Action {
    match action.0 {
        "archive" if archived => ("archive", "archive-restore", "Unarchive"),
        _ => *action,
    }
}

/// Page-level action shown in the header (creates something, not tied to a
/// row): (command id, label).
pub fn primary(page: Page) -> (&'static str, &'static str) {
    match page {
        Page::Machines => ("add_machine", "Add machine"),
        Page::Sessions => ("new", "Launch session"),
        Page::Accounts => ("add_account", "Add account"),
        Page::Profiles => ("add_profile", "Add profile"),
    }
}

/// The "⋯" menu of a row (workspace.rs `row_menu_items`): a Machines row
/// offers "Update {provider}…" only while an update is available or the
/// tool is not installed; a Sessions row's Archive reads Unarchive when
/// `archived`.
pub fn menu(
    page: Page,
    archived: bool,
    machine: Option<&Machine>,
    latest_provider_versions: &HashMap<String, String>,
) -> Vec<MenuItem> {
    row_menu(page)
        .iter()
        .filter(|(id, _, _)| match *id {
            "update_codex" => machine.is_some_and(|m| {
                provider_update_action_available(m, latest_provider_versions, "codex")
            }),
            "update_claude" => machine.is_some_and(|m| {
                provider_update_action_available(m, latest_provider_versions, "claude")
            }),
            "update_open_code" => machine.is_some_and(|m| {
                provider_update_action_available(m, latest_provider_versions, "opencode")
            }),
            _ => true,
        })
        .map(|action| {
            let (id, icon, label) = row_action_for(action, archived);
            MenuItem {
                id: id.into(),
                label: label.into(),
                icon: icon.into(),
            }
        })
        .collect()
}

/// The list's text when it has no rows (ui.rs:1611-1633).
pub fn empty_text(
    page: Page,
    loaded: bool,
    loading: bool,
    query_nonempty: bool,
    hide_non_running: bool,
) -> String {
    if !loaded && loading {
        return "Refreshing Ocho…".into();
    }
    match page {
        Page::Profiles => "No launch profiles yet. Add one with the button above.",
        Page::Accounts => {
            "No accounts connected. Add one above, choose Claude, Codex or OpenCode, and sign in."
        }
        Page::Machines => "No machines enrolled. Add one with the button above.",
        Page::Sessions => {
            if query_nonempty {
                "No sessions match your search."
            } else if hide_non_running {
                "No running sessions here. Turn off Hide non-running to show other sessions."
            } else {
                "No sessions here. Launch one, or show untracked and archived sessions from View."
            }
        }
    }
    .into()
}

/// Drop blank lines (ui.rs `non_empty`).
fn non_empty(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().filter(|l| !l.trim().is_empty()).collect()
}

/// The badge color for a session state (ui.rs `badge_color`).
fn session_badge_color(state: &str, theme: &Theme) -> String {
    match state {
        "RUNNING" => theme.good,
        "IDLE" | "CLOSED" | "UNAVAILABLE" | "PAUSED" => theme.muted,
        "BLOCKED" | "LIMITED" => theme.warn,
        _ => theme.accent,
    }
    .css()
}

/// The buttons of a row: Hand off… only for a limited session, Archive
/// swapped for Unarchive on an archived one.
fn actions(page: Page, archived: bool, handoff: bool) -> Vec<RowAction> {
    row_actions(page)
        .iter()
        .filter(|(id, _, _)| *id != "handoff" || handoff)
        .map(|action| {
            let (id, icon, label) = row_action_for(action, archived);
            RowAction {
                id: id.into(),
                icon: icon.into(),
                label: label.into(),
                hint: String::new(),
            }
        })
        .collect()
}

/// A row with nothing but its identity, to be filled in.
fn blank(id: String) -> Row {
    Row {
        id,
        header: String::new(),
        badge: String::new(),
        badge_color: String::new(),
        badge_bg: String::new(),
        title: String::new(),
        imessage: false,
        prs: Vec::new(),
        lines: Vec::new(),
        first_line: String::new(),
        markdown_line: -1,
        attention: false,
        working: false,
        working_glyph: String::new(),
        working_head: String::new(),
        working_head_color: String::new(),
        working_rest: String::new(),
        usage: Vec::new(),
        usage_note: String::new(),
        archived: false,
        selected: false,
        actions: Vec::new(),
    }
}

/// The usage meters of an account (workspace.rs `account_usage_display`,
/// ui.rs 1750-1825): (meters, note). Nothing for an account that is not
/// connected or has no load yet.
fn usage_display(ctx: &Rows, name: &str, status: &str, provider: &str) -> (Vec<Meter>, String) {
    if status != "connected" {
        return (Vec::new(), String::new());
    }
    let Some(load) = ctx.usage.get(name) else {
        return (Vec::new(), String::new());
    };
    if load.error {
        return (Vec::new(), "Usage limits unavailable".into());
    }
    let Some(usage) = &load.usage else {
        return (Vec::new(), "Loading usage limits…".into());
    };
    let color = provider_usage_color(provider, ctx.theme);
    let mut meters = Vec::new();
    for (label, window) in [
        ("5 hour", usage.five_hour.as_ref()),
        ("Weekly", usage.weekly.as_ref()),
        ("Weekly (Fable)", usage.fable_weekly.as_ref()),
    ] {
        if let Some(window) = window {
            let percent = window.used_percent.clamp(0.0, 100.0);
            meters.push(Meter {
                label: label.into(),
                percent,
                text: format!("{percent:.0}%"),
                reset: format_usage_reset(&window.resets_at, ctx.now_epoch_s),
                color: color.clone(),
            });
        }
    }
    if meters.is_empty() {
        (Vec::new(), "Usage limits unavailable".into())
    } else {
        (meters, String::new())
    }
}

/// The rows of the page (ui.rs `rows`).
pub fn rows(ctx: &Rows) -> Vec<Row> {
    let t = ctx.theme;
    let mut out: Vec<Row> = match ctx.page {
        Page::Machines => machine_rows(ctx.state)
            .map(|m| {
                let (status, spec) = machine_status(m);
                let providers = provider_versions(m, ctx.latest_provider_versions);
                let permissions = permissions_summary(&m.permissions);
                let permissions = if permissions.is_empty() {
                    String::new()
                } else {
                    format!("Permissions · {permissions}")
                };
                let attention = ["codex", "claude"]
                    .into_iter()
                    .any(|provider| provider_outdated(m, ctx.latest_provider_versions, provider));
                let mut title = m.name.clone();
                if m.local {
                    title.push_str("  · this computer");
                }
                let (badge, color) = if !m.error.is_empty() {
                    ("unreachable", t.danger)
                } else if m.last.is_some() {
                    ("online", t.good)
                } else {
                    ("pending", t.muted)
                };
                Row {
                    badge: badge.into(),
                    badge_color: color.css(),
                    title,
                    lines: non_empty(vec![providers, permissions, status, spec]),
                    attention,
                    actions: actions(Page::Machines, false, false),
                    ..blank(m.id.clone())
                }
            })
            .collect(),
        Page::Sessions => session_refs(
            ctx.state,
            ctx.query,
            ctx.history,
            ctx.all_sessions,
            ctx.hide_non_running,
        )
        .into_iter()
        .map(|(machine, s)| {
            let available = session_observation_available(machine, s, ctx.now_ms);
            let state = session_state(s);
            let badge = session_badge(s);
            let account = match account(ctx.state, &s.account) {
                Some(a) => account_email(a),
                None if s.account.is_empty() => "native login".to_string(),
                None => s.account.clone(),
            };
            let archived = if s.archived { "archived · " } else { "" };
            let working = available && state == "RUNNING" && !s.status_text.is_empty();
            let summary_is_message = available
                && !working
                && !s.last_message.is_empty()
                && !matches!(state, "RUNNING" | "BLOCKED" | "UNAVAILABLE");
            let mut row = Row {
                badge,
                badge_color: if available {
                    session_badge_color(state, t)
                } else {
                    t.muted.css()
                },
                title: clean(&s.title),
                imessage: s.imessage_attached,
                prs: s.pull_requests.iter().map(|pr| pr.label()).collect(),
                lines: non_empty(vec![
                    format!(
                        "{archived}{} · {} · {} · {}",
                        machine.name, s.provider, account, s.cwd
                    ),
                    if working {
                        String::new()
                    } else if available {
                        session_summary(s)
                    } else {
                        stale_session_summary(s)
                    },
                ]),
                markdown_line: if summary_is_message { 1 } else { -1 },
                archived: s.archived,
                working,
                actions: actions(Page::Sessions, s.archived, available && state == "LIMITED"),
                ..blank(format!("{}/{}", machine.id, s.id))
            };
            if working {
                let ind = indicator::indicator(&s.provider, &s.status_text, ctx.now_ms, t);
                row.working_glyph = ind.glyph;
                row.working_head = ind.head;
                row.working_head_color = ind.head_color;
                row.working_rest = match ind.progress {
                    // The desktop draws a bar and the percent after "Compacting".
                    Some(percent) => format!(" {percent}%"),
                    None => ind.rest,
                };
            }
            row
        })
        .collect(),
        Page::Accounts => {
            let mut previous = String::new();
            ctx.state
                .accounts
                .iter()
                .map(|a| {
                    let header = if a.provider != previous {
                        provider_label(&a.provider).to_string()
                    } else {
                        String::new()
                    };
                    previous = a.provider.clone();
                    let (badge, color) = if !a.shared {
                        ("finish connecting".to_string(), t.warn)
                    } else if a.status == "connected" {
                        (a.status.clone(), t.good)
                    } else {
                        (a.status.clone(), t.warn)
                    };
                    let mut lines = Vec::new();
                    if !a.shared {
                        lines.push(
                            "Press Enter to sign in and finish saving this account to Cloudflare"
                                .to_string(),
                        );
                    }
                    lines.push(a.notes.clone());
                    let (usage, usage_note) = usage_display(ctx, &a.name, &a.status, &a.provider);
                    Row {
                        header,
                        badge,
                        badge_color: color.css(),
                        title: account_email(a),
                        lines: non_empty(lines),
                        usage,
                        usage_note,
                        actions: actions(Page::Accounts, false, false),
                        ..blank(a.name.clone())
                    }
                })
                .collect()
        }
        Page::Profiles => profiles(ctx.state)
            .iter()
            .map(|p| {
                let (badge, color) = if p.profile.shortcut == 0 {
                    ("no shortcut".to_string(), t.muted)
                } else {
                    (format!("⌘⌥{}", p.profile.shortcut), t.accent)
                };
                let machine = machine(ctx.state, &p.profile.machine_id)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| "choose a machine · e to edit".into());
                let model = if p.profile.model.is_empty() {
                    "default model".to_string()
                } else {
                    p.profile.model.clone()
                };
                let effort = if p.profile.effort.is_empty() {
                    "default effort".to_string()
                } else {
                    p.profile.effort.clone()
                };
                let account = account(ctx.state, &p.profile.account)
                    .map(account_email)
                    .unwrap_or_else(|| "machine login".into());
                let permissions = if p.profile.permissions.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", p.profile.permissions)
                };
                let lines = vec![
                    format!(
                        "{machine} · {} · {model} · {effort}{permissions}",
                        provider_label(&p.profile.provider)
                    ),
                    format!("{} · {account}", p.profile.cwd),
                ];
                Row {
                    badge,
                    badge_color: color.css(),
                    title: p.name.clone(),
                    lines: non_empty(lines),
                    actions: actions(Page::Profiles, false, false),
                    ..blank(p.name.clone())
                }
            })
            .collect(),
    };
    for row in &mut out {
        row.badge_bg = crate::theme::Rgba::parse(&row.badge_color)
            .map(|c| c.alpha(0.15).css())
            .unwrap_or_default();
        row.first_line = row.lines.first().cloned().unwrap_or_default();
    }
    if let Some(row) = out.get_mut(ctx.selected) {
        row.selected = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Account, LaunchProfile, PullRequest, Session, Snapshot, UsageWindow};

    fn fleet() -> State {
        let mut versions = HashMap::new();
        versions.insert("codex".to_string(), "codex-cli 0.153.4".to_string());
        versions.insert("claude".to_string(), "2.1.266 (Claude Code)".to_string());
        versions.insert("opencode".to_string(), "1.18.4".to_string());
        let mut permissions = HashMap::new();
        permissions.insert("codex".to_string(), "yolo".to_string());
        let mut presets = HashMap::new();
        presets.insert(
            "daily".to_string(),
            LaunchProfile {
                provider: "claude".into(),
                account: "acct".into(),
                cwd: "~/src".into(),
                machine_id: "m1".into(),
                shortcut: 1,
                ..Default::default()
            },
        );
        presets.insert(
            "spare".to_string(),
            LaunchProfile {
                provider: "codex".into(),
                cwd: "/tmp".into(),
                permissions: "auto".into(),
                ..Default::default()
            },
        );
        State {
            machines: vec![
                Machine {
                    id: "m1".into(),
                    name: "mac".into(),
                    local: true,
                    permissions,
                    last: Some(Snapshot {
                        platform: "darwin".into(),
                        cores: 8,
                        cpu: "M2".into(),
                        cpu_percent: 5.0,
                        memory_used: 1 << 30,
                        memory_total: 16 << 30,
                        live_inventory: true,
                        versions,
                        sessions: vec![
                            Session {
                                id: "s-work".into(),
                                title: "Fix\u{7} login".into(),
                                provider: "claude".into(),
                                account: "acct".into(),
                                cwd: "~/src".into(),
                                state: "running".into(),
                                pid: 10,
                                managed: true,
                                status_text: "Thinking… (3s)".into(),
                                observation_received_at: Some(1_000.0),
                                started_epoch: 20.0,
                                imessage_attached: true,
                                pull_requests: vec![PullRequest {
                                    number: 9,
                                    ..Default::default()
                                }],
                                ..Default::default()
                            },
                            Session {
                                id: "s-idle".into(),
                                title: "Docs".into(),
                                provider: "codex".into(),
                                state: "idle".into(),
                                pid: 11,
                                tracked: true,
                                last_message: "**done**\nall good".into(),
                                observation_received_at: Some(1_000.0),
                                started_epoch: 30.0,
                                ..Default::default()
                            },
                            Session {
                                id: "s-limited".into(),
                                title: "Limited".into(),
                                provider: "codex".into(),
                                account: "ghost".into(),
                                state: "limited".into(),
                                pid: 12,
                                managed: true,
                                archived: true,
                                observation_received_at: Some(1_000.0),
                                started_epoch: 40.0,
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Machine {
                    id: "m2".into(),
                    name: "box".into(),
                    error: "ssh: timed out".into(),
                    ..Default::default()
                },
                Machine {
                    id: "m3".into(),
                    name: "new".into(),
                    ..Default::default()
                },
            ],
            accounts: vec![
                Account {
                    name: "acct".into(),
                    email: "me@example.com".into(),
                    provider: "claude".into(),
                    shared: true,
                    status: "connected".into(),
                    ..Default::default()
                },
                Account {
                    name: "cx".into(),
                    email: "cx@example.com".into(),
                    provider: "codex".into(),
                    shared: true,
                    status: "connected".into(),
                    notes: "team".into(),
                },
                Account {
                    name: "pending".into(),
                    provider: "codex".into(),
                    shared: false,
                    ..Default::default()
                },
            ],
            presets,
        }
    }

    fn ctx<'a>(
        page: Page,
        state: &'a State,
        latest: &'a HashMap<String, String>,
        usage: &'a HashMap<String, UsageLoad>,
        theme: &'a Theme,
    ) -> Rows<'a> {
        Rows {
            page,
            state,
            latest_provider_versions: latest,
            usage,
            query: "",
            history: false,
            all_sessions: false,
            hide_non_running: false,
            selected: 0,
            now_ms: 2_000.0,
            now_epoch_s: 0.0,
            theme,
        }
    }

    #[test]
    fn machine_rows_carry_versions_permissions_status_and_badges() {
        let state = fleet();
        let mut latest = HashMap::new();
        latest.insert("codex".to_string(), "0.154.0".to_string());
        let usage = HashMap::new();
        let t = Theme::ocho_dark();
        let rows = rows(&ctx(Page::Machines, &state, &latest, &usage, &t));
        assert_eq!(rows.len(), 3);
        let mac = &rows[0];
        assert_eq!(mac.id, "m1");
        assert_eq!(mac.title, "mac  · this computer");
        assert_eq!(mac.badge, "online");
        assert_eq!(mac.badge_color, t.good.css());
        assert_eq!(mac.badge_bg, t.good.alpha(0.15).css());
        assert_eq!(mac.first_line, mac.lines[0]);
        assert!(mac.attention);
        assert!(mac.selected);
        assert_eq!(
            mac.lines,
            [
                "Codex 0.153.4 → 0.154.0 available · Claude 2.1.266 · OpenCode 1.18.4",
                "Permissions · Codex yolo",
                "3 sessions · 1 working · CPU 5% · memory 1.0 GB / 16.0 GB",
                "darwin · 8 cores · M2",
            ]
        );
        assert_eq!(
            mac.actions
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["connect_fly", "open", "shell", "new"]
        );
        assert_eq!(mac.actions[0].icon, "plus");
        assert_eq!(mac.actions[0].label, "Connect Fly.io…");
        assert_eq!(rows[1].badge, "unreachable");
        assert_eq!(rows[1].badge_color, t.danger.css());
        assert_eq!(rows[1].lines, ["UNREACHABLE · ssh: timed out"]);
        assert!(!rows[1].selected);
        assert_eq!(rows[2].badge, "pending");
        assert_eq!(rows[2].badge_color, t.muted.css());
        assert_eq!(rows[2].lines, ["not contacted"]);
    }

    #[test]
    fn session_rows_show_indicator_summary_markdown_and_actions() {
        let state = fleet();
        let latest = HashMap::new();
        let usage = HashMap::new();
        let t = Theme::ocho_dark();
        let mut c = ctx(Page::Sessions, &state, &latest, &usage, &t);
        c.selected = 99;
        let rows = rows(&c);
        // Newest first; the archived session is hidden by default.
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["m1/s-idle", "m1/s-work"]
        );
        assert!(rows.iter().all(|r| !r.selected));

        let idle = &rows[0];
        assert_eq!(idle.badge, "○  READY");
        assert_eq!(idle.badge_color, t.muted.css());
        assert_eq!(
            idle.lines,
            ["mac · codex · native login · ", "**done** all good"]
        );
        assert_eq!(idle.markdown_line, 1);
        assert!(!idle.working);
        assert_eq!(
            idle.actions
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["open", "message", "archive"]
        );

        let work = &rows[1];
        assert_eq!(work.title, "Fix login");
        assert!(work.imessage);
        assert_eq!(work.prs, ["#9"]);
        assert_eq!(work.badge, "✳  WORKING");
        assert_eq!(work.badge_color, t.good.css());
        assert_eq!(work.lines, ["mac · claude · me@example.com · ~/src"]);
        assert_eq!(work.markdown_line, -1);
        assert!(work.working);
        assert_eq!(work.working_glyph, "·");
        assert_eq!(work.working_head, "Thinking…");
        assert_eq!(work.working_head_color, t.warn.css());
        assert_eq!(work.working_rest, " (3s)");
    }

    #[test]
    fn stale_sessions_fade_and_limited_archived_sessions_offer_handoff_and_unarchive() {
        let state = fleet();
        let latest = HashMap::new();
        let usage = HashMap::new();
        let t = Theme::ocho_dark();
        let mut c = ctx(Page::Sessions, &state, &latest, &usage, &t);
        c.all_sessions = true;
        c.now_ms = 1_000.0 + 31_000.0;
        let rows = rows(&c);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["m1/s-idle", "m1/s-work", "m1/s-limited"]
        );
        let work = &rows[1];
        assert!(!work.working);
        assert_eq!(work.badge_color, t.muted.css());
        assert_eq!(work.lines[1], "◷  Thinking…");
        let idle = &rows[0];
        assert_eq!(idle.markdown_line, -1);
        assert_eq!(idle.lines[1], "◷  **done** all good");
        // Fresh again: the limited session has the handoff button and reads Unarchive.
        c.now_ms = 2_000.0;
        let rows = super::rows(&c);
        let limited = &rows[2];
        assert!(limited.archived);
        assert_eq!(limited.badge, "◷  LIMITED");
        assert_eq!(limited.badge_color, t.warn.css());
        assert_eq!(
            limited.lines,
            ["archived · mac · codex · ghost · ", "Usage limit reached"]
        );
        assert_eq!(
            limited
                .actions
                .iter()
                .map(|a| (a.id.as_str(), a.icon.as_str(), a.label.as_str()))
                .collect::<Vec<_>>(),
            [
                ("open", "play", "Attach"),
                ("message", "message-square", "Last assistant message"),
                ("handoff", "arrow-right-left", "Hand off…"),
                ("archive", "archive-restore", "Unarchive"),
            ]
        );
        // Search narrows by machine name too.
        c.query = "BOX";
        assert!(super::rows(&c).is_empty());
        c.query = "mac";
        assert_eq!(super::rows(&c).len(), 3);
    }

    #[test]
    fn account_rows_group_by_provider_and_show_usage() {
        let state = fleet();
        let latest = HashMap::new();
        let mut usage = HashMap::new();
        usage.insert(
            "acct".to_string(),
            UsageLoad {
                at_ms: 0.0,
                usage: Some(ProviderUsage {
                    five_hour: Some(UsageWindow {
                        used_percent: 137.6,
                        resets_at: "1970-01-01T00:30:00Z".into(),
                    }),
                    weekly: Some(UsageWindow {
                        used_percent: 12.4,
                        resets_at: "1970-01-03T00:00:00Z".into(),
                    }),
                    fable_weekly: None,
                }),
                error: false,
            },
        );
        usage.insert("cx".to_string(), UsageLoad::default());
        let t = Theme::ocho_dark();
        let rows = rows(&ctx(Page::Accounts, &state, &latest, &usage, &t));
        assert_eq!(
            rows.iter().map(|r| r.header.as_str()).collect::<Vec<_>>(),
            ["Claude", "Codex", ""]
        );
        let claude = &rows[0];
        assert_eq!(claude.id, "acct");
        assert_eq!(claude.title, "me@example.com");
        assert_eq!(claude.badge, "connected");
        assert_eq!(claude.badge_color, t.good.css());
        assert!(claude.lines.is_empty());
        assert_eq!(claude.usage_note, "");
        assert_eq!(
            claude.usage,
            [
                Meter {
                    label: "5 hour".into(),
                    percent: 100.0,
                    text: "100%".into(),
                    reset: "resets in 30 min".into(),
                    color: "#D97757".into()
                },
                Meter {
                    label: "Weekly".into(),
                    percent: 12.4,
                    text: "12%".into(),
                    reset: "resets in 2 days".into(),
                    color: "#D97757".into()
                },
            ]
        );
        assert_eq!(
            claude
                .actions
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["open", "edit"]
        );
        let codex = &rows[1];
        assert_eq!(codex.lines, ["team"]);
        assert_eq!(codex.usage_note, "Loading usage limits…");
        assert!(codex.usage.is_empty());
        let pending = &rows[2];
        assert_eq!(pending.title, "Email unavailable");
        assert_eq!(pending.badge, "finish connecting");
        assert_eq!(pending.badge_color, t.warn.css());
        assert_eq!(
            pending.lines,
            ["Press Enter to sign in and finish saving this account to Cloudflare"]
        );
        assert_eq!(pending.usage_note, "");

        // A failed fetch, and a fetch with no windows, both read unavailable.
        usage.insert(
            "acct".to_string(),
            UsageLoad {
                at_ms: 0.0,
                usage: None,
                error: true,
            },
        );
        usage.insert(
            "cx".to_string(),
            UsageLoad {
                at_ms: 0.0,
                usage: Some(ProviderUsage::default()),
                error: false,
            },
        );
        let rows = super::rows(&ctx(Page::Accounts, &state, &latest, &usage, &t));
        assert_eq!(rows[0].usage_note, "Usage limits unavailable");
        assert_eq!(rows[1].usage_note, "Usage limits unavailable");
    }

    #[test]
    fn profile_rows_sort_and_describe_the_launch() {
        let state = fleet();
        let latest = HashMap::new();
        let usage = HashMap::new();
        let t = Theme::ocho_dark();
        let rows = rows(&ctx(Page::Profiles, &state, &latest, &usage, &t));
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["daily", "spare"]
        );
        assert_eq!(rows[0].badge, "⌘⌥1");
        assert_eq!(rows[0].badge_color, t.accent.css());
        assert_eq!(
            rows[0].lines,
            [
                "mac · Claude · default model · default effort",
                "~/src · me@example.com"
            ]
        );
        assert_eq!(rows[1].badge, "no shortcut");
        assert_eq!(rows[1].badge_color, t.muted.css());
        assert_eq!(
            rows[1].lines,
            [
                "choose a machine · e to edit · Codex · default model · default effort · auto",
                "/tmp · machine login"
            ]
        );
        assert_eq!(
            rows[1]
                .actions
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["open", "edit", "delete"]
        );
    }

    #[test]
    fn menus_hide_current_provider_updates() {
        let state = fleet();
        let mut latest = HashMap::new();
        latest.insert("codex".to_string(), "0.154.0".to_string());
        latest.insert("claude".to_string(), "2.1.266".to_string());
        let ids = |items: Vec<MenuItem>| items.into_iter().map(|i| i.id).collect::<Vec<_>>();
        assert_eq!(
            ids(menu(Page::Machines, false, state.machines.first(), &latest)),
            [
                "edit",
                "update_codex",
                "update_open_code",
                "update_machine",
                "delete"
            ]
        );
        assert_eq!(
            ids(menu(Page::Machines, false, state.machines.get(2), &latest)),
            [
                "edit",
                "update_codex",
                "update_claude",
                "update_open_code",
                "update_machine",
                "delete"
            ]
        );
        assert_eq!(
            ids(menu(Page::Machines, false, None, &latest)),
            ["edit", "update_machine", "delete"]
        );
        let sessions = menu(Page::Sessions, true, None, &latest);
        assert_eq!(sessions.len(), 9);
        assert_eq!(
            sessions[0],
            MenuItem {
                id: "view_read_only".into(),
                label: "Attach read-only".into(),
                icon: "eye".into()
            }
        );
        assert_eq!(ids(menu(Page::Accounts, false, None, &latest)), ["edit"]);
        assert_eq!(
            ids(menu(Page::Profiles, false, None, &latest)),
            ["edit", "delete"]
        );
    }

    #[test]
    fn header_actions_and_empty_states() {
        assert_eq!(primary(Page::Machines), ("add_machine", "Add machine"));
        assert_eq!(primary(Page::Sessions), ("new", "Launch session"));
        assert_eq!(primary(Page::Accounts), ("add_account", "Add account"));
        assert_eq!(primary(Page::Profiles), ("add_profile", "Add profile"));
        assert_eq!(
            empty_text(Page::Sessions, false, true, false, false),
            "Refreshing Ocho…"
        );
        assert_eq!(
            empty_text(Page::Profiles, true, false, false, false),
            "No launch profiles yet. Add one with the button above."
        );
        assert_eq!(
            empty_text(Page::Accounts, true, false, false, false),
            "No accounts connected. Add one above, choose Claude, Codex or OpenCode, and sign in."
        );
        assert_eq!(
            empty_text(Page::Machines, true, false, false, false),
            "No machines enrolled. Add one with the button above."
        );
        assert_eq!(
            empty_text(Page::Sessions, true, false, true, true),
            "No sessions match your search."
        );
        assert_eq!(
            empty_text(Page::Sessions, true, false, false, true),
            "No running sessions here. Turn off Hide non-running to show other sessions."
        );
        assert_eq!(
            empty_text(Page::Sessions, true, false, false, false),
            "No sessions here. Launch one, or show untracked and archived sessions from View."
        );
    }

    #[test]
    fn rows_serialize_as_the_row_shape() {
        let state = fleet();
        let latest = HashMap::new();
        let usage = HashMap::new();
        let t = Theme::ocho_dark();
        let rows = rows(&ctx(Page::Sessions, &state, &latest, &usage, &t));
        let json = serde_json::to_value(&rows[1]).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        let expected = match &crate::shapes::ROW {
            crate::shapes::Shape::Record(fields) => {
                fields.iter().map(|(name, _)| *name).collect::<Vec<_>>()
            }
            _ => unreachable!(),
        };
        let mut sorted = expected.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted);
        assert_eq!(json["markdownLine"], -1);
        assert_eq!(json["workingHead"], "Thinking…");
        assert_eq!(json["actions"][0]["id"], "open");
        // The shape reads it without a missing field.
        let _ = crate::shapes::read(&crate::shapes::ROW, &json);
    }
}
