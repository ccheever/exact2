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
            return 0;
        }
        let reply = handle(p, &line);
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
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
    let reply = answer(p, line);
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
    let host_reply = matches!(
        op.as_deref(),
        Some("layout" | "tap" | "type" | "clock" | "screenshot")
    );
    if !host_reply || !reply.ends_with('}') || reply.starts_with("{\"error\"") {
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
    match field_str(line, "op").as_deref() {
        Some("tree") => unavailable_tree(p),
        Some("state") => {
            // The runner's state, then the sections a painter cannot observe
            // (LLP 1035.002 D2): present as `unavailable`, never absent, so a
            // reader can tell "no keyboard" from "no report".
            let mut s = p.host().agent(line);
            if s.ends_with('}') && !s.starts_with("{\"error\"") {
                s.pop();
                s.push_str(&format!(",\"raster\":{}", p.images().diagnostics()));
                s.push_str(",\"focus\":{\"unavailable\":true},\"keyboard\":{\"unavailable\":true},\"navigation\":{\"unavailable\":true}}");
            }
            s
        }
        Some("layout") => p.layout_json(id()),
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
            // A held contact (LLP 1035.003 D1) rides evdev when that carrier
            // lands (LLP 1015's lane); until then it is unsupported, said so.
            if field_str(line, "phase").is_some() {
                return error(
                    "unsupported: the Linux carrier cannot hold a contact yet (LLP 1035.003 D3)",
                );
            }
            let Some(id) = id() else {
                return error("tap needs an id");
            };
            let r = match field_pair(line, "wheel") {
                Some((dx, dy)) => p.wheel(id, dx as f32, dy as f32),
                None => p.tap(id),
            };
            r.unwrap_or_else(|e| error(&e))
        }
        Some("type") => {
            let Some(id) = id() else {
                return error("type needs an id");
            };
            let text = field_str(line, "text").unwrap_or_default();
            p.type_text(id, &text).unwrap_or_else(|e| error(&e))
        }
        Some("clock") => clock(p, line),
        Some("screenshot") => match field_str(line, "path") {
            Some(path) => p.screenshot(&path).unwrap_or_else(|e| error(&e)),
            None => error("screenshot needs a path"),
        },
        _ => p.host().agent(line),
    }
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

/// Linux carries an iframe's box but has no web engine (LLP 1020 D5).
fn unavailable_tree<D: DataSource>(p: &Presenter<D>) -> String {
    p.host().agent("{\"op\":\"tree\"}").replace(
        "\"type\":\"WebView\",\"props\":",
        "\"type\":\"WebView\",\"unavailable\":true,\"props\":",
    )
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
    let from = p.host().now();
    let settle_to_end = field_bool(line, "settle");
    let mut to = field_num(line, "to");
    if settle_to_end {
        // A request in flight is waited for first (LLP 1016): its reply
        // commits, and may start motion, before the fixed point is measured.
        wait_for_replies(p);
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
        let (landed, e) = p.clock(to);
        if let Some(e) = e {
            let mut s = String::from("{\"error\":");
            exact_runner::agent::quote(&format!("clock: {e}"), &mut s);
            return format!("{s},\"clock\":{}}}", num(landed));
        }
        if !settle_to_end {
            return format!("{{\"clock\":{}}}", num(landed));
        }
        if p.pending() {
            rounds += 1;
            if rounds >= 16 {
                return format!("{{\"clock\":{},\"settled\":false}}", num(landed));
            }
            wait_for_replies(p);
            continue;
        }
        let next = landed.max(settle(p).unwrap_or(landed));
        if next <= landed {
            return format!("{{\"clock\":{},\"settled\":true}}", num(landed));
        }
        rounds += 1;
        if rounds >= 16 {
            return format!("{{\"clock\":{},\"settled\":false}}", num(landed));
        }
        to = next;
    }
}

/// `"key":[a,b]` in a flat request.
/// Pump the executor until no request is in flight, or for at most twenty
/// seconds (a network's worth; `settled: false` past it).
fn wait_for_replies<D: DataSource>(p: &mut Presenter<D>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while p.pending() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        if let Some(e) = p.pump(p.host().now()) {
            eprintln!("exact: {e}");
        }
    }
}

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
}
