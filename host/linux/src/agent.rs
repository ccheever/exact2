//! The agent API's presenter half (LLP 1012), over stdio: the same
//! protocol as the macOS presenter's `Agent.swift`, so `scripts/agent.mjs`
//! drives this host with the carrier it already has. Under `EXACT_AGENT=1`
//! the driver owns the process: requests arrive as JSON lines on stdin,
//! replies leave as JSON lines on stdout, and the clock is the last `clock`
//! value — no timer advances the runner, the engine is seeked to it.
//! `tree`, `state`, `logs`, and `settle` go to the host; `layout`, `tap`,
//! `type`, `clock`, and `screenshot` are the presenter's.
//!
//! @ref LLP 1015 §5; LLP 1012 §3–§4

pub(crate) mod contact;

use crate::presenter::Presenter;
use exact_runner::agent::{error, field_bool, field_num, field_str, num};
use exact_runner::DataSource;
use std::io::{BufRead, Write};

/// Serve requests until stdin closes or `quit` arrives. The first line out
/// is `{"ready":true,"boot":ms,"views":n,"error":null|"…"}`.
pub fn serve<D: DataSource + Default>(
    p: &mut Presenter<D>,
    boot_ms: f64,
    boot_error: Option<&str>,
) -> i32 {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut ready = format!(
        "{{\"ready\":true,\"boot\":{},\"views\":{},\"error\":",
        num(boot_ms),
        p.node_count()
    );
    match boot_error {
        Some(e) => exact_runner::agent::quote(e, &mut ready),
        None => ready.push_str("null"),
    }
    ready.push('}');
    let _ = writeln!(out, "{ready}");
    let _ = out.flush();
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if field_str(&line, "op").as_deref() == Some("quit") {
            break;
        }
        let reply = handle(p, &line);
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
    let _ = p.pointer_cancel(p.host().now());
    0
}

/// Answer one request.
pub fn handle<D: DataSource + Default>(p: &mut Presenter<D>, line: &str) -> String {
    p.agent = true;
    p.poll_images();
    // A reply that landed since the last operation commits before this one
    // (the other hosts apply it as it lands; here nothing runs between) —
    // a finished update check likewise, and the commands the last
    // operation's commits asked for run before this one is answered.
    if let Some(e) = p.pump(p.host().now()) {
        eprintln!("exact: {e}");
    }
    p.poll_update();
    p.poll_development(D::default);
    p.run_commands(D::default);
    p.sync_surfaces();
    let reply = answer(p, line);
    p.sync_surfaces();
    let reply = p.merge_surfaces(line, reply);
    p.sync_surfaces();
    let reply = p.surfaces.error.take().map_or(reply, |e| error(&e));
    p.run_commands(D::default);
    // In the headless carrier a completed paint is presentation. A command
    // may activate a generation after the initial boot's frame was counted.
    if p.dirty() {
        let _ = p.frame();
    }
    p.first_pixel();
    tagged(p, line, reply)
}

/// Every reply carries the runner's `epoch`, `incarnation` and `clock`
/// (LLP 1035.002 D3), read after the operation; a reply that already has a
/// `clock` (where a `clock` call landed) keeps it. The runner's own replies
/// (`tree`, `state`, `node`) are tagged at the source; an error is left
/// alone.
fn tagged<D: DataSource>(p: &Presenter<D>, line: &str, mut reply: String) -> String {
    let op = field_str(line, "op");
    let host_reply = op.is_some();
    if !host_reply || !reply.ends_with('}') || reply.starts_with("{\"error\"") {
        return reply;
    }
    if field_num(&reply, "epoch").is_some() {
        return reply;
    }
    let tags = p.host().agent("{\"op\":\"tags\"}");
    let (Some(epoch), Some(incarnation), Some(clock)) = (
        field_num(&tags, "epoch"),
        field_num(&tags, "incarnation"),
        field_num(&tags, "clock"),
    ) else {
        return reply;
    };
    reply.pop();
    reply.push_str(&format!(
        ",\"epoch\":{},\"incarnation\":{}",
        num(epoch),
        num(incarnation)
    ));
    if !reply.contains("\"clock\":") {
        reply.push_str(&format!(",\"clock\":{}", num(clock)));
    }
    reply.push('}');
    reply
}

pub(crate) fn answer<D: DataSource>(p: &mut Presenter<D>, line: &str) -> String {
    // An agent's taps and contacts reach a canvas as a finger, as on the web
    // and iOS. An explicit contextmenu is a mouse's secondary button (LLP 1015.000).
    p.agent_finger(!field_bool(line, "contextmenu"));
    let reply = answer_line(p, line);
    p.agent_finger(false);
    reply
}
fn answer_line<D: DataSource>(p: &mut Presenter<D>, line: &str) -> String {
    let id = || field_num(line, "id").map(|n| n as u32);
    let q: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
    if let Some(view) = id() {
        if q["entity"].is_string()
            || field_bool(line, "world")
            || (q["op"] == "state"
                && p.host()
                    .kernel()
                    .node(view)
                    .is_some_and(|n| n.node_type == exact_kernel::NodeType::Canvas))
        {
            return p.surface_request(view, q).to_string();
        }
    }
    // `tap @t` / `type @t` answer a held device request before any view is
    // looked up (LLP 1069.007 D4).
    if let Some(reply) = p.host_mut().answer_hold(line) {
        // A picker's answer is delivered here (LLP 1069.002 D9).
        p.answer_picker(line, &reply);
        p.answer_save(line, &reply);
        p.answer_document(line, &reply);
        return reply;
    }
    match field_str(line, "op").as_deref() {
        Some("tree") => tree(p, line),
        // The agent's clock presents no frame (LLP 1079 D4); the display
        // loop's are sampled there (frames.rs). `perf <target>` is the runner's.
        Some("perf") if field_bool(line, "frames") => r#"{"virtual":true}"#.to_string(),
        Some("state") => {
            p.boxes();
            // The runner's state, then the sections a painter cannot observe
            // (LLP 1035.002 D2): present as `unavailable`, never absent, so a
            // reader can tell "no keyboard" from "no report".
            let mut s = p.host().agent(line);
            if s.ends_with('}') && !s.starts_with("{\"error\"") {
                s.pop();
                let presence: Vec<_> = p
                    .boxes()
                    .to_vec()
                    .iter()
                    .filter_map(|b| {
                        let node = p.host().kernel().node(b.id)?;
                        if node.style.layout_transition.0.is_empty()
                            && node.style.exit_animation.0.is_empty()
                        {
                            return None;
                        }
                        let shown = p.host().presented(b.id);
                        let (x, y, w, h) = b.surface(shown)?;
                        Some(
                            serde_json::json!({"id": b.id, "x": x, "y": y, "w": w, "h": h,
                        "opacity": shown.opacity, "exiting": false}),
                        )
                    })
                    .collect();
                s.push_str(&format!(",\"presence\":{}", serde_json::json!(presence)));
                s.push_str(&format!(",\"raster\":{}", p.images().diagnostics()));
                s.push_str(&format!(
                    ",\"focus\":{{\"logical\":{}}}",
                    p.focus().map_or("null".into(), |id| id.to_string())
                ));
                if let Some((paint, readback)) = p.last_frame_ms() {
                    s.push_str(&format!(
                        ",\"paint\":{{\"ms\":{paint},\"readbackMs\":{readback}}}"
                    ));
                }
                s.push_str(
                    ",\"keyboard\":{\"unavailable\":true},\"navigation\":{\"unavailable\":true}}",
                );
            }
            s
        }
        // LLP 1080.001: neither inspection form has platform views here.
        Some("layout") if line.contains("\"agree\"") => format!(
            "{{\"agreement\":{{\"unavailable\":\"no platform views\"}},{}",
            &exact_runner::agent::tags(p.host.runner())[1..]
        ),
        Some("layout") if line.contains("\"native\"") => {
            let mut reply: serde_json::Value =
                match serde_json::from_str(&p.layout_json(id(), field_bool(line, "plan"))) {
                    Ok(reply) => reply,
                    Err(_) => return error("layout: unreadable"),
                };
            if let Some(o) = reply.as_object_mut() {
                o.remove("nodes");
            }
            if let Some(native) = reply.pointer_mut("/node/native") {
                native["subviews"] = serde_json::json!({ "unavailable": "no platform views" });
            }
            reply.to_string()
        }
        Some("layout") => p.layout_json(id(), field_bool(line, "plan")),
        Some("tap") => {
            // LLP 1041 §8: optional input variant, never a ninth operation.
            // Parse this bounded pair strictly; the legacy wheel pair reader
            // intentionally accepts a smaller flat-request vocabulary.
            let request: serde_json::Value = match serde_json::from_str(line) {
                Ok(request) => request,
                Err(_) => return error("unreadable tap request"),
            };
            if request.get("resize").is_some() {
                return resize(p, &request);
            }
            if field_bool(line, "contextmenu") {
                return match id() {
                    Some(id) => p.contextmenu(id).unwrap_or_else(|e| error(&e)),
                    None => error("contextmenu needs an id"),
                };
            }
            if let Some(reply) = p.control_tap(&request) {
                return reply.to_string();
            }
            if request.get("phase").is_some() {
                return contact::answer(p, &request);
            }
            let Some(id) = id() else {
                return error("tap needs an id");
            };
            if let Some(into) = request.get("into") {
                let text = |name: &str, default: &str| {
                    into.get(name)
                        .and_then(|v| v.as_str())
                        .unwrap_or(default)
                        .to_string()
                };
                return match p.into_view(
                    id,
                    &text("key", ""),
                    &text("block", "start"),
                    &text("inline", "nearest"),
                ) {
                    Ok(reply) => reply,
                    Err(e) => error(&e),
                };
            }
            // A click with modifiers held (gallery F20: `tap <id> modifiers Shift`).
            let held = match field_str(line, "modifiers")
                .map(|m| exact_runner::KeyModifiers::held(&m))
            {
                Some(None) => {
                    return error("tap: modifiers are Shift, Control, Alt and Meta, joined by +")
                }
                Some(Some(held)) => held,
                None => Default::default(),
            };
            let codes = [
                (held.shift, "ShiftLeft"),
                (held.ctrl, "ControlLeft"),
                (held.alt, "AltLeft"),
                (held.meta, "MetaLeft"),
            ];
            for (on, code) in codes {
                if on {
                    p.hold_modifier(code, true);
                }
            }
            let r = match field_pair(line, "wheel") {
                Some((dx, dy)) => p.wheel(id, dx as f32, dy as f32),
                None if field_bool(line, "hover") => p.hover(id),
                None => p.tap(id),
            };
            for (on, code) in codes {
                if on {
                    p.hold_modifier(code, false);
                }
            }
            r.unwrap_or_else(|e| error(&e))
        }
        Some("type") => {
            let Some(id) = id() else {
                return error("type needs an id");
            };
            if let Some(edit) = field_str(line, "clipboard") {
                let text = field_str(line, "text").unwrap_or_default();
                return p.clipboard(id, &edit, &text).unwrap_or_else(|e| error(&e));
            }
            if let Some(chord) = field_str(line, "key") {
                // A chord's modifiers are held for its key (`Shift+Enter`,
                // `Meta+s`), as a keyboard's are, then released.
                let (held, key) = exact_runner::KeyModifiers::split(&chord);
                let modifiers = [
                    (held.shift, "ShiftLeft"),
                    (held.ctrl, "ControlLeft"),
                    (held.alt, "AltLeft"),
                    (held.meta, "MetaLeft"),
                ];
                for (on, code) in modifiers {
                    if on {
                        p.hold_modifier(code, true);
                    }
                }
                let phase = field_str(line, "phase");
                let r = p.type_key(id, key, key, phase.as_deref() != Some("up"), false);
                if phase.is_none() && r.is_ok() {
                    let _ = p.type_key(id, key, key, false, false);
                }
                for (on, code) in modifiers {
                    if on {
                        p.hold_modifier(code, false);
                    }
                }
                return r.unwrap_or_else(|e| error(&e));
            }
            let text = field_str(line, "text").unwrap_or_default();
            p.type_text(id, &text).unwrap_or_else(|e| error(&e))
        }
        // Before a tap or a type: a target out of view, scrolled into it.
        Some("reveal") => match id() {
            Some(id) => p.reveal(id).unwrap_or_else(|e| error(&e)),
            None => error("reveal needs an id"),
        },
        Some("clock") => clock(p, line),
        Some("prefer") => prefer(p, line),
        Some("screenshot") => match field_str(line, "path") {
            Some(path) => p.screenshot(&path).unwrap_or_else(|e| error(&e)),
            None => error("screenshot needs a path"),
        },
        _ => p.host().agent(line),
    }
}

/// `prefer` (LLP 1061 D5; LLP 1069.000 D6): the device facts by their web
/// names, grouped as LLP 1069.007 D2 groups them — `media`, the display
/// preferences `exactViewport()` answers (the scheme is also the system
/// appearance `setScheme("system")` follows); `page`, what `exactPage()`
/// answers and the root font size; `fold` (LLP 1078 D7), the posture and
/// the segment grid this host, having no fold, makes by splitting its
/// viewport evenly with the gap centred on each divider. A fact not named
/// stays as it is; nothing applies unless all are known.
fn prefer<D: DataSource>(p: &mut Presenter<D>, line: &str) -> String {
    let request: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
    let empty = serde_json::Map::new();
    let group = |name: &str| request.get(name).and_then(|m| m.as_object());
    let (media, page_facts, fold) = (group("media"), group("page"), group("fold"));
    if media.is_none() && page_facts.is_none() && fold.is_none() {
        return error(
            "prefer needs media, page or fold: {\"prefers-reduced-motion\": \"reduce\", …}",
        );
    }
    if let Some(fold) = fold {
        if let Some(e) = prefer_fold(p, fold) {
            return e;
        }
    }
    let (media, page_facts) = (media.unwrap_or(&empty), page_facts.unwrap_or(&empty));
    let mut preferences = p.host().runner().viewport().preferences;
    let mut page = p.host().runner().page();
    let mut root_font_size = None;
    let mut dark = p.scheme.1;
    for (name, value) in media {
        match (name.as_str(), value.as_str().unwrap_or_default()) {
            ("prefers-reduced-motion", v @ ("reduce" | "no-preference")) => {
                preferences.reduced_motion = v == "reduce"
            }
            ("prefers-reduced-transparency", v @ ("reduce" | "no-preference")) => {
                preferences.reduced_transparency = v == "reduce"
            }
            ("prefers-contrast", v @ ("more" | "less" | "custom" | "no-preference")) => {
                preferences.contrast = match v {
                    "more" => exact_runner::Contrast::More,
                    "less" => exact_runner::Contrast::Less,
                    "custom" => exact_runner::Contrast::Custom,
                    _ => exact_runner::Contrast::NoPreference,
                }
            }
            ("prefers-color-scheme", v @ ("light" | "dark")) => dark = v == "dark",
            _ => {
                return error(&format!(
                    "prefer: {name}: {value} is not a preference this host sets"
                ))
            }
        }
    }
    for (name, value) in page_facts {
        let text = value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string());
        match (name.as_str(), text.as_str()) {
            ("visibility-state", v @ ("visible" | "hidden")) => page.hidden = v == "hidden",
            ("online", v @ ("true" | "false")) => page.on_line = v == "true",
            ("can-share", v @ ("true" | "false")) => page.can_share = v == "true",
            ("root-font-size", v) if v.parse::<f64>().is_ok_and(|n| n.is_finite() && n > 0.0) => {
                root_font_size = v.parse::<f64>().ok()
            }
            _ => {
                return error(&format!(
                    "prefer: {name}: {value} is not a page fact this host sets"
                ))
            }
        }
    }
    preferences.dark = dark;
    if let Some(e) = p.set_preferences(preferences) {
        return error(&e);
    }
    if let Some(e) = p.set_page(page) {
        return error(&e);
    }
    if let Some(px) = root_font_size {
        if let Some(e) = p.set_root_font_size(px) {
            return error(&e);
        }
    }
    if media.contains_key("prefers-color-scheme") {
        p.set_system_scheme(dark);
    }
    let (preferences, page) = (
        p.host().runner().viewport().preferences,
        p.host().runner().page(),
    );
    let keyword = |on: bool| if on { "reduce" } else { "no-preference" };
    let fold = p.host().runner().viewport().fold;
    serde_json::json!({"media": {
        "prefers-reduced-motion": keyword(preferences.reduced_motion),
        "prefers-reduced-transparency": keyword(preferences.reduced_transparency),
        "prefers-contrast": preferences.contrast.keyword(),
        "prefers-color-scheme": if p.scheme.1 { "dark" } else { "light" },
    }, "page": {
        "visibility-state": page.visibility_state(),
        "online": page.on_line,
        "can-share": page.can_share,
        "root-font-size": p.host().runner().root_font_size(),
    }, "fold": {
        "device-posture": fold.posture.keyword(),
        "horizontal-viewport-segments": fold.cols,
        "vertical-viewport-segments": fold.rows,
        "viewport-segments": p.segments.iter().map(|r| [r.x, r.y, r.width, r.height]).collect::<Vec<_>>(),
    }})
    .to_string()
}

/// The `fold` group: `posture` (`folded` | `continuous`), `cols` and `rows`
/// (each at least 1), `gap` (points, 0 by default); the rects are the even
/// split of the viewport. Each refusal names its fact (LLP 1078 D10).
fn prefer_fold<D: DataSource>(
    p: &mut Presenter<D>,
    fold: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    let current = p.host().runner().viewport().fold;
    let (mut posture, mut cols, mut rows, mut gap) =
        (current.posture, current.cols, current.rows, 0.0);
    for (name, value) in fold {
        let count = || value.as_u64().and_then(|n| u32::try_from(n).ok());
        match name.as_str() {
            "posture" => match value.as_str().and_then(exact_runner::Posture::from_keyword) {
                Some(p) => posture = p,
                None => {
                    return Some(error(&format!(
                        "prefer: posture: {value} is folded or continuous"
                    )))
                }
            },
            "cols" => match count() {
                Some(n) => cols = n,
                None => {
                    return Some(error(&format!(
                        "prefer: segments: {value} columns is not a count"
                    )))
                }
            },
            "rows" => match count() {
                Some(n) => rows = n,
                None => {
                    return Some(error(&format!(
                        "prefer: segments: {value} rows is not a count"
                    )))
                }
            },
            "gap" => match value.as_f64() {
                Some(g) => gap = g,
                None => {
                    return Some(error(&format!(
                        "prefer: segments: gap {value} is not a length"
                    )))
                }
            },
            _ => {
                return Some(error(&format!(
                    "prefer: {name} is not a fold fact this host sets"
                )))
            }
        }
    }
    let (w, h) = p.viewport();
    let rects =
        match exact_runner::viewport::even_segments(f64::from(w), f64::from(h), cols, rows, gap) {
            Ok(rects) => rects
                .into_iter()
                .map(|[x, y, w, h]| exact_kernel::Rect::new(x as f32, y as f32, w as f32, h as f32))
                .collect(),
            Err(e) => return Some(error(&format!("prefer: {e}"))),
        };
    p.set_segments(posture, cols, rows, rects)
        .map(|e| error(&e))
}

/// Actual presenter resize and CPU/GPU frame construction before replying.
/// No DRM mode switch, desktop compositor or physical display is involved.
fn resize<D: DataSource>(p: &mut Presenter<D>, request: &serde_json::Value) -> String {
    let invalid = || {
        error("tap resize needs exactly two integer dimensions in 64...4096, area <= 8388608, and no other input fields")
    };
    let Some(object) = request.as_object() else {
        return invalid();
    };
    if object
        .keys()
        .any(|k| !matches!(k.as_str(), "op" | "session" | "resize"))
    {
        return invalid();
    }
    let Some(pair) = request["resize"].as_array().filter(|p| p.len() == 2) else {
        return invalid();
    };
    let Some(w) = pair[0].as_f64() else {
        return invalid();
    };
    let Some(h) = pair[1].as_f64() else {
        return invalid();
    };
    if [w, h]
        .iter()
        .any(|v| !v.is_finite() || v.fract() != 0.0 || !(64.0..=4096.0).contains(v))
        || w * h > 8_388_608.0
    {
        return invalid();
    }
    if let Some(e) = p.resize(w as f32, h as f32) {
        return error(&e);
    }
    let pixels = p.frame();
    serde_json::json!({
        "resized": [w, h], "viewport": [p.viewport().0, p.viewport().1],
        "painted": [pixels.width(), pixels.height()], "delivery": "presenter",
        "native": "Presenter.resize + frame", "paint": "headless frame; presentation unobserved"
    })
    .to_string()
}

/// Linux carries an iframe's box but has no web engine (LLP 1020 D5), and
/// a native module's box but no module (LLP 1024 D1).
fn tree<D: DataSource>(p: &mut Presenter<D>, line: &str) -> String {
    p.boxes();
    // @ref LLP 1080.002 D2 — this host exposes no AT-SPI tree (LLP 1015 §7).
    if field_bool(line, "ax") {
        return serde_json::json!({"ax": {"unavailable": true, "reason": "no AT-SPI tree (LLP 1015 §7)"}})
            .to_string();
    }
    // The request as asked: the runner scopes a target and `shallow`.
    let mut tree: serde_json::Value = serde_json::from_str(&p.host().agent(line)).unwrap();
    if let Some(nodes) = tree["nodes"].as_array_mut() {
        nodes.retain(|row| {
            row["id"]
                .as_u64()
                .is_none_or(|id| !p.placement_hidden(id as u32))
        });
        for row in nodes {
            let Some(id) = row["id"].as_u64().map(|id| id as u32) else {
                continue;
            };
            row["focused"] = (p.focus() == Some(id)).into();
            if row["type"] == "WebView" || row["type"] == "Video" {
                row["unavailable"] = true.into();
            }
            // @ref LLP 1024 D1 — declared, but this host loads no module.
            if row["type"] == "NativeView" {
                row["module"] = serde_json::json!({
                    "name": row["props"]["nativeViewModuleName"],
                    "state": "unavailable",
                    "error": "the Linux host loads no native modules"
                });
            }
        }
    }
    tree.to_string()
}

/// The engine's settle time, milliseconds, when a transition is in flight.
fn settle<D: DataSource>(p: &Presenter<D>) -> Option<f64> {
    field_num(&p.host().agent("{\"op\":\"settle\"}"), "settle")
}

/// Move both clocks to one instant: the runner's (timers, each fired at its
/// own due time) and the motion engine's (a seek). The clock lands where
/// the runner says: a timer's refusal stops it at that timer's due time and
/// is the reply's error. `settle` is a fixed point: advance to when the last
/// transition in flight ends, and if the timers crossed on the way started
/// more, again — bounded, `settled: false` when the bound is hit (LLP 1012
/// §2).
fn clock<D: DataSource>(p: &mut Presenter<D>, line: &str) -> String {
    let reply = clock_within(p, line, SETTLE_BOUND);
    retell_offset(p);
    reply
}

/// The zone's offset at the virtual date the clock now reads: a move across
/// a DST change re-answers `exactTime()` (LLP 1069.007 D2). The runner's own
/// date and zone are the drive's (`--epoch`, `--time-zone`); an unchanged
/// offset commits nothing.
fn retell_offset<D: DataSource>(p: &mut Presenter<D>) {
    let time = p.host().runner().wall_time();
    let zone = p.host().runner().place().time_zone.clone();
    if time.epoch_at_zero <= 0.0 {
        return;
    }
    let offset = crate::zone::offset_minutes_at(&zone, time.epoch_at_zero + p.host().now());
    if offset != time.utc_offset {
        if let Some(e) = p.set_time(time.epoch_at_zero, offset) {
            eprintln!("exact: {e}");
        }
    }
}

/// `clock settle`'s bound on requests in flight: a network's worth.
const SETTLE_BOUND: std::time::Duration = std::time::Duration::from_secs(20);

/// [`clock`] with its request bound as a parameter. One deadline covers the
/// whole call, not each round: a request that never answers ends `settle` at
/// the bound, not sixteen times it.
fn clock_within<D: DataSource>(
    p: &mut Presenter<D>,
    line: &str,
    bound: std::time::Duration,
) -> String {
    let deadline = std::time::Instant::now() + bound;
    let from = p.host().now();
    let settle_to_end = field_bool(line, "settle");
    let mut to = field_num(line, "to");
    if settle_to_end {
        // A request in flight is waited for first (LLP 1016): its reply
        // commits, and may start motion, before the fixed point is measured.
        wait_for_replies(p, deadline);
        to = Some(from.max(settle(p).unwrap_or(from)));
    }
    let Some(mut to) = to.filter(|t| t.is_finite()) else {
        return error("clock needs \"to\" (ms) or \"settle\": true");
    };
    if to < from {
        return error(&format!("the clock cannot go backwards ({from} → {to})"));
    }
    let mut rounds = 0;
    loop {
        let (landed, e) = clock_stepped(p, to, deadline);
        if let Some(e) = e {
            let mut s = String::from("{\"error\":");
            exact_runner::agent::quote(&format!("clock: {e}"), &mut s);
            return format!("{s},\"clock\":{}}}", num(landed));
        }
        p.sync_surfaces();
        if settle_to_end {
            // Its error is the next report's to raise; the pass is bounded.
            let _ = p.settle_collections();
        }
        let world = p.worlds(serde_json::json!({"op":"clock","settle":settle_to_end}));
        p.sync_surfaces();
        let response = |settled: Option<bool>, requests: bool| {
            let mut r = format!("{{\"clock\":{}", num(landed));
            if let Some(s) = settled {
                r.push_str(&format!(",\"settled\":{s}"));
            }
            if !world.is_empty() {
                r.push_str(&format!(",\"world\":{}", serde_json::json!(world)));
            }
            if settled == Some(false) && world.iter().any(|w| w["quiescent"] == false) {
                r.push_str(",\"reason\":\"world\"");
            } else if settled == Some(false) && requests {
                r.push_str(",\"reason\":\"requests\"");
            }
            r.push('}');
            r
        };
        if !settle_to_end {
            return response(None, false);
        }
        if p.pending() {
            rounds += 1;
            if rounds >= 16 || std::time::Instant::now() >= deadline {
                return response(Some(false), true);
            }
            wait_for_replies(p, deadline);
            continue;
        }
        let next = world
            .iter()
            .filter(|w| w["quiescent"] == false)
            .filter_map(|w| w["settleAt"].as_f64())
            .fold(landed.max(settle(p).unwrap_or(landed)), f64::max);
        if next <= landed {
            // A held device request never settles on its own and is never
            // waited on: the fixed point is reached, and the agent is told
            // what still waits for it (LLP 1069.007 D3).
            let held: serde_json::Value =
                serde_json::from_str(&p.host().agent("{\"op\":\"holds\"}")).unwrap_or_default();
            if held["holds"].as_array().is_some_and(|h| !h.is_empty()) {
                let mut r = response(Some(false), false);
                r.pop();
                r.push_str(&format!(
                    ",\"reason\":\"device\",\"tickets\":{}}}",
                    held["tickets"]
                ));
                return r;
            }
            return response(Some(true), false);
        }
        rounds += 1;
        if rounds >= 16 {
            return response(Some(false), false);
        }
        to = next;
    }
}

/// To `to`, and what is in flight lands before a timer fires — the runner
/// keeps one request per target (LLP 1016 D5), so a tick's send would drop
/// the reply of the one before it: the jump stops after each timer that
/// sends, and its reply is waited for. Past the deadline, or 4096 stops, the
/// rest is one advance.
fn clock_stepped<D: DataSource>(
    p: &mut Presenter<D>,
    to: f64,
    deadline: std::time::Instant,
) -> (f64, Option<String>) {
    for _ in 0..4096 {
        if !p.host().timer_due_ms().is_some_and(|d| d <= to) || !wait_for_replies(p, deadline) {
            break;
        }
        let (landed, e) = p.clock_until_request(to);
        if e.is_some() {
            return (landed, e);
        }
    }
    // After 4096 stops, what is in flight still lands first. A stop at `to`
    // may leave a timer due there: only a plain advance ends.
    if p.host().timer_due_ms().is_some_and(|d| d <= to) {
        wait_for_replies(p, deadline);
    }
    p.clock(to)
}

/// Pump the executor until no request is in flight, or until the call's
/// deadline (`settled: false` past it), false then.
fn wait_for_replies<D: DataSource>(p: &mut Presenter<D>, deadline: std::time::Instant) -> bool {
    while p.pending() {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        if let Some(e) = p.pump(p.host().now()) {
            eprintln!("exact: {e}");
        }
    }
    true
}

/// `"key":[a,b]` in a flat request.
fn field_pair(json: &str, key: &str) -> Option<(f64, f64)> {
    let needle = format!("\"{key}\"");
    let at = json.find(&needle)?;
    let rest = json[at + needle.len()..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    let rest = rest.strip_prefix('[')?;
    let end = rest.find(']')?;
    let mut parts = rest[..end].split(',').map(|s| s.trim().parse::<f64>().ok());
    let a = parts.next()??;
    let b = parts.next()??;
    Some((a, b))
}

#[cfg(test)]
mod tests;
