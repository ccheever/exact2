//! The sends: `submit` (a prompt or a slash command), `approve`,
//! `interrupt`, `setModel`, `setKey`, `retire`. Each returns whether it was taken; the work a
//! prompt starts runs on the agent's thread.

use crate::agent;
use crate::art;
use crate::catalog;
use crate::keys;
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
        // A prompt waits its turn (P16): queued, shown, started when the
        // running turn ends however it ends.
        let queued = {
            let mut s = shared.lock();
            s.queue.push_back(text.to_string());
            s.busy()
        };
        if queued {
            shared.changed();
        } else {
            agent::pump(shared);
        }
        return true;
    };
    let (name, rest) = command
        .split_once(char::is_whitespace)
        .map(|(n, r)| (n, r.trim()))
        .unwrap_or((command, ""));
    if name == "model" {
        return set_model(shared, rest);
    }
    // Commands run at once, even during a turn; only /clear waits.
    if name == "clear" && shared.lock().busy() {
        shared.toast("Can't clear while a turn runs — esc to interrupt");
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
            // A new transcript: a fresh banner under a new epoch, which the
            // screen keys its log by so an inline terminal clears as well.
            let mut s = shared.lock();
            s.entries.clear();
            s.history.clear();
            s.round = None;
            s.epoch += 1.0;
            let blocks = art::banner(&s.model, &s.cwd, &s.branch);
            s.push(Entry {
                kind: "banner".into(),
                blocks,
                ..Entry::default()
            });
        }
        "stress" => {
            // `/stress [n]` is one burst; `/stress n paced` releases the
            // same entries a screenful at a time (LLP 1101.002 P14).
            let mut words = rest.split_whitespace();
            let n = match words.next() {
                None => Some(200),
                Some(w) => w.parse::<usize>().ok(),
            };
            let paced = match words.next() {
                None => Some(false),
                Some("paced") => Some(true),
                Some(_) => None,
            };
            let (Some(n), Some(paced)) = (n, paced) else {
                shared.lock().error(&format!(
                    "/stress takes a count and optionally `paced`, not {rest:?}"
                ));
                return false;
            };
            let entries = art::stress(n.min(20_000));
            if paced {
                pace(shared, entries);
            } else {
                let mut s = shared.lock();
                for e in entries {
                    s.push(e);
                }
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

fn provider_name(provider: &str) -> &'static str {
    match provider {
        "openrouter" => "OpenRouter",
        "anthropic" => "Anthropic",
        "openai" => "OpenAI",
        _ => "That provider",
    }
}

/// `setKey(provider, key)`: keep the key, save it, and pick a model for
/// it if the session is still on the mock. `None` when refused; `Some`
/// says whether an OpenRouter key was set (the catalog is fetched again).
/// The key itself goes nowhere but the key store.
pub fn set_key(shared: &Arc<Shared>, provider: &str, key: &str) -> Option<bool> {
    let Some(name) = keys::var_for(provider) else {
        shared.toast(&format!("No provider {provider:?} takes a key"));
        return None;
    };
    let key = key.trim();
    let who = provider_name(provider);
    let (saved, model) = {
        let mut s = shared.lock();
        let saved = s.keys.set(name, key);
        s.refresh_models();
        let current = s.provider();
        if key.is_empty() && current == provider {
            s.model = "mock".into();
        } else if !key.is_empty() && s.model == "mock" {
            s.model = match provider {
                "openrouter" => catalog::default_model(&s.catalog),
                _ => s
                    .models
                    .iter()
                    .find(|m| m.provider == provider && m.available)
                    .map(|m| m.id.clone())
                    .unwrap_or_else(|| "mock".into()),
            };
        }
        let label = s
            .models
            .iter()
            .find(|m| m.id == s.model)
            .map(|m| m.label.clone())
            .unwrap_or_else(|| s.model.clone());
        (saved, label)
    };
    let toast = match (saved, key.is_empty()) {
        (Err(e), _) => format!("{who} key kept for this session; not saved ({e})"),
        (Ok(()), true) => format!("{who} key removed — model: {model}"),
        (Ok(()), false) => format!("{who} key saved — model: {model}"),
    };
    shared.toast(&toast);
    Some(provider == "openrouter" && !key.is_empty())
}

/// Entries released per paced batch: about a screenful.
pub const BATCH: usize = 20;
/// How long a paced batch waits for the host to print and retire it. A
/// host that never retires (full screen) is found out by the first batch,
/// and the rest follow it at [`PAUSE`].
const SETTLE: std::time::Duration = std::time::Duration::from_millis(1000);
/// The pause between batches once the host is known not to retire.
const PAUSE: std::time::Duration = std::time::Duration::from_millis(30);

/// Release `entries` from a thread in batches of [`BATCH`], each once the
/// host has printed and retired the one before (LLP 1101.002 §0 P14). A
/// `/clear` meanwhile ends it: the old workload does not run on into the
/// new transcript.
fn pace(shared: &Arc<Shared>, entries: Vec<Entry>) {
    let shared = shared.clone();
    std::thread::spawn(move || {
        let epoch = shared.lock().epoch;
        let mut retires = true;
        let mut entries = entries.into_iter().peekable();
        while entries.peek().is_some() {
            let last = {
                let mut s = shared.lock();
                if s.epoch != epoch {
                    return;
                }
                let mut last = String::new();
                for e in entries.by_ref().take(BATCH) {
                    last = s.push(e);
                }
                last
            };
            shared.changed();
            if !retires {
                std::thread::sleep(PAUSE);
                continue;
            }
            let until = std::time::Instant::now() + SETTLE;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(5));
                let s = shared.lock();
                if s.epoch != epoch || !s.entries.iter().any(|e| e.id == last) {
                    break;
                }
                if std::time::Instant::now() > until {
                    retires = false;
                    break;
                }
            }
        }
    });
}
