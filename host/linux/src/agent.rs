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

mod contact;

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

fn answer<D: DataSource>(p: &mut Presenter<D>, line: &str) -> String {
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
        return reply;
    }
    match field_str(line, "op").as_deref() {
        Some("tree") => accessibility_tree(p),
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
            if let Some(reply) = p.control_tap(&request) {
                return reply.to_string();
            }
            if request.get("phase").is_some() {
                return contact::answer(p, &request);
            }
            let Some(id) = id() else {
                return error("tap needs an id");
            };
            let r = match field_pair(line, "wheel") {
                Some((dx, dy)) => p.wheel(id, dx as f32, dy as f32),
                None if field_bool(line, "hover") => p.hover(id),
                None => p.tap(id),
            };
            r.unwrap_or_else(|e| error(&e))
        }
        Some("type") => {
            let Some(id) = id() else {
                return error("type needs an id");
            };
            if let Some(key) = field_str(line, "key") {
                let phase = field_str(line, "phase");
                let r = p.type_key(id, &key, &key, phase.as_deref() != Some("up"), false);
                if phase.is_none() && r.is_ok() {
                    let _ = p.type_key(id, &key, &key, false, false);
                }
                return r.unwrap_or_else(|e| error(&e));
            }
            let text = field_str(line, "text").unwrap_or_default();
            p.type_text(id, &text).unwrap_or_else(|e| error(&e))
        }
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
/// answers and the root font size. A fact not named stays as it is;
/// nothing applies unless all are known.
fn prefer<D: DataSource>(p: &mut Presenter<D>, line: &str) -> String {
    let request: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
    let empty = serde_json::Map::new();
    let group = |name: &str| request.get(name).and_then(|m| m.as_object());
    let (media, page_facts) = (group("media"), group("page"));
    if media.is_none() && page_facts.is_none() {
        return error("prefer needs media or page: {\"prefers-reduced-motion\": \"reduce\", …}");
    }
    let (media, page_facts) = (media.unwrap_or(&empty), page_facts.unwrap_or(&empty));
    let mut preferences = p.host().runner().viewport().preferences;
    let mut page = p.host().runner().page();
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
    if media.contains_key("prefers-color-scheme") {
        p.set_system_scheme(dark);
    }
    let (preferences, page) = (
        p.host().runner().viewport().preferences,
        p.host().runner().page(),
    );
    let keyword = |on: bool| if on { "reduce" } else { "no-preference" };
    serde_json::json!({"media": {
        "prefers-reduced-motion": keyword(preferences.reduced_motion),
        "prefers-reduced-transparency": keyword(preferences.reduced_transparency),
        "prefers-contrast": preferences.contrast.keyword(),
        "prefers-color-scheme": if p.scheme.1 { "dark" } else { "light" },
    }, "page": {
        "visibility-state": page.visibility_state(),
        "online": page.on_line,
        "can-share": page.can_share,
    }})
    .to_string()
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
fn accessibility_tree<D: DataSource>(p: &mut Presenter<D>) -> String {
    p.boxes();
    use exact_kernel::generated::PropId;
    fn text<D: DataSource>(p: &Presenter<D>, id: u32) -> String {
        let Some(node) = p.host().kernel().node(id) else {
            return String::new();
        };
        if let Some(s) = node.props.str(PropId::Text) {
            return s.into();
        }
        node.children()
            .iter()
            .map(|&id| text(p, id))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
    let mut tree: serde_json::Value =
        serde_json::from_str(&p.host().agent(r#"{"op":"tree"}"#)).unwrap();
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
            if let Some(node) = p.host().kernel().node(id) {
                if node.props.str(PropId::AccessibilityLabel).is_some()
                    || node.props.str(PropId::Text).is_some()
                    || matches!(
                        node.props.str(PropId::AccessibilityRole),
                        Some("button" | "link")
                    )
                {
                    row["accessibleName"] = node
                        .props
                        .str(PropId::AccessibilityLabel)
                        .map(str::to_owned)
                        .unwrap_or_else(|| text(p, id))
                        .into();
                }
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
mod tests {
    use super::*;
    use crate::presenter::PainterChoice;
    use exact_runner::{DataError, Value};

    #[derive(Default)]
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }

    /// A source whose one request is handed to work that never replies: the
    /// reply is leaked, so neither an outcome nor the drop's abort arrives.
    #[derive(Default)]
    struct Hung;
    impl DataSource for Hung {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(
            &mut self,
            _: &mut exact_runner::Store,
            source: &str,
            _: &[Value],
        ) -> Result<exact_runner::Answer, DataError> {
            Ok(match source {
                "fallback" => exact_runner::Answer::Now(Value::Bool(false)),
                _ => exact_runner::Answer::Later(exact_runner::Request::continuation(1)),
            })
        }
        fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
            exact_runner::Dispatch::Run(exact_runner::Work::Later(Box::new(std::mem::forget)))
        }
    }

    /// Answers `save` later, on the I/O worker, with 1.
    #[derive(Default)]
    struct Echo;
    impl DataSource for Echo {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(
            &mut self,
            _: &mut exact_runner::Store,
            _: &str,
            _: &[Value],
        ) -> Result<exact_runner::Answer, DataError> {
            Ok(exact_runner::Answer::Later(
                exact_runner::Request::continuation(1),
            ))
        }
        fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
            exact_runner::Dispatch::Run(exact_runner::Work::Now(Box::new(|| {
                exact_runner::Outcome::Storage(b"1".to_vec())
            })))
        }
        fn parse(
            &mut self,
            _: &mut exact_runner::Store,
            _: &str,
            _: &[Value],
            _: exact_runner::Outcome,
        ) -> Result<exact_runner::Answer, DataError> {
            Ok(exact_runner::Answer::Now(Value::Number(1.0)))
        }
    }

    /// A jump stops at each timer that sends and lands its reply before
    /// the next fires (LLP 1016 D5); a stop at the target still fires the
    /// other timers due there.
    #[test]
    fn a_jump_lands_every_reply_and_fires_every_timer_due() {
        let plan = contract::compile(
            "component App\n  state count = 0\n  mutation result as shape number\n  action ping writes result\n    send result = save()\n  action tock writes count\n    count = count + 1\n  task pings mount\n    every(300, ping)\n  task tocks mount\n    every(300, tock)\n  view\n    text toString(count)\n",
        )
        .unwrap();
        let (mut p, boot_error) = Presenter::boot_with(
            &plan.encode(),
            Echo,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(boot_error.is_none(), "{boot_error:?}");
        let count = |p: &mut Presenter<Echo>| {
            let state: serde_json::Value =
                serde_json::from_str(&handle(p, r#"{"op":"state"}"#)).unwrap();
            state["slots"]["count"].clone()
        };
        let reply = handle(&mut p, r#"{"op":"clock","to":300}"#);
        assert_eq!(count(&mut p), serde_json::json!(1), "{reply}");
        let reply = handle(&mut p, r#"{"op":"clock","to":1500}"#);
        assert_eq!(count(&mut p), serde_json::json!(5), "{reply}");
        let logs = handle(&mut p, r#"{"op":"logs"}"#);
        assert_eq!(logs.matches("fulfil ").count(), 5, "{logs}");
        assert!(!logs.contains("dropped"), "{logs}");
    }

    /// LLP 1061 D5: `prefer` sets what `exactViewport()` answers and the
    /// system appearance; an unknown feature is refused and nothing applies.
    #[test]
    fn prefer_sets_the_display_preferences_by_their_media_names() {
        let plan = contract::compile(
            "shape M\n  prefersReducedMotion: bool\ncomponent App\n  resource m = exactViewport() as shape M\n  view\n    text (m.prefersReducedMotion ? \"still\" : \"moving\") testId=\"t\"\n",
        )
        .unwrap();
        let bytes = contract::bake(plan, NoData).unwrap().encode();
        let (mut p, _) = Presenter::boot_with(
            &bytes,
            NoData,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        let text = |p: &mut Presenter<NoData>| handle(p, r#"{"op":"tree"}"#);
        assert!(text(&mut p).contains("moving"));
        let refused = handle(
            &mut p,
            r#"{"op":"prefer","media":{"prefers-reduced-motion":"reduce","prefers-contrast":"loud"}}"#,
        );
        assert!(refused.contains("\"error\""), "{refused}");
        assert!(text(&mut p).contains("moving"), "nothing applied");
        let reply: serde_json::Value = serde_json::from_str(&handle(
            &mut p,
            r#"{"op":"prefer","media":{"prefers-reduced-motion":"reduce","prefers-color-scheme":"dark"}}"#,
        ))
        .unwrap();
        assert_eq!(reply["media"]["prefers-reduced-motion"], "reduce");
        assert_eq!(
            reply["media"]["prefers-reduced-transparency"],
            "no-preference"
        );
        assert_eq!(reply["media"]["prefers-color-scheme"], "dark");
        assert!(text(&mut p).contains("still"));
        assert!(p.dark(), "no app override: the system's dark");
        p.app_scheme(Some(false));
        assert!(!p.dark(), "the app's own choice wins");
    }

    /// LLP 1069.000 D1, D2, D6: `prefer` sets contrast, the system's scheme
    /// beneath an app's own, and what `exactPage()` answers; `state.device`
    /// shows them without an app declaring either source.
    #[test]
    fn prefer_sets_contrast_scheme_and_the_page_facts() {
        let plan = contract::compile(
            "shape M\n  prefersContrast: string\n  prefersColorScheme: string\nshape P\n  visibilityState: string\n  onLine: bool\n  canShare: bool\ncomponent App\n  resource m = exactViewport() as shape M\n  resource g = exactPage() as shape P\n  view\n    text `${m.prefersContrast} ${m.prefersColorScheme} ${g.visibilityState} ${g.onLine ? \"online\" : \"offline\"} ${g.canShare ? \"share\" : \"no-share\"}` testId=\"t\"\n",
        )
        .unwrap();
        let bytes = contract::bake(plan, NoData).unwrap().encode();
        let (mut p, _) = Presenter::boot_with(
            &bytes,
            NoData,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        let text = |p: &mut Presenter<NoData>| handle(p, r#"{"op":"tree"}"#);
        assert!(
            text(&mut p).contains("no-preference light visible online no-share"),
            "{}",
            text(&mut p)
        );
        p.app_scheme(Some(false));
        let reply: serde_json::Value = serde_json::from_str(&handle(
            &mut p,
            r#"{"op":"prefer","media":{"prefers-contrast":"more","prefers-color-scheme":"dark"},"page":{"visibility-state":"hidden","online":false,"can-share":"true"}}"#,
        ))
        .unwrap();
        assert_eq!(reply["media"]["prefers-contrast"], "more");
        assert_eq!(reply["page"]["online"], false);
        assert!(
            text(&mut p).contains("more dark hidden offline share"),
            "{}",
            text(&mut p)
        );
        assert!(!p.dark(), "the app's own scheme still paints");
        let state: serde_json::Value =
            serde_json::from_str(&handle(&mut p, r#"{"op":"state"}"#)).unwrap();
        assert_eq!(state["device"]["prefersColorScheme"], "dark");
        assert_eq!(state["device"]["visibilityState"], "hidden");
        let refused = handle(&mut p, r#"{"op":"prefer","page":{"online":"maybe"}}"#);
        assert!(refused.contains("\"error\""), "{refused}");
    }

    /// Answers `item` with 1 a moment later, from another thread: a fetch
    /// still in flight when the clock is asked to move.
    #[derive(Default)]
    struct Slow;
    impl DataSource for Slow {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(
            &mut self,
            _: &mut exact_runner::Store,
            source: &str,
            _: &[Value],
        ) -> Result<exact_runner::Answer, DataError> {
            Ok(match source {
                "fallback" => exact_runner::Answer::Now(Value::Number(0.0)),
                _ => exact_runner::Answer::Later(exact_runner::Request::continuation(1)),
            })
        }
        fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
            exact_runner::Dispatch::Run(exact_runner::Work::Later(Box::new(|reply| {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    reply.send(exact_runner::Outcome::Storage(b"1".to_vec()));
                });
            })))
        }
        fn parse(
            &mut self,
            _: &mut exact_runner::Store,
            _: &str,
            _: &[Value],
            _: exact_runner::Outcome,
        ) -> Result<exact_runner::Answer, DataError> {
            Ok(exact_runner::Answer::Now(Value::Number(1.0)))
        }
    }

    /// LLP 1069.007 §5 item 4, with a synthetic capability standing in for
    /// the first real one: a held device request, a due app timer and an
    /// unfinished fetch together. `clock +N` fires the timer without waiting
    /// on the hold; `clock settle` waits for the fetch, never for the hold,
    /// and says `device` with the tickets; `tap @t` / `type @t` answer it
    /// once, `substituted`; a hold whose node goes is retired.
    #[test]
    fn a_held_device_request_is_answered_by_ticket_and_never_waited_on() {
        let plan = contract::compile(
            "component App\n  state count = 0\n  state show = true\n  resource item = item() as shape number else fallback()\n  action tock writes count\n    count = count + 1\n  action hide writes show\n    show = false\n  task tocks mount\n    every(300, tock)\n  view\n    column width=300 height=300\n      box testId=\"picker\" width=100 height=40\n      when show\n        box testId=\"doc\" width=100 height=40\n      button press=hide testId=\"hide\" width=100 height=40\n        text \"Hide\"\n      text `${count} ${item}` testId=\"log\" height=20\n",
        )
        .unwrap();
        let (mut p, boot_error) = Presenter::boot_with(
            &plan.encode(),
            Slow,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(boot_error.is_none(), "{boot_error:?}");
        let id = |p: &Presenter<Slow>, test_id: &str| {
            let k = p.host().kernel();
            k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
        };
        let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
        let (picker, doc, hide) = (id(&p, "picker"), id(&p, "doc"), id(&p, "hide"));
        let args = r#"{"id":"picker","accept":["image/*"],"multiple":false}"#;
        let t = p
            .host_mut()
            .runner_mut()
            .hold("pick", Some(picker), args, &[], true);
        let state = json(handle(&mut p, r#"{"op":"state"}"#));
        let held = state["pending"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["ticket"] == t)
            .cloned()
            .unwrap_or_default();
        assert_eq!(held["name"], "picker", "{state}");
        assert_eq!(held["device"]["capability"], "pick");
        assert_eq!(held["device"]["args"]["accept"][0], "image/*");
        assert!(
            state["pending"].as_array().unwrap().len() >= 2,
            "the fetch is pending beside the hold: {state}"
        );

        let bound = std::time::Duration::from_secs(3);
        let started = std::time::Instant::now();
        let reply = clock_within(&mut p, r#"{"op":"clock","to":600}"#, bound);
        assert!(started.elapsed() < bound, "clock +N waited: {reply}");
        let state = json(handle(&mut p, r#"{"op":"state"}"#));
        assert_eq!(state["slots"]["count"], 2, "both timers fired: {reply}");

        let started = std::time::Instant::now();
        let reply = json(clock_within(
            &mut p,
            r#"{"op":"clock","settle":true}"#,
            bound,
        ));
        assert!(started.elapsed() < bound, "settle waited on the hold");
        assert_eq!(reply["settled"], false, "{reply}");
        assert_eq!(reply["reason"], "device", "{reply}");
        assert_eq!(reply["tickets"], serde_json::json!([t]), "{reply}");
        assert_eq!(
            json(handle(&mut p, r#"{"op":"state"}"#))["resources"]["item"],
            1,
            "settle waited for the fetch"
        );

        let wrong = handle(
            &mut p,
            &format!(r#"{{"op":"tap","ticket":{t},"choice":"allow"}}"#),
        );
        assert!(wrong.contains("tap takes cancel"), "{wrong}");
        let answered = json(handle(
            &mut p,
            &format!(r#"{{"op":"type","ticket":{t},"text":"fixtures/cat.jpg"}}"#),
        ));
        assert_eq!(answered["delivery"], "substituted", "{answered}");
        assert_eq!(answered["answered"], "value");
        let again = handle(
            &mut p,
            &format!(r#"{{"op":"tap","ticket":{t},"choice":"cancel"}}"#),
        );
        assert!(again.contains(&format!("not pending: @{t}")), "{again}");
        let logs = handle(&mut p, r#"{"op":"logs"}"#);
        assert!(
            logs.contains(&format!("device pick {t} held (agent)")),
            "{logs}"
        );
        assert!(logs.contains(&format!("device pick {t} answered: a value")));
        assert!(
            !logs.contains("cat.jpg"),
            "a typed value is never journalled"
        );
        let reply = json(clock_within(
            &mut p,
            r#"{"op":"clock","settle":true}"#,
            bound,
        ));
        assert_eq!(reply["settled"], true, "{reply}");

        let u = p
            .host_mut()
            .runner_mut()
            .hold("pick", Some(doc), "{}", &[], true);
        handle(&mut p, &format!(r#"{{"op":"tap","id":{hide}}}"#));
        let state = json(handle(&mut p, r#"{"op":"state"}"#));
        assert!(
            !state["pending"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["ticket"] == u),
            "the node went, and its hold with it: {state}"
        );
        let logs = handle(&mut p, r#"{"op":"logs"}"#);
        assert!(logs.contains(&format!("device pick {u} retired")), "{logs}");
        let late = handle(
            &mut p,
            &format!(r#"{{"op":"tap","ticket":{u},"choice":"cancel"}}"#),
        );
        assert!(late.contains(&format!("not pending: @{u}")), "{late}");
    }

    /// LLP 1069.007 D2: the offset follows the virtual date across a DST
    /// change — Los Angeles, an hour before 2026's spring-forward, then two
    /// hours on.
    #[test]
    fn a_clock_move_across_a_dst_change_recomputes_the_offset() {
        let plan = contract::compile("component App\n  view\n    text \"x\" height=20\n").unwrap();
        let (mut p, _) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        let place = exact_runner::time::Place {
            locale: "en-US".into(),
            time_zone: "America/Los_Angeles".into(),
            seed: 1.0,
        };
        assert!(p.set_place(&place).is_none());
        assert!(p.set_time(1_772_960_400_000.0, -480.0).is_none());
        let offset = |p: &mut Presenter<NoData>| {
            let state: serde_json::Value =
                serde_json::from_str(&handle(p, r#"{"op":"state"}"#)).unwrap();
            state["time"]["utcOffset"].clone()
        };
        handle(&mut p, r#"{"op":"clock","to":1800000}"#);
        assert_eq!(offset(&mut p), -480, "still standard time at 09:30Z");
        handle(&mut p, r#"{"op":"clock","to":7200000}"#);
        assert_eq!(offset(&mut p), -420, "daylight time from 10:00Z");
    }

    #[test]
    fn settle_takes_one_bound_for_a_request_that_never_answers() {
        let plan = contract::compile(
            "component App\n  resource item = item() as shape bool else fallback()\n  view\n    text \"x\" height=20\n",
        )
        .unwrap();
        let (mut p, boot_error) = Presenter::boot_with(
            &plan.encode(),
            Hung,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(boot_error.is_none(), "{boot_error:?}");
        let bound = std::time::Duration::from_millis(200);
        let started = std::time::Instant::now();
        let reply = clock_within(&mut p, r#"{"op":"clock","settle":true}"#, bound);
        let took = started.elapsed();
        assert!(p.pending(), "the request is still out: {reply}");
        assert!(reply.contains("\"settled\":false"), "{reply}");
        assert!(reply.contains("\"reason\":\"requests\""), "{reply}");
        assert!(
            took < bound * 2,
            "settle took {took:?} for a {bound:?} bound"
        );
    }

    #[test]
    fn resize_input_uses_presenter_and_paints_before_ack() {
        let plan = contract::compile("component App\n  view\n    view width=\"100%\" height=\"100%\" background-color=\"#f00\"\n").unwrap();
        let (mut p, boot_error) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (390.0, 844.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(boot_error.is_none(), "{boot_error:?}");
        let reply: serde_json::Value =
            serde_json::from_str(&handle(&mut p, r#"{"op":"tap","resize":[640,480]}"#)).unwrap();
        assert_eq!(reply["resized"], serde_json::json!([640.0, 480.0]));
        assert_eq!(p.viewport(), (640.0, 480.0));
        assert!(!p.dirty(), "a resize must paint before acknowledgment");
        assert_eq!(reply["painted"], serde_json::json!([640, 480]));
        let layout: serde_json::Value =
            serde_json::from_str(&handle(&mut p, r#"{"op":"layout"}"#)).unwrap();
        assert_eq!(layout["viewport"]["w"], 640);
        assert_eq!(layout["viewport"]["h"], 480);
        for request in [
            r#"{"op":"tap","resize":[0,480]}"#,
            r#"{"op":"tap","resize":[-1,480]}"#,
            r#"{"op":"tap","resize":[true,480]}"#,
            r#"{"op":"tap","resize":["640",480]}"#,
            r#"{"op":"tap","resize":[null,480]}"#,
            r#"{"op":"tap","resize":[NaN,480]}"#,
            r#"{"op":"tap","resize":[1e300,480]}"#,
            r#"{"op":"tap","resize":[4096,4096]}"#,
            r#"{"op":"tap","resize":[640.5,480]}"#,
            r#"{"op":"tap","resize":[640,480,1]}"#,
            r#"{"op":"tap","resize":[640,480],"wheel":[0,1]}"#,
        ] {
            let reply = handle(&mut p, request);
            assert!(reply.starts_with("{\"error\""), "{request}: {reply}");
            assert_eq!(
                p.viewport(),
                (640.0, 480.0),
                "invalid input mutated size: {request}"
            );
        }
    }

    #[test]
    fn a_hover_never_presses_and_a_key_is_never_text() {
        let plan = contract::compile("component App\n  state hot = false\n  state presses = 0\n  state text = \"kept\"\n  state lastKey = \"\"\n  action hovered(value) writes hot\n    hot = value\n  action pressed writes presses\n    presses = presses + 1\n  action edit(value) writes text\n    text = value\n  action keyed(value) writes lastKey\n    lastKey = value\n  view\n    column width=300 height=300\n      box hover=hovered press=pressed testId=\"hot\" width=200 height=60\n      box testId=\"away\" width=200 height=60\n      input value=text change=edit key=keyed testId=\"field\" height=32\n      text `${hot} ${presses} ${text} ${lastKey}` testId=\"log\" height=20\n").unwrap();
        let (mut p, boot_error) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(boot_error.is_none(), "{boot_error:?}");
        let id = |p: &Presenter<NoData>, test_id: &str| {
            let k = p.host().kernel();
            k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
        };
        let log = |p: &Presenter<NoData>| {
            let k = p.host().kernel();
            let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
            node.props
                .str(exact_kernel::PropId::Text)
                .unwrap()
                .to_string()
        };
        let (hot, away, field) = (id(&p, "hot"), id(&p, "away"), id(&p, "field"));
        let reply = handle(
            &mut p,
            &format!(r#"{{"op":"tap","id":{hot},"hover":true}}"#),
        );
        assert!(reply.contains("\"hover\":true"), "{reply}");
        assert_eq!(log(&p), "true 0 kept ", "a hover enters and never presses");
        handle(
            &mut p,
            &format!(r#"{{"op":"tap","id":{away},"hover":true}}"#),
        );
        assert_eq!(log(&p), "false 0 kept ", "the pointer left");
        handle(
            &mut p,
            &format!(r#"{{"op":"type","id":{field},"key":"Escape"}}"#),
        );
        assert_eq!(
            log(&p),
            "false 0 kept Escape",
            "a key is heard by name and types nothing"
        );
    }
}
