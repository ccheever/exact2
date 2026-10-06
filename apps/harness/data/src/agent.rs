//! The agent loop, on its own thread per turn: ask the model with the
//! conversation and the tools, stream its text into a busy assistant entry,
//! run the tools it calls (waiting for approval where one is needed), and
//! ask again until it answers without tools. An interrupt settles the
//! turn's entries from the caller's thread; this thread then finds its turn
//! is no longer current and leaves without touching anything.

use crate::markdown;
use crate::providers::{self, Ask, Route};
use crate::sse::Event;
use crate::state::{Block, Choice, Entry, Line, Msg, Part, Role, Run, Shared, State, Turn};
use crate::tools;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A turn never runs more model rounds than this.
const MAX_ROUNDS: usize = 40;

/// Start a turn for `prompt`. The caller has checked nothing else runs.
pub fn start(shared: &Arc<Shared>, prompt: &str) {
    let route = {
        let mut s = shared.lock();
        s.push(Entry {
            kind: "user".into(),
            blocks: vec![Block::lines(
                prompt
                    .lines()
                    .map(|l| Line::plain(tools::clean_line(l)))
                    .collect(),
            )],
            ..Entry::default()
        });
        match providers::route(&s.model, &s.provider(), &s.sources()) {
            Err(e) => {
                let message = format!("Can't reach {}: {e}", s.model);
                s.error_scrubbed(&message);
                None
            }
            Ok(route) => {
                s.history.push(Msg::user(prompt));
                s.next_turn += 1;
                s.turn = Some(Turn {
                    id: s.next_turn,
                    cancel: Arc::new(AtomicBool::new(false)),
                });
                s.phase = "thinking".into();
                Some(route)
            }
        }
    };
    shared.changed();
    let Some(route) = route else {
        return;
    };
    let turn = shared.lock().turn.clone().expect("just set");
    let shared = shared.clone();
    std::thread::spawn(move || run(&shared, &turn, &route));
}

fn run(shared: &Arc<Shared>, turn: &Turn, route: &Route) {
    let mut done = false;
    for _ in 0..MAX_ROUNDS {
        match round(shared, turn, route) {
            Some(true) => continue,
            Some(false) => {
                done = true;
                break;
            }
            None => return,
        }
    }
    let mut s = shared.lock();
    if s.current(turn) {
        if !done {
            s.notice(vec![Block::p(vec![Run::dim(format!(
                "Stopped after {MAX_ROUNDS} rounds of tool calls."
            ))])]);
        }
        end(&mut s);
        drop(s);
        shared.changed();
    }
}

/// Finish the turn: nothing busy, nothing waiting.
fn end(s: &mut State) {
    s.turn = None;
    s.phase = "idle".into();
    s.approval = Default::default();
    s.decision = None;
    if let Some((assistant, results)) = s.round.take() {
        finish_round(s, assistant, results);
    }
}

/// Put a round into the history; a tool call with no result gets one.
fn finish_round(s: &mut State, assistant: Msg, mut results: Vec<Part>) {
    let calls: Vec<String> = assistant
        .parts
        .iter()
        .filter_map(|p| match p {
            Part::ToolUse { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();
    if assistant.parts.is_empty() {
        return;
    }
    s.history.push(assistant);
    for id in calls {
        let answered = results
            .iter()
            .any(|r| matches!(r, Part::ToolResult { id: rid, .. } if *rid == id));
        if !answered {
            results.push(Part::ToolResult {
                id,
                content: "interrupted by user".into(),
                is_error: true,
            });
        }
    }
    if !results.is_empty() {
        s.history.push(Msg {
            role: Role::User,
            parts: results,
        });
    }
}

/// Interrupt the running turn, from any thread: its busy entries settle
/// with a dim "[interrupted]" note. Whether one was running.
pub fn interrupt(shared: &Arc<Shared>) -> bool {
    let mut s = shared.lock();
    let Some(turn) = s.turn.clone() else {
        return false;
    };
    turn.cancel.store(true, Ordering::SeqCst);
    for e in s.entries.iter_mut().filter(|e| e.busy) {
        if e.kind == "tool" {
            e.status = "error".into();
        }
        e.blocks.push(Block::p(vec![Run::dim("[interrupted]")]));
        e.busy = false;
    }
    end(&mut s);
    drop(s);
    shared.wake.notify_all();
    shared.changed();
    true
}

/// One model round: `Some(true)` to ask again (tools ran), `Some(false)`
/// when the model is done, `None` when the turn stopped being current.
fn round(shared: &Arc<Shared>, turn: &Turn, route: &Route) -> Option<bool> {
    let (history, system, base_in, base_out) = {
        let mut s = shared.lock();
        if !s.current(turn) {
            return None;
        }
        s.phase = "thinking".into();
        s.round = Some((
            Msg {
                role: Role::Assistant,
                parts: vec![],
            },
            vec![],
        ));
        (
            s.history.clone(),
            providers::system(&s.cwd),
            s.tokens_in,
            s.tokens_out,
        )
    };
    shared.changed();
    let mut text = String::new();
    let mut entry: Option<String> = None;
    let mut calls: Vec<(String, String, serde_json::Value)> = Vec::new();
    let mut stale = false;
    let ask = Ask {
        system: &system,
        history: &history,
        cancel: &turn.cancel,
    };
    let result = providers::stream(route, &ask, &mut |event| {
        if stale {
            return;
        }
        match event {
            Event::Text(t) => {
                text.push_str(&t);
                let blocks = markdown::blocks(&text);
                let mut s = shared.lock();
                if !s.current(turn) {
                    stale = true;
                    return;
                }
                let id = match &entry {
                    Some(id) => id.clone(),
                    None => {
                        let id = s.push(Entry {
                            kind: "assistant".into(),
                            busy: true,
                            ..Entry::default()
                        });
                        entry = Some(id.clone());
                        id
                    }
                };
                if let Some(e) = s.busy_entry(&id) {
                    e.blocks = blocks;
                }
                if let Some((msg, _)) = s.round.as_mut() {
                    msg.parts = vec![Part::Text(text.clone())];
                }
                s.phase = "streaming".into();
                drop(s);
                shared.changed();
            }
            Event::ToolCall { id, name, input } => calls.push((id, name, input)),
            Event::Usage { input, output } => {
                let mut s = shared.lock();
                if !s.current(turn) {
                    stale = true;
                    return;
                }
                if let Some(i) = input {
                    s.context = i;
                    s.tokens_in = base_in + i;
                }
                if let Some(o) = output {
                    s.tokens_out = base_out + o;
                }
                drop(s);
                shared.changed();
            }
            Event::Done | Event::Error(_) => {}
        }
    });
    let mut s = shared.lock();
    if !s.current(turn) {
        return None;
    }
    if let Some(e) = entry.as_deref().and_then(|id| s.busy_entry(id)) {
        e.busy = false;
    }
    if let Err(e) = result {
        s.error_scrubbed(&e);
        end(&mut s);
        drop(s);
        shared.changed();
        return None;
    }
    let parts: Vec<Part> = (!text.is_empty())
        .then(|| Part::Text(text.clone()))
        .into_iter()
        .chain(calls.iter().map(|(id, name, input)| Part::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        }))
        .collect();
    if let Some((msg, _)) = s.round.as_mut() {
        msg.parts = parts;
    }
    drop(s);
    shared.changed();
    if calls.is_empty() {
        return Some(false);
    }
    for (id, name, input) in calls {
        let result = tool(shared, turn, &id, &name, &input)?;
        let mut s = shared.lock();
        if !s.current(turn) {
            return None;
        }
        if let Some((_, results)) = s.round.as_mut() {
            results.push(result);
        }
    }
    let mut s = shared.lock();
    if !s.current(turn) {
        return None;
    }
    if let Some((assistant, results)) = s.round.take() {
        finish_round(&mut s, assistant, results);
    }
    Some(true)
}

fn body(lines: Vec<Line>) -> (Vec<Block>, f64) {
    let more = lines.len().saturating_sub(tools::SHOWN);
    let shown = lines.into_iter().take(tools::SHOWN).collect();
    (vec![Block::lines(shown)], more as f64)
}

/// Run one call, asking first when it needs approval; its result for the
/// model, or `None` when the turn stopped.
fn tool(
    shared: &Arc<Shared>,
    turn: &Turn,
    call: &str,
    name: &str,
    input: &serde_json::Value,
) -> Option<Part> {
    let title = tools::title(name, input);
    let needs = tools::needs_approval(name) && crate::sse::invalid_arguments(input).is_none();
    let preview = needs.then(|| tools::preview(name, input));
    let entry = {
        let mut s = shared.lock();
        if !s.current(turn) {
            return None;
        }
        let ask = needs && !s.always.iter().any(|t| t == name);
        let id = s.push(Entry {
            kind: "tool".into(),
            busy: true,
            title,
            status: if ask { "waiting" } else { "running" }.into(),
            ..Entry::default()
        });
        if ask {
            let (summary, lines) = preview.clone().unwrap_or_default();
            s.approval = crate::state::Approval {
                id: id.clone(),
                tool: name.into(),
                summary,
                lines,
            };
            s.decision = None;
            s.phase = "waiting".into();
        } else {
            s.phase = "tool".into();
        }
        (id, ask)
    };
    shared.changed();
    let (id, ask) = entry;
    if ask {
        let choice = {
            let mut s = shared.lock();
            loop {
                if !s.current(turn) {
                    return None;
                }
                if s.decision.as_ref().is_some_and(|(d, _)| *d == id) {
                    break s.decision.take().map(|d| d.1).unwrap_or(Choice::No);
                }
                s = shared
                    .wake
                    .wait_timeout(s, Duration::from_millis(100))
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        };
        let mut s = shared.lock();
        if !s.current(turn) {
            return None;
        }
        s.approval = Default::default();
        match choice {
            Choice::No => {
                if let Some(e) = s.busy_entry(&id) {
                    e.status = "denied".into();
                    e.blocks = vec![Block::lines(vec![Line {
                        runs: vec![Run::dim("denied by user")],
                    }])];
                    e.busy = false;
                }
                s.phase = "thinking".into();
                drop(s);
                shared.changed();
                return Some(Part::ToolResult {
                    id: call.into(),
                    content: "denied by user".into(),
                    is_error: true,
                });
            }
            Choice::Always => s.always.push(name.into()),
            Choice::Yes => {}
        }
        if let Some(e) = s.busy_entry(&id) {
            e.status = "running".into();
        }
        s.phase = "tool".into();
        drop(s);
        shared.changed();
    }
    let done = tools::run(name, input, &turn.cancel);
    let mut s = shared.lock();
    if !s.current(turn) {
        return None;
    }
    if let Some(e) = s.busy_entry(&id) {
        e.status = if done.is_error { "error" } else { "ok" }.into();
        (e.blocks, e.more) = body(done.lines);
        e.busy = false;
    }
    s.phase = "thinking".into();
    drop(s);
    shared.changed();
    Some(Part::ToolResult {
        id: call.into(),
        content: done.text,
        is_error: done.is_error,
    })
}
