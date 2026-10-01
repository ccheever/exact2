//! The view-model: what the contract paints, built from the model on every
//! ask. Pure; the strings and rules here are ui.rs's.

use crate::indicator;
use crate::keymap;
use crate::model::{exec_confirm_texts, Overlay, Page, Region, Workspace};
use crate::palette::Command;
use crate::rows::{self, Rows};
use crate::session::{self, session_state};
use crate::shapes;
use crate::theme::Theme;
use exact_plan::Value;
use serde_json::{json, Value as Json};

/// The whole `View` shape.
pub fn render(ws: &Workspace, _versions: &[f64]) -> Value {
    let theme = &ws.theme;
    let on_tab = ws.tabs.active > 0;
    let title = match ws.tabs.active_tab() {
        Some(tab) => format!("{} — Ocho", tab.title),
        None => "Ocho".to_string(),
    };
    let pairing = pairing_view(ws);
    let json = json!({
        "theme": theme.view(),
        "windowTitle": title,
        "loaded": ws.loaded,
        "rail": rail(ws, theme),
        "manager": manager(ws, theme),
        "onTab": on_tab,
        "tab": tab_view(ws, theme),
        "overlay": overlay(ws, theme),
        "popup": ws.popup_view(),
        "toast": toast(ws),
        "focusId": ws.focus_id,
        "scrollTo": ws.scroll_to,
        "uiFontSize": ws.settings.ui_font_size.unwrap_or(13.0),
        "termFontSize": ws.settings.terminal_font_size.unwrap_or(13.0),
        "whatsNew": ws.whats_new.view(),
        "pairing": pairing,
        "secrets": ws.secrets_view(),
        "region": match ws.region() {
            Region::Rail => "rail",
            Region::Manager => "manager",
            Region::Terminal => "terminal",
            Region::Panel => "panel",
            Region::Transcript => "transcript",
        },
    });
    shapes::read(&shapes::VIEW, &json)
}

fn toast(ws: &Workspace) -> Json {
    // ui.rs:1932: the message, else the Accounts page's account error.
    if !ws.toast.text.is_empty() {
        return json!({ "text": session::clean(&ws.toast.text), "error": ws.toast.error, "dismissible": true });
    }
    if ws.page == Page::Accounts && ws.tabs.active == 0 && !ws.account_error.is_empty() {
        return json!({ "text": ws.account_error, "error": true, "dismissible": false });
    }
    json!({ "text": "", "error": false, "dismissible": false })
}

fn rail(ws: &Workspace, theme: &Theme) -> Json {
    let pages: Vec<Json> = Page::ALL
        .iter()
        .map(|page| {
            json!({
                "id": page.id(),
                "title": page.title(),
                "icon": page.icon(),
                "count": page_count(ws, *page),
                "selected": ws.tabs.active == 0 && ws.page == *page,
            })
        })
        .collect();
    let tabs: Vec<Json> = ws
        .tabs
        .visible()
        .into_iter()
        .map(|i| rail_tab(ws, theme, i))
        .collect();
    json!({
        "width": ws.rail_width,
        "nav": ws.nav,
        "pages": pages,
        "tabs": tabs,
        "hint": "Enter attaches a session · c opens a shell",
        "updateLabel": "",
        "updateTip": "",
        "selectedTab": ws.tabs.active_tab().map(|t| t.key.clone()).unwrap_or_default(),
        "resizing": ws.rail_resizing,
    })
}

fn rail_tab(ws: &Workspace, theme: &Theme, i: usize) -> Json {
    let tab = &ws.tabs.tabs[i];
    let active = ws.tabs.active == i + 1;
    let (title, machine) = match tab.title.rsplit_once(" · ") {
        Some((t, m)) => (t.to_string(), m.to_string()),
        None => (tab.title.clone(), String::new()),
    };
    let session = tab.session.as_ref().and_then(|(m, s, _)| {
        let machine = ws.state.machines.iter().find(|x| &x.id == m)?;
        let session = machine
            .last
            .as_ref()?
            .sessions
            .iter()
            .find(|x| &x.id == s)?;
        Some((machine, session))
    });
    let dead = ws.exited.contains(&tab.key);
    let available =
        session.is_some_and(|(m, s)| session::session_observation_available(m, s, ws.now));
    let working = session
        .filter(|(_, s)| available && session_state(s) == "RUNNING" && !s.status_text.is_empty());
    let (detail, detail_is_message) = match session {
        Some((_, s)) if available => (
            session::session_summary(s),
            !s.last_message.is_empty()
                && !matches!(session_state(s), "RUNNING" | "BLOCKED" | "UNAVAILABLE"),
        ),
        Some((_, s)) => (session::stale_session_summary(s), false),
        None if tab.session.is_some() => ("Waiting for session observation".to_string(), false),
        None => (String::new(), false),
    };
    // The machine badge (ui.rs:95-140): healthy, waiting, lost, or ended.
    let local = tab
        .session
        .as_ref()
        .map(|(id, _, _)| id.clone())
        .or(tab.machine.clone())
        .and_then(|id| session::machine(&ws.state, &id).map(|m| m.local))
        .unwrap_or(false);
    let (tint, faded) = if dead && tab.session.is_some() {
        (theme.good, true)
    } else if machine.is_empty() || local {
        (theme.muted, false)
    } else {
        (theme.good, false)
    };
    let badge_bg = tint.alpha(0.18).css();
    let tint = tint.css();
    let badge = if !machine.is_empty() || dead {
        if local {
            "L".to_string()
        } else {
            machine
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default()
        }
    } else {
        String::new()
    };
    let ind =
        working.map(|(_, s)| indicator::indicator(&s.provider, &s.status_text, ws.now, theme));
    let drop = ws.tab_drop.filter(|d| d.target == i);
    let (before, after, into) = match drop.map(|d| d.zone) {
        Some(crate::tab_tree::DropZone::Before) => (true, false, false),
        Some(crate::tab_tree::DropZone::After) => (false, true, false),
        Some(crate::tab_tree::DropZone::Into) => (false, false, true),
        None => (false, false, false),
    };
    json!({
        "key": tab.key,
        // The key without ':' (a driver reads `id:selector` in a target).
        "slug": tab.key.replace(':', "-"),
        "title": title,
        "machine": machine,
        "badge": badge,
        "badgeTint": tint,
        "badgeBg": badge_bg,
        "badgeFaded": faded,
        "depth": tab.depth,
        "folder": tab.folder,
        "collapsed": tab.collapsed,
        "hasChildren": ws.tabs.has_children(i),
        "active": active,
        "ancestor": ws.tabs.is_ancestor_of_active(i),
        "status": if detail_is_message { crate::markdown::inline_text(&detail) } else { detail },
        "statusMarkdown": detail_is_message,
        "working": ind.is_some(),
        "workingGlyph": ind.as_ref().map(|x| x.glyph.clone()).unwrap_or_default(),
        "workingHead": ind.as_ref().map(|x| x.head.clone()).unwrap_or_default(),
        "workingHeadColor": ind.as_ref().map(|x| x.head_color.clone()).unwrap_or_default(),
        "workingRest": ind.as_ref().map(|x| x.rest.clone()).unwrap_or_default(),
        "imessage": session.is_some_and(|(_, s)| s.imessage_attached),
        "number": ws.tabs.tab_number(i).to_string(),
        "dropBefore": before,
        "dropAfter": after,
        "dropInto": into,
    })
}

fn page_count(ws: &Workspace, page: Page) -> String {
    let n = match page {
        Page::Machines => session::machine_rows(&ws.state).count(),
        Page::Sessions => ws.sessions().len(),
        Page::Accounts => ws.state.accounts.len(),
    };
    n.to_string()
}

fn manager(ws: &Workspace, theme: &Theme) -> Json {
    let usage: std::collections::HashMap<String, rows::UsageLoad> = ws
        .usage
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                rows::UsageLoad {
                    at_ms: v.at,
                    usage: v.usage.clone(),
                    error: v.error,
                },
            )
        })
        .collect();
    let ctx = Rows {
        page: ws.page,
        state: &ws.state,
        latest_provider_versions: &ws.latest_provider_versions,
        usage: &usage,
        query: &ws.query,
        history: ws.history,
        all_sessions: ws.all_sessions,
        hide_non_running: ws.hide_non_running,
        selected: ws.index,
        now_ms: ws.now,
        now_epoch_s: ws.epoch_s(),
        theme,
    };
    let rows = rows::rows(&ctx);
    let empty = if rows.is_empty() {
        rows::empty_text(
            ws.page,
            ws.loaded,
            ws.loading,
            !ws.query.is_empty(),
            ws.hide_non_running,
        )
    } else {
        String::new()
    };
    let mut active: Vec<&str> = Vec::new();
    if ws.all_sessions {
        active.push("untracked & archived");
    }
    if ws.history {
        active.push("history");
    }
    if ws.hide_non_running {
        active.push("non-running hidden");
    }
    let (primary_id, primary) = rows::primary(ws.page);
    let primary_hint = Command::from_id(&primary_id.replace('_', "-"))
        .map(|c| ws.hint(c))
        .unwrap_or_default();
    let rows: Vec<Json> = rows
        .into_iter()
        .map(|mut row| {
            if row.markdown_line >= 0 {
                if let Some(line) = row.lines.get_mut(row.markdown_line as usize) {
                    *line = crate::markdown::inline_text(line);
                }
            }
            for a in &mut row.actions {
                if let Some(cmd) = crate::model::row_command(&a.id) {
                    a.hint = ws.hint(cmd);
                    a.id = cmd.id();
                }
            }
            serde_json::to_value(row).unwrap_or(Json::Null)
        })
        .collect();
    json!({
        "page": ws.page.id(),
        "title": ws.page.title(),
        "filterSummary": active.join(" · "),
        "query": ws.query,
        "searching": ws.searching,
        "primary": primary,
        "primaryHint": primary_hint,
        "rows": rows,
        "empty": empty,
        "selected": ws.index,
        "focused": ws.region() == Region::Manager,
        "viewOpen": ws.popup.is_some_and(|p| p.kind == crate::picker::PopupKind::ViewMenu && p.at.is_none()),
    })
}

fn tab_view(ws: &Workspace, _theme: &Theme) -> Json {
    let Some(tab) = ws.tabs.active_tab() else {
        return Json::Null;
    };
    let exited = ws.exited.contains(&tab.key);
    json!({
        "key": tab.key,
        "title": tab.title,
        "argv": tab.reconnect,
        "argvJson": serde_json::to_string(&tab.reconnect).unwrap_or_else(|_| "[]".into()),
        "machine": tab.machine.clone().unwrap_or_default(),
        "cwd": "",
        "readOnly": tab.session.as_ref().is_some_and(|s| s.2),
        "strip": "",
        "stripLabel": "",
        "stripDetail": "",
        "stripColor": "",
        "stripRetry": false,
        "exitBanner": if exited { "Connection ended." } else { "" },
        "limited": false,
        "limitedStatus": "",
        "limitedProvider": "",
        "overlay": "",
        "overlayTitle": "",
        "overlayDetail": "",
        "overlayButtons": [],
        "overlayNote": "",
        "panelOpen": false,
        "panelArgv": [],
        "panelCwd": "",
        "panelHeight": 260,
        "panelFocused": false,
        "transcript": ws.transcript_visible(),
        "conversation": ws.transcript_view(),
        "dropTarget": false,
    })
}

/// The message dialog with no observed reply (ui.rs).
const NO_MESSAGE: &str = "No assistant message observed yet. New Ocho launches install status hooks. Existing sessions can use fleet hooks codex|claude|opencode on their machine. Ocho pre-trusts its Codex hooks; native hook reload rules still apply.";

fn card(kind: &str, width: f64, top: bool) -> Json {
    json!({
        "kind": kind, "width": width, "top": top, "title": "", "subtitle": "", "pill": "", "pillColor": "",
        "glyph": "", "placeholder": "", "query": "", "status": "", "statusError": false, "rows": [],
        "index": 0, "footer": "", "body": "", "bodyMarkdown": false, "blocks": [], "fields": [], "buttons": [], "hint": "", "focusId": "",
    })
}

fn merge(mut base: Json, extra: Json) -> Json {
    if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

fn confirm(title: String, body: String, yes: &str) -> Json {
    merge(
        card("confirm", 520.0, false),
        json!({
            "title": title,
            "body": body,
            "buttons": [
                { "id": "confirm-no", "label": "Cancel", "hint": "n", "primary": false, "disabled": false },
                { "id": "confirm-yes", "label": yes, "hint": "y", "primary": true, "disabled": false },
            ],
        }),
    )
}

fn overlay(ws: &Workspace, theme: &Theme) -> Json {
    if let Some(v) = ws.dialog_view() {
        let kind = v
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("form")
            .to_string();
        let width = v.get("width").and_then(|w| w.as_f64()).unwrap_or(680.0);
        let top = kind == "picker";
        return merge(card(&kind, width, top), v);
    }
    match &ws.overlay {
        Overlay::None => Json::Null,
        Overlay::Help => {
            let mut rows = Vec::new();
            for (section, keys) in keymap::help_sections() {
                rows.push(json!({ "id": format!("section-{section}"), "chip": "section", "label": section, "detail": "", "hint": "", "second": "", "selected": false, "swatch": "", "swatchBorder": "", "accent": true }));
                for (n, (key, description)) in keys.into_iter().enumerate() {
                    rows.push(json!({ "id": format!("{section}-{n}"), "chip": "", "label": key, "detail": description, "hint": "", "second": "", "selected": false, "swatch": "", "swatchBorder": "", "accent": false }));
                }
            }
            merge(
                card("help", 760.0, false),
                json!({ "title": "Ocho keys", "hint": "Esc · q · ? close", "rows": rows }),
            )
        }
        Overlay::Message(item) => {
            // ui.rs `Overlay::Message`: the last agent message as Markdown,
            // the hook warning under it.
            let s = &item.session;
            let mut message = if s.last_message.is_empty() {
                NO_MESSAGE.to_string()
            } else {
                session::clean(&s.last_message)
            };
            if !s.hook_warning.is_empty() {
                message.push_str("\n\n");
                message.push_str(&s.hook_warning);
            }
            let blocks = crate::markdown_doc::to_json(&crate::markdown_doc::parse(&message), false);
            merge(
                card("message", 720.0, false),
                json!({
                    "title": session::clean(&s.title),
                    "subtitle": format!("{} · source: {} · {}", s.state, s.status_source, item.machine.name),
                    "body": message,
                    "bodyMarkdown": true,
                    "blocks": blocks,
                    "buttons": [{ "id": "message-close", "label": "Close", "hint": "Esc", "primary": true }],
                }),
            )
        }
        Overlay::Confirm { action, item } => {
            let (title, yes) = exec_confirm_texts(*action, &session::clean(&item.session.title));
            confirm(
                title,
                format!("{} on {}.", item.session.cwd, item.machine.name),
                yes,
            )
        }
        Overlay::MachineDelete(machine) => confirm(
            format!("Remove {} from Ocho?", machine.name),
            "Its sessions keep running on the machine; Ocho stops listing them.".into(),
            "Remove from Ocho",
        ),
        Overlay::ProviderUpdate { machine, provider } => confirm(
            format!(
                "Install/Update {} on {}?",
                session::provider_label(provider),
                machine.name
            ),
            "Runs the provider's installer on that machine.".into(),
            "Update",
        ),
        Overlay::ProfileDelete(name) => confirm(
            format!("Remove launch profile {name}?"),
            "The profile and its shortcut go; sessions it launched stay.".into(),
            "Remove",
        ),
        Overlay::Palette(picker) => {
            let items = ws.palette_items();
            let scopes = ws.key_scopes();
            let rows: Vec<Json> = items
                .iter()
                .enumerate()
                .map(|(i, (info, _))| {
                    json!({
                        "id": i.to_string(), "chip": "", "label": info.label,
                        "detail": "", "hint": keymap::hints(info.command, &scopes), "second": "",
                        "selected": i == picker.index, "swatch": "", "swatchBorder": "", "accent": false,
                    })
                })
                .collect();
            let status = if rows.is_empty() {
                "No matching commands"
            } else {
                ""
            };
            merge(
                card("picker", 600.0, true),
                json!({ "glyph": "›", "placeholder": "Type a command…", "query": picker.query, "rows": rows, "index": picker.index, "status": status }),
            )
        }
        Overlay::Settings(page) => {
            let v = crate::preferences::view(&ws.settings, &theme.name, page);
            merge(card("settings", 680.0, false), v)
        }
        Overlay::Form(_)
        | Overlay::PullRequests(_)
        | Overlay::Conversations
        | Overlay::Themes(_) => Json::Null,
        // These two draw from their own shapes (`whatsNew`, `pairing`); the overlay names the card.
        Overlay::WhatsNew => card("whats-new", crate::whats_new::WIDTH, false),
        Overlay::PairIMessage => card("pair-imessage", crate::imessage_pair::WIDTH, false),
        Overlay::Launch(launch) => {
            let v = if launch.quick.is_some() {
                crate::quick::view(launch, &ws.state)
            } else {
                launch.view(&ws.state)
            };
            let kind = v
                .get("kind")
                .and_then(|k| k.as_str())
                .unwrap_or("launch")
                .to_string();
            let width = v.get("width").and_then(|w| w.as_f64()).unwrap_or(680.0);
            let mut v = merge(card(&kind, width, false), v);
            // The composer's controls carry their flags in `options` and the
            // provider dot as a theme name in `hint`; rows carry the dot in
            // `swatch`. The contract reads flags from `placeholder` (a list has
            // no membership test there) and paints resolved colors.
            let color = |name: &str| match name {
                "warn" => theme.warn.css(),
                "good" => theme.good.css(),
                "accent" => theme.accent.css(),
                "danger" => theme.danger.css(),
                "muted" => theme.muted.css(),
                other => other.to_string(),
            };
            if let Some(fields) = v.get_mut("fields").and_then(|f| f.as_array_mut()) {
                for f in fields.iter_mut() {
                    if f.get("kind").and_then(|k| k.as_str()) == Some("control") {
                        let flags: Vec<String> = f
                            .get("options")
                            .and_then(|o| o.as_array())
                            .map(|o| {
                                o.iter()
                                    .filter_map(|x| x.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        f["placeholder"] = Json::String(flags.join(" "));
                        if let Some(h) = f.get("hint").and_then(|h| h.as_str()).map(String::from) {
                            if !h.is_empty() {
                                f["hint"] = Json::String(color(&h));
                            }
                        }
                    }
                }
            }
            if let Some(rows) = v.get_mut("rows").and_then(|r| r.as_array_mut()) {
                for r in rows.iter_mut() {
                    if let Some(sw) = r.get("swatch").and_then(|h| h.as_str()).map(String::from) {
                        if !sw.is_empty() {
                            r["swatch"] = Json::String(color(&sw));
                        }
                    }
                }
            }
            // Pill colors come as theme names.
            if let Some(color) = v.get("pillColor").and_then(|c| c.as_str()) {
                let resolved = match color {
                    "warn" => theme.warn.css(),
                    "good" => theme.good.css(),
                    "accent" => theme.accent.css(),
                    other => other.to_string(),
                };
                v["pillColor"] = Json::String(resolved);
            }
            v
        }
    }
}

/// The pairing card's JSON, its QR as rows of dark/light cells (Contract has no string split).
fn pairing_view(ws: &Workspace) -> Json {
    // The QR as rows of dark/light cells (Contract has no string split).
    let mut v = ws.imessage.view();
    let rows: Vec<Json> = v
        .get("qr")
        .and_then(|q| q.as_array())
        .map(|q| {
            q.iter()
                .enumerate()
                .map(|(i, r)| {
                    let cells: Vec<Json> = r
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .enumerate()
                        .map(|(j, c)| json!({ "id": format!("{i}-{j}"), "dark": c == '1' }))
                        .collect();
                    json!({ "id": format!("qr-{i}"), "cells": cells })
                })
                .collect()
        })
        .unwrap_or_default();
    v["qr"] = Json::Array(rows);
    v
}
