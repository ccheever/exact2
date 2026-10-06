//! The model providers: which models exist and whether each is usable,
//! and one call that streams a reply as [`Event`]s. The HTTP is `ureq`; a
//! reader thread hands the body's lines over a channel, so an interrupt is
//! seen within 100 ms and a stream silent for two minutes fails.

pub mod anthropic;
pub mod mock;
pub mod openai;

use crate::keys::{self, Keys};
use crate::sse::{Event, Parser, Sse};
use crate::state::{ModelChoice, Msg};
use std::io::BufRead;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, DefaultConnector, NextTimeout, Transport,
};

/// The Anthropic models offered.
pub const CLAUDE: [(&str, &str); 3] = [
    ("claude-sonnet-5-5", "Claude Sonnet 5.5"),
    ("claude-opus-5-5", "Claude Opus 5.5"),
    ("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
];

/// OpenRouter's API.
pub const OPENROUTER: &str = "https://openrouter.ai/api/v1";

const CONNECT: Duration = Duration::from_secs(30);
/// The longest one socket read waits before looking at the cancel flag.
const SLICE: Duration = Duration::from_millis(200);

/// HTTP body readers alive now (tests watch it).
pub static HTTP_READERS: AtomicUsize = AtomicUsize::new(0);

struct Alive;

impl Drop for Alive {
    fn drop(&mut self) {
        HTTP_READERS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The last link of the connector chain: wraps the connection (TLS or
/// plain) so every read waits at most [`SLICE`] at a time and checks the
/// cancel flag between slices. A cancelled read fails, so the body reader
/// and its thread end within a slice of an interrupt, even when the server
/// has gone silent.
#[derive(Debug)]
struct CancelConnector(Arc<AtomicBool>);

impl Connector<Box<dyn Transport>> for CancelConnector {
    type Out = CancelTransport;

    fn connect(
        &self,
        _: &ConnectionDetails,
        chained: Option<Box<dyn Transport>>,
    ) -> Result<Option<CancelTransport>, ureq::Error> {
        Ok(chained.map(|inner| CancelTransport {
            inner,
            cancel: self.0.clone(),
        }))
    }
}

#[derive(Debug)]
struct CancelTransport {
    inner: Box<dyn Transport>,
    cancel: Arc<AtomicBool>,
}

impl Transport for CancelTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.inner.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        let deadline = (!timeout.after.is_not_happening()).then(|| Instant::now() + *timeout.after);
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                // Not `Interrupted`, which `read_line` would retry.
                return Err(ureq::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "cancelled",
                )));
            }
            let left = deadline.map(|d| d.saturating_duration_since(Instant::now()));
            let slice = left.map_or(SLICE, |l| l.min(SLICE));
            let last = left.is_some_and(|l| l <= SLICE);
            let next = NextTimeout {
                after: slice.into(),
                reason: timeout.reason,
            };
            match self.inner.await_input(next) {
                Err(ureq::Error::Timeout(_)) if !last => continue,
                other => return other,
            }
        }
    }

    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }

    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}
/// The longest a stream may go without a line.
const IDLE: Duration = Duration::from_secs(120);

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Where an OpenAI-compatible model is served.
pub struct Endpoint {
    pub base: String,
    pub key: Option<String>,
    pub model: String,
    /// Headers besides the key (OpenRouter's attribution).
    pub headers: Vec<(&'static str, String)>,
    /// Fields added to the request body (OpenRouter's usage accounting).
    pub extra: serde_json::Value,
}

/// How to reach a model: its provider and, for OpenAI-compatible ones,
/// the endpoint.
pub enum Route {
    Mock,
    Anthropic { key: String, model: String },
    OpenAi(Endpoint),
}

/// What the model list and the routes are made from.
pub struct Sources<'a> {
    pub keys: &'a Keys,
    pub ollama_up: bool,
    pub catalog: &'a [ModelChoice],
    pub openrouter_base: &'a str,
}

fn openai_model() -> String {
    env("EXACT_HARNESS_OPENAI_MODEL").unwrap_or_else(|| "gpt-5.5".into())
}

fn ollama_model() -> String {
    env("EXACT_HARNESS_OLLAMA_MODEL").unwrap_or_else(|| "llama3.2".into())
}

/// Ollama's address, from the keys or the default.
pub fn ollama_host(keys: &Keys) -> String {
    keys.get("OLLAMA_HOST")
        .map(|h| {
            if h.starts_with("http") {
                h.to_string()
            } else {
                format!("http://{h}")
            }
        })
        .unwrap_or_else(|| "http://localhost:11434".into())
        .trim_end_matches('/')
        .to_string()
}

/// Every model: the direct providers (mock, Anthropic, OpenAI, Ollama),
/// then OpenRouter's catalog, each available when its key is set.
pub fn models(src: &Sources<'_>) -> Vec<ModelChoice> {
    let mut out = vec![ModelChoice {
        id: "mock".into(),
        label: "Mock (scripted, offline)".into(),
        provider: "mock".into(),
        available: true,
    }];
    let anthropic = src.keys.has("ANTHROPIC_API_KEY");
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
        available: src.keys.has("OPENAI_API_KEY"),
    });
    let model = ollama_model();
    out.push(ModelChoice {
        id: format!("ollama:{model}"),
        label: format!("Ollama {model}"),
        provider: "ollama".into(),
        available: src.keys.has("OLLAMA_HOST") || src.ollama_up,
    });
    let openrouter = src.keys.has("OPENROUTER_API_KEY");
    out.extend(src.catalog.iter().map(|c| ModelChoice {
        available: openrouter,
        ..c.clone()
    }));
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
    for provider in ["anthropic", "openai"] {
        if let Some(m) = models
            .iter()
            .find(|m| m.provider == provider && m.available)
        {
            return m.id.clone();
        }
    }
    if models
        .iter()
        .any(|m| m.provider == "openrouter" && m.available)
    {
        let catalog: Vec<ModelChoice> = models
            .iter()
            .filter(|m| m.provider == "openrouter")
            .cloned()
            .collect();
        return crate::catalog::default_model(&catalog);
    }
    "mock".into()
}

/// Whether Ollama answers at its address, within a short timeout.
pub fn probe_ollama(host: &str) -> bool {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(400)))
        .build()
        .into();
    agent
        .get(format!("{host}/api/tags"))
        .call()
        .is_ok_and(|r| r.status().as_u16() == 200)
}

fn key(keys: &Keys, name: &str) -> Result<String, String> {
    keys.get(name)
        .map(str::to_string)
        .ok_or_else(|| format!("{name} is not set"))
}

/// How to reach model `id` of `provider`, or why it can't be.
pub fn route(id: &str, provider: &str, src: &Sources<'_>) -> Result<Route, String> {
    let keys = src.keys;
    let endpoint = |base: String, key: Option<String>, model: &str| Endpoint {
        base,
        key,
        model: model.into(),
        headers: Vec::new(),
        extra: serde_json::json!({}),
    };
    match provider {
        "mock" => Ok(Route::Mock),
        "anthropic" => Ok(Route::Anthropic {
            key: key(keys, "ANTHROPIC_API_KEY")?,
            model: id.into(),
        }),
        "openai" => Ok(Route::OpenAi(endpoint(
            env("OPENAI_BASE_URL").unwrap_or_else(|| "https://api.openai.com/v1".into()),
            Some(key(keys, "OPENAI_API_KEY")?),
            id,
        ))),
        "openrouter" => {
            let mut e = endpoint(
                src.openrouter_base.to_string(),
                Some(key(keys, "OPENROUTER_API_KEY")?),
                id.trim_start_matches("openrouter:"),
            );
            e.headers = vec![
                ("HTTP-Referer", "https://github.com/ccheever/exact2".into()),
                ("X-Title", "Exact harness".into()),
            ];
            e.extra = serde_json::json!({"usage": {"include": true}});
            Ok(Route::OpenAi(e))
        }
        "ollama" => Ok(Route::OpenAi(endpoint(
            format!("{}/v1", ollama_host(keys)),
            None,
            id.trim_start_matches("ollama:"),
        ))),
        other => Err(format!("unknown provider {other}")),
    }
}

/// One request: the conversation so far and what the model may call.
pub struct Ask<'a> {
    pub system: &'a str,
    pub history: &'a [Msg],
    pub cancel: &'a Arc<AtomicBool>,
}

/// Stream one reply from `route`, handing each event to `sink`.
pub fn stream(route: &Route, ask: &Ask<'_>, sink: &mut dyn FnMut(Event)) -> Result<(), String> {
    match route {
        Route::Mock => mock::stream(ask, sink),
        Route::Anthropic { key, model } => anthropic::stream(key, model, ask, sink),
        Route::OpenAi(endpoint) => openai::stream(endpoint, ask, sink),
    }
}

/// The server's message from an error body: OpenAI's, Anthropic's and
/// OpenRouter's `{"error":{"message":…}}`, with OpenRouter's upstream
/// provider and raw message when it gives them.
pub fn error_message(text: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return text.trim().chars().take(400).collect();
    };
    let e = &v["error"];
    let mut message = e["message"]
        .as_str()
        .or(e.as_str())
        .or(v["message"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| text.chars().take(400).collect());
    let meta = &e["metadata"];
    if let Some(p) = meta["provider_name"].as_str() {
        message.push_str(&format!(" ({p})"));
    }
    if let Some(raw) = meta["raw"].as_str() {
        let raw: String = raw.chars().take(200).collect();
        message.push_str(&format!(": {raw}"));
    }
    message
}

/// POST `body` as JSON and read the reply as server-sent events, calling
/// `each` per event until it, the stream or `cancel` stops. A status that
/// is not 2xx is an error carrying the server's message. No error carries
/// a header's value.
pub fn post_sse(
    url: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
    cancel: &Arc<AtomicBool>,
    each: &mut dyn FnMut(Sse) -> bool,
) -> Result<(), String> {
    let secrets: Vec<String> = headers
        .iter()
        .flat_map(|(_, v)| [v.clone(), v.trim_start_matches("Bearer ").to_string()])
        .filter(|v| v.len() >= 6)
        .collect();
    post(url, headers, body, cancel, each).map_err(|e| keys::scrub(&e, &secrets))
}

fn post(
    url: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
    cancel: &Arc<AtomicBool>,
    each: &mut dyn FnMut(Sse) -> bool,
) -> Result<(), String> {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(CONNECT))
        .timeout_recv_response(Some(IDLE))
        .build();
    let connector = DefaultConnector::new().chain(CancelConnector(cancel.clone()));
    let agent = ureq::Agent::with_parts(config, connector, DefaultResolver::default());
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
        return Err(format!("HTTP {status}: {}", error_message(&text)));
    }
    // The body's lines arrive over a channel, so a silent server can't
    // keep an interrupt or the idle limit from being seen.
    let (tx, rx) = mpsc::sync_channel::<Result<String, String>>(64);
    let reader = body.into_reader();
    HTTP_READERS.fetch_add(1, Ordering::SeqCst);
    std::thread::spawn(move || {
        let _alive = Alive;
        let mut reader = std::io::BufReader::new(reader);
        loop {
            let mut line = String::new();
            let item = match reader.read_line(&mut line) {
                Ok(0) => return,
                Ok(_) => Ok(line),
                Err(e) => Err(format!("stream broke: {e}")),
            };
            let stop = item.is_err();
            if tx.send(item).is_err() || stop {
                return;
            }
        }
    });
    let mut parser = Parser::default();
    let mut last = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(line)) => {
                last = Instant::now();
                let text = line.trim_end_matches(['\n', '\r']);
                if let Some(sse) = parser.line(text) {
                    if !each(sse) {
                        return Ok(());
                    }
                }
            }
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if last.elapsed() > IDLE {
                    return Err(format!("no data from the server for {} s", IDLE.as_secs()));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if let Some(sse) = parser.flush() {
        each(sse);
    }
    Ok(())
}

/// The system prompt.
pub fn system(cwd: &str) -> String {
    let tools: Vec<&str> = crate::tools::definitions()
        .iter()
        .map(|(name, _, _)| *name)
        .collect();
    format!(
        "You are a coding agent running in the user's terminal, in the directory {cwd}. \
         Tools: {}. Use them to look at files and run commands rather than guessing; \
         bash, write_file and edit_file wait for the user's approval. \
         Answer in short Markdown.",
        tools.join(", ")
    )
}
