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

/// Native HTTP scheduling; only an explicit source promise permits overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttpScheduling {
    /// Native transport, storage and continuations share one ordered lane.
    #[default]
    Ordered,
    /// Explicit source promise: this HTTP operation and its settlement may
    /// overlap and reorder relative to other operations, including mutations.
    /// This is not inferred from GET, origins or grants. The host still limits
    /// admission and enforces the response ceiling while receiving bytes.
    Independent {
        /// Largest accepted response body. Zero or over 64 MiB is refused.
        max_response_bytes: u32,
    },
}

/// One host request, with ordered native execution unless explicitly opted in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Native HTTP scheduling; storage and continuations must remain ordered.
    pub http: HttpScheduling,
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
    /// Native ordered lane is mandatory for storage and continuations, even
    /// when an invalid HTTP annotation will cause their admission to refuse.
    pub fn is_ordered(&self) -> bool {
        self.http == HttpScheduling::Ordered
            || self.storage.is_some()
            || self.continuation.is_some()
    }

    /// A `GET`.
    pub fn get(url: &str) -> Request {
        Request {
            http: HttpScheduling::Ordered,
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
            http: HttpScheduling::Ordered,
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

    /// Opt HTTP into bounded independent transport. The source promises that
    /// neither the external effect nor parsing the reply needs FIFO ordering.
    /// Storage and continuation requests with this annotation are refused.
    pub fn independent_http(mut self, max_response_bytes: u32) -> Self {
        self.http = HttpScheduling::Independent { max_response_bytes };
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

#[cfg(test)]
mod scheduling_tests {
    use super::*;

    #[test]
    fn http_is_ordered_until_the_source_explicitly_opts_in() {
        for request in [
            Request::get("https://example.test"),
            Request::post_json("https://example.test", "{}"),
            Request::continuation(1),
            Request::storage(vec![]),
        ] {
            assert_eq!(request.http, HttpScheduling::Ordered);
        }
        let request = Request::get("https://example.test").independent_http(4096);
        assert_eq!(
            request.http,
            HttpScheduling::Independent {
                max_response_bytes: 4096
            }
        );
    }
}
