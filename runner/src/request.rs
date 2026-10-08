//! A request the host runs, and what comes back.
//!
//! @ref LLP 1016 D1 (a data source answers now, or hands back a request) /
//! D2 (the host executes; a ticket names the reply) / D4 (failure is data)
//!
//! `Request` and `Response` are the runner's own two structs — the fields of
//! ibex2's `stdlib::fetch::{Request, Response}` minus what a plan runner
//! does not decide — because the runner builds for wasm and depends on
//! nothing; a host converts, one line each way.

use exact_num::text;
use exact_plan::Value;

/// Maximum bytes in one portable host-work request or outcome.
pub const MAX_HOST_WORK_BYTES: usize = 16 << 20;

/// Grants understood by the platform I/O executor. Surface capabilities are
/// enforced by the presenter, and device grants by the OS and the capability
/// that asks (LLP 1069.008 D3); neither may make an older I/O parser reject
/// the otherwise independent filesystem, network, database, or secret grants.
pub fn io_grants(spec: &str) -> String {
    spec.lines()
        .map(str::trim)
        .filter(|line| !crate::grants::own(line) && !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A data source's answer: a value now, or a request for the host.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The value, now.
    Now(Value),
    /// The host runs this; `parse` reads what comes back.
    Later(Request),
}

impl Answer {
    /// A stream (LLP 1016.000 D1): the host opens `request` and delivers
    /// each message it carries to the same ticket, and every message is
    /// parsed and commits as its own settlement. A stream is never on the
    /// ordered lane (it would hold everything behind it), so answering one is
    /// the explicit promise independent HTTP needs; a request still ordered
    /// takes a 1 MiB per-message ceiling. A `Later` whose request says
    /// `stream` is the same answer: forwarding sources carry it unchanged.
    pub fn stream(mut request: Request) -> Answer {
        request.stream = true;
        if request.http == HttpScheduling::Ordered {
            request.http = HttpScheduling::Independent {
                max_response_bytes: 1 << 20,
            };
        }
        Answer::Later(request)
    }
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
    /// A named surface capture or restore, executed by its owning presenter.
    pub surface: Option<Box<SurfaceRequest>>,
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
    /// An answer that keeps coming (LLP 1016.000): the host delivers each
    /// message as an [`Outcome::Message`], and anything else ends it.
    /// Set by [`Answer::stream`].
    pub stream: bool,
    /// A deadline for the whole exchange, in milliseconds: headers and body.
    /// The host cancels the request when it passes and replies
    /// [`FailureKind::Timeout`]. `None` keeps the host's own limits (on Apple,
    /// URLSession's 60-second idle timeout). A stream has no deadline.
    /// Set by TypeScript's `fetch(url, {exactTimeout})` and [`Request::timeout`].
    pub timeout_ms: Option<u32>,
    /// The body is this app file's bytes (`app:/…`), read by the host when it
    /// runs the request, under the `fs.read` grant, at most
    /// [`MAX_BODY_FROM_BYTES`]: the bytes never cross the source. `body` is
    /// then empty. Set by TypeScript's `fetch(url, {exactBodyFrom})` and
    /// [`Request::body_from`] (LLP 1108 D6 R2).
    pub body_from: Option<String>,
}

/// The largest file [`Request::body_from`] sends: 64 MiB.
pub const MAX_BODY_FROM_BYTES: u64 = 64 << 20;

/// The longest request deadline a source may ask for: one hour.
pub const MAX_TIMEOUT_MS: u32 = 3_600_000;

/// The URL of a long native call (`native.later` in TypeScript): not HTTP.
/// A host hands its body to the source's [`crate::Native`] handler (the app's
/// native module; on the web, its page module) instead of the network.
pub const NATIVE_URL: &str = "exact-native:";

impl Request {
    /// A long native call with this JSON body (`native.later`'s request, for
    /// a Rust source): the independent lane with a 1 MiB answer (LLP 1067
    /// D3). The answer is an HTTP-shaped outcome: 200 carries the JSON reply.
    pub fn native(body: Vec<u8>) -> Request {
        Request {
            http: HttpScheduling::Independent {
                max_response_bytes: 1 << 20,
            },
            continuation: None,
            storage: None,
            surface: None,
            grants: None,
            method: "POST".into(),
            url: NATIVE_URL.into(),
            headers: Vec::new(),
            body,
            stream: false,
            timeout_ms: None,
            body_from: None,
        }
    }

    /// An auth session (`openAuthSession`, LLP 1069.006 D1): the body is
    /// [`crate::auth::Session`]'s JSON; the independent lane, a 64 KiB reply.
    pub fn auth(body: Vec<u8>) -> Request {
        Request {
            url: crate::auth::AUTH_URL.into(),
            http: HttpScheduling::Independent {
                max_response_bytes: 64 << 10,
            },
            ..Request::native(body)
        }
    }

    /// An auth session, for the host's auth arm, not the network.
    pub fn is_auth(&self) -> bool {
        self.url == crate::auth::AUTH_URL
            && self.continuation.is_none()
            && self.storage.is_none()
            && self.surface.is_none()
    }

    /// A long native call, for the source's native handler, not the network.
    pub fn is_native(&self) -> bool {
        self.url == NATIVE_URL
            && self.continuation.is_none()
            && self.storage.is_none()
            && self.surface.is_none()
    }

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
            surface: None,
            grants: None,
            method: "GET".into(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
            stream: false,
            timeout_ms: None,
            body_from: None,
        }
    }

    /// A `POST` of a JSON text.
    pub fn post_json(url: &str, json: &str) -> Request {
        Request {
            http: HttpScheduling::Ordered,
            continuation: None,
            storage: None,
            surface: None,
            grants: None,
            method: "POST".into(),
            url: url.into(),
            headers: vec![("content-type".into(), "application/json".into())],
            body: json.as_bytes().to_vec(),
            stream: false,
            timeout_ms: None,
            body_from: None,
        }
    }

    /// Host storage work as a bounded protocol payload (LLP 1027.001 D2).
    pub fn storage(payload: Vec<u8>) -> Self {
        Self {
            storage: Some(payload),
            ..Self::get("")
        }
    }

    /// Capture the complete carried state of one named live surface.
    pub fn capture_surface(name: impl Into<String>) -> Self {
        Self {
            surface: Some(Box::new(SurfaceRequest::Capture { name: name.into() })),
            ..Self::get("")
        }
    }

    /// Restore one named live surface from its complete carried state.
    pub fn restore_surface(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            surface: Some(Box::new(SurfaceRequest::Restore {
                name: name.into(),
                bytes,
            })),
            ..Self::get("")
        }
    }

    /// Check this surface request against its admitted app grants and optional
    /// source scope. Scopes can only select complete lines already admitted.
    pub fn check_surface_grant(&self, admitted: &str) -> Result<(), String> {
        let Some(surface) = self.surface.as_deref() else {
            return Err("request is not surface work".into());
        };
        if self.continuation.is_some()
            || self.storage.is_some()
            || self.method != "GET"
            || !self.url.is_empty()
            || !self.headers.is_empty()
            || !self.body.is_empty()
        {
            return Err("surface request combines multiple host-work kinds".into());
        }
        let capability = match surface {
            SurfaceRequest::Capture { name } => format!("surface.read {name}"),
            SurfaceRequest::Restore { name, .. } => format!("surface.write {name}"),
        };
        let admitted: Vec<_> = admitted
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let effective: Vec<_> = self.grants.as_deref().map_or_else(
            || admitted.clone(),
            |scope| {
                scope
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .collect()
            },
        );
        if self.grants.is_some()
            && effective
                .iter()
                .any(|line| !admitted.iter().any(|allowed| allowed == line))
        {
            return Err("surface request scope exceeds the app grants".into());
        }
        effective
            .iter()
            .any(|line| *line == capability)
            .then_some(())
            .ok_or_else(|| format!("outside the app's grants ({capability})"))
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

    /// With a deadline for the whole exchange (see [`Request::timeout_ms`]).
    pub fn timeout(mut self, ms: u32) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// With its body read from the app file `path` when the host runs it
    /// (see [`Request::body_from`]): `fetch(url, {exactBodyFrom})`.
    pub fn body_from(mut self, path: impl Into<String>) -> Self {
        self.body_from = Some(path.into());
        self
    }

    /// Why this request's [`Request::body_from`] is refused, if it is: a
    /// path that is not `app:/`, a body beside it, a `GET` or `HEAD`, or work
    /// that is not HTTP. The path's grant and the file are the host's to check.
    pub fn body_from_refusal(&self) -> Option<&'static str> {
        let path = self.body_from.as_deref()?;
        if !path.starts_with("app:/") {
            Some("exactBodyFrom needs an app:/ path")
        } else if !self.body.is_empty() {
            Some("exactBodyFrom: a request has one body, body or exactBodyFrom")
        } else if ["GET", "HEAD"]
            .iter()
            .any(|m| self.method.eq_ignore_ascii_case(m))
        {
            Some("exactBodyFrom: a GET or HEAD request cannot have a body")
        } else if self.storage.is_some()
            || self.continuation.is_some()
            || self.surface.is_some()
            || self.is_native()
            || self.is_auth()
        {
            Some("only HTTP takes exactBodyFrom")
        } else {
            None
        }
    }

    /// Why this request's deadline is refused, if it is: zero, over
    /// [`MAX_TIMEOUT_MS`], on a stream, or on work that is not HTTP.
    pub fn timeout_refusal(&self) -> Option<&'static str> {
        let ms = self.timeout_ms?;
        if ms == 0 || ms > MAX_TIMEOUT_MS {
            Some("a request timeout must be 1 to 3600000 ms")
        } else if self.stream {
            Some("a stream has no timeout")
        } else if self.storage.is_some()
            || self.continuation.is_some()
            || self.surface.is_some()
            || self.is_native()
            || self.is_auth()
        {
            Some("only HTTP takes a timeout")
        } else {
            None
        }
    }

    /// Yield to a host-owned executor without inventing a network URL or grant.
    pub fn continuation(token: u64) -> Self {
        Self {
            continuation: Some(token),
            ..Self::get("")
        }
    }
}

/// Presenter-owned work against one named surface in the current session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceRequest {
    /// Return the complete bytes from the surface's `carry` method.
    Capture {
        /// The authored canvas surface name.
        name: String,
    },
    /// Open complete bytes with the surface's `restore` method.
    Restore {
        /// The authored canvas surface name.
        name: String,
        /// The complete carried state; hosts never project or truncate it.
        bytes: Vec<u8>,
    },
}

/// The result of presenter-owned surface work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceOutcome {
    /// The complete carried state.
    Captured(Vec<u8>),
    /// Restore committed successfully.
    Restored,
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

/// One message of an answer that keeps coming: a server-sent event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    /// The event's type (`event:`); empty is `message`.
    pub event: String,
    /// The last event id (`id:`), the cursor a re-ask sends back as
    /// `Last-Event-ID` (LLP 1016.000 D6); empty when the stream set none.
    pub id: String,
    /// The event's data (`data:` lines joined by newlines).
    pub data: String,
    /// Messages the host dropped for this one because the runner had not
    /// taken them yet (LLP 1016.000 D4: display data coalesces to the
    /// newest). Non-zero is a gap a log re-asks across from its cursor.
    pub coalesced: u32,
}

/// A request's outcome: a response (any status), or no response at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A portable storage operation completed (LLP 1027.001 D2).
    Storage(Vec<u8>),
    /// Named surface work completed.
    Surface(SurfaceOutcome),
    /// The server answered.
    Response(Response),
    /// One message of a stream (LLP 1016.000 D1): the stream stays open.
    /// Every other outcome delivered to a stream's ticket ends it.
    Message(Message),
    /// Nothing came back: the executor says why.
    Failed {
        /// The kind.
        kind: FailureKind,
        /// The executor's message.
        message: String,
    },
}

impl Outcome {
    /// What came back, for the journal: a status and a size, or why nothing
    /// did, cut to 200 characters — never a body. A failure the app catches
    /// is still on the record (LLP 1016 D4).
    pub fn summary(&self) -> String {
        match self {
            Outcome::Response(r) => text!("HTTP {}, {} bytes", r.status, r.body.len()),
            Outcome::Message(m) if m.coalesced > 0 => {
                text!("message, {} bytes, {} coalesced", m.data.len(), m.coalesced)
            }
            Outcome::Message(m) => text!("message, {} bytes", m.data.len()),
            Outcome::Failed { kind, message } => {
                let cut = message
                    .char_indices()
                    .nth(200)
                    .map_or(message.len(), |(i, _)| i);
                let more = if cut < message.len() { "…" } else { "" };
                // Variant names are ASCII: ASCII lowering is the whole
                // lowering, and links no Unicode case tables.
                let kind = format!("{kind:?}").to_ascii_lowercase();
                format!("{kind}: {}{more}", &message[..cut])
            }
            Outcome::Storage(bytes) => text!("storage, {} bytes", bytes.len()),
            Outcome::Surface(SurfaceOutcome::Captured(bytes)) => {
                text!("surface captured, {} bytes", bytes.len())
            }
            Outcome::Surface(SurfaceOutcome::Restored) => "surface restored".into(),
        }
    }
}

/// Why a request produced no response.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// No connection, TLS, a rejected fetch.
    Network,
    /// Outside the app's grant (LLP 1016 D6) or its limits: admission, or a
    /// response over its size limit, on every host (LLP 1109 D3).
    Refused,
    /// The host has no executor (Linux before its transport).
    Unsupported,
    /// The executor aborted it.
    Aborted,
    /// Its deadline ([`Request::timeout_ms`]) passed; the host cancelled it.
    Timeout,
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
/// still ends and a queue behind it can move. A stream's reply
/// ([`Reply::stream`]) takes any number of `message`s before its `send`.
pub struct Reply(Option<Deliver>);

enum Deliver {
    Once(Box<dyn FnOnce(Outcome) + Send>),
    Many(Box<dyn FnMut(Outcome) + Send>),
}

impl Reply {
    /// A reply that delivers through `deliver`, once.
    pub fn new(deliver: impl FnOnce(Outcome) + Send + 'static) -> Reply {
        Reply(Some(Deliver::Once(Box::new(deliver))))
    }

    /// A stream's reply (LLP 1016.000 D1): each `message` goes through
    /// `deliver` and leaves it open; `send`, or a drop, ends it.
    pub fn stream(deliver: impl FnMut(Outcome) + Send + 'static) -> Reply {
        Reply(Some(Deliver::Many(Box::new(deliver))))
    }

    /// Deliver one message. The reply stays open unless it answers once
    /// ([`Reply::new`]): then the message is its one answer.
    pub fn message(&mut self, message: Message) {
        match self.0.take() {
            Some(Deliver::Many(mut deliver)) => {
                deliver(Outcome::Message(message));
                self.0 = Some(Deliver::Many(deliver));
            }
            Some(Deliver::Once(deliver)) => deliver(Outcome::Message(message)),
            None => {}
        }
    }

    /// Deliver the outcome; for a stream, the last one.
    pub fn send(mut self, outcome: Outcome) {
        match self.0.take() {
            Some(Deliver::Once(deliver)) => deliver(outcome),
            Some(Deliver::Many(mut deliver)) => deliver(outcome),
            None => {}
        }
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        if let Some(deliver) = self.0.take() {
            let aborted = Outcome::Failed {
                kind: FailureKind::Aborted,
                message: "the owner ended without a reply".into(),
            };
            match deliver {
                Deliver::Once(deliver) => deliver(aborted),
                Deliver::Many(mut deliver) => deliver(aborted),
            }
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

#[cfg(test)]
mod summary_tests {
    use super::*;

    #[test]
    fn a_summary_is_the_text_format_wrote() {
        for (status, len) in [(200, 0), (404, 5), (u16::MAX, 70_000)] {
            let outcome = Outcome::Response(Response {
                status,
                headers: vec![],
                body: vec![0; len],
            });
            assert_eq!(outcome.summary(), format!("HTTP {status}, {len} bytes"));
            let storage = Outcome::Storage(vec![1; len]);
            assert_eq!(storage.summary(), format!("storage, {len} bytes"));
            let captured = Outcome::Surface(SurfaceOutcome::Captured(vec![2; len]));
            assert_eq!(captured.summary(), format!("surface captured, {len} bytes"));
        }
    }

    #[test]
    fn a_failure_summary_lowers_its_kind_as_unicode_would() {
        for kind in [
            FailureKind::Network,
            FailureKind::Refused,
            FailureKind::Unsupported,
            FailureKind::Aborted,
            FailureKind::Timeout,
        ] {
            // Every variant is listed: adding one breaks this match.
            match kind {
                FailureKind::Network
                | FailureKind::Refused
                | FailureKind::Unsupported
                | FailureKind::Aborted
                | FailureKind::Timeout => {}
            }
            let name = format!("{kind:?}");
            assert!(name.is_ascii(), "{name}");
            let outcome = Outcome::Failed {
                kind,
                message: "no".into(),
            };
            assert_eq!(outcome.summary(), format!("{}: no", name.to_lowercase()));
        }
    }
}

#[cfg(test)]
mod reply_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Seen = Arc<Mutex<Vec<String>>>;

    fn recorder() -> (Seen, impl FnMut(Outcome) + Send + 'static) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let into = seen.clone();
        (seen, move |o: Outcome| {
            into.lock().unwrap().push(o.summary())
        })
    }

    #[test]
    fn a_stream_reply_sends_many_then_ends_once() {
        let (seen, deliver) = recorder();
        let mut reply = Reply::stream(deliver);
        for data in ["a", "bb"] {
            reply.message(Message {
                data: data.into(),
                ..Message::default()
            });
        }
        reply.send(Outcome::Failed {
            kind: FailureKind::Network,
            message: "closed".into(),
        });
        assert_eq!(
            *seen.lock().unwrap(),
            ["message, 1 bytes", "message, 2 bytes", "network: closed"]
        );
    }

    #[test]
    fn a_stream_reply_dropped_without_an_end_aborts_once() {
        let (seen, deliver) = recorder();
        let mut reply = Reply::stream(deliver);
        reply.message(Message::default());
        drop(reply);
        assert_eq!(
            *seen.lock().unwrap(),
            [
                "message, 0 bytes",
                "aborted: the owner ended without a reply"
            ]
        );
    }

    #[test]
    fn a_message_on_a_single_reply_is_its_one_answer() {
        let (seen, deliver) = recorder();
        let mut reply = Reply::new(deliver);
        reply.message(Message::default());
        reply.message(Message::default());
        drop(reply);
        assert_eq!(*seen.lock().unwrap(), ["message, 0 bytes"]);
    }

    #[test]
    fn a_stream_answer_is_independent_and_marked() {
        let Answer::Later(request) = Answer::stream(Request::get("https://example.test/e")) else {
            panic!("a stream is a request")
        };
        assert!(request.stream);
        assert_eq!(
            request.http,
            HttpScheduling::Independent {
                max_response_bytes: 1 << 20
            }
        );
        let Answer::Later(kept) =
            Answer::stream(Request::get("https://example.test/e").independent_http(4096))
        else {
            panic!("a stream is a request")
        };
        assert_eq!(
            kept.http,
            HttpScheduling::Independent {
                max_response_bytes: 4096
            }
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_scope_must_be_admitted_and_name_the_operation() {
        let admitted = "surface.read world\nsurface.write world\nsurface.read map";
        assert!(Request::capture_surface("world")
            .check_surface_grant(admitted)
            .is_ok());
        assert!(Request::restore_surface("world", vec![])
            .check_surface_grant(admitted)
            .is_ok());
        let mut narrowed = Request::capture_surface("world");
        narrowed.grants = Some("surface.read map".into());
        assert!(narrowed.check_surface_grant(admitted).is_err());
        narrowed.grants = Some("surface.read world\nsurface.write other".into());
        assert!(narrowed.check_surface_grant(admitted).is_err());
        let mut mixed = Request::capture_surface("world");
        mixed.storage = Some(Vec::new());
        assert!(mixed
            .check_surface_grant(admitted)
            .unwrap_err()
            .contains("multiple host-work kinds"));
        assert_eq!(
            io_grants(
                "fs.read app:/data\nsurface.read world\ndevice.microphone p\nsurface.write world\nauth.session https://x.test\nauth.callback a.b:/c"
            ),
            "fs.read app:/data"
        );
    }

    #[test]
    fn a_body_from_a_file_is_http_with_no_other_body() {
        let post = |path: &str| Request::post_json("https://x.test", "").body_from(path);
        assert_eq!(post("app:/tmp/a.jpg").body_from_refusal(), None);
        assert_eq!(Request::get("https://x.test").body_from_refusal(), None);
        assert!(post("/tmp/a.jpg")
            .body_from_refusal()
            .unwrap()
            .contains("app:/"));
        let mut both = post("app:/tmp/a.jpg");
        both.body = b"x".to_vec();
        assert!(both.body_from_refusal().unwrap().contains("one body"));
        for method in ["GET", "head"] {
            let mut read = post("app:/tmp/a.jpg");
            read.method = method.into();
            assert!(read.body_from_refusal().unwrap().contains("GET or HEAD"));
        }
        let native = Request::native(Vec::new()).body_from("app:/tmp/a.jpg");
        assert_eq!(
            native.body_from_refusal(),
            Some("only HTTP takes exactBodyFrom")
        );
        let mut storage = Request::storage(Vec::new()).body_from("app:/tmp/a.jpg");
        storage.method = "POST".into();
        assert_eq!(
            storage.body_from_refusal(),
            Some("only HTTP takes exactBodyFrom")
        );
    }
}
