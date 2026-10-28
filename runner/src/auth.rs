//! `openAuthSession` (LLP 1069.006): a request to `exact-auth:` whose body
//! names an authorization URL, the callback it returns to and the `state` it
//! carries. Every host's arm asks this module first, so one rule checks the
//! grants, holds the request for the agent, accepts a callback, and keeps the
//! one-session rule; the host only opens the platform's surface (an
//! `ASWebAuthenticationSession`, a popup) and reports what came back.
//!
//! @ref LLP 1069.006 D1 (the request and its statuses) / D2 (`auth.session`
//! and `auth.callback`) / D3 (the host's own scheme, host and path match;
//! one session per window; supersession) / D7 (the substitute: a hold the
//! agent answers with a callback URL, checked the same way)
//!
//! The reply is HTTP-shaped: 200 carries the callback URL (delivery, not a
//! sign-in), 499 `cancelled`, 403 the grant, 409 `already open`, 428 `popup
//! blocked` (web), 501 no system browser, 502 anything else. A callback URL
//! carries a code, so no journal line, hold summary or refusal here ever
//! includes one.

use crate::agent::{field_bool, field_str, quote};
use crate::{DataSource, Outcome, RequestOut, Response, Runner};

/// The reserved URL of an auth session request.
pub const AUTH_URL: &str = "exact-auth:";

/// The path of the web's callback page, on the app's own origin (D4).
pub const WEB_CALLBACK_PATH: &str = "/.exact/auth/callback";

/// One request's session: what the host opens and what it accepts back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    /// The authorization URL.
    pub url: String,
    /// The callback this request returns to, exactly.
    pub callback: String,
    /// The `state` the callback must carry, exactly once.
    pub state: String,
    /// Ask for no shared cookies (`prefersEphemeralWebBrowserSession`).
    pub ephemeral: bool,
}

impl Session {
    /// The request body, `{"url","callback","state","ephemeral"}`.
    pub fn from_json(json: &str) -> Result<Session, String> {
        let text = |k: &str| field_str(json, k).filter(|v| !v.is_empty());
        let (Some(url), Some(callback), Some(state)) =
            (text("url"), text("callback"), text("state"))
        else {
            return Err("an auth session needs url, callback and a non-empty state".into());
        };
        if bent_http(&url) {
            return Err("the authorization URL is not a plain http: or https: URL".into());
        }
        if bent_http(&callback) {
            return Err("the callback is not a scheme:/path or https://host/path".into());
        }
        let parts = Url::parse(&url).ok_or("the authorization URL is not absolute")?;
        if !matches!(parts.scheme.as_str(), "https" | "http") || parts.authority.is_none() {
            return Err("the authorization URL is not http: or https:".into());
        }
        if parts.authority.is_some_and(|a| a.contains('@')) {
            return Err("the authorization URL carries userinfo".into());
        }
        let callback_parts = Url::parse(&callback)
            .filter(|c| c.fragment.is_none() && c.query.is_none())
            .ok_or("the callback is not a scheme:/path or https://host/path")?;
        if matches!(callback_parts.scheme.as_str(), "http" | "https")
            && callback_parts.authority.is_none()
        {
            return Err("the callback is not a scheme:/path or https://host/path".into());
        }
        if callback_parts.authority.is_some_and(|a| a.contains('@')) {
            return Err("the callback carries userinfo".into());
        }
        Ok(Session {
            url,
            callback,
            state,
            ephemeral: field_bool(json, "ephemeral"),
        })
    }

    /// The body a source sends: `openAuthSession`'s own spelling.
    pub fn to_json(&self) -> String {
        let mut s = String::from("{\"url\":");
        quote(&self.url, &mut s);
        s.push_str(",\"callback\":");
        quote(&self.callback, &mut s);
        s.push_str(",\"state\":");
        quote(&self.state, &mut s);
        s.push_str(&format!(",\"ephemeral\":{}}}", self.ephemeral));
        s
    }

    /// The hold's inspection summary (D7): the authorization URL's origin
    /// and path, its query's parameter names, and the values of `client_id`
    /// and `request_uri` only (a URL can carry an `id_token_hint`); the
    /// callback, `state` and `ephemeral`.
    pub fn summary(&self) -> String {
        let parts = Url::parse(&self.url);
        let (origin, path, query) = parts.as_ref().map_or((String::new(), "", ""), |p| {
            (
                p.origin().unwrap_or_default(),
                p.path,
                p.query.unwrap_or(""),
            )
        });
        let mut s = String::from("{\"origin\":");
        quote(&origin, &mut s);
        s.push_str(",\"path\":");
        quote(path, &mut s);
        s.push_str(",\"params\":[");
        let mut shown = String::new();
        for (i, (name, value)) in query_pairs(query).enumerate() {
            if i > 0 {
                s.push(',');
            }
            quote(&name, &mut s);
            if matches!(name.as_str(), "client_id" | "request_uri") {
                shown.push(',');
                quote(&name, &mut shown);
                shown.push(':');
                quote(&value, &mut shown);
            }
        }
        s.push(']');
        s.push_str(&shown);
        s.push_str(",\"callback\":");
        if Url::parse(&self.callback).is_some_and(|p| {
            p.authority.is_some_and(|a| a.contains('@'))
                || (matches!(p.scheme.as_str(), "http" | "https") && p.authority.is_none())
        }) {
            quote("<redacted callback>", &mut s);
        } else {
            quote(&self.callback, &mut s);
        }
        s.push_str(",\"state\":");
        quote(&self.state, &mut s);
        s.push_str(&format!(",\"ephemeral\":{}}}", self.ephemeral));
        s
    }
}

/// The pieces of an absolute URL this module compares. Enough for
/// `scheme:/path`, `https://host[:port]/path?query#fragment`; not a general
/// parser.
#[derive(Debug)]
struct Url<'a> {
    scheme: String,
    authority: Option<&'a str>,
    path: &'a str,
    query: Option<&'a str>,
    fragment: Option<&'a str>,
}

impl<'a> Url<'a> {
    fn parse(text: &'a str) -> Option<Url<'a>> {
        let colon = text.find(':')?;
        let scheme = &text[..colon];
        let mut chars = scheme.chars();
        if !chars.next()?.is_ascii_alphabetic()
            || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            || text.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return None;
        }
        let rest = &text[colon + 1..];
        let (rest, fragment) = match rest.split_once('#') {
            Some((r, f)) => (r, Some(f)),
            None => (rest, None),
        };
        let (rest, query) = match rest.split_once('?') {
            Some((r, q)) => (r, Some(q)),
            None => (rest, None),
        };
        let (authority, path) = match rest.strip_prefix("//") {
            Some(after) => {
                let end = after.find('/').unwrap_or(after.len());
                (Some(&after[..end]), &after[end..])
            }
            None => (None, rest),
        };
        if authority.is_some_and(str::is_empty) {
            return None;
        }
        if authority.is_none() && !(path.starts_with('/') && !path.starts_with("//")) {
            return None;
        }
        Some(Url {
            scheme: scheme.to_ascii_lowercase(),
            authority,
            path,
            query,
            fragment,
        })
    }

    fn origin(&self) -> Option<String> {
        Some(format!(
            "{}://{}",
            self.scheme,
            self.authority?.rsplit('@').next()?.to_ascii_lowercase()
        ))
    }
}

/// A query's `name=value` pairs, percent-decoded, `+` as a space.
fn query_pairs(query: &str) -> impl Iterator<Item = (String, String)> + '_ {
    query.split('&').filter(|p| !p.is_empty()).map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        (decode(name), decode(value))
    })
}

/// `%` and two hex digits as a byte (a trailing one too), `+` a space,
/// anything else as written: the loopback receiver's rule as well
/// (`exact-listen`'s `loopback.rs`), so the two read one `state`.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| Some(hex(*bytes.get(i + 1)?)? << 4 | hex(*bytes.get(i + 2)?)?))
            .flatten();
        match (bytes[i], escaped) {
            (_, Some(b)) => {
                out.push(b);
                i += 2;
            }
            (b'+', _) => out.push(b' '),
            (b, _) => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `a` and `b` are equal, in time that depends on their lengths only.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// Whether `url` is on a loopback host (`localhost`, `127.0.0.1`, `[::1]`).
fn loopback(url: &Url<'_>) -> bool {
    let host = url.authority.unwrap_or("");
    let host = host.rsplit_once(':').map_or(host, |(h, port)| {
        if port.chars().all(|c| c.is_ascii_digit()) {
            h
        } else {
            host
        }
    });
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// `net.fetch`'s grammar for one `auth.session` target: an origin, or
/// `scheme://*.domain` (a subdomain of a domain of two or more labels).
fn origin_admits(grant: &str, url: &Url<'_>) -> bool {
    let Some(origin) = url.origin() else {
        return false;
    };
    if let Some((scheme, domain)) = grant.split_once("://*.") {
        let host = url
            .authority
            .unwrap_or("")
            .rsplit('@')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let domain = domain.to_ascii_lowercase();
        return scheme.eq_ignore_ascii_case(&url.scheme)
            && domain.split('.').count() >= 2
            && !domain.contains([':', '/', '*'])
            && host.len() > domain.len() + 1
            && host.ends_with(&format!(".{domain}"));
    }
    !grant.contains('*') && grant.trim_end_matches('/').eq_ignore_ascii_case(&origin)
}

/// The grant lines that apply: the app's, narrowed by a mixed app's source
/// scope, which may only select lines the app has.
fn effective<'a>(admitted: &'a str, scope: Option<&'a str>) -> Result<Vec<&'a str>, String> {
    let lines = |s: &'a str| -> Vec<&'a str> {
        s.lines().map(str::trim).filter(|l| !l.is_empty()).collect()
    };
    let admitted_lines = lines(admitted);
    match scope {
        None => Ok(admitted_lines),
        Some(scope) => {
            let scoped = lines(scope);
            if scoped.iter().any(|l| !admitted_lines.contains(l)) {
                return Err("the source's grants exceed the app's".into());
            }
            Ok(scoped)
        }
    }
}

/// Every `auth.callback` the grants name, in order.
pub fn callbacks(grants: &str) -> Vec<String> {
    grants
        .lines()
        .filter_map(|l| l.trim().strip_prefix("auth.callback "))
        .map(|c| c.trim().to_string())
        .collect()
}

/// This carrier's callback, known before PAR (D2): on the web
/// `<origin>/.exact/auth/callback`; natively the first private-use scheme
/// the app grants, else its first claimed https callback that is not the
/// web's page. `None` when the grants name none.
pub fn carrier_callback(grants: &str, web_origin: Option<&str>) -> Option<String> {
    if let Some(origin) = web_origin {
        return Some(format!(
            "{}{WEB_CALLBACK_PATH}",
            origin.trim_end_matches('/')
        ));
    }
    let all = callbacks(grants);
    all.iter()
        .find(|c| Url::parse(c).is_some_and(|u| u.authority.is_none()))
        // A loopback grant names no port, so it is no callback by itself.
        .or_else(|| {
            all.iter()
                .find(|c| !c.ends_with(WEB_CALLBACK_PATH) && !c.starts_with("http:"))
        })
        .cloned()
}

/// A browser's URL parser reads a backslash in an http(s) URL as a slash and
/// drops tabs and newlines, so such a URL can carry userinfo this parser does
/// not see as authority: it is refused before anything echoes it.
fn bent_http(raw: &str) -> bool {
    let lower = raw.trim_start().to_ascii_lowercase();
    (lower.starts_with("http:") || lower.starts_with("https:"))
        && raw.contains(['\\', '\t', '\n', '\r'])
}

/// D2's checks: the authorization URL is on a granted `auth.session`
/// origin, and the callback is a granted `auth.callback` — or, on a
/// loopback development page, that page's own callback. `Err` is the 403
/// text, naming the grant it lacks.
pub fn check_grants(
    session: &Session,
    admitted: &str,
    scope: Option<&str>,
    web_origin: Option<&str>,
) -> Result<(), String> {
    let lines = effective(admitted, scope)?;
    if bent_http(&session.url) || bent_http(&session.callback) {
        return Err("the authorization URL or callback is not a plain http(s) URL".into());
    }
    let url = Url::parse(&session.url).ok_or("the authorization URL is not absolute")?;
    if url.authority.is_some_and(|a| a.contains('@')) {
        return Err("the authorization URL carries userinfo".into());
    }
    if Url::parse(&session.callback).is_some_and(|u| {
        u.authority.is_some_and(|a| a.contains('@'))
            || (matches!(u.scheme.as_str(), "http" | "https") && u.authority.is_none())
    }) {
        return Err("the callback has userinfo or malformed HTTP(S) authority".into());
    }
    let origin = url.origin().unwrap_or_default();
    if !lines.iter().any(|l| {
        l.strip_prefix("auth.session ")
            .is_some_and(|g| origin_admits(g.trim(), &url))
    }) {
        return Err(format!("outside the app's grants (auth.session {origin})"));
    }
    // A loopback callback (LLP 1133.005 D9) is granted at any port; on a
    // native host an `http:` callback is one or nothing.
    let looped = loopback_callback(&session.callback);
    let http = Url::parse(&session.callback).is_some_and(|u| u.scheme == "http");
    if web_origin.is_none() && http && looped.is_none() {
        return Err(format!(
            "a loopback callback is http://127.0.0.1:<port>/<path> or http://[::1]:<port>/<path>, not {} (RFC 8252 §8.3)",
            session.callback
        ));
    }
    let granted = lines.iter().any(|l| {
        l.strip_prefix("auth.callback ")
            .map(str::trim)
            .is_some_and(|g| {
                g == session.callback || looped.is_some() && loopback_admits(g, &session.callback)
            })
    });
    let development = web_origin.is_some_and(|o| {
        session.callback == format!("{}{WEB_CALLBACK_PATH}", o.trim_end_matches('/'))
            && Url::parse(o).is_some_and(|u| loopback(&u))
    });
    if !granted && !development {
        return Err(format!(
            "outside the app's grants (auth.callback {})",
            session.callback
        ));
    }
    Ok(())
}

/// A loopback callback's host (`127.0.0.1` or `[::1]`), port and path
/// (LLP 1133.005 D9): `http://127.0.0.1:<port>/<path>` with a port of 1 to
/// 65535 as a browser writes it in `Host` (no sign or leading zero, not the
/// default 80, which `Host` leaves out), no query and no fragment.
/// `localhost` is none: RFC 8252 §8.3.
pub fn loopback_callback(callback: &str) -> Option<(&'static str, u16, String)> {
    let url = Url::parse(callback)?;
    if url.scheme != "http" || url.query.is_some() || url.fragment.is_some() {
        return None;
    }
    let (host, port) = url.authority?.rsplit_once(':')?;
    let host = match host {
        "127.0.0.1" => "127.0.0.1",
        "[::1]" => "[::1]",
        _ => return None,
    };
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|p| *p > 0 && *p != 80 && p.to_string() == port)?;
    (url.path.starts_with('/') && url.path.len() > 1).then(|| (host, port, url.path.to_string()))
}

/// Whether the grant `http://127.0.0.1/<path>` (or `[::1]`, with no port)
/// admits the loopback `callback`: the same host and path, at any port, as
/// RFC 8252 §7.3 asks of an authorization server.
fn loopback_admits(grant: &str, callback: &str) -> bool {
    let (Some(g), Some((host, _, path))) = (Url::parse(grant), loopback_callback(callback)) else {
        return false;
    };
    g.scheme == "http"
        && g.authority == Some(host)
        && g.path == path
        && g.query.is_none()
        && g.fragment.is_none()
}

/// D3's match, on every host: `got`'s scheme, host and path equal
/// `session.callback`'s exactly (no userinfo; the same port), and its query
/// carries `state` exactly once, equal to the request's. `Err` says which
/// check failed, never the URL.
pub fn accept(session: &Session, got: &str) -> Result<(), String> {
    let want = Url::parse(&session.callback).ok_or("the request's callback is unreadable")?;
    let url = Url::parse(got).ok_or("the callback is not an absolute URL")?;
    if url.scheme != want.scheme {
        return Err("the callback's scheme is not the request's".into());
    }
    if url.authority.is_some_and(|a| a.contains('@')) {
        return Err("the callback carries userinfo".into());
    }
    let host = |u: &Url<'_>| u.authority.map(str::to_ascii_lowercase);
    if host(&url) != host(&want) {
        return Err("the callback's host is not the request's".into());
    }
    if url.path != want.path {
        return Err("the callback's path is not the request's".into());
    }
    let states: Vec<String> = query_pairs(url.query.unwrap_or(""))
        .filter(|(n, _)| n == "state")
        .map(|(_, v)| v)
        .collect();
    match states.as_slice() {
        [one] if same(one, &session.state) => Ok(()),
        [] => Err("the callback carries no state".into()),
        [_] => Err("the callback's state is not the request's".into()),
        _ => Err("the callback carries state more than once".into()),
    }
}

/// The live sessions and the answers settled for them, kept by the runner.
#[derive(Debug, Default)]
pub struct Sessions {
    live: Vec<(u64, String, Session)>,
    settled: Vec<(u64, Outcome)>,
    /// Tickets whose session a host opened and must cancel once forgotten.
    opened: Vec<u64>,
}

/// What a host does with an `exact-auth:` request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arm {
    /// Answered now (a refusal); the host delivers the settled outcome.
    Settled,
    /// Held for the agent: nothing is shown.
    Held,
    /// Open the platform's surface for this session.
    Present(Session),
}

/// How a host reaches a system browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Browser {
    /// An authentication session (Apple).
    Native,
    /// A popup to the app's callback page, on this origin (the web).
    Web,
    /// None (Linux, D5).
    None,
}

fn response(status: u16, body: &str) -> Outcome {
    Outcome::Response(Response {
        status,
        headers: vec![("content-type".into(), "text/plain".into())],
        body: body.as_bytes().to_vec(),
    })
}

/// Settle `ticket` with `status` and `body` (a host's report, a refusal, or
/// the agent's answer), for the host's next delivery. The live session ends.
pub fn settle<D: DataSource>(runner: &mut Runner<D>, ticket: u64, status: u16, body: &str) {
    let auth = runner.auth_mut();
    auth.live.retain(|(t, _, _)| *t != ticket);
    auth.opened.retain(|t| *t != ticket);
    auth.settled.push((ticket, response(status, body)));
    let line = match status {
        200 => "callback accepted".to_string(),
        499 => "cancelled".to_string(),
        other => format!("answered {other}"),
    };
    runner.log(format!("auth {ticket}: {line}"));
}

/// A host's report of how the session for `ticket` ended (D3, D4):
/// `Ok(url)` a callback (accepted here, or 502 with the failed check),
/// `Err((status, message))` a cancellation (499) or a failure. Nothing
/// happens for a ticket with no live session (superseded, or answered).
pub fn complete<D: DataSource>(
    runner: &mut Runner<D>,
    ticket: u64,
    result: Result<&str, (u16, &str)>,
) {
    let Some(session) = runner.auth_mut().session(ticket).cloned() else {
        runner.log(format!("auth {ticket}: a late completion, dropped"));
        return;
    };
    match result {
        Ok(url) => match accept(&session, url) {
            Ok(()) => settle(runner, ticket, 200, url),
            Err(why) => settle(runner, ticket, 502, &why),
        },
        Err((status, message)) => settle(runner, ticket, status, message),
    }
}

impl Sessions {
    fn session(&self, ticket: u64) -> Option<&Session> {
        self.live
            .iter()
            .find(|(t, _, _)| *t == ticket)
            .map(|(_, _, s)| s)
    }

    /// Whether an outcome waits for delivery on a ticket still `held`.
    pub fn has_settled_for(&self, held: impl Fn(u64) -> bool) -> bool {
        self.settled.iter().any(|(t, _)| held(*t))
    }

    /// The live session for `ticket`, as the agent's answer is checked.
    pub fn live(&self, ticket: u64) -> Option<&Session> {
        self.session(ticket)
    }
}

/// D1–D3 and D7 for one request, on every host: refuse what the grants or
/// the host can't admit (settled now), hold it under the agent, or hand the
/// host the session to open. `web_origin` is the page's origin on the web.
pub fn arm<D: DataSource>(
    runner: &mut Runner<D>,
    out: &RequestOut,
    agent: bool,
    browser: Browser,
    web_origin: Option<&str>,
) -> Arm {
    let ticket = out.ticket;
    let body = String::from_utf8_lossy(&out.request.body).into_owned();
    let session = match Session::from_json(&body) {
        Ok(s) => s,
        Err(why) => {
            settle(runner, ticket, 502, &why);
            return Arm::Settled;
        }
    };
    let admitted = runner.data().grants().to_string();
    if let Err(why) = check_grants(
        &session,
        &admitted,
        out.request.grants.as_deref(),
        web_origin,
    ) {
        settle(runner, ticket, 403, &why);
        return Arm::Settled;
    }
    if browser == Browser::None {
        settle(runner, ticket, 501, "no system browser on this host");
        return Arm::Settled;
    }
    // A loopback callback (LLP 1133.005 D9) other than the page's own.
    let page = web_origin.map(|o| format!("{}{WEB_CALLBACK_PATH}", o.trim_end_matches('/')));
    if browser == Browser::Web
        && loopback_callback(&session.callback).is_some()
        && page.as_deref() != Some(session.callback.as_str())
    {
        settle(runner, ticket, 501, "a page cannot listen");
        return Arm::Settled;
    }
    // One session per window: a request for the same target superseded its
    // predecessor (the runner forgot that ticket); another target's live
    // session refuses this one.
    let still: Vec<u64> = runner.auth_mut().live.iter().map(|(t, _, _)| *t).collect();
    for t in still {
        if !runner.holds(t) {
            let auth = runner.auth_mut();
            auth.live.retain(|(x, _, _)| *x != t);
            auth.opened.retain(|x| *x != t);
        }
    }
    if runner
        .auth_mut()
        .live
        .iter()
        .any(|(_, target, _)| *target != out.target)
    {
        settle(runner, ticket, 409, "already open");
        return Arm::Settled;
    }
    runner
        .auth_mut()
        .live
        .push((ticket, out.target.clone(), session.clone()));
    if agent {
        runner.hold_request(ticket, "auth", &out.target, &session.summary());
        return Arm::Held;
    }
    runner.auth_mut().opened.push(ticket);
    runner.log(format!("auth {ticket}: opened"));
    Arm::Present(session)
}

/// A host that decides agent mode itself (the Apple session, as for
/// `share`) holds a session it was handed instead of opening it (D7).
pub fn hold_opened<D: DataSource>(runner: &mut Runner<D>, ticket: u64) {
    let auth = runner.auth_mut();
    let Some(at) = auth.opened.iter().position(|t| *t == ticket) else {
        return;
    };
    auth.opened.remove(at);
    let Some((_, target, session)) = auth.live.iter().find(|(t, _, _)| *t == ticket).cloned()
    else {
        return;
    };
    runner.hold_request(ticket, "auth", &target, &session.summary());
}

/// The outcome settled for `ticket`, taken for delivery (the web's path).
pub fn take_settled<D: DataSource>(runner: &mut Runner<D>, ticket: u64) -> Option<Outcome> {
    let auth = runner.auth_mut();
    let at = auth.settled.iter().position(|(t, _)| *t == ticket)?;
    Some(auth.settled.remove(at).1)
}

/// Any settled outcome still wanted, for a host that delivers through its
/// admission path (Apple, Linux); settled outcomes for forgotten tickets are
/// dropped.
pub fn take_any_settled<D: DataSource>(runner: &mut Runner<D>) -> Option<(u64, Outcome)> {
    let tickets: Vec<u64> = runner.auth_mut().settled.iter().map(|(t, _)| *t).collect();
    let held: Vec<u64> = tickets.into_iter().filter(|t| runner.holds(*t)).collect();
    let auth = runner.auth_mut();
    auth.settled.retain(|(t, _)| held.contains(t));
    (!auth.settled.is_empty()).then(|| auth.settled.remove(0))
}

/// Sessions a host opened whose tickets the runner no longer holds
/// (superseded, the resource gone, a reload): the host cancels each and
/// drops its late completion (D3). Asked after every commit.
pub fn forgotten<D: DataSource>(runner: &mut Runner<D>) -> Vec<u64> {
    let opened = std::mem::take(&mut runner.auth_mut().opened);
    let (keep, gone): (Vec<u64>, Vec<u64>) = opened.into_iter().partition(|t| runner.holds(*t));
    let auth = runner.auth_mut();
    auth.opened = keep;
    auth.live.retain(|(t, _, _)| !gone.contains(t));
    for t in &gone {
        runner.log(format!("auth {t}: cancelled by supersession"));
    }
    gone
}

/// The agent's answer to an auth hold (D7): `type @t <callback URL>` is
/// checked as a host checks a completion, before the hold is spent, so a
/// wrong URL can be corrected; `tap @t cancel` is 499.
pub(crate) fn check_answer<D: DataSource>(
    runner: &mut Runner<D>,
    ticket: u64,
    url: &str,
) -> Result<(), String> {
    let session = runner
        .auth_mut()
        .live(ticket)
        .cloned()
        .ok_or_else(|| format!("not pending: @{ticket}"))?;
    accept(&session, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(callback: &str) -> Session {
        Session {
            url: "https://bsky.social/oauth/authorize?client_id=https%3A%2F%2Fapp.example%2Fc.json&request_uri=urn%3Ax&id_token_hint=secret".into(),
            callback: callback.into(),
            state: "s1".into(),
            ephemeral: false,
        }
    }

    #[test]
    fn a_callback_matches_scheme_host_path_and_one_state() {
        let s = session("social.exact.bluesky:/oauth");
        assert!(accept(&s, "social.exact.bluesky:/oauth?code=c&state=s1&iss=x").is_ok());
        for (bad, why) in [
            ("social.exact.bluesky:/other?state=s1", "path"),
            ("social.exact.other:/oauth?state=s1", "scheme"),
            ("social.exact.bluesky:/oauth?code=c", "no state"),
            ("social.exact.bluesky:/oauth?state=s2", "not the request's"),
            (
                "social.exact.bluesky:/oauth?state=s1&state=s1",
                "more than once",
            ),
            ("social.exact.bluesky://evil/oauth?state=s1", "host"),
        ] {
            let e = accept(&s, bad).unwrap_err();
            assert!(e.contains(why), "{bad}: {e}");
            assert!(!e.contains("code="), "never the URL");
        }
        let https = session("https://app.example/.exact/auth/callback");
        assert!(accept(&https, "https://APP.example/.exact/auth/callback?state=s1").is_ok());
        assert!(accept(
            &https,
            "https://u@app.example/.exact/auth/callback?state=s1"
        )
        .is_err());
        assert!(accept(
            &https,
            "https://app.example:444/.exact/auth/callback?state=s1"
        )
        .is_err());
        assert!(accept(&https, "http://app.example/.exact/auth/callback?state=s1").is_err());
        assert!(accept(&https, "https://app.example/.exact/auth/callback/?state=s1").is_err());
    }

    #[test]
    fn grants_admit_the_session_origin_and_the_exact_callback() {
        let grants = "net.fetch https://bsky.social\nauth.session https://bsky.social\nauth.callback social.exact.bluesky:/oauth\nauth.callback https://app.example/.exact/auth/callback";
        let s = session("social.exact.bluesky:/oauth");
        assert!(check_grants(&s, grants, None, None).is_ok());
        let mut other = s.clone();
        other.url = "https://evil.example/authorize".into();
        assert!(check_grants(&other, grants, None, None)
            .unwrap_err()
            .contains("auth.session https://evil.example"));
        let wild = "auth.session https://*.host.bsky.network\nauth.callback x.y:/cb";
        let mut pds = session("x.y:/cb");
        pds.url = "https://morel.us-east.host.bsky.network/oauth/authorize".into();
        assert!(check_grants(&pds, wild, None, None).is_ok());
        let mut unknown = s.clone();
        unknown.callback = "social.exact.bluesky:/other".into();
        assert!(check_grants(&unknown, grants, None, None).is_err());
        // A narrowed source is checked against its own lines.
        assert!(check_grants(&s, grants, Some("auth.session https://bsky.social"), None).is_err());
        // The web: the page's own callback, granted, or implicit on loopback.
        let web = session("https://app.example/.exact/auth/callback");
        assert!(check_grants(&web, grants, None, Some("https://app.example")).is_ok());
        let dev = session("http://127.0.0.1:4173/.exact/auth/callback");
        assert!(check_grants(&dev, grants, None, Some("http://127.0.0.1:4173")).is_ok());
        let far = session("https://other.example/.exact/auth/callback");
        assert!(check_grants(&far, grants, None, Some("https://other.example")).is_err());
    }

    #[test]
    fn a_loopback_callback_is_granted_at_any_port_on_its_ip_literal() {
        let grants =
            "auth.session https://auth.openai.com\nauth.callback http://127.0.0.1/auth/callback";
        let mut s = session("http://127.0.0.1:1455/auth/callback");
        s.url = "https://auth.openai.com/oauth/authorize?state=s1".into();
        assert!(check_grants(&s, grants, None, None).is_ok());
        assert_eq!(
            loopback_callback(&s.callback),
            Some(("127.0.0.1", 1455, "/auth/callback".into()))
        );
        s.callback = "http://127.0.0.1:2/auth/callback".into();
        assert!(check_grants(&s, grants, None, None).is_ok(), "any port");
        for bad in [
            "http://127.0.0.1:1455/auth/other",
            "http://[::1]:1455/auth/callback",
            "http://localhost:1455/auth/callback",
            "http://127.0.0.1/auth/callback",
            "http://127.0.0.1:0/auth/callback",
            "http://127.0.0.1:80/auth/callback",
            "http://127.0.0.1:01455/auth/callback",
            "http://127.0.0.1:+1455/auth/callback",
            "http://127.0.0.1:1455/auth/callback?x=1",
        ] {
            s.callback = bad.into();
            assert!(check_grants(&s, grants, None, None).is_err(), "{bad}");
        }
        s.callback = "http://localhost:1455/auth/callback".into();
        assert!(check_grants(&s, grants, None, None)
            .unwrap_err()
            .contains("RFC 8252"));
        let s = session("http://127.0.0.1:1455/auth/callback");
        assert!(accept(&s, "http://127.0.0.1:1455/auth/callback?code=c&state=s1").is_ok());
        assert!(accept(&s, "http://127.0.0.1:1456/auth/callback?state=s1").is_err());
        // The receiver's rule: a name is decoded too, a trailing escape
        // decodes, a signed one does not.
        let mut t = session("http://127.0.0.1:1455/auth/callback");
        t.state = "s A".into();
        assert!(accept(&t, "http://127.0.0.1:1455/auth/callback?st%61te=s+%41").is_ok());
        assert_eq!(decode("%+4"), "% 4");
        assert_eq!(
            carrier_callback("auth.callback http://127.0.0.1/cb", None),
            None
        );
    }

    #[test]
    fn the_carrier_callback_and_the_summary() {
        let grants = "auth.callback https://app.example/.exact/auth/callback\nauth.callback social.exact.bluesky:/oauth";
        assert_eq!(
            carrier_callback(grants, None).as_deref(),
            Some("social.exact.bluesky:/oauth")
        );
        assert_eq!(
            carrier_callback(grants, Some("http://127.0.0.1:9/")).as_deref(),
            Some("http://127.0.0.1:9/.exact/auth/callback")
        );
        assert_eq!(carrier_callback("net.fetch https://x", None), None);
        let summary = session("social.exact.bluesky:/oauth").summary();
        assert!(summary.starts_with(r#"{"origin":"https://bsky.social","path":"/oauth/authorize","params":["client_id","request_uri","id_token_hint"],"client_id":"https://app.example/c.json","request_uri":"urn:x""#), "{summary}");
        assert!(
            !summary.contains("secret"),
            "never another parameter's value"
        );
        let body = session("a.b:/c").to_json();
        assert_eq!(Session::from_json(&body).unwrap(), session("a.b:/c"));
        assert!(
            Session::from_json(r#"{"url":"https://x/","callback":"a.b:/c","state":""}"#).is_err()
        );
        assert!(
            Session::from_json(r#"{"url":"ftp://x/","callback":"a.b:/c","state":"s"}"#).is_err()
        );
        assert!(
            Session::from_json(r#"{"url":"https://x/","callback":"a.b://c","state":"s"}"#).is_ok()
        );
        assert!(
            Session::from_json(r#"{"url":"https://x/","callback":"a.b:c","state":"s"}"#).is_err()
        );
    }

    #[test]
    fn authorization_userinfo_is_refused_without_disclosure() {
        let body = r#"{"url":"https://user:review-secret@login.example.com/authorize?code=hidden","callback":"a.b:/cb","state":"s"}"#;
        let error = Session::from_json(body).unwrap_err();
        assert!(error.contains("userinfo"), "{error}");
        assert!(!error.contains("review-secret"), "{error}");
        let body = r#"{"url":"https://login.example.com/authorize","callback":"https://user:review-secret@app.example/cb","state":"s"}"#;
        let error = Session::from_json(body).unwrap_err();
        assert!(error.contains("userinfo"), "{error}");
        assert!(!error.contains("review-secret"), "{error}");
        let mut direct = session("a.b:/cb");
        direct.url = "https://user:review-secret@login.example.com/authorize".into();
        let summary = direct.summary();
        assert!(summary.contains("https://login.example.com"), "{summary}");
        assert!(!summary.contains("review-secret"), "{summary}");
        let error =
            check_grants(&direct, "auth.session https://*.example.com", None, None).unwrap_err();
        assert!(
            error.contains("userinfo") && !error.contains("review-secret"),
            "{error}"
        );
        direct.url = "https://login.example.com/authorize".into();
        direct.callback = "https://user:review-secret@app.example/cb".into();
        assert!(!direct.summary().contains("review-secret"));
        let error = check_grants(
            &direct,
            "auth.session https://login.example.com",
            None,
            None,
        )
        .unwrap_err();
        assert!(
            error.contains("userinfo") && !error.contains("review-secret"),
            "{error}"
        );
    }

    #[test]
    fn backslashed_https_urls_never_reach_a_grant_refusal() {
        for callback in [
            r"https:\/user:review-secret@app.example/cb",
            r"https://\/user:review-secret@app.example/cb",
            "https://app.example/c\tb@user:review-secret",
        ] {
            let body = format!(
                r#"{{"url":"https://login.example.com/authorize","callback":{callback:?},"state":"s"}}"#
            );
            let error = Session::from_json(&body).unwrap_err();
            assert!(!error.contains("review-secret"), "{error}");
            let mut direct = session(callback);
            direct.url = "https://login.example.com/authorize".into();
            let error = check_grants(
                &direct,
                "auth.session https://login.example.com",
                None,
                None,
            )
            .unwrap_err();
            assert!(!error.contains("review-secret"), "{error}");
        }
    }

    #[test]
    fn single_slash_https_callback_never_reaches_a_grant_refusal() {
        let body = r#"{"url":"https://login.example.com/authorize","callback":"https:/user:review-secret@app.example/cb","state":"s"}"#;
        let error = Session::from_json(body).unwrap_err();
        assert!(!error.contains("review-secret"), "{error}");
        let mut direct = session("https:/user:review-secret@app.example/cb");
        direct.url = "https://login.example.com/authorize".into();
        assert!(!direct.summary().contains("review-secret"));
        let error = check_grants(
            &direct,
            "auth.session https://login.example.com",
            None,
            None,
        )
        .unwrap_err();
        assert!(!error.contains("review-secret"), "{error}");
    }
}
