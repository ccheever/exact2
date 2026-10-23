//! A long poll, natively (LLP 1016.000 D8): a stream whose request says
//! `long_poll`. The host asks the URL, delivers each 2xx reply with a body
//! as one message (its `id` the new cursor, its `data` the body's text), and
//! asks again with the cursor the reply gives, set in the named query
//! parameter. One request is in flight at a time, on this stream's own
//! thread. A heartbeat (a 204, a 304, an empty body, or, unless
//! `same_cursor_event`, a reply whose cursor is the one asked with) is asked
//! again with no message. A transient failure (the network, a 408, 429,
//! 502, 503 or 504) is asked again after `Retry-After` (at most 60 s) or a
//! backoff of 1, 2, 4, 8, 16 and then 30 s, each within a fifth either way,
//! reset by any 2xx. Anything else ends the stream as a stream's one answer
//! does: a status that is not one of those, a 2xx that is not JSON, one
//! with no cursor, an event stream, a body past the ceiling. Forgetting the
//! ticket aborts the request in flight and any wait.
use super::super::{check_headers, failed, fetch_failure, fetch_request, Shared};
use super::message;
use exact_runner::{FailureKind, LongPoll, Message, Outcome, Request, Response, Retrying};
use ibex2::stdlib::abort::AbortController;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The longest `Retry-After` honoured, and the backoff's ceiling.
const RETRY_AFTER_MAX_MS: u64 = 60_000;
const BACKOFF_MAX_MS: u64 = 30_000;
/// The least time from one heartbeat's request to the next (a test's shorter).
const HEARTBEAT_FLOOR: Duration = Duration::from_millis(if cfg!(test) { 40 } else { 1000 });
/// The longest cursor, in bytes.
const CURSOR_MAX: usize = 4096;

/// What one ask came to.
enum Asked {
    /// An event: its cursor and the body's text.
    Event(String, String),
    /// A reply that is no event: a 204, a 304, an empty body.
    Heartbeat,
    /// A reply asking to be asked again later (a 408, 429, 502, 503 or 504),
    /// with its `Retry-After` in ms.
    Busy(u16, Option<u64>),
    /// No reply: the network failed, or the request's own deadline passed.
    Down(String),
    /// It ended the stream.
    End(Outcome),
}

impl super::super::Core {
    /// Each long poll's backoff wait, by its stream ticket, read live: what
    /// `state.streams` shows as `retrying`.
    pub(crate) fn retry_reader(&self) -> exact_runner::RetryReader {
        let shared = self.shared.clone();
        Arc::new(move |ticket| {
            let retrying = shared.retrying.lock().unwrap();
            let (attempt, at, error) = retrying.get(&ticket)?;
            Some(Retrying {
                attempt: *attempt,
                in_ms: at.saturating_duration_since(Instant::now()).as_millis() as u64,
                error: error.clone(),
            })
        })
    }
}

/// Run the long poll `ticket` (its first request `request`) until the
/// runner lets it go or a reply ends it.
pub(super) fn run(
    shared: &Shared,
    ticket: u64,
    request: Request,
    forced: bool,
    b: &ibex2::host::Bindings,
    limit: usize,
    abort: AbortController,
) {
    let terms = request.long_poll.as_deref().cloned().unwrap_or(LongPoll {
        cursor: String::new(),
        from: String::new(),
        same_cursor_event: false,
    });
    let aborted = || failed(FailureKind::Aborted, "native request aborted");
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // (a Rust source's request is checked here; the prelude refused a TypeScript one already)
        if let Some(why) = request.long_poll_refusal() {
            return failed(FailureKind::Refused, why);
        }
        // Each reply that is no event, as the runner counts it (`heldReplies`).
        let replied = || {
            let m = Message {
                event: exact_runner::LONG_POLL_REPLY.into(),
                ..Message::default()
            };
            message(shared, ticket, m)
        };
        let mut url = request.url.clone();
        let mut failures: u32 = 0;
        loop {
            if abort.signal().aborted() {
                return aborted();
            }
            let asked_with = query_value(&url, &terms.cursor);
            let mut req = request.clone();
            req.url = url.clone();
            // (a reply asks again at most once a second: a server that answers at once is not held on)
            let asked_at = Instant::now();
            let floor = || millis_up(HEARTBEAT_FLOOR.saturating_sub(asked_at.elapsed()));
            let (wait, error) = match ask(b, req, forced, limit, &abort) {
                Asked::End(outcome) => return outcome,
                Asked::Event(next, _)
                    if !terms.same_cursor_event && asked_with.as_deref() == Some(next.as_str()) =>
                {
                    failures = 0;
                    if !replied() {
                        return aborted();
                    }
                    (floor(), None)
                }
                Asked::Event(next, data) => {
                    failures = 0;
                    let m = Message {
                        id: next.clone(),
                        data,
                        ..Message::default()
                    };
                    if !message(shared, ticket, m) {
                        return failed(FailureKind::Aborted, "the stream was let go");
                    }
                    url = with_query_value(&url, &terms.cursor, &next);
                    continue;
                }
                Asked::Heartbeat => {
                    failures = 0;
                    if !replied() {
                        return aborted();
                    }
                    (floor(), None)
                }
                Asked::Busy(status, after) => {
                    if !replied() {
                        return aborted();
                    }
                    let wait = match after {
                        Some(ms) => {
                            failures = failures.saturating_add(1);
                            ms
                        }
                        None => backoff(ticket, &mut failures),
                    };
                    (wait.max(floor()), Some(format!("HTTP {status}")))
                }
                Asked::Down(_) if abort.signal().aborted() => return aborted(),
                Asked::Down(error) => {
                    let error = if error.is_empty() {
                        "a network error".into()
                    } else {
                        error
                    };
                    (backoff(ticket, &mut failures), Some(error))
                }
            };
            // (a heartbeat's wait is no retry)
            if let Some(error) = &error {
                let at = Instant::now() + Duration::from_millis(wait);
                (shared.retrying.lock().unwrap()).insert(ticket, (failures, at, error.clone()));
            }
            let waited = wait == 0 || pause(&abort, wait);
            if error.is_some() {
                shared.retrying.lock().unwrap().remove(&ticket);
            }
            if !waited {
                return aborted();
            }
        }
    }))
    .unwrap_or_else(|_| failed(FailureKind::Aborted, "the long poll panicked"));
    shared.retrying.lock().unwrap().remove(&ticket);
    super::super::complete(shared, ticket, run);
}

/// `d` in whole ms, rounded up: a floor is never cut short.
fn millis_up(d: Duration) -> u64 {
    d.as_micros().div_ceil(1000).min(u128::from(u64::MAX)) as u64
}

/// One request of the poll: what it came to, judged by its status and
/// headers first, so a reply that asks again or is an event stream is never
/// read; an event's body read whole within `limit`.
fn ask(
    b: &ibex2::host::Bindings,
    request: Request,
    forced: bool,
    limit: usize,
    abort: &AbortController,
) -> Asked {
    let from = request
        .long_poll
        .as_deref()
        .map_or(String::new(), |p| p.from.clone());
    let mut req = fetch_request(request, forced);
    if !req.headers.has("accept") {
        req.headers.set("accept", "application/json");
    }
    req.max_body = Some(usize::MAX);
    let r = match b.fetch.stream(req, &abort.signal()).and_then(check_headers) {
        Ok(r) => r,
        Err(e) => {
            return match fetch_failure(e, abort) {
                Outcome::Failed {
                    kind: FailureKind::Network | FailureKind::Timeout,
                    message,
                } => Asked::Down(message),
                other => Asked::End(other),
            }
        }
    };
    let (status, headers) = (r.status, r.headers.entries().to_vec());
    let mut body = r.body;
    let event_stream = headers.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("content-type")
            && v.split(';')
                .next()
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("text/event-stream"))
    });
    match status {
        204 | 304 => {
            body.cancel();
            return Asked::Heartbeat;
        }
        408 | 429 | 502 | 503 | 504 => {
            body.cancel();
            return Asked::Busy(status, retry_after(&headers, now_epoch_ms()));
        }
        200..=299 if event_stream => {
            body.cancel();
            return Asked::End(failed(
                FailureKind::Refused,
                "an event stream under exactLongPoll: a long poll's replies are JSON",
            ));
        }
        _ => {}
    }
    let mut bytes = Vec::new();
    let mut chunk = [0; 16 * 1024];
    loop {
        match body.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) if bytes.len() + n > limit => {
                body.cancel();
                return Asked::End(failed(FailureKind::Refused, "HTTP response exceeds limit"));
            }
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(_) if abort.signal().aborted() => {
                return Asked::End(failed(FailureKind::Aborted, "native request aborted"))
            }
            Err(e) => return Asked::Down(e.to_string()),
        }
    }
    if !(200..=299).contains(&status) {
        return Asked::End(Outcome::Response(Response {
            status,
            headers,
            body: bytes,
        }));
    }
    // (a byte order mark is no part of the JSON, as a browser's decoder drops it)
    let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    if text
        .iter()
        .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
    {
        return Asked::Heartbeat;
    }
    match cursor_of(text, &from) {
        Ok(Some(next)) => Asked::Event(next, String::from_utf8_lossy(text).into_owned()),
        Ok(None) => Asked::End(Outcome::Response(Response {
            status,
            headers,
            body: bytes,
        })),
        Err(why) => Asked::End(failed(FailureKind::Refused, why)),
    }
}

/// The cursor a reply's JSON gives at `from` (`a` or `a.b`): a string as it
/// is, a whole number within 2^53 as JSON writes it; none for anything else
/// (no JSON, nothing there, another kind). One that would not round-trip
/// every host, or not fit a message's id, is refused.
fn cursor_of(body: &[u8], from: &str) -> Result<Option<String>, &'static str> {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) else {
        return Ok(None);
    };
    let mut at = &json;
    for part in from.splitn(2, '.') {
        match at.get(part) {
            Some(v) => at = v,
            None => return Ok(None),
        }
    }
    match at {
        serde_json::Value::String(s) if s.len() > CURSOR_MAX || s.chars().any(char::is_control) => {
            Err("a long poll's cursor is at most 4096 bytes, with no control characters")
        }
        serde_json::Value::String(s) => Ok(Some(s.clone())),
        // (`1e3` and `1.0` are whole numbers too, as a browser reads them)
        serde_json::Value::Number(n) => match n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 9e15).map(|f| f as i64))
            .filter(|n| n.unsigned_abs() < 1 << 53)
        {
            Some(n) => Ok(Some(n.to_string())),
            None => Err("a long poll's numeric cursor is a whole number within 2^53: send a larger one as a string"),
        },
        _ => Ok(None),
    }
}

/// The value of query parameter `name` in `url`, decoded; none when absent.
pub(crate) fn query_value(url: &str, name: &str) -> Option<String> {
    let query = url.split('#').next()?.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (decode(k) == name).then(|| decode(v))
    })
}

/// `url` with query parameter `name` set to `value` (replaced where it is,
/// else appended), every other parameter and the fragment kept.
pub(crate) fn with_query_value(url: &str, name: &str, value: &str) -> String {
    let (base, fragment) = match url.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (url, None),
    };
    let (path, query) = base.split_once('?').unwrap_or((base, ""));
    let pair = format!("{}={}", encode(name), encode(value));
    let mut found = false;
    let mut parts: Vec<String> = Vec::new();
    for p in query.split('&').filter(|p| !p.is_empty()) {
        let k = p.split_once('=').map_or(p, |(k, _)| k);
        if decode(k) == name {
            if !found {
                parts.push(pair.clone());
                found = true;
            }
        } else {
            parts.push(p.to_string());
        }
    }
    if !found {
        parts.push(pair);
    }
    let mut out = format!("{path}?{}", parts.join("&"));
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(
                    std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                    16,
                ) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The next backoff wait, in ms, for a stream's `failures`-th failure in a
/// row: 1, 2, 4, 8, 16, then 30 s, within a fifth either way.
fn backoff(ticket: u64, failures: &mut u32) -> u64 {
    let base = (1000u64 << (*failures).min(5)).min(BACKOFF_MAX_MS);
    *failures = failures.saturating_add(1);
    // (a jitter of its own, from the ticket and the attempt: no shared randomness)
    let mut x = ticket.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(*failures).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    let fifth = base / 5;
    base - fifth + x % (2 * fifth + 1)
}

/// `Retry-After` in ms (seconds, or an HTTP date from `now`), at most a minute.
pub(crate) fn retry_after(headers: &[(String, String)], now_ms: u64) -> Option<u64> {
    let v = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
        .map(|(_, v)| v.trim())?;
    let ms = match v.parse::<u64>() {
        Ok(secs) => secs.saturating_mul(1000),
        Err(_) => http_date_ms(v)?.saturating_sub(now_ms),
    };
    Some(ms.min(RETRY_AFTER_MAX_MS))
}

/// An IMF-fixdate (`Sun, 06 Nov 1994 08:49:37 GMT`) as ms since the epoch.
fn http_date_ms(v: &str) -> Option<u64> {
    let parts: Vec<&str> = v.split_whitespace().collect();
    let [_, day, month, year, time, "GMT"] = parts.as_slice() else {
        return None;
    };
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let m = months.iter().position(|x| x == month)? as i64 + 1;
    let (d, y): (i64, i64) = (day.parse().ok()?, year.parse().ok()?);
    let hms: Vec<i64> = time
        .split(':')
        .map(|x| x.parse().ok())
        .collect::<Option<_>>()?;
    let [h, mi, s] = hms.as_slice() else {
        return None;
    };
    // Days from the civil date (Howard Hinnant's algorithm).
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + mi * 60 + s;
    u64::try_from(secs).ok().map(|s| s * 1000)
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Wait `ms`, or less when `abort` fires: whether it waited it out.
fn pause(abort: &AbortController, ms: u64) -> bool {
    let woken = Arc::new((Mutex::new(false), Condvar::new()));
    let _registration = {
        let woken = woken.clone();
        abort.signal().register(move || {
            *woken.0.lock().unwrap() = true;
            woken.1.notify_all();
        })
    };
    let deadline = std::time::Instant::now() + Duration::from_millis(ms);
    let mut aborted = woken.0.lock().unwrap();
    while !*aborted {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return true;
        }
        aborted = woken.1.wait_timeout(aborted, left).unwrap().0;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_parameter_is_set_where_it_is_or_appended() {
        assert_eq!(
            with_query_value("https://x.test/changes?since=0&wait=25", "since", "7"),
            "https://x.test/changes?since=7&wait=25"
        );
        assert_eq!(
            with_query_value("https://x.test/changes?wait=25", "since", "a b"),
            "https://x.test/changes?wait=25&since=a%20b"
        );
        assert_eq!(
            with_query_value("https://x.test/changes", "since", "7"),
            "https://x.test/changes?since=7"
        );
        assert_eq!(
            with_query_value("https://x.test/c?since=1#top", "since", "2"),
            "https://x.test/c?since=2#top"
        );
        assert_eq!(
            query_value("https://x.test/c?a=1&since=a%20b", "since").as_deref(),
            Some("a b")
        );
        assert_eq!(query_value("https://x.test/c?a=1", "since"), None);
    }

    #[test]
    fn the_cursor_is_a_string_or_a_whole_number_at_its_field() {
        let at = |body: &str, from| cursor_of(body.as_bytes(), from);
        assert_eq!(
            at(r#"{"seq":12,"changes":[]}"#, "seq"),
            Ok(Some("12".into()))
        );
        assert_eq!(
            at(r#"{"meta":{"next":"abc"}}"#, "meta.next"),
            Ok(Some("abc".into()))
        );
        assert_eq!(
            at(r#"{"seq":-9007199254740991}"#, "seq"),
            Ok(Some("-9007199254740991".into()))
        );
        assert_eq!(at(r#"{"seq":1e3}"#, "seq"), Ok(Some("1000".into())));
        assert_eq!(at(r#"{"seq":null}"#, "seq"), Ok(None));
        assert_eq!(at("not json", "seq"), Ok(None));
        // What would not round-trip a browser, or would break a message's id, is refused.
        for bad in [
            r#"{"seq":9007199254740993}"#,
            r#"{"seq":1.5}"#,
            r#"{"seq":"a\nb"}"#,
        ] {
            assert!(at(bad, "seq").is_err(), "{bad}");
        }
        assert!(at(&format!(r#"{{"seq":"{}"}}"#, "x".repeat(4097)), "seq").is_err());
    }

    #[test]
    fn retry_after_and_backoff_are_bounded() {
        let h = |v: &str| vec![("Retry-After".to_string(), v.to_string())];
        assert_eq!(retry_after(&h("3"), 0), Some(3000));
        assert_eq!(retry_after(&h("3600"), 0), Some(60_000));
        let now = http_date_ms("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
        assert_eq!(
            retry_after(&h("Sun, 06 Nov 1994 08:49:42 GMT"), now),
            Some(5000)
        );
        assert_eq!(retry_after(&[], 0), None);
        let mut failures = 0;
        let waits: Vec<u64> = (0..8).map(|_| backoff(7, &mut failures)).collect();
        for (w, base) in waits
            .iter()
            .zip([1000, 2000, 4000, 8000, 16000, 30000, 30000, 30000])
        {
            assert!(
                *w >= base - base / 5 && *w <= base + base / 5,
                "{w} near {base}"
            );
        }
    }

    #[test]
    fn a_pause_ends_at_once_when_aborted() {
        let abort = AbortController::new();
        let a = abort.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            a.abort();
        });
        let start = std::time::Instant::now();
        assert!(!pause(&abort, 10_000));
        assert!(start.elapsed() < Duration::from_secs(2));
        t.join().unwrap();
    }
}

#[cfg(test)]
#[path = "executor_longpoll_tests.rs"]
mod loop_tests;
