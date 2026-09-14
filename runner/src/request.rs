//! A request the host runs, and what comes back.
//!
//! @ref LLP 1016 D1 (a data source answers now, or hands back a request) /
//! D2 (the host executes; a ticket names the reply) / D4 (failure is data)
//!
//! `Request` and `Response` are the runner's own two structs — the fields of
//! ibex2's `stdlib::fetch::{Request, Response}` minus what a plan runner
//! does not decide — because the runner builds for wasm and depends on
//! nothing; a host converts, one line each way.

use exact_plan::Value;

/// A data source's answer: a value now, or a request for the host.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The value, now.
    Now(Value),
    /// The host runs this; `parse` reads what comes back.
    Later(Request),
}

impl From<Value> for Answer {
    fn from(v: Value) -> Self {
        Answer::Now(v)
    }
}

/// A request for the host to run (LLP 1016 D1): the fields of ibex2's
/// `Request` a plan runner decides — not its redirect mode, not the final URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Executor-local continuation token, not an HTTP request. The browser
    /// drains its module's microtasks; native hosts take source-owned worker work.
    pub continuation: Option<u64>,
    /// Portable storage operation; independent of HTTP and native closures.
    pub storage: Option<Vec<u8>>,
    /// Admitted source scope inside a mixed app; may only narrow host grants.
    pub grants: Option<String>,
    /// `GET`, `POST`, …
    pub method: String,
    /// The URL.
    pub url: String,
    /// Header name–value pairs.
    pub headers: Vec<(String, String)>,
    /// The body bytes (empty for a `GET`).
    pub body: Vec<u8>,
}

impl Request {
    /// A `GET`.
    pub fn get(url: &str) -> Request {
        Request {
            continuation: None,
            storage: None,
            grants: None,
            method: "GET".into(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// A `POST` of a JSON text.
    pub fn post_json(url: &str, json: &str) -> Request {
        Request {
            continuation: None,
            storage: None,
            grants: None,
            method: "POST".into(),
            url: url.into(),
            headers: vec![("content-type".into(), "application/json".into())],
            body: json.as_bytes().to_vec(),
        }
    }

    /// Host storage work as a bounded protocol payload (LLP 1027.001 D2).
    pub fn storage(payload: Vec<u8>) -> Self {
        Self {
            storage: Some(payload),
            ..Self::get("")
        }
    }

    /// With a header.
    pub fn header(mut self, name: &str, value: &str) -> Request {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Yield to a host-owned executor without inventing a network URL or grant.
    pub fn continuation(token: u64) -> Self {
        Self {
            continuation: Some(token),
            ..Self::get("")
        }
    }
}

/// What the host brought back for a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The HTTP status.
    pub status: u16,
    /// Header name–value pairs, as received.
    pub headers: Vec<(String, String)>,
    /// The body bytes.
    pub body: Vec<u8>,
}

/// A request's outcome: a response (any status), or no response at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A portable storage operation completed (LLP 1027.001 D2).
    Storage(Vec<u8>),
    /// The server answered.
    Response(Response),
    /// Nothing came back: the executor says why.
    Failed {
        /// The kind.
        kind: FailureKind,
        /// The executor's message.
        message: String,
    },
}

/// Why a request produced no response.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// No connection, TLS, a rejected fetch.
    Network,
    /// Outside the app's grant (LLP 1016 D6).
    Refused,
    /// The host has no executor (Linux before its transport).
    Unsupported,
    /// The executor aborted it.
    Aborted,
}

/// Where a module instance runs (LLP 1027.002 D1): on the runner's thread,
/// or on an owner of its own. `Main` is the default and, for a source that
/// never says otherwise, the only behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    /// The runner's thread: answers may be `Now`, inside the transaction.
    #[default]
    Main,
    /// An owner the host runs for the module: every answer is `Later`, its
    /// turn runs against a snapshot, and the runner commits its writes.
    Worker,
}

impl Placement {
    /// The manifest's spelling, `main` or `worker`.
    pub fn parse(name: &str) -> Option<Placement> {
        match name {
            "main" => Some(Placement::Main),
            "worker" => Some(Placement::Worker),
            _ => None,
        }
    }

    /// The manifest's spelling.
    pub fn name(self) -> &'static str {
        match self {
            Placement::Main => "main",
            Placement::Worker => "worker",
        }
    }
}

/// The reply to host work that finishes on another owner (LLP 1027.002 D3):
/// `send` once, when the outcome exists. Dropped unsent — the owner died
/// mid-turn, or never took the job — it reports `Aborted`, so the ticket
/// still ends and a queue behind it can move.
pub struct Reply(Option<Box<dyn FnOnce(Outcome) + Send>>);

impl Reply {
    /// A reply that delivers through `deliver`, once.
    pub fn new(deliver: impl FnOnce(Outcome) + Send + 'static) -> Reply {
        Reply(Some(Box::new(deliver)))
    }

    /// Deliver the outcome.
    pub fn send(mut self, outcome: Outcome) {
        if let Some(deliver) = self.0.take() {
            deliver(outcome);
        }
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        if let Some(deliver) = self.0.take() {
            deliver(Outcome::Failed {
                kind: FailureKind::Aborted,
                message: "the owner ended without a reply".into(),
            });
        }
    }
}

/// Host work behind a continuation token, as `DataSource::dispatch` hands
/// it out.
pub enum Work {
    /// Runs on the host's I/O worker; what it returns is the outcome.
    Now(Box<dyn FnOnce() -> Outcome + Send>),
    /// Hands the reply to another owner and returns at once; the outcome
    /// arrives when that owner sends it. The I/O worker is never held
    /// while a module computes (LLP 1027.002 D4).
    Later(Box<dyn FnOnce(Reply) + Send>),
}

/// What a continuation token is at dispatch — asked on the runner's thread,
/// after the commit that handed the request out, with the store as
/// committed then (LLP 1027.002 D3).
pub enum Dispatch {
    /// Work for the host to run.
    Run(Work),
    /// The host's own executor runs its registry token its way (the
    /// browser's turn registry).
    Host(u64),
    /// Not yet: the source holds it — a turn is reserved ahead of it — and
    /// releases it from `DataSource::release` after a later commit.
    Held,
    /// No work: the token is unknown or already consumed. The host refuses
    /// the request as it does a missing continuation.
    Missing,
}

/// A request the host is to run: its ticket, the resource or mutation it
/// answers, and the request. Taken by [`Runner::take_requests`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestOut {
    /// Names the reply: [`Runner::fulfill`] takes it back.
    pub ticket: u64,
    /// The resource's or mutation's name.
    pub target: String,
    /// What to run.
    pub request: Request,
    /// The app forced it (`refresh`): the executor bypasses its cache.
    pub forced: bool,
}
