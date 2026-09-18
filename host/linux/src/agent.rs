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
        Some("layout") => p.layout_json(id()),
        Some("tap") => {
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
            if let Some(key) = field_str(line, "key") {
                let phase = field_str(line, "phase");
                let r = p.type_key(id, &key, phase.as_deref() != Some("up"));
                if phase.is_none() && r.is_ok() {
                    let _ = p.type_key(id, &key, false);
                }
                return r.unwrap_or_else(|e| error(&e));
            }
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

/// Linux carries an iframe's box but has no web engine (LLP 1020 D5).
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
            if row["type"] == "WebView" {
                row["unavailable"] = true.into();
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
        p.sync_surfaces();
        let world = p.worlds(serde_json::json!({"op":"clock","settle":settle_to_end}));
        p.sync_surfaces();
        let response = |settled: Option<bool>| {
            let mut r = format!("{{\"clock\":{}", num(landed));
            if let Some(s) = settled {
                r.push_str(&format!(",\"settled\":{s}"));
            }
            if !world.is_empty() {
                r.push_str(&format!(",\"world\":{}", serde_json::json!(world)));
            }
            if settled == Some(false) && world.iter().any(|w| w["quiescent"] == false) {
                r.push_str(",\"reason\":\"world\"");
            }
            r.push('}');
            r
        };
        if !settle_to_end {
            return response(None);
        }
        if p.pending() {
            rounds += 1;
            if rounds >= 16 {
                return response(Some(false));
            }
            wait_for_replies(p);
            continue;
        }
        let next = world
            .iter()
            .filter(|w| w["quiescent"] == false)
            .filter_map(|w| w["settleAt"].as_f64())
            .fold(landed.max(settle(p).unwrap_or(landed)), f64::max);
        if next <= landed {
            return response(Some(true));
        }
        rounds += 1;
        if rounds >= 16 {
            return response(Some(false));
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
