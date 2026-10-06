//! The coding-agent harness's data module (LLP 1101): one `session()`
//! snapshot the screen draws, the sends that change it, and pure
//! `animation(...)` frames. Its values are exactly `../shapes.contract`.
//!
//! The session is pushed: its answer watches the topic "session", and the
//! agent's threads (model streams, tools, timers) change the shared state
//! and announce the topic through the source's [`Native`] handle; the host
//! wakes and the runner asks again. Nothing starts from a resource's
//! answer, which bake calls too — only from sends.
//!
//! Sources:
//! - `session()` → `Session` (topic "session")
//! - `animation(name, tick, cols, rows)` → `Frame`
//! - `submit(text)`, `approve(id, choice)`, `interrupt()`, `setModel(id)` → `Ack`
//!
//! @ref LLP 1101 (terminal apps)

#![forbid(unsafe_code)]

mod agent;
mod anim;
mod art;
mod commands;
mod highlight;
mod markdown;
pub mod providers;
pub mod sse;
mod state;
mod tools;
mod value;

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Native, Store};
use state::{Entry, Shared, State};
use std::sync::Arc;

pub use anim::NAMES as ANIMATIONS;
pub use providers::mock::FAST as MOCK_FAST;

/// The harness's data source. Clones share one session, as [`Native`]
/// handles do: a clone kept outside the runner reaches the same state.
#[derive(Clone)]
pub struct Harness {
    shared: Arc<Shared>,
}

impl Default for Harness {
    fn default() -> Self {
        Harness::new()
    }
}

fn home_relative(path: &std::path::Path) -> String {
    let shown = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && shown.starts_with(&home) => {
            format!("~{}", &shown[home.len()..])
        }
        _ => shown,
    }
}

fn git_branch() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

impl Harness {
    /// A session in the process's working directory, with the banner.
    pub fn new() -> Harness {
        let models = providers::models();
        let model = providers::default_model(&models);
        let cwd = home_relative(&std::env::current_dir().unwrap_or_default());
        let branch = git_branch();
        let mut state = State {
            model: model.clone(),
            cwd: cwd.clone(),
            branch: branch.clone(),
            phase: "idle".into(),
            models,
            ..State::default()
        };
        state.push(Entry {
            kind: "banner".into(),
            blocks: art::banner(&model, &cwd, &branch),
            ..Entry::default()
        });
        let shared = Shared::new(state, Native::default());
        if std::env::var_os("OLLAMA_HOST").is_none() {
            let probe = shared.clone();
            std::thread::spawn(move || {
                if providers::probe_ollama() {
                    let mut s = probe.lock();
                    for m in s.models.iter_mut().filter(|m| m.provider == "ollama") {
                        m.available = true;
                    }
                    drop(s);
                    probe.changed();
                }
            });
        }
        Harness { shared }
    }

    /// The session as `shape Session`.
    pub fn session(&self) -> Value {
        value::session(&self.shared.lock())
    }

    fn call(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let text = |i: usize| -> Result<&str, DataError> {
            args.get(i)
                .and_then(Value::as_str)
                .ok_or_else(|| DataError::BadArguments(format!("{source}: argument {i} is text")))
        };
        let number = |i: usize| -> Result<f64, DataError> {
            args.get(i).and_then(Value::as_number).ok_or_else(|| {
                DataError::BadArguments(format!("{source}: argument {i} is a number"))
            })
        };
        let ok = match source {
            "session" => return Ok(self.session()),
            "animation" => {
                let (title, lines) = anim::frame(
                    text(0)?,
                    number(1)?,
                    number(2)?.max(0.0) as usize,
                    number(3)?.max(0.0) as usize,
                );
                return Ok(value::frame(&title, &lines));
            }
            "submit" => commands::submit(&self.shared, text(0)?),
            "approve" => commands::approve(&self.shared, text(0)?, text(1)?),
            "interrupt" => agent::interrupt(&self.shared),
            "setModel" => commands::set_model(&self.shared, text(0)?),
            other => return Err(DataError::UnknownSource(other.into())),
        };
        Ok(value::ack(ok))
    }
}

impl DataSource for Harness {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.call(source, args)
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if source == "session" {
            store.observe_topic("session");
        }
        self.call(source, args).map(Answer::Now)
    }

    fn native(&self) -> Option<Native> {
        Some(self.shared.native.clone())
    }
}

#[cfg(test)]
mod tests;
