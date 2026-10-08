//! Answers that keep coming, natively (LLP 1016.000 D3; LLP 1069.004 slice
//! 2): a streamed `fetch` whose body is server-sent events, opened by the
//! worker that owns the transport and read on a thread of its own, so an
//! open stream holds no I/O worker (LLP 1067 D3). Each event is one
//! `Outcome::Message` to the ticket; anything that ends the body ends the
//! stream. The host never reconnects on its own (1069.004 slice 2).
//!
//! A `ws:` or `wss:` URL is a WebSocket instead (1069.004 slice 3), admitted
//! by `net.websocket`: each text message is one `Message`, and the far
//! side's close, a binary message or one over the ceiling ends it. Nothing
//! is sent: the request's method, headers and body are not used, as a
//! browser's `WebSocket` could not send them either.
use super::{
    check_headers, complete, failed, fetch_failure, fetch_request, message, Shared, MAX_BODY,
};
use exact_runner::{FailureKind, HttpScheduling, Message, Outcome, Request, Response};
use ibex2::stdlib::{
    abort::AbortController,
    fetch::StreamingResponse,
    websocket::{Incoming, MessageSource},
};

/// What a stream opened as.
enum Opened {
    Events(StreamingResponse, usize),
    Socket(Box<dyn MessageSource>),
}

/// A WebSocket's URL (the scheme is the whole spelling: LLP 1069.004).
pub(super) fn is_socket(url: &str) -> bool {
    let scheme = url.split(':').next().unwrap_or("");
    scheme.eq_ignore_ascii_case("ws") || scheme.eq_ignore_ascii_case("wss")
}

/// A stream's own thread: it opens with a transport of its own (so it never
/// waits behind held replies for a worker or a lease), then reads to the end.
pub(super) fn run(
    shared: &Shared,
    ticket: u64,
    mut request: Request,
    forced: bool,
    grants: &str,
    host: &dyn Fn() -> ibex2::host::Host,
    abort: AbortController,
) {
    // Retirement aborts every stream, as it does every job.
    let _retiring = {
        let abort = abort.clone();
        shared.abort.signal().register(move || abort.abort())
    };
    if shared.abort.signal().aborted() {
        abort.abort();
    }
    let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // A source's scope narrows the app's grants, as on the workers.
        let admitted = match request.grants.as_deref() {
            Some(scope) => exact_data::storage::scope(grants, Some(scope)).map(str::to_string),
            None => Ok(grants.to_string()),
        };
        let bindings = admitted.and_then(|g| {
            ibex2::grant::GrantSet::parse(&exact_runner::io_grants(&g))
                .map(|g| host().endow(g))
                .map_err(|e| format!("the app's grants did not parse: {e}"))
        });
        match bindings {
            Ok(b) if is_socket(&request.url) => open_socket(&b, request, &abort).map(|s| (s, b)),
            // A body from an app file, read as the stream opens (LLP 1108 D6 R2).
            Ok(b) => super::body::resolve(shared.roots.get(), grants, &mut request, &|| {
                abort
                    .signal()
                    .aborted()
                    .then(|| failed(FailureKind::Aborted, "native request aborted"))
            })
            .and_then(|()| open(Ok(&b), request, forced, &abort))
            .map(|(r, limit)| (Opened::Events(r, limit), b)),
            Err(message) => Err(failed(FailureKind::Refused, message)),
        }
    }))
    .unwrap_or_else(|_| Err(failed(FailureKind::Aborted, "the stream opener panicked")));
    match opened {
        // The bindings live as long as the body they opened.
        Ok((Opened::Events(response, limit), _bindings)) => {
            read_events(shared, ticket, response, limit, abort)
        }
        Ok((Opened::Socket(socket), _bindings)) => read_socket(shared, ticket, socket, abort),
        Err(outcome) => complete(shared, ticket, outcome),
    }
}

/// Open a WebSocket, admitted by `net.websocket`; its ceiling is per message.
fn open_socket(
    b: &ibex2::host::Bindings,
    request: Request,
    abort: &AbortController,
) -> Result<Opened, Outcome> {
    let limit = match request.http {
        HttpScheduling::Ordered => MAX_BODY,
        HttpScheduling::Independent { max_response_bytes } => max_response_bytes as usize,
    };
    b.websocket
        .open(&request.url, limit, &abort.signal())
        .map(Opened::Socket)
        .map_err(|e| fetch_failure(e, abort))
}

/// Read an open socket to its end, delivering each text message.
fn read_socket(
    shared: &Shared,
    ticket: u64,
    mut socket: Box<dyn MessageSource>,
    abort: AbortController,
) {
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop {
        match socket.next() {
            Ok(Incoming::Text(data)) => {
                let m = Message {
                    data,
                    ..Message::default()
                };
                if !message(shared, ticket, m) {
                    return failed(FailureKind::Aborted, "the stream was let go");
                }
            }
            Ok(Incoming::Binary(_)) => {
                return failed(
                    FailureKind::Refused,
                    "a binary message: a socket's messages are text",
                )
            }
            Ok(Incoming::TooLarge) => {
                return failed(
                    FailureKind::Refused,
                    "a message exceeds the response ceiling",
                )
            }
            Ok(Incoming::Closed { code, reason }) => {
                return failed(FailureKind::Network, socket_closed(code, &reason))
            }
            Err(_) if abort.signal().aborted() => {
                return failed(FailureKind::Aborted, "native request aborted")
            }
            Err(e) => return failed(FailureKind::Network, e.to_string()),
        }
    }))
    .unwrap_or_else(|_| failed(FailureKind::Aborted, "the stream reader panicked"));
    drop(socket);
    complete(shared, ticket, run);
}

/// How a socket's close reads, on every host: `the socket closed (1000)`,
/// or with the far side's reason after a colon.
pub fn socket_closed(code: u16, reason: &str) -> String {
    if reason.is_empty() {
        format!("the socket closed ({code})")
    } else {
        format!("the socket closed ({code}: {reason})")
    }
}

/// Open `request` as a stream. `Ok` is an event stream to read, with its
/// per-message ceiling; anything else is the stream's one answer, whole:
/// a status that is not 2xx, a body that is not `text/event-stream`, or
/// the failure that kept it from opening.
fn open(
    bindings: Result<&ibex2::host::Bindings, &str>,
    request: Request,
    forced: bool,
    abort: &AbortController,
) -> Result<(StreamingResponse, usize), Outcome> {
    if abort.signal().aborted() {
        return Err(failed(FailureKind::Aborted, "native request aborted"));
    }
    let b = bindings.map_err(|unbound| failed(FailureKind::Refused, unbound))?;
    let limit = match request.http {
        HttpScheduling::Ordered => MAX_BODY,
        HttpScheduling::Independent { max_response_bytes } => max_response_bytes as usize,
    };
    let mut req = fetch_request(request, forced);
    if !req.headers.has("accept") {
        req.headers.set("accept", "text/event-stream");
    }
    // The ceiling is per message: the stream as a whole has none.
    req.max_body = Some(usize::MAX);
    let r = b
        .fetch
        .stream(req, &abort.signal())
        .and_then(check_headers)
        .map_err(|e| fetch_failure(e, abort))?;
    let events = r.ok()
        && r.headers.get("content-type").is_some_and(|t| {
            t.split(';')
                .next()
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("text/event-stream"))
        });
    if events {
        return Ok((r, limit));
    }
    let (status, headers) = (r.status, r.headers.entries().to_vec());
    let mut body = r.body;
    let mut bytes = Vec::new();
    let mut chunk = [0; 16 * 1024];
    loop {
        match body.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) if bytes.len() + n > limit => {
                body.cancel();
                return Err(failed(FailureKind::Refused, "HTTP response exceeds limit"));
            }
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(fetch_failure(e, abort)),
        }
    }
    Err(Outcome::Response(Response {
        status,
        headers,
        body: bytes,
    }))
}

/// Read the open stream `ticket` to its end, delivering each event. It ends
/// when the far side closes, the connection drops, an event is over the
/// ceiling, or the runner lets the ticket go (forgetting aborts the read).
fn read_events(
    shared: &Shared,
    ticket: u64,
    response: StreamingResponse,
    limit: usize,
    abort: AbortController,
) {
    let mut body = response.body;
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut events = EventStream::new(limit);
        let mut chunk = [0; 16 * 1024];
        loop {
            match body.read(&mut chunk) {
                Ok(0) => return failed(FailureKind::Network, "the event stream ended"),
                Ok(n) => match events.push(&chunk[..n]) {
                    Ok(messages) => {
                        for m in messages {
                            if !message(shared, ticket, m) {
                                return failed(FailureKind::Aborted, "the stream was let go");
                            }
                        }
                    }
                    Err(e) => return failed(FailureKind::Refused, e),
                },
                Err(_) if abort.signal().aborted() => {
                    return failed(FailureKind::Aborted, "native request aborted")
                }
                Err(e) => {
                    return failed(
                        FailureKind::Network,
                        e.to_string().chars().take(2048).collect::<String>(),
                    )
                }
            }
        }
    }))
    .unwrap_or_else(|_| failed(FailureKind::Aborted, "the stream reader panicked"));
    body.cancel();
    complete(shared, ticket, run);
}

/// A `text/event-stream` parser (HTML's "Parsing an event stream"): bytes
/// in, dispatched events out. Lines end at CR, LF, or CRLF, even split
/// across reads; `data` lines join with LF; `id` persists as the cursor;
/// comments and `retry` are read and dropped (the host never reconnects).
/// One event, or one line, over `limit` bytes is refused, never truncated.
pub struct EventStream {
    line: Vec<u8>,
    after_cr: bool,
    started: bool,
    event: String,
    data: String,
    has_data: bool,
    id: String,
    limit: usize,
}

impl EventStream {
    /// A parser whose events may carry up to `limit` bytes of data.
    pub fn new(limit: usize) -> Self {
        Self {
            line: Vec::new(),
            after_cr: false,
            started: false,
            event: String::new(),
            data: String::new(),
            has_data: false,
            id: String::new(),
            limit,
        }
    }

    /// The events these bytes complete, in order.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Message>, &'static str> {
        let mut out = Vec::new();
        for &b in bytes {
            if std::mem::take(&mut self.after_cr) && b == b'\n' {
                continue;
            }
            match b {
                b'\r' | b'\n' => {
                    self.after_cr = b == b'\r';
                    let line = std::mem::take(&mut self.line);
                    if let Some(m) = self.line_done(&line)? {
                        out.push(m);
                    }
                }
                _ => {
                    if self.line.len() >= self.limit.saturating_add(64) {
                        return Err("an event exceeds the response ceiling");
                    }
                    self.line.push(b);
                }
            }
        }
        Ok(out)
    }

    fn line_done(&mut self, line: &[u8]) -> Result<Option<Message>, &'static str> {
        let mut line = String::from_utf8_lossy(line).into_owned();
        if !std::mem::replace(&mut self.started, true) {
            if let Some(rest) = line.strip_prefix('\u{feff}') {
                line = rest.to_string();
            }
        }
        if line.is_empty() {
            let event = std::mem::take(&mut self.event);
            if !std::mem::take(&mut self.has_data) {
                self.data.clear();
                return Ok(None);
            }
            let mut data = std::mem::take(&mut self.data);
            if data.ends_with('\n') {
                data.pop();
            }
            return Ok(Some(Message {
                event,
                id: self.id.clone(),
                data,
                coalesced: 0,
            }));
        }
        if line.starts_with(':') {
            return Ok(None);
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_str(), ""),
        };
        match field {
            "data" => {
                if self.data.len() + value.len() + 1 > self.limit {
                    return Err("an event exceeds the response ceiling");
                }
                self.data.push_str(value);
                self.data.push('\n');
                self.has_data = true;
            }
            "event" => self.event = value.to_string(),
            "id" if !value.contains('\0') => self.id = value.to_string(),
            _ => {}
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "executor_stream_tests.rs"]
mod tests;
