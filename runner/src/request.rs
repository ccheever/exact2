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

/// Maximum bytes in one portable host-work request or outcome.
pub const MAX_HOST_WORK_BYTES: usize = 16 << 20;

/// Grants understood by the platform I/O executor. Surface capabilities are
/// enforced by the presenter and must not make an older I/O parser reject the
/// otherwise independent filesystem, network, database, or secret grants.
pub fn io_grants(spec: &str) -> String {
    spec.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("surface.read ") && !line.starts_with("surface.write "))
        .filter(|line| !line.is_empty())
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
            surface: None,
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
            surface: None,
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

/// A request's outcome: a response (any status), or no response at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A portable storage operation completed (LLP 1027.001 D2).
    Storage(Vec<u8>),
    /// Named surface work completed.
    Surface(SurfaceOutcome),
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
            io_grants("fs.read app:/data\nsurface.read world\nsurface.write world"),
            "fs.read app:/data"
        );
    }
}
