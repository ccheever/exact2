//! The model providers: which models exist and whether each is usable,
//! and one call that streams a reply as [`Event`]s. The HTTP is `ureq` on
//! the agent's thread, its body read line by line so an interrupt is seen
//! at the next line.

pub mod anthropic;
pub mod mock;
pub mod openai;

use crate::sse::{Event, Parser, Sse};
use crate::state::{ModelChoice, Msg};
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The Anthropic models offered.
pub const CLAUDE: [(&str, &str); 3] = [
    ("claude-sonnet-5-5", "Claude Sonnet 5.5"),
    ("claude-opus-5-5", "Claude Opus 5.5"),
    ("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
];

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Where an OpenAI-compatible model is served.
pub struct Endpoint {
    pub base: String,
    pub key: Option<String>,
    pub model: String,
}

/// How to reach a model: its provider and, for OpenAI-compatible ones,
/// the endpoint.
pub enum Route {
    Mock,
    Anthropic { key: String, model: String },
    OpenAi(Endpoint),
}

fn openai_model() -> String {
    env("EXACT_HARNESS_OPENAI_MODEL").unwrap_or_else(|| "gpt-5.5".into())
}

fn ollama_model() -> String {
    env("EXACT_HARNESS_OLLAMA_MODEL").unwrap_or_else(|| "llama3.2".into())
}

fn openrouter_model() -> String {
    env("EXACT_HARNESS_OPENROUTER_MODEL").unwrap_or_else(|| "openrouter/auto".into())
}

/// Ollama's address.
pub fn ollama_host() -> String {
    env("OLLAMA_HOST")
        .map(|h| {
            if h.starts_with("http") {
                h
            } else {
                format!("http://{h}")
            }
        })
        .unwrap_or_else(|| "http://localhost:11434".into())
        .trim_end_matches('/')
        .to_string()
}

/// Every model, and whether its key is set. Ollama counts as available
/// when `OLLAMA_HOST` is set; [`probe_ollama`] may find it otherwise.
pub fn models() -> Vec<ModelChoice> {
    let mut out = vec![ModelChoice {
        id: "mock".into(),
        label: "Mock (scripted, offline)".into(),
        provider: "mock".into(),
        available: true,
    }];
    let anthropic = env("ANTHROPIC_API_KEY").is_some();
    for (id, label) in CLAUDE {
        out.push(ModelChoice {
            id: id.into(),
            label: label.into(),
            provider: "anthropic".into(),
            available: anthropic,
        });
    }
    let model = openai_model();
    out.push(ModelChoice {
        label: format!("OpenAI {model}"),
        id: model,
        provider: "openai".into(),
        available: env("OPENAI_API_KEY").is_some(),
    });
    let model = openrouter_model();
    out.push(ModelChoice {
        id: format!("openrouter:{model}"),
        label: format!("OpenRouter {model}"),
        provider: "openrouter".into(),
        available: env("OPENROUTER_API_KEY").is_some(),
    });
    let model = ollama_model();
    out.push(ModelChoice {
        id: format!("ollama:{model}"),
        label: format!("Ollama {model}"),
        provider: "ollama".into(),
        available: env("OLLAMA_HOST").is_some(),
    });
    out
}

/// The model to start with: `EXACT_HARNESS_MODEL`, else the first
/// provider with a key, else the mock.
pub fn default_model(models: &[ModelChoice]) -> String {
    if let Some(m) = env("EXACT_HARNESS_MODEL") {
        if models.iter().any(|c| c.id == m) {
            return m;
        }
    }
    for provider in ["anthropic", "openai", "openrouter"] {
        if let Some(m) = models
            .iter()
            .find(|m| m.provider == provider && m.available)
        {
            return m.id.clone();
        }
    }
    "mock".into()
}

/// Whether Ollama answers at its address, within a short timeout.
pub fn probe_ollama() -> bool {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(400)))
        .build()
        .into();
    agent
        .get(format!("{}/api/tags", ollama_host()))
        .call()
        .is_ok_and(|r| r.status().as_u16() == 200)
}

/// How to reach `model` of `provider`, or why it can't be.
pub fn route(id: &str, provider: &str) -> Result<Route, String> {
    match provider {
        "mock" => Ok(Route::Mock),
        "anthropic" => Ok(Route::Anthropic {
            key: env("ANTHROPIC_API_KEY").ok_or("ANTHROPIC_API_KEY is not set")?,
            model: id.into(),
        }),
        "openai" => Ok(Route::OpenAi(Endpoint {
            base: env("OPENAI_BASE_URL").unwrap_or_else(|| "https://api.openai.com/v1".into()),
            key: Some(env("OPENAI_API_KEY").ok_or("OPENAI_API_KEY is not set")?),
            model: id.into(),
        })),
        "openrouter" => Ok(Route::OpenAi(Endpoint {
            base: "https://openrouter.ai/api/v1".into(),
            key: Some(env("OPENROUTER_API_KEY").ok_or("OPENROUTER_API_KEY is not set")?),
            model: id.trim_start_matches("openrouter:").into(),
        })),
        "ollama" => Ok(Route::OpenAi(Endpoint {
            base: format!("{}/v1", ollama_host()),
            key: None,
            model: id.trim_start_matches("ollama:").into(),
        })),
        other => Err(format!("unknown provider {other}")),
    }
}

/// One request: the conversation so far and what the model may call.
pub struct Ask<'a> {
    pub system: &'a str,
    pub history: &'a [Msg],
    pub cancel: &'a AtomicBool,
}

/// Stream one reply from `route`, handing each event to `sink`.
pub fn stream(route: &Route, ask: &Ask<'_>, sink: &mut dyn FnMut(Event)) -> Result<(), String> {
    match route {
        Route::Mock => mock::stream(ask, sink),
        Route::Anthropic { key, model } => anthropic::stream(key, model, ask, sink),
        Route::OpenAi(endpoint) => openai::stream(endpoint, ask, sink),
    }
}

/// POST `body` as JSON and read the reply as server-sent events, calling
/// `each` per event until it, the stream or `cancel` stops. A status that
/// is not 2xx is an error carrying the server's message.
pub fn post_sse(
    url: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
    cancel: &AtomicBool,
    each: &mut dyn FnMut(Sse) -> bool,
) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(300)))
        .build()
        .into();
    let mut request = agent.post(url).header("content-type", "application/json");
    for (k, v) in headers {
        request = request.header(*k, v.as_str());
    }
    let response = request
        .send(body.to_string())
        .map_err(|e| format!("request to {url} failed: {e}"))?;
    let status = response.status().as_u16();
    let mut body = response.into_body();
    if !(200..300).contains(&status) {
        let text = body.read_to_string().unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| {
                v["error"]["message"]
                    .as_str()
                    .or(v["error"].as_str())
                    .or(v["message"].as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| text.chars().take(400).collect());
        return Err(format!("HTTP {status}: {message}"));
    }
    let mut reader = std::io::BufReader::new(body.into_reader());
    let mut parser = Parser::default();
    let mut line = String::new();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let text = line.trim_end_matches(['\n', '\r']);
                if let Some(sse) = parser.line(text) {
                    if !each(sse) {
                        return Ok(());
                    }
                }
            }
            Err(e) => return Err(format!("stream broke: {e}")),
        }
    }
    if let Some(sse) = parser.flush() {
        each(sse);
    }
    Ok(())
}

/// The system prompt.
pub fn system(cwd: &str) -> String {
    format!(
        "You are a coding agent in a terminal. The working directory is {cwd}. \
         Use the tools to look before you answer; keep replies short and in Markdown. \
         Commands and file edits wait for the person's approval."
    )
}
