//! `fetch` — delegating, **capability-bearing** (LLP 0059.000 §3.5).
//!
//! The largest v1 item, and the first one that carries a capability.
//!
//! **Rust owns:** header name case-folding and the forbidden-header list,
//! redirect following and its count limit, the body state machine,
//! and the error taxonomy — so failures are identical on every platform.
//!
//! **The platform owns:** sockets, TLS, proxy configuration, HTTP/2 and /3,
//! connection pooling, and the system certificate store. That split is why the
//! transport is a trait here and not a hard-coded client: inverting it gives
//! four platforms four different `fetch`es, which is the failure mode a
//! cross-platform runtime exists to prevent.
//!
//! @ref LLP 0059.000#35-fetch--delegating-capability-bearing — the surface this implements
//! @ref LLP 0057#3-the-boundary — Rust owns semantics, the platform owns transport

use super::abort::AbortSignal;
pub use super::fetch_body::{Body, BodySource, StreamingResponse};
use crate::boundary::HostError;
use crate::grant::{GrantSet, Operation, Origin};

/// How many redirects to follow before giving up. The web platform's limit.
pub const MAX_REDIRECTS: u32 = 20;

/// The response ceiling a transport applies when the caller names none.
///
/// Applies to both buffered helpers and cumulative stream consumption.
/// Backpressure bounds unread data independently of this total byte ceiling.
pub const DEFAULT_MAX_BODY: usize = 64 * 1024 * 1024;

/// The one spelling of "the response was too big", so a caller sees the same
/// failure whichever transport is underneath. The error taxonomy is Rust's
/// (LLP 0057 §3); a transport reports the overflow, it does not word it.
pub fn over_limit(limit: usize) -> HostError {
    HostError::Failed(format!(
        "TypeError: Failed to fetch — response exceeded the {limit}-byte limit"
    ))
}

/// Header names are case-insensitive; values are not.
///
/// Stored folded so lookup and duplicate detection are exact, and so two
/// platforms cannot disagree about whether `Content-Type` and `content-type`
/// are the same header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers {
    entries: Vec<(String, String)>,
}

/// Headers a caller may not set, because the transport owns them.
///
/// Per the Fetch spec's forbidden request-header list, trimmed to what this
/// transport actually controls. Silently ignoring an attempt is what the spec
/// requires — not an error, but not honored either.
const FORBIDDEN_REQUEST_HEADERS: &[&str] = &[
    "host",
    "connection",
    "content-length",
    "transfer-encoding",
    "upgrade",
    "keep-alive",
    "proxy-connection",
    "te",
    "trailer",
];

/// Is this a valid header name? The spec's token production: one or more of
/// the allowed ASCII characters, and nothing else.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

/// Is this a valid header value?
///
/// Two rules, and the ORDER matters. The value is normalized first — leading
/// and trailing HTTP whitespace stripped — and only then checked, because
/// `"\r\n newLine"` is a legal way to write `"newLine"` while
/// `"bad\r\ninjection"` is header injection. Validating before normalizing
/// rejects both, which is wrong for the first.
///
/// Then: no NUL, CR, or LF anywhere in what remains, and no code point above
/// U+00FF. A header value is a byte sequence, so WebIDL's ByteString
/// conversion throws on anything wider — which is why `"invalidValueĀ"` is a
/// TypeError and `"newLine\u{a0}"` is not.
pub fn is_valid_value(value: &str) -> bool {
    let normalized = normalize_value(value);
    !normalized
        .chars()
        .any(|c| matches!(c, '\0' | '\r' | '\n') || (c as u32) > 0xFF)
}

/// Strip leading and trailing HTTP whitespace, which is what "normalize" means
/// for a header value. Interior whitespace is preserved.
pub fn normalize_value(value: &str) -> &str {
    value.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    fn fold(name: &str) -> String {
        name.to_ascii_lowercase()
    }

    /// Set a header, replacing any existing value.
    ///
    /// **No forbidden-header filtering happens here**, and that is the spec's
    /// design, not an omission: filtering is a property of the header list's
    /// *guard*, and a standalone `new Headers()` has guard "none". A bare
    /// Headers object may hold `Host` quite legitimately. The request guard is
    /// applied where a header list actually becomes a request — see
    /// `for_request`.
    pub fn set(&mut self, name: &str, value: &str) {
        let folded = Self::fold(name);
        let value = normalize_value(value).to_string();
        match self.entries.iter_mut().find(|(k, _)| *k == folded) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((folded, value)),
        }
    }

    /// Append, combining with any existing value as `a, b` per the spec.
    pub fn append(&mut self, name: &str, value: &str) {
        let folded = Self::fold(name);
        let value = normalize_value(value).to_string();
        match self.entries.iter_mut().find(|(k, _)| *k == folded) {
            Some(entry) => {
                entry.1.push_str(", ");
                entry.1.push_str(&value);
            }
            None => self.entries.push((folded, value)),
        }
    }

    /// A response's headers. Identical to `set` now that the guard lives at the
    /// request boundary; kept as a name so call sites still say which side they
    /// are on.
    pub fn set_response(&mut self, name: &str, value: &str) {
        self.set(name, value);
    }

    /// Apply the request guard: drop the headers the transport owns.
    ///
    /// Called when a header list becomes an actual request, which is the point
    /// at which the forbidden list means anything.
    pub fn for_request(&self) -> Self {
        Self {
            entries: self
                .entries
                .iter()
                .filter(|(name, _)| !FORBIDDEN_REQUEST_HEADERS.contains(&name.as_str()))
                .cloned()
                .collect(),
        }
    }

    /// Entries in the order the iterator must yield them: sorted by name.
    ///
    /// The spec sorts; insertion order is not observable through `Headers`.
    pub fn sorted_entries(&self) -> Vec<(String, String)> {
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        sorted
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        let folded = Self::fold(name);
        self.entries
            .iter()
            .find(|(k, _)| *k == folded)
            .map(|(_, v)| v.as_str())
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn delete(&mut self, name: &str) {
        let folded = Self::fold(name);
        self.entries.retain(|(k, _)| *k != folded);
    }

    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// What a redirect response should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectMode {
    /// Follow, up to `MAX_REDIRECTS`. The default.
    Follow,
    /// Return the redirect response itself.
    Manual,
    /// Treat a redirect as a network error.
    Error,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Headers,
    pub body: Option<Vec<u8>>,
    pub redirect: RedirectMode,
    /// The largest response body this request will accept, in bytes;
    /// [`DEFAULT_MAX_BODY`] when `None`.
    ///
    /// **Per request, because only the caller knows what it asked for.** A
    /// 64 KB envelope and a 64 MB plan want different numbers, and a consumer
    /// that must fetch both cannot say so with one process-wide constant —
    /// which is what made a bounded read impossible from outside this crate.
    ///
    /// **Enforced while the bytes arrive, never after.** A check on a body
    /// already in memory has paid for the attack it was meant to refuse, so a
    /// transport that cannot stop mid-response cannot honor this field.
    pub max_body: Option<usize>,
}

impl Request {
    pub fn get(url: &str) -> Self {
        Self {
            method: "GET".into(),
            url: url.into(),
            headers: Headers::new(),
            body: None,
            redirect: RedirectMode::Follow,
            max_body: None,
        }
    }

    /// The ceiling this request asked for, resolved.
    pub fn body_limit(&self) -> usize {
        self.max_body.unwrap_or(DEFAULT_MAX_BODY)
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: Headers,
    pub body: Vec<u8>,
    /// The URL the response came from, after any redirects.
    pub url: String,
    pub redirected: bool,
}

impl Response {
    /// `Response.ok` — the 200-299 range, and nothing else.
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// What the platform must provide. Everything above this line is Rust's.
pub trait Transport: Send + Sync {
    /// Perform exactly one request. **No redirect following** — that is
    /// semantics and belongs to Rust, above.
    ///
    /// **`request.body_limit()` binds.** An implementation must stop receiving
    /// and fail with [`over_limit`] the moment the body would exceed it, and a
    /// declared length already over it is refused before the body is read at
    /// all. Only the transport holds the socket, so this is the one rung where
    /// the ceiling can be enforced rather than observed after the fact.
    fn open(&self, request: &Request, signal: &AbortSignal)
        -> Result<StreamingResponse, HostError>;
    fn send(&self, request: &Request) -> Result<Response, HostError> {
        self.open(request, &AbortSignal::default())?.collect()
    }
}

/// Extract the origin a URL belongs to, for the capability check.
pub fn origin_of(url: &str) -> Result<Origin, HostError> {
    let parsed = url::Url::parse(url)
        .map_err(|e| HostError::Failed(format!("TypeError: invalid URL: {e}")))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| HostError::Failed("TypeError: URL has no host".into()))?;
    let port = parsed.port_or_known_default().ok_or_else(|| {
        HostError::Failed(format!(
            "TypeError: no default port for scheme {}",
            parsed.scheme()
        ))
    })?;
    Ok(Origin::new(parsed.scheme(), host, port))
}

/// Perform a fetch: check the grant, then follow redirects up to the limit.
///
/// **The capability check happens per-origin and it happens again on every
/// redirect hop.** A grant for `a.example` must not become a grant for
/// `b.example` because `a.example` answered with a `Location` pointing there —
/// that is the whole redirect-laundering class, and checking only the initial
/// URL is how a runtime falls into it.
pub fn fetch(
    transport: &dyn Transport,
    grants: &GrantSet,
    request: Request,
) -> Result<Response, HostError> {
    fetch_stream(transport, grants, request, &AbortSignal::default())?.collect()
}

pub fn fetch_stream(
    transport: &dyn Transport,
    grants: &GrantSet,
    request: Request,
    signal: &AbortSignal,
) -> Result<StreamingResponse, HostError> {
    let mut current = request;
    let mut redirects = 0;
    let mut redirected = false;

    loop {
        signal.check()?;
        let origin = origin_of(&current.url)?;
        // Through boundary::admit, not an inline permits() check: the
        // capability name and the shape of a denial belong in one place, or
        // "one chokepoint" is a claim about the design rather than a fact
        // about the code.
        crate::boundary::admit(grants, &Operation::Fetch { origin })?;

        // The request guard applies here, where a header list becomes a
        // request — not in Headers::set, where the guard is "none".
        let mut guarded = current.clone();
        guarded.headers = current.headers.for_request();
        let result = transport.open(&guarded, signal);
        signal.check()?;
        let mut response = result?;
        response.redirected = redirected;

        let is_redirect = matches!(response.status, 301 | 302 | 303 | 307 | 308);
        if !is_redirect {
            return Ok(response);
        }

        match current.redirect {
            RedirectMode::Manual => return Ok(response),
            RedirectMode::Error => {
                return Err(HostError::Failed(
                    "TypeError: Failed to fetch — redirect not allowed".into(),
                ))
            }
            RedirectMode::Follow => {}
        }

        let Some(location) = response.headers.get("location").map(str::to_string) else {
            // A redirect status with no Location is not a redirect; the spec
            // says return it as-is rather than inventing a destination.
            return Ok(response);
        };

        redirects += 1;
        if redirects > MAX_REDIRECTS {
            return Err(HostError::Failed(
                "TypeError: Failed to fetch — too many redirects".into(),
            ));
        }

        // Resolve relative Locations against the URL we just used.
        let base = url::Url::parse(&current.url)
            .map_err(|e| HostError::Failed(format!("TypeError: invalid URL: {e}")))?;
        let next = base
            .join(&location)
            .map_err(|e| HostError::Failed(format!("TypeError: invalid redirect target: {e}")))?;

        // 303, and 301/302 on POST, become GET and drop the body — the web's
        // long-standing behaviour, which surprises people who expect the
        // method to be preserved.
        if response.status == 303
            || (matches!(response.status, 301 | 302)
                && current.method != "GET"
                && current.method != "HEAD")
        {
            current.method = "GET".into();
            current.body = None;
        }
        current.url = next.to_string();
        redirected = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grant::Grant;
    use std::sync::Mutex;

    /// Records what it was asked for and replays canned responses.
    struct StubTransport {
        responses: Mutex<Vec<Response>>,
        seen: Mutex<Vec<(String, String)>>,
    }

    impl StubTransport {
        fn new(responses: Vec<Response>) -> Self {
            Self {
                responses: Mutex::new(responses),
                seen: Mutex::new(Vec::new()),
            }
        }
        fn seen(&self) -> Vec<(String, String)> {
            self.seen.lock().unwrap().clone()
        }
    }

    impl Transport for StubTransport {
        fn open(
            &self,
            request: &Request,
            signal: &AbortSignal,
        ) -> Result<StreamingResponse, HostError> {
            self.seen
                .lock()
                .unwrap()
                .push((request.method.clone(), request.url.clone()));
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                return Err(HostError::Failed("stub ran out of responses".into()));
            }
            Ok(responses
                .remove(0)
                .into_stream(request.body_limit(), signal.clone()))
        }
    }

    fn response(status: u16, location: Option<&str>) -> Response {
        let mut headers = Headers::new();
        if let Some(location) = location {
            headers.set_response("Location", location);
        }
        Response {
            status,
            status_text: "".into(),
            headers,
            body: b"body".to_vec(),
            url: String::new(),
            redirected: false,
        }
    }

    fn granted(host: &str) -> GrantSet {
        GrantSet::none().with(Grant::Fetch(Origin::new("https", host, 443)))
    }

    #[test]
    fn preaborted_fetch_never_reaches_transport() {
        let transport = StubTransport::new(vec![]);
        let controller = crate::stdlib::abort::AbortController::new();
        controller.abort();
        assert!(fetch_stream(
            &transport,
            &granted("example.com"),
            Request::get("https://example.com/"),
            &controller.signal()
        )
        .unwrap_err()
        .to_string()
        .starts_with("AbortError"));
        assert!(transport.seen().is_empty());
    }

    #[test]
    fn streamed_redirect_keeps_admission_and_does_not_collect_redirect_body() {
        let mut redirect = response(302, Some("https://other.example/"));
        redirect.body = vec![1; 100];
        let transport = StubTransport::new(vec![redirect, response(200, None)]);
        let mut request = Request::get("https://example.com/");
        request.max_body = Some(4);
        let error = fetch_stream(
            &transport,
            &granted("example.com"),
            request,
            &AbortSignal::default(),
        )
        .unwrap_err();
        assert!(matches!(error, HostError::Denied { .. }));
        assert_eq!(transport.seen().len(), 1);
    }

    #[test]
    fn header_names_fold_case_and_values_do_not() {
        let mut headers = Headers::new();
        headers.set("Content-Type", "Application/JSON");
        assert_eq!(headers.get("content-type"), Some("Application/JSON"));
        assert_eq!(headers.get("CONTENT-TYPE"), Some("Application/JSON"));
        headers.set("content-type", "text/plain");
        assert_eq!(headers.len(), 1, "same header, replaced not duplicated");
        assert_eq!(headers.get("Content-Type"), Some("text/plain"));
    }

    #[test]
    fn append_combines_with_a_comma() {
        let mut headers = Headers::new();
        headers.append("Accept", "text/html");
        headers.append("accept", "application/json");
        assert_eq!(headers.get("accept"), Some("text/html, application/json"));
    }

    /// A standalone Headers has guard "none" and holds anything. This is what
    /// `new Headers({Host: ...})` must do, and filtering here was a bug WPT
    /// found.
    #[test]
    fn a_standalone_header_list_holds_forbidden_names() {
        let mut headers = Headers::new();
        headers.set("Host", "example.com");
        headers.set("Content-Length", "0");
        assert_eq!(headers.get("host"), Some("example.com"));
        assert_eq!(headers.get("content-length"), Some("0"));
    }

    /// ...but the request guard drops them where it matters.
    #[test]
    fn the_request_guard_drops_transport_owned_headers() {
        let mut headers = Headers::new();
        headers.set("Host", "evil.example");
        headers.set("Content-Length", "0");
        headers.set("Connection", "close");
        headers.set("X-Fine", "yes");

        let guarded = headers.for_request();
        assert!(!guarded.has("host"));
        assert!(!guarded.has("content-length"));
        assert!(!guarded.has("connection"));
        assert_eq!(guarded.get("x-fine"), Some("yes"));
    }

    #[test]
    fn names_and_values_are_validated_the_way_the_spec_says() {
        assert!(is_valid_name("Content-Type"));
        assert!(is_valid_name("X_custom"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("has space"));
        assert!(!is_valid_name("(paren)"));
        assert!(!is_valid_name("colon:"));

        assert!(is_valid_value("fine"));
        assert!(is_valid_value("has spaces inside"));
        assert!(!is_valid_value("bad\r\ninjection"));
        assert!(!is_valid_value("nul\0byte"));

        // Normalization happens before validation, so surrounding CRLF is a
        // way of writing the trimmed value rather than an injection.
        assert!(is_valid_value("\r\n newLine"));
        assert!(is_valid_value("newLine\r\n "));
        assert_eq!(normalize_value("\r\n newLine"), "newLine");
        // ...but form feed is not HTTP whitespace and survives.
        assert_eq!(normalize_value("\t\u{c}\tnewLine\n"), "\u{c}\tnewLine");

        // A header value is a byte sequence: U+00A0 fits, U+0100 does not.
        assert!(is_valid_value("newLine\u{a0}"));
        assert!(!is_valid_value("invalidValue\u{100}"));
    }

    #[test]
    fn values_are_normalized_but_interior_space_survives() {
        let mut headers = Headers::new();
        headers.set("X", "  padded value  ");
        assert_eq!(headers.get("x"), Some("padded value"));
    }

    #[test]
    fn iteration_order_is_sorted_by_name_not_insertion() {
        let mut headers = Headers::new();
        headers.set("zebra", "1");
        headers.set("alpha", "2");
        headers.set("Mike", "3");
        let names: Vec<_> = headers
            .sorted_entries()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, vec!["alpha", "mike", "zebra"]);
    }

    #[test]
    fn a_response_may_carry_content_length() {
        let mut headers = Headers::new();
        headers.set_response("Content-Length", "42");
        assert_eq!(headers.get("content-length"), Some("42"));
    }

    #[test]
    fn ok_is_exactly_the_two_hundreds() {
        for (status, expected) in [
            (199, false),
            (200, true),
            (299, true),
            (300, false),
            (404, false),
        ] {
            let mut r = response(status, None);
            r.status = status;
            assert_eq!(r.ok(), expected, "status {status}");
        }
    }

    #[test]
    fn an_ungranted_origin_is_refused_before_any_request_is_made() {
        let transport = StubTransport::new(vec![response(200, None)]);
        let err = fetch(
            &transport,
            &GrantSet::none(),
            Request::get("https://example.com/"),
        )
        .unwrap_err();
        assert_eq!(
            err,
            HostError::Denied {
                capability: "net.fetch"
            }
        );
        assert!(
            transport.seen().is_empty(),
            "a denied fetch must not reach the transport at all"
        );
    }

    #[test]
    fn a_granted_origin_succeeds() {
        let transport = StubTransport::new(vec![response(200, None)]);
        let got = fetch(
            &transport,
            &granted("example.com"),
            Request::get("https://example.com/x"),
        )
        .unwrap();
        assert!(got.ok());
        assert_eq!(transport.seen().len(), 1);
    }

    /// The redirect-laundering case. A grant for one origin must not become a
    /// grant for another just because the first one redirected there.
    #[test]
    fn a_redirect_to_an_ungranted_origin_is_refused() {
        let transport = StubTransport::new(vec![
            response(302, Some("https://evil.example/steal")),
            response(200, None),
        ]);
        let err = fetch(
            &transport,
            &granted("example.com"),
            Request::get("https://example.com/"),
        )
        .unwrap_err();
        assert_eq!(
            err,
            HostError::Denied {
                capability: "net.fetch"
            }
        );
        assert_eq!(
            transport.seen().len(),
            1,
            "the second hop must never be sent"
        );
    }

    /// A `Location` of `///evil.example/steal` is, per WHATWG, a way to change
    /// the ORIGIN — special schemes skip leading slashes, so it resolves to
    /// `http://evil.example/steal` rather than to a path on the current host.
    ///
    /// Our URL layer refuses that form outright (a known WPT divergence, three
    /// cases). Either behaviour is safe here — refusing produces a fetch error,
    /// and resolving would be caught by the per-hop grant check — but this test
    /// pins the outcome so a future URL change cannot quietly turn a refusal
    /// into a redirect to an ungranted origin without failing here first.
    #[test]
    fn an_origin_changing_triple_slash_redirect_does_not_reach_another_origin() {
        let transport = StubTransport::new(vec![
            response(302, Some("///evil.example/steal")),
            response(200, None),
        ]);
        let result = fetch(
            &transport,
            &granted("example.com"),
            Request::get("https://example.com/"),
        );
        assert!(result.is_err(), "must not deliver a cross-origin body");
        assert_eq!(
            transport.seen().len(),
            1,
            "the second hop must never be sent"
        );
    }

    #[test]
    fn a_redirect_within_a_granted_origin_is_followed() {
        let transport =
            StubTransport::new(vec![response(302, Some("/moved")), response(200, None)]);
        let got = fetch(
            &transport,
            &granted("example.com"),
            Request::get("https://example.com/start"),
        )
        .unwrap();
        assert!(got.ok());
        assert!(got.redirected);
        assert_eq!(
            transport.seen(),
            vec![
                ("GET".to_string(), "https://example.com/start".to_string()),
                ("GET".to_string(), "https://example.com/moved".to_string()),
            ]
        );
    }

    #[test]
    fn a_303_turns_a_post_into_a_get_and_drops_the_body() {
        let transport = StubTransport::new(vec![response(303, Some("/done")), response(200, None)]);
        let mut request = Request::get("https://example.com/submit");
        request.method = "POST".into();
        request.body = Some(b"a=1".to_vec());
        fetch(&transport, &granted("example.com"), request).unwrap();
        assert_eq!(transport.seen()[1].0, "GET");
    }

    #[test]
    fn a_307_preserves_the_method() {
        let transport =
            StubTransport::new(vec![response(307, Some("/again")), response(200, None)]);
        let mut request = Request::get("https://example.com/submit");
        request.method = "POST".into();
        fetch(&transport, &granted("example.com"), request).unwrap();
        assert_eq!(transport.seen()[1].0, "POST");
    }

    #[test]
    fn redirect_manual_returns_the_redirect_itself() {
        let transport = StubTransport::new(vec![response(302, Some("/moved"))]);
        let mut request = Request::get("https://example.com/");
        request.redirect = RedirectMode::Manual;
        let got = fetch(&transport, &granted("example.com"), request).unwrap();
        assert_eq!(got.status, 302);
        assert_eq!(transport.seen().len(), 1);
    }

    #[test]
    fn too_many_redirects_is_an_error_not_an_infinite_loop() {
        let responses = (0..MAX_REDIRECTS + 2)
            .map(|_| response(302, Some("/loop")))
            .collect();
        let transport = StubTransport::new(responses);
        let err = fetch(
            &transport,
            &granted("example.com"),
            Request::get("https://example.com/"),
        )
        .unwrap_err();
        assert!(format!("{err}").contains("too many redirects"));
    }

    #[test]
    fn origin_of_uses_the_schemes_default_port() {
        assert_eq!(
            origin_of("https://example.com/x").unwrap(),
            Origin::new("https", "example.com", 443)
        );
        assert_eq!(
            origin_of("http://example.com/x").unwrap(),
            Origin::new("http", "example.com", 80)
        );
        assert_eq!(
            origin_of("https://example.com:8443/x").unwrap(),
            Origin::new("https", "example.com", 8443)
        );
    }
}
