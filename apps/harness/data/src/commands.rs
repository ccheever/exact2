//! The sends: `submit` (a prompt or a slash command), `approve`,
//! `interrupt`, `setModel`. Each returns whether it was taken; the work a
//! prompt starts runs on the agent's thread.

use crate::agent;
use crate::art;
use crate::markdown;
use crate::state::{Block, Choice, Entry, Run, Shared};
use crate::tools;
use std::sync::Arc;

/// `submit(text)`.
pub fn submit(shared: &Arc<Shared>, text: &str) -> bool {
    let text = text.trim_end();
    if text.trim().is_empty() {
        return false;
    }
    let Some(command) = text.trim_start().strip_prefix('/') else {
        if shared.lock().busy() {
            shared.toast("Still working — esc to interrupt");
            return false;
        }
        agent::start(shared, text);
        return true;
    };
    let (name, rest) = command
        .split_once(char::is_whitespace)
        .map(|(n, r)| (n, r.trim()))
        .unwrap_or((command, ""));
    if name == "model" {
        return set_model(shared, rest);
    }
    if shared.lock().busy() {
        shared.toast("Still working — esc to interrupt");
        return false;
    }
    let ok = run(shared, name, rest);
    shared.changed();
    ok
}

fn notice_md(shared: &Shared, md: &str) {
    shared.lock().notice(markdown::blocks(md));
}

fn run(shared: &Arc<Shared>, name: &str, rest: &str) -> bool {
    match name {
        "help" => notice_md(shared, art::HELP),
        "clear" => {
            let mut s = shared.lock();
            s.entries.retain(|e| e.kind == "banner");
            s.history.clear();
            s.round = None;
        }
        "stress" => {
            let n = if rest.is_empty() {
                Some(200)
            } else {
                rest.parse::<usize>().ok()
            };
            let Some(n) = n else {
                shared
                    .lock()
                    .error(&format!("/stress takes a count, not {rest:?}"));
                return false;
            };
            let entries = art::stress(n.min(20_000));
            let mut s = shared.lock();
            for e in entries {
                s.push(e);
            }
        }
        "ascii" => {
            shared.lock().notice(art::ascii());
        }
        "ansi" => {
            shared.lock().notice(art::ansi());
        }
        "unicode" => {
            shared.lock().notice(art::unicode());
        }
        "diff" => {
            shared.lock().notice(art::diff_demo());
        }
        "image" => return image(shared, rest),
        "models" => {
            let s = shared.lock();
            let mut md = String::from("**Models** — `/model <id>` to choose\n\n");
            for m in &s.models {
                let mark = if m.id == s.model { " ← current" } else { "" };
                let avail = if m.available { "" } else { " *(no key)*" };
                md.push_str(&format!("- `{}` — {}{avail}{mark}\n", m.id, m.label));
            }
            drop(s);
            notice_md(shared, &md);
        }
        "tools" => {
            let mut md = String::from("**Tools** the agent can call\n\n");
            for (name, description, _) in tools::definitions() {
                let gate = if tools::needs_approval(name) {
                    " *(asks first)*"
                } else {
                    ""
                };
                md.push_str(&format!("- `{name}` — {description}{gate}\n"));
            }
            notice_md(shared, &md);
        }
        other => {
            shared
                .lock()
                .error(&format!("Unknown command /{other} — /help lists them"));
            return false;
        }
    }
    true
}

fn image(shared: &Arc<Shared>, rest: &str) -> bool {
    let path = if rest.is_empty() {
        let n = {
            let mut s = shared.lock();
            s.next_file += 1;
            s.next_file
        };
        let path = std::env::temp_dir().join(format!("exact-harness-{n}.png"));
        if let Err(e) = art::write_png(&path, 320, 200) {
            shared
                .lock()
                .error(&format!("Couldn't write the image: {e}"));
            return false;
        }
        path
    } else {
        let p = std::path::PathBuf::from(rest);
        let p = if p.is_absolute() {
            p
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        };
        if !p.is_file() {
            shared.lock().error(&format!("No image at {}", p.display()));
            return false;
        }
        p
    };
    let (cols, rows) = match art::image_size(&path) {
        Some((w, h)) => art::cell_box(w, h, 48),
        None => (48, 16),
    };
    let mut s = shared.lock();
    s.push(Entry {
        kind: "image".into(),
        title: path.display().to_string(),
        image: path.display().to_string(),
        cols: cols as f64,
        rows: rows as f64,
        blocks: vec![Block::p(vec![Run::dim(path.display().to_string())])],
        ..Entry::default()
    });
    true
}

/// `setModel(id)`, and `/model <id>`.
pub fn set_model(shared: &Arc<Shared>, id: &str) -> bool {
    let id = id.trim();
    let found = {
        let s = shared.lock();
        s.models.iter().find(|m| m.id == id).cloned()
    };
    match found {
        None if id.is_empty() => {
            let current = shared.lock().model.clone();
            shared.toast(&format!("Model is {current} — /models lists them"));
            false
        }
        None => {
            shared.toast(&format!("No model {id:?} — /models lists them"));
            false
        }
        Some(m) if !m.available => {
            shared.toast(&format!("{} needs a key for {}", m.id, m.provider));
            false
        }
        Some(m) => {
            shared.lock().model = m.id.clone();
            shared.toast(&format!("Model set to {}", m.label));
            true
        }
    }
}

/// `approve(id, choice)`.
pub fn approve(shared: &Arc<Shared>, id: &str, choice: &str) -> bool {
    let choice = match choice {
        "yes" => Choice::Yes,
        "always" => Choice::Always,
        "no" => Choice::No,
        _ => return false,
    };
    let mut s = shared.lock();
    if s.approval.id.is_empty() || s.approval.id != id {
        return false;
    }
    s.decision = Some((id.to_string(), choice));
    drop(s);
    shared.wake.notify_all();
    true
}
