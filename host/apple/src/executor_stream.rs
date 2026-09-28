//! Answers that keep coming, natively (LLP 1016.000 D3; LLP 1069.004 slice
//! 2): a streamed `fetch` whose body is server-sent events, opened by the
//! worker that owns the transport and read on a thread of its own, so an
//! open stream holds no I/O worker (LLP 1067 D3). Each event is one
//! `Outcome::Message` to the ticket; anything that ends the body ends the
//! stream. The host never reconnects on its own (1069.004 slice 2).
use super::{
    check_headers, complete, failed, fetch_failure, fetch_request, message, Shared, MAX_BODY,
};
use exact_runner::{FailureKind, HttpScheduling, Message, Outcome, Request, Response};
use ibex2::stdlib::{abort::AbortController, fetch::StreamingResponse};

/// Open `request` as a stream. `Ok` is an event stream to read, with its
/// per-message ceiling; anything else is the stream's one answer, whole:
/// a status that is not 2xx, a body that is not `text/event-stream`, or
/// the failure that kept it from opening.
pub(super) fn open(
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
pub(super) fn read_events(
    shared: &Shared,
    ticket: u64,
    response: StreamingResponse,
    limit: usize,
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
