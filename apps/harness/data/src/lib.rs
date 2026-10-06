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
//! - `submit(text)`, `approve(id, choice)`, `interrupt()`, `setModel(id)`,
//!   `setKey(provider, key)`, `retire(id)` → `Ack`
//!
//! @ref LLP 1101 (terminal apps)

#![forbid(unsafe_code)]

mod agent;
mod anim;
mod art;
mod ascii_anim;
mod catalog;
mod commands;
mod highlight;
mod keys;
mod markdown;
pub mod providers;
mod settle;
pub mod shapes;
mod shell;
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

/// Where a harness keeps its configuration and what it may reach at
/// startup. [`Options::default`] is the real thing; tests go offline.
#[derive(Clone, Debug)]
pub struct Options {
    /// Where `keys` and `models.json` live; `None` keeps keys in memory.
    pub config_dir: Option<std::path::PathBuf>,
    /// OpenRouter's API base.
    pub openrouter_base: String,
    /// Fetch OpenRouter's catalog and probe Ollama in the background.
    pub network: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            config_dir: keys::default_dir(),
            openrouter_base: providers::OPENROUTER.into(),
            network: true,
        }
    }
}

impl Options {
    /// No background requests, configuration in `dir`.
    pub fn offline(dir: Option<std::path::PathBuf>) -> Options {
        Options {
            config_dir: dir,
            network: false,
            ..Options::default()
        }
    }
}

/// Fetch OpenRouter's catalog on a thread and rebuild the model list.
fn refresh_catalog(shared: &Arc<Shared>) {
    let (base, dir) = {
        let s = shared.lock();
        if !s.network {
            return;
        }
        (s.openrouter_base.clone(), s.config_dir.clone())
    };
    let shared = shared.clone();
    std::thread::spawn(move || {
        if let Ok(catalog) = catalog::fetch(&base, dir.as_deref()) {
            let mut s = shared.lock();
            s.catalog = catalog;
            s.refresh_models();
            drop(s);
            shared.changed();
        }
    });
}

impl Harness {
    /// A session in the process's working directory, with the banner.
    pub fn new() -> Harness {
        Harness::with_options(Options::default())
    }

    /// A session configured by `options`.
    pub fn with_options(options: Options) -> Harness {
        let keys = keys::Keys::load(options.config_dir.clone());
        let catalog = options
            .config_dir
            .as_deref()
            .and_then(catalog::cached)
            .unwrap_or_else(catalog::fallback);
        let cwd = home_relative(&std::env::current_dir().unwrap_or_default());
        let branch = git_branch();
        let mut state = State {
            cwd: cwd.clone(),
            branch: branch.clone(),
            phase: "idle".into(),
            keys,
            catalog,
            openrouter_base: options.openrouter_base.clone(),
            config_dir: options.config_dir.clone(),
            network: options.network,
            ..State::default()
        };
        state.refresh_models();
        state.model = providers::default_model(&state.models);
        let model = state.model.clone();
        state.push(Entry {
            kind: "banner".into(),
            blocks: art::banner(&model, &cwd, &branch),
            ..Entry::default()
        });
        let probe = (!state.keys.has("OLLAMA_HOST")).then(|| providers::ollama_host(&state.keys));
        let shared = Shared::new(state, Native::default());
        if options.network {
            refresh_catalog(&shared);
            if let Some(host) = probe {
                let shared = shared.clone();
                std::thread::spawn(move || {
                    if providers::probe_ollama(&host) {
                        let mut s = shared.lock();
                        s.ollama_up = true;
                        s.refresh_models();
                        drop(s);
                        shared.changed();
                    }
                });
            }
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
            "retire" => {
                let retired = self.shared.lock().retire(text(0)?);
                if retired {
                    self.shared.changed();
                }
                retired
            }
            "setModel" => commands::set_model(&self.shared, text(0)?),
            "setKey" => {
                let saved = commands::set_key(&self.shared, text(0)?, text(1)?);
                if saved == Some(true) {
                    refresh_catalog(&self.shared);
                }
                saved.is_some()
            }
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
