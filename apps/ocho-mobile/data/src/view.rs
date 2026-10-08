//! The picture: what the contract paints, built from the model on every ask.

use crate::fleet::{split_title, SendRoute, Session, Window};
use crate::model::{Key, Model};
use crate::shapes;
use exact_plan::Value;
use ocho_data::markdown_doc;
use serde_json::{json, Value as Json};

/// The most entries drawn; a long conversation shows its end.
/// Entries drawn when a conversation opens, and how many more each "Show
/// earlier" adds: every entry is Markdown blocks the page lays out at once,
/// so 160 of them made opening a long thread a ~170 ms main-thread stall.
pub const PAGE_ENTRIES: usize = 40;

/// The whole `View`.
pub fn render(m: &Model) -> Value {
    let json = json!({
        "paired": m.conn.is_some(),
        "home": home(m),
        "session": session(m),
        "pair": { "draft": m.pair_draft, "error": m.pair_error },
        "compose": compose(m),
    });
    shapes::read(&shapes::VIEW, &json)
}

fn home(m: &Model) -> Json {
    let windows = &m.windows;
    let index = m.window.min(windows.len().saturating_sub(1));
    let labels: Vec<Json> = if windows.len() > 1 {
        windows
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let label = if windows.len() <= 3 {
                    format!("Window {}", i + 1)
                } else {
                    format!("{}", i + 1)
                };
                json!({ "id": format!("{i}"), "label": label, "selected": i == index })
            })
            .collect()
    } else {
        Vec::new()
    };
    let (rows, empty) = match windows.get(index) {
        Some(window) => (
            tab_rows(m, window),
            "No tabs open in this window.".to_string(),
        ),
        None => (
            session_rows(m),
            if m.fleet.is_some() {
                "No sessions yet."
            } else {
                ""
            }
            .to_string(),
        ),
    };
    let rows = close_sections(rows);
    let (subtitle, warn) = connection_line(m);
    // Why, when the Mac can't be reached and nothing else answers for it.
    let detail = if warn && !m.fresh {
        m.poll_error.clone()
    } else {
        String::new()
    };
    json!({
        "title": "Ocho",
        "subtitle": subtitle,
        "detail": detail,
        "warn": warn,
        "windows": labels,
        "rows": rows,
        "empty": empty,
        "refreshing": m.refreshing,
        "build": format!("Ocho {}", crate::telemetry::build()),
    })
}

/// Mark each row that ends its section (before a folder, a row that starts
/// a section of its own, or the end), so the list can round its corners.
fn close_sections(mut rows: Vec<Json>) -> Vec<Json> {
    for i in 0..rows.len() {
        let folder = |r: &Json| r.get("kind").and_then(Json::as_str) == Some("folder");
        let opens = |r: &Json| folder(r) || r.get("first").and_then(Json::as_bool) == Some(true);
        let last = !folder(&rows[i]) && rows.get(i + 1).is_none_or(opens);
        rows[i]["last"] = json!(last);
    }
    rows
}

/// The line under the title: who answers, and whether the Mac is away.
fn connection_line(m: &Model) -> (String, bool) {
    let home = m.home_name();
    if m.refused {
        return (format!("{home} refused this pairing — pair again"), true);
    }
    if m.fleet.is_none() {
        return if m.poll.failures > 0 {
            (format!("Can't reach {home} — retrying"), true)
        } else {
            (format!("Connecting to {home}…"), false)
        };
    }
    if !m.fresh {
        return (
            format!("Can't reach {home} — showing the last update"),
            true,
        );
    }
    if m.via != m.home() {
        return (
            format!("Can't reach {home} · {} is answering", m.name_of(&m.via)),
            true,
        );
    }
    // A healthy connection answers every few seconds: say so when it hasn't,
    // so a stall never hides behind the Mac's name.
    let quiet = (m.since_answer() / 1000.0).floor();
    if quiet >= 15.0 {
        let ago = if quiet < 120.0 {
            format!("{quiet}s")
        } else {
            format!("{}m", (quiet / 60.0).floor())
        };
        return (format!("{home} · updated {ago} ago"), true);
    }
    (home, false)
}

/// The window's tabs as sections: a folder is a heading over the tabs nested
/// under it, and a tab outside every folder is in a section with none. The
/// sidebar is a flat pre-order list: a tab is in the nearest folder before it
/// at a smaller depth. A server that publishes no nesting (no tab says
/// `folder`) leaves only the keys, so each folder runs to the next.
fn tab_rows(m: &Model, window: &Window) -> Vec<Json> {
    let nested = window.tabs.iter().any(|t| t.folder);
    let mut rows = Vec::new();
    let mut first = true;
    // The folders the current tab is inside, by depth.
    let mut folders: Vec<u32> = Vec::new();
    for (i, tab) in window.tabs.iter().enumerate() {
        let current = window.active == i + 1;
        if nested {
            let before = folders.len();
            while folders.last().is_some_and(|&d| d >= tab.depth) {
                folders.pop();
            }
            if folders.len() < before {
                // Out of a folder: what follows is a section of its own.
                first = true;
            }
        }
        // Indented under its innermost folder's heading, not its raw depth.
        let indent = |depth: u32| match folders.last() {
            Some(&f) => depth.saturating_sub(f + 1),
            None => depth,
        };
        if tab.is_folder() {
            rows.push(json!({
                "id": format!("t{i}"), "kind": "folder", "title": tab.title,
                "depth": indent(tab.depth), "current": current,
            }));
            folders.push(tab.depth);
            first = true;
            continue;
        }
        let depth = if nested { indent(tab.depth) } else { 0 };
        let (title, host) = split_title(&tab.title);
        if tab.machine.is_empty() || tab.session.is_empty() {
            rows.push(json!({
                "id": format!("t{i}"), "kind": "other", "title": title,
                "status": "Terminal — open it on the desktop", "offline": true,
                "current": current, "depth": depth, "first": first,
            }));
            first = false;
            continue;
        }
        let key: Key = (tab.machine.clone(), tab.session.clone());
        let host = if host.is_empty() {
            m.name_of(&tab.machine)
        } else {
            host
        };
        rows.push(session_row(
            m,
            format!("t{i}"),
            &key,
            title,
            host,
            (current, m.unread(&tab.key)),
            depth,
            first,
        ));
        first = false;
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn session_row(
    m: &Model,
    id: String,
    key: &Key,
    title: String,
    host: String,
    (current, unread): (bool, bool),
    depth: u32,
    first: bool,
) -> Json {
    let live = m.live_session(key);
    let known = live.or_else(|| m.known_session(key));
    json!({
        "id": id, "kind": "session", "machine": key.0, "session": key.1,
        "title": title, "host": host,
        "status": known.map(Session::status_line).unwrap_or_default(),
        "working": live.is_some_and(Session::working),
        "blocked": live.is_some_and(Session::blocked),
        "offline": live.is_none(),
        "current": current, "unread": unread, "depth": depth, "first": first,
    })
}

/// No desktop layout (a server with no desktop, never paired to one): every
/// live session, grouped by machine, the busiest first.
fn session_rows(m: &Model) -> Vec<Json> {
    let Some(fleet) = &m.fleet else {
        return Vec::new();
    };
    let mut machines: Vec<(&String, &crate::fleet::Machine)> = fleet.machines.iter().collect();
    machines.sort_by(|a, b| (a.0 != &m.via, &a.1.name).cmp(&(b.0 != &m.via, &b.1.name)));
    let rank = |s: &Session| match s.state.as_str() {
        "blocked" => 0,
        "running" => 1,
        "starting" => 2,
        "idle" => 3,
        _ => 4,
    };
    let mut rows = Vec::new();
    for (id, machine) in machines {
        let mut sessions: Vec<&Session> = machine
            .sessions
            .values()
            .filter(|s| !s.ended() && s.state != "unknown")
            .collect();
        if sessions.is_empty() {
            continue;
        }
        sessions.sort_by(|a, b| (rank(a), &a.title).cmp(&(rank(b), &b.title)));
        rows.push(json!({ "id": format!("m{id}"), "kind": "folder", "title": machine.name }));
        for (i, s) in sessions.iter().enumerate() {
            let key = (id.clone(), s.id.clone());
            let title = if s.title.is_empty() {
                s.provider.clone()
            } else {
                s.title.clone()
            };
            rows.push(session_row(
                m,
                format!("s{id}-{}", s.id),
                &key,
                title,
                machine.name.clone(),
                (false, false),
                0,
                i == 0,
            ));
        }
    }
    rows
}

pub(crate) fn session(m: &Model) -> Json {
    let voice = m.voice();
    let upload = m.upload();
    let Some(key) = &m.open else {
        return json!({ "open": false });
    };
    let live = m.live_session(key);
    let known = live.or_else(|| m.known_session(key));
    let (title, host) = tab_title(m, key).unwrap_or_else(|| {
        (
            known.map(|s| s.title.clone()).unwrap_or_default(),
            m.name_of(&key.0),
        )
    });
    let entries = m.entries(key);
    let transcript = m.conversations.get(key);
    let shown = transcript.map_or(PAGE_ENTRIES, |c| c.shown.max(PAGE_ENTRIES));
    let start = entries.len().saturating_sub(shown);
    let mut out: Vec<Json> = Vec::new();
    if let Some(conversation) = transcript {
        for (i, e) in entries.iter().enumerate().skip(start) {
            if e.is_injected_instructions() {
                continue;
            }
            let subdued = conversation.transcript.is_progress(i);
            let kind = match e.kind.as_str() {
                "tools" => "tools",
                "user" => "user",
                _ => "assistant",
            };
            out.push(json!({
                "id": format!("e{i}"),
                "kind": kind,
                "subdued": subdued,
                "queued": false,
                "blocks": if kind == "tools" { json!([]) } else { json!(markdown_doc::to_json(&e.blocks, subdued)) },
                "summary": if kind == "tools" { e.summary() } else { String::new() },
            }));
        }
    }
    // Fleet holds a queued message until the running turn ends: those wait in
    // the tray over the composer (`queue`), each with "Send now", as Codex's
    // own app shows them; the rest are bubbles at the transcript's end.
    let working = live.is_some_and(|s| s.working() && s.send_route() == SendRoute::Queue);
    let mut queue: Vec<Json> = Vec::new();
    for (i, p) in m.pending_for(key).enumerate() {
        if working && p.queued {
            queue.push(
                json!({ "id": p.request_id, "text": p.text, "interrupting": p.interrupting }),
            );
            continue;
        }
        out.push(json!({
            "id": format!("p{i}-{}", p.after),
            "kind": "user",
            "pending": true,
            "queued": false,
            "blocks": markdown_doc::to_json(&markdown_doc::parse(&p.text), false),
        }));
    }
    let loaded = transcript.is_some_and(|c| c.loaded);
    let error = transcript.map(|c| c.error.clone()).unwrap_or_default();
    let empty = if !out.is_empty() {
        ""
    } else if !loaded {
        "Loading the conversation…"
    } else if !error.is_empty() {
        "Couldn't load the conversation."
    } else {
        "No messages yet."
    };
    let (can_send, note) = match live.map(Session::send_route) {
        None if m.fresh => (false, format!("{host} is offline.")),
        None => (false, "Reconnecting…".to_string()),
        Some(SendRoute::None(why)) => (false, why.to_string()),
        Some(_) => (true, String::new()),
    };
    let failed = match &m.failed {
        Some((k, _, why)) if k == key => why.clone(),
        _ => String::new(),
    };
    json!({
        "open": true,
        "title": title,
        "host": host,
        "working": live.is_some_and(Session::working),
        "blocked": live.is_some_and(Session::blocked),
        "offline": live.is_none(),
        "status": live.filter(|s| s.working() || s.blocked()).map(Session::status_line).unwrap_or_default(),
        "entries": out,
        "earlier": if start > 0 { format!("Show {} earlier messages", start.min(PAGE_ENTRIES)) } else { String::new() },
        "empty": empty,
        "error": if out.is_empty() { String::new() } else { short(&error) },
        "canSend": can_send,
        "note": note,
        "failed": failed,
        "scrollRevision": m.scroll_revision,
        "composerHeight": m.composer_height.max(44.0),
        "queue": queue,
        "uploadUrl": upload.as_ref().map(|u| u.0.clone()).unwrap_or_default(),
        "auth": upload.map(|u| u.1).unwrap_or_default(),
        "canTalk": voice.is_some(),
        "voiceUrl": voice.as_ref().map(|v| v.0.clone()).unwrap_or_default(),
        "voiceAuth": voice.map(|v| v.1).unwrap_or_default(),
        "draft": m.open.as_ref().map(|k| m.draft_for(k)).unwrap_or_default(),
    })
}

/// An inline picker's width for its text: the symbol, the text at body size
/// (about 8.6 pt a character), ⇕; native views can't report their size.
fn picker_width(text: &str) -> f64 {
    (36.0 + text.chars().count() as f64 * 8.6 + 30.0).clamp(120.0, 330.0)
}

/// A native menu's items: `{id, title, selected}`.
fn menu(items: impl IntoIterator<Item = (String, String, bool)>) -> String {
    let items: Vec<Json> = items
        .into_iter()
        .map(|(id, title, selected)| json!({ "id": id, "title": title, "selected": selected }))
        .collect();
    Json::Array(items).to_string()
}

/// The accounts' menu: each with its provider's mark.
fn account_menu(items: Vec<(String, String, bool, String)>) -> String {
    use crate::model::launch::provider_symbol;
    let items: Vec<Json> = items
        .into_iter()
        .map(|(id, title, selected, provider)| {
            let (symbol, tint) = provider_symbol(&provider);
            json!({ "id": id, "title": title, "selected": selected, "symbol": symbol, "tint": tint })
        })
        .collect();
    Json::Array(items).to_string()
}

/// The new-session screen: where, as whom, which model, how hard.
fn compose(m: &Model) -> Json {
    use crate::model::launch::{effort_name, provider_name};
    let l = &m.launcher;
    let machines = m.launch_machines();
    let machine = machines
        .iter()
        .find(|(id, _)| *id == l.machine)
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| "Choose a machine".into());
    let accounts = m.launch_accounts();
    // Fleet names a signed-in account `<provider>-<hash>`: say the provider,
    // numbered when there are several of it.
    let account_title = |name: &str, provider: &str| {
        if let Some(a) = accounts
            .iter()
            .find(|a| a.name == name && a.provider == provider && !a.email.is_empty())
        {
            return a.email.clone();
        }
        let generated = name
            .strip_prefix(provider)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|rest| rest.len() >= 8 && rest.chars().all(|c| c.is_ascii_hexdigit()));
        if name.is_empty() || generated {
            let same: Vec<&str> = accounts
                .iter()
                .filter(|a| a.provider == provider)
                .map(|a| a.name.as_str())
                .collect();
            match same.iter().position(|n| *n == name) {
                Some(i) if same.len() > 1 => format!("{} {}", provider_name(provider), i + 1),
                _ => provider_name(provider),
            }
        } else {
            format!("{name} ({})", provider_name(provider))
        }
    };
    let mut models: Vec<(String, String, bool)> =
        vec![(String::new(), "Default model".into(), l.model.is_empty())];
    models.extend(
        l.models
            .iter()
            .map(|c| (c.id.clone(), c.name.clone(), c.id == l.model)),
    );
    let account = account_title(&l.account, &l.provider);
    let upload = m.launch_upload();
    json!({
        "machine": machine,
        "machineMenu": menu(machines.iter().map(|(id, name)| (id.clone(), name.clone(), *id == l.machine))),
        "account": account,
        "accountSymbol": crate::model::launch::provider_symbol(&l.provider).0,
        "accountTint": crate::model::launch::provider_symbol(&l.provider).1,
        "accountMenu": account_menu(accounts.iter().map(|a| (
            format!("{}/{}", a.provider, a.name),
            account_title(&a.name, &a.provider),
            a.name == l.account && a.provider == l.provider,
            a.provider.clone(),
        )).collect()),
        "model": m.launch_model_name(),
        "modelMenu": menu(models),
        "effort": effort_name(&l.effort),
        "effortMenu": menu(m.launch_efforts().iter().map(|e| (e.to_string(), effort_name(e).to_string(), *e == l.effort))),
        "prompt": format!("Ask {}", provider_name(&l.provider)),
        "launching": l.launching,
        "error": l.error,
        "canSend": !l.launching && !l.machine.is_empty() && m.conn.is_some(),
        "composerHeight": m.composer_height.max(44.0),
        "uploadUrl": upload.as_ref().map(|u| u.0.clone()).unwrap_or_default(),
        "auth": upload.map(|u| u.1).unwrap_or_default(),
        "machineWidth": picker_width(&machine),
        "accountWidth": picker_width(&account),
    })
}

/// The desktop's title for a session, from any window.
fn tab_title(m: &Model, key: &Key) -> Option<(String, String)> {
    let tab = m
        .windows
        .iter()
        .flat_map(|w| &w.tabs)
        .find(|t| t.machine == key.0 && t.session == key.1)?;
    let (title, host) = split_title(&tab.title);
    Some((
        title,
        if host.is_empty() {
            m.name_of(&key.0)
        } else {
            host
        },
    ))
}

fn short(error: &str) -> String {
    if error.is_empty() {
        return String::new();
    }
    let line = error.lines().next().unwrap_or("");
    format!(
        "Couldn't refresh: {}",
        line.chars().take(120).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model(windows: serde_json::Value) -> Model {
        let mut m = Model::default();
        m.load(
            Some(r#"{"relay":"https://fleet-relay.fly.dev","machine":"mac","token":"t","name":"Mac"}"#),
            None,
            None,
        );
        m.poll_request();
        m.poll_done(Ok(json!({
            "instance": "i", "version": 1,
            "fleet": {"machines": [
                {"id": "mac", "name": "Mac", "last": {"sessions": [
                    {"id": "s1", "state": "running", "status_text": "Reading", "tmux_pane": "%1", "capabilities": ["attach"]}
                ]}},
                {"id": "sprite", "name": "Sprite", "error": "Sprite is sleeping", "last": {"sessions": [
                    {"id": "s2", "state": "idle", "last_message": "Done"}
                ]}}
            ]},
            "desktop": {"windows": windows}
        })));
        m
    }

    fn tab(m: &str, s: &str, title: &str) -> serde_json::Value {
        json!({"key": format!("{m}:{s}:false"), "title": title, "machine": m, "session": s})
    }

    #[test]
    fn windows_become_segments_and_rows_mirror_the_tabs() {
        let mut m = model(json!([
            {"tabs": [{"key": "folder:a", "title": "work"}, tab("mac", "s1", "One · Mac"), tab("sprite", "s2", "Two · Sprite")], "active": 3},
            {"tabs": [tab("mac", "s1", "One · Mac")], "active": 1}
        ]));
        let h = home(&m);
        assert_eq!(h["windows"].as_array().unwrap().len(), 2);
        assert_eq!(h["windows"][0]["label"], "Window 1");
        assert_eq!(h["windows"][0]["selected"], true);
        let rows = h["rows"].as_array().unwrap();
        assert_eq!(rows[0]["kind"], "folder");
        assert_eq!(
            (rows[1]["title"].as_str(), rows[1]["host"].as_str()),
            (Some("One"), Some("Mac"))
        );
        assert_eq!(
            (rows[1]["working"].as_bool(), rows[1]["offline"].as_bool()),
            (Some(true), Some(false))
        );
        // A sleeping machine's session is greyed but keeps its last line.
        assert_eq!(
            (rows[2]["offline"].as_bool(), rows[2]["status"].as_str()),
            (Some(true), Some("Done"))
        );
        assert_eq!(
            (rows[2]["current"].as_bool(), rows[2]["last"].as_bool()),
            (Some(true), Some(true))
        );
        m.pick_window(1);
        let h = home(&m);
        assert_eq!(h["windows"][1]["selected"], true);
        assert_eq!(h["rows"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn tabs_outside_every_folder_are_their_own_sections() {
        let folder = |k: &str, title: &str, depth: u32| json!({"key": k, "title": title, "depth": depth, "folder": true});
        let at = |s: &str, depth: u32| {
            let mut t = tab("mac", s, &format!("{s} · Mac"));
            t["depth"] = json!(depth);
            t
        };
        let m = model(json!([{"tabs": [
            at("s1", 0),
            folder("folder:a", "work", 0), at("a1", 1), folder("folder:b", "inner", 1), at("b1", 2), at("a2", 1),
            at("loose", 0),
            folder("folder:c", "courses", 0), at("c1", 1)
        ], "active": 0}]));
        let rows = home(&m)["rows"].as_array().unwrap().clone();
        let row = |title: &str| rows.iter().find(|r| r["title"] == title).unwrap().clone();
        let flags = |title: &str| {
            let r = row(title);
            (
                r["depth"].as_u64().unwrap(),
                r["first"].as_bool().unwrap_or(false),
                r["last"].as_bool().unwrap_or(false),
            )
        };
        assert_eq!(flags("s1"), (0, true, true), "before every folder, alone");
        assert_eq!(
            flags("a1"),
            (0, true, true),
            "alone under `work` before `inner`"
        );
        assert_eq!(
            row("inner")["depth"],
            0,
            "a folder in `work` heads its own tabs"
        );
        assert_eq!(flags("b1"), (0, true, true));
        assert_eq!(flags("a2"), (0, true, true), "back in `work` after `inner`");
        assert_eq!(
            flags("loose"),
            (0, true, true),
            "outside every folder: not in `work`"
        );
        assert_eq!(flags("c1"), (0, true, true));
    }

    #[test]
    fn an_offline_session_says_why_it_cannot_send() {
        let mut m = model(json!([{"tabs": [tab("sprite", "s2", "Two · Sprite")], "active": 1}]));
        m.open("sprite", "s2");
        let s = session(&m);
        assert_eq!(
            (s["offline"].as_bool(), s["canSend"].as_bool()),
            (Some(true), Some(false))
        );
        assert_eq!(s["note"], "Sprite is offline.");
        m.open("mac", "s1");
        let s = session(&m);
        assert_eq!(
            (s["canSend"].as_bool(), s["status"].as_str()),
            (Some(true), Some("Reading"))
        );
    }

    #[test]
    fn without_a_desktop_every_live_session_is_listed_by_machine() {
        let m = model(json!([]));
        let rows = home(&m)["rows"].as_array().unwrap().clone();
        assert_eq!(rows[0]["title"], "Mac");
        assert_eq!(rows[1]["session"], "s1");
    }
}
