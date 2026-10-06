//! The scripted provider: deterministic, offline, and shaped like a real
//! one. It "thinks", streams Markdown in small chunks, then walks a fixed
//! script of tool calls (list the directory, read a small file, run a
//! command or propose an edit that waits for approval), then closes.
//! Prompt keywords vary it: "long" streams ~200 lines, "error" fails
//! mid-stream, "edit" proposes an edit to a scratch copy.

use super::Ask;
use crate::sse::Event;
use crate::state::{Msg, Part, Role};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Skip the mock's pauses (tests).
pub static FAST: AtomicBool = AtomicBool::new(false);

const THINK: Duration = Duration::from_millis(700);
const CHUNK: Duration = Duration::from_millis(15);

/// Sleep, waking early for an interrupt; whether it was interrupted.
fn pause(d: Duration, cancel: &AtomicBool) -> bool {
    if FAST.load(Ordering::Relaxed) {
        return cancel.load(Ordering::SeqCst);
    }
    let step = Duration::from_millis(10);
    let mut left = d;
    while !left.is_zero() {
        if cancel.load(Ordering::SeqCst) {
            return true;
        }
        let s = left.min(step);
        std::thread::sleep(s);
        left -= s;
    }
    cancel.load(Ordering::SeqCst)
}

/// Text in small chunks, as a model streams it.
fn chunks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, c) in text.chars().enumerate() {
        cur.push(c);
        if cur.len() >= 4 + (i * 7) % 5 && (c == ' ' || c == '\n' || cur.len() >= 10) {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

const ANSWER: &str = "## Looking at your project\n\n\
Here is what I'll do: take a **quick look** around, read *one* file, and run `ls -la` \
to see what's here. Markdown renders as it streams — see the [docs](https://example.com/docs).\n\n\
- Survey the working directory\n  - list it\n  - pick a small file\n- Read that file\n\
- Run a command (it asks first)\n\n\
1. First step\n2. Second step\n   1. a nested step\n3. Third step\n\n\
```rust\n// A taste of highlighting.\nfn main() {\n    let answer: u32 = 42;\n    println!(\"{answer} is {}\", Some(\"the answer\").unwrap());\n}\n```\n\n\
> The best way to find out is to look.\n\n";

fn long_answer() -> String {
    let mut s = String::from("# A long answer\n\n");
    for i in 1..=200 {
        match i % 50 {
            0 => s.push_str(&format!(
                "\n```sh\n# block {i}\necho \"line {i}\" | wc -c\n```\n\n"
            )),
            25 => s.push_str(&format!("\n### Section {}\n\n", i / 25)),
            _ => s.push_str(&format!(
                "- Line {i}: the quick brown fox jumps over the lazy dog, `{i}` times.\n"
            )),
        }
    }
    s
}

/// The prompt this turn answers, and how many tool rounds it has had.
fn progress(history: &[Msg]) -> (String, usize) {
    let start = history
        .iter()
        .rposition(|m| m.role == Role::User && !m.text().is_empty())
        .unwrap_or(0);
    let prompt = history.get(start).map(Msg::text).unwrap_or_default();
    let rounds = history[start..]
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .count();
    (prompt.to_lowercase(), rounds)
}

fn last_result(history: &[Msg]) -> Option<String> {
    history.last().and_then(|m| {
        m.parts.iter().rev().find_map(|p| match p {
            Part::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
    })
}

/// A small file in the working directory to read.
fn small_file() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    for name in ["Cargo.toml", "README.md", "package.json"] {
        let p = cwd.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    let mut files: Vec<(u64, PathBuf)> = std::fs::read_dir(&cwd)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            (m.is_file() && m.len() < 16_000).then(|| (m.len(), e.path()))
        })
        .collect();
    files.sort();
    files.into_iter().map(|f| f.1).next()
}

fn shown(p: &std::path::Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    p.strip_prefix(&cwd).unwrap_or(p).display().to_string()
}

/// A scratch copy to edit, and an edit to it: (path, old, new).
fn scratch_edit() -> (String, String, String) {
    let dir = std::env::temp_dir().join(format!("exact-harness-mock-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let source = small_file()
        .and_then(|p| Some((p.file_name()?.to_owned(), std::fs::read_to_string(&p).ok()?)));
    let (name, text) = match source {
        Some((name, text)) => (name.to_string_lossy().into_owned(), text),
        None => (
            "demo.rs".into(),
            "fn main() {\n    println!(\"hello\");\n}\n".into(),
        ),
    };
    let path = dir.join(&name);
    let _ = std::fs::write(&path, &text);
    let old = text
        .lines()
        .find(|l| !l.trim().is_empty() && text.matches(*l).count() == 1)
        .unwrap_or(text.as_str())
        .to_string();
    let new = format!("{old}\n# edited by the harness mock (a scratch copy)");
    (path.display().to_string(), old, new)
}

fn tokens(text_len: usize) -> f64 {
    (text_len as f64 / 4.0).ceil()
}

/// Stream the next step of the script.
pub fn stream(ask: &Ask<'_>, sink: &mut dyn FnMut(Event)) -> Result<(), String> {
    let cancel = ask.cancel;
    let (prompt, round) = progress(ask.history);
    let context: usize = ask
        .history
        .iter()
        .map(|m| format!("{:?}", m.parts).len())
        .sum::<usize>()
        + ask.system.len();
    sink(Event::Usage {
        input: Some(tokens(context)),
        output: Some(0.0),
    });
    if pause(THINK, cancel) {
        return Ok(());
    }
    let edit = prompt.contains("edit");
    let (text, call): (String, Option<(&str, serde_json::Value)>) = if prompt.contains("long") {
        (long_answer(), None)
    } else if prompt.contains("error") {
        let cut: String = ANSWER.chars().take(160).collect();
        for c in chunks(&cut) {
            if pause(CHUNK, cancel) {
                return Ok(());
            }
            sink(Event::Text(c));
        }
        return Err("mock: overloaded_error: the scripted stream failed mid-reply".into());
    } else {
        match round {
            0 => (ANSWER.to_string(), Some(("list_dir", json!({"path": "."})))),
            1 => match small_file() {
                Some(p) => (
                    format!("Let me read `{}`.", shown(&p)),
                    Some(("read_file", json!({"path": shown(&p), "limit": 40}))),
                ),
                None => ("Nothing small to read here.".into(), None),
            },
            2 if edit => {
                let (path, old, new) = scratch_edit();
                (
                    "I'll propose a small edit — to a **scratch copy**, never your file.".into(),
                    Some(("edit_file", json!({"path": path, "old": old, "new": new}))),
                )
            }
            2 => (
                "Now let me run a command; it waits for your approval.".into(),
                Some(("bash", json!({"command": "ls -la"}))),
            ),
            _ => {
                let denied = last_result(ask.history).is_some_and(|r| r.contains("denied by user"));
                (
                    if denied {
                        "Understood — I won't run that. Everything else is above.".into()
                    } else {
                        "That's the tour: **text** streamed, tools ran, and one asked first. \
                         Try `/help` for commands, or a prompt with *long*, *edit* or *error*."
                            .into()
                    },
                    None,
                )
            }
        }
    };
    for c in chunks(&text) {
        if pause(CHUNK, cancel) {
            return Ok(());
        }
        sink(Event::Text(c));
    }
    if let Some((name, input)) = call {
        if pause(CHUNK * 4, cancel) {
            return Ok(());
        }
        sink(Event::ToolCall {
            id: format!("mock_{round}_{name}"),
            name: name.into(),
            input,
        });
    }
    sink(Event::Usage {
        input: Some(tokens(context)),
        output: Some(tokens(text.len())),
    });
    sink(Event::Done);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_rejoin_to_the_text() {
        let c = chunks(ANSWER);
        assert!(c.len() > 40);
        assert_eq!(c.concat(), ANSWER);
    }
}
