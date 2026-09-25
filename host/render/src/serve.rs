//! The server (LLP 1048.000 D10, D11): `<app>-render --serve <dist>`
//! renders a page per request, on loopback, from the built web app.
//!
//! A route declared `render=build`, `cached` or `request`, and any location
//! the router sends to its not-found route, is rendered with a new
//! runner and module realm (D10), then composed over the
//! built shell ([`crate::page`]). A `client` route gets the shell. Files
//! under `dist/` are served as they are; `/.exact/health` answers `ok`.
//! Renders run on a fixed set of workers behind a bounded queue: a request
//! that finds the queue full is a 503 at once. A failed render, or one that
//! panics, is a 500 and the server keeps serving. Each render prints one
//! line: location, status, time, answers, pending and bytes.
//! Successful `cached` pages are also kept at the origin for their public
//! lifetime, bounded to 64 locations and 32 MiB. Other routes render fresh.

use crate::encode::{self, Accepts, Variants};
use crate::{page, render, Rendered};
use exact_plan::{Plan, RenderPolicy};
use exact_runner::DataSource;
use exact_web::document::{canonical_location, route_at, Site};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::{Read, Seek, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The largest page the server sends (D10's bound on output bytes).
pub(crate) const MAX_PAGE: usize = 16 << 20;
const MAX_CACHED_PAGES: usize = 64;
const MAX_CACHE_BYTES: usize = 32 << 20;

/// How the server runs.
pub struct Serve {
    /// The built web app: its files, its shell (`shell.html`, or
    /// `index.html` when nothing renders at build) and its plan (`app.plan`).
    pub dist: PathBuf,
    /// The loopback port; 0 picks one.
    pub port: u16,
    /// The app's name: the title of a page whose head sets none.
    pub name: String,
    /// The origin canonical URLs are built from — never a request's `Host`.
    pub origin: Option<String>,
    /// Each render's deadline.
    pub deadline: Duration,
    /// Renders at once.
    pub renders: usize,
    /// Requests that may wait for a render before the server answers 503.
    pub queue: usize,
    /// The page viewport documents render at (D4).
    pub viewport: exact_runner::Viewport,
    /// A rendered route's shared-cache lifetime (`s-maxage`), until routes
    /// declare their own.
    pub lifetime: Duration,
}

/// A bound server, not yet serving.
pub struct Server {
    listener: TcpListener,
    shared: Shared,
    stop: Arc<AtomicBool>,
}

/// Drains a running server (D10): it stops accepting, answers what it has
/// taken — the renders in flight included — and [`Server::run`] returns.
#[derive(Clone)]
pub struct Stopper(Arc<AtomicBool>);

impl Stopper {
    /// Start draining.
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

struct Shared {
    serve: Serve,
    plan: Plan,
    shell: String,
    csp: String,
    pages: Mutex<VecDeque<CachedPage>>,
    variants: Variants,
}

struct CachedPage {
    target: String,
    created: Instant,
    response: Response,
    /// Its bodies at the best compression, made by a thread of their own;
    /// until then a hit is compressed as it is sent, as a render is.
    best: Arc<OnceLock<encode::Best>>,
}

impl Server {
    /// Bind `127.0.0.1:<port>` over `serve.dist`, with `plan` (the one the
    /// dist's wasm carries) and `grants` (the app's, for the pages' CSP).
    pub fn bind(serve: Serve, plan: Plan, grants: &str) -> std::io::Result<Server> {
        let shell = ["shell.html", "index.html"]
            .iter()
            .find_map(|name| std::fs::read_to_string(serve.dist.join(name)).ok())
            .ok_or_else(|| std::io::Error::other("the dist has no shell"))?;
        let listener = TcpListener::bind(("127.0.0.1", serve.port))?;
        let csp = csp(grants, &serve.dist);
        // The transport's first start in a process is slow (Apple's takes
        // seconds); pay it here, not in the first request.
        drop(crate::Executor::start(grants));
        let variants = Variants::default();
        variants.warm(encode::files(&serve.dist));
        Ok(Server {
            listener,
            stop: Arc::new(AtomicBool::new(false)),
            shared: Shared {
                serve,
                plan,
                shell,
                csp,
                pages: Mutex::new(VecDeque::new()),
                variants,
            },
        })
    }

    /// Where it listens.
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().expect("a bound listener")
    }

    /// What drains it.
    pub fn stopper(&self) -> Stopper {
        Stopper(self.stop.clone())
    }

    /// Serve until drained ([`Stopper`]). `data` makes each render's source.
    pub fn run<D: DataSource + 'static>(self, data: fn() -> D) -> std::io::Result<()> {
        let shared = Arc::new(self.shared);
        // The connections waiting, and how many workers are free to take one.
        let waiting = Arc::new((
            Mutex::new((VecDeque::<TcpStream>::new(), 0usize)),
            Condvar::new(),
        ));
        for _ in 0..shared.serve.renders.max(1) {
            let (shared, waiting) = (shared.clone(), waiting.clone());
            std::thread::spawn(move || {
                let (state, ready) = &*waiting;
                let mut answered = None;
                loop {
                    // Free for the next request while the last one closes.
                    state.lock().unwrap().1 += 1;
                    if let Some(stream) = answered.take() {
                        close(stream);
                    }
                    let stream = {
                        let mut state = state.lock().unwrap();
                        loop {
                            if let Some(stream) = state.0.pop_front() {
                                state.1 -= 1;
                                break stream;
                            }
                            state = ready.wait(state).unwrap();
                        }
                    };
                    answered = Some(handle(stream, &shared, data));
                }
            });
        }
        // Accepting polls, so a drain is noticed within a tick.
        self.listener.set_nonblocking(true)?;
        while !self.stop.load(Ordering::SeqCst) {
            let mut stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(_) => continue,
            };
            // An accepted socket inherits the listener's mode on macOS.
            let _ = stream.set_nonblocking(false);
            let (state, ready) = &*waiting;
            let mut state = state.lock().unwrap();
            if state.0.len() >= state.1 + shared.serve.queue {
                drop(state);
                let busy = Response::text(503, "busy\n")
                    .header("Cache-Control", "no-store")
                    .header("Retry-After", "1");
                // Read what was sent first: closing on unread bytes resets
                // the connection before the client reads the answer.
                let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                let _ = read_request(&mut stream);
                busy.write(&mut stream, false, &shared.csp);
                close(stream);
                continue;
            }
            state.0.push_back(stream);
            ready.notify_one();
        }
        // Draining: no new connection is taken, and what was taken is
        // answered — within a render's deadline, twice over, and a margin.
        drop(self.listener);
        let renders = shared.serve.renders.max(1);
        let until = Instant::now() + shared.serve.deadline * 2 + Duration::from_secs(10);
        while Instant::now() < until {
            let state = waiting.0.lock().unwrap();
            if state.0.is_empty() && state.1 == renders {
                break;
            }
            drop(state);
            std::thread::sleep(Duration::from_millis(10));
        }
        // A worker is free while it closes its last connection.
        std::thread::sleep(Duration::from_millis(250));
        Ok(())
    }
}

/// The request fields that affect a response.
struct Request {
    method: String,
    target: String,
    if_none_match: Option<String>,
    revalidate: bool,
    no_store: bool,
    accepts: Accepts,
    /// A CDN forwarded it (RFC 8586's `CDN-Loop`): its response carries the
    /// surrogate keys.
    cdn: bool,
}

#[derive(Clone)]
struct Response {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
    file: Option<(Arc<std::fs::File>, u64)>,
}

impl Response {
    fn text(status: u16, body: &str) -> Response {
        Response {
            status,
            headers: vec![("Content-Type", "text/plain; charset=utf-8".into())],
            body: body.as_bytes().to_vec(),
            file: None,
        }
    }

    fn header(mut self, name: &'static str, value: impl Into<String>) -> Response {
        self.headers.push((name, value.into()));
        self
    }

    fn write(&self, stream: &mut TcpStream, head: bool, csp: &str) {
        // Header values can contain app data; refuse the entire response before
        // writing anything, including on the 304 path.
        if self.headers.iter().any(|(_, value)| invalid_header(value)) || invalid_header(csp) {
            Response::text(500, "invalid response header\n").write(stream, head, "");
            return;
        }
        let reason = match self.status {
            200 => "OK",
            301 => "Moved Permanently",
            304 => "Not Modified",
            400 => "Bad Request",
            404 => "Not Found",
            405 => "Method Not Allowed",
            410 => "Gone",
            500 => "Internal Server Error",
            503 => "Service Unavailable",
            _ => "",
        };
        let mut out = format!("HTTP/1.1 {} {reason}\r\n", self.status);
        // A 304 carries what updates the copy the client holds (RFC 9110
        // §15.4.5), nothing of the representation's own: its CSP, surrogate
        // keys and length are the stored response's already.
        if self.status == 304 {
            for (name, value) in &self.headers {
                if matches!(*name, "ETag" | "Cache-Control" | "Vary" | "Expires" | "Age") {
                    let _ = write!(out, "{name}: {value}\r\n");
                }
            }
            out.push_str("Connection: close\r\n\r\n");
            let _ = stream.write_all(out.as_bytes());
            let _ = stream.flush();
            return;
        }
        for (name, value) in &self.headers {
            let _ = write!(out, "{name}: {value}\r\n");
        }
        // A page's policy. On a script's response a CSP would govern only a
        // worker made from it, and a module worker evaluates the module text
        // it verified against the receipt (LLP 1027.002 D2).
        let page = self
            .headers
            .iter()
            .any(|(name, value)| *name == "Content-Type" && value.starts_with("text/html"));
        if page {
            let _ = write!(out, "Content-Security-Policy: {csp}\r\n");
        }
        let _ = write!(
            out,
            "X-Content-Type-Options: nosniff\r\nReferrer-Policy: strict-origin-when-cross-origin\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.file.as_ref().map_or(self.body.len() as u64, |(_, size)| *size)
        );
        let _ = stream.write_all(out.as_bytes());
        if !head {
            if let Some((file, size)) = &self.file {
                let _ = std::io::copy(&mut file.as_ref().take(*size), stream);
            } else {
                let _ = stream.write_all(&self.body);
            }
        }
        let _ = stream.flush();
    }
}

fn invalid_header(value: &str) -> bool {
    value
        .bytes()
        .any(|byte| byte == 0 || byte == b'\r' || byte == b'\n')
}

/// Close gracefully: nothing more to send, and whatever the client still
/// sends is read and dropped, so an answer isn't lost to a reset.
fn close(mut stream: TcpStream) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let mut sink = [0u8; 4096];
    let mut drained = 0;
    while let Ok(n) = stream.read(&mut sink) {
        drained += n;
        if n == 0 || drained > 64 << 10 {
            break;
        }
    }
}

/// Answer one connection; the worker closes it.
fn handle<D: DataSource>(mut stream: TcpStream, shared: &Shared, data: fn() -> D) -> TcpStream {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let Ok(request) = read_request(&mut stream) else {
        Response::text(400, "bad request\n").write(&mut stream, false, &shared.csp);
        return stream;
    };
    let head = request.method == "HEAD";
    let response = if request.method != "GET" && !head {
        Response::text(405, "GET or HEAD\n").header("Allow", "GET, HEAD")
    } else {
        finish(respond(&request, shared, data), &request)
    };
    response.write(&mut stream, head, &shared.csp);
    stream
}

/// A response as the client accepts it (D11): a page, the sitemap or any
/// other body made for this request, compressed now — brotli, else gzip —
/// with the encoding in its ETag, and a 304 when the client holds that
/// representation. A dist file carries its own `Vary` and made variant.
/// The surrogate keys go only to a CDN, which purges by them; a browser
/// never reads them.
fn finish(mut response: Response, request: &Request) -> Response {
    if !request.cdn {
        response
            .headers
            .retain(|(name, _)| !matches!(*name, "Surrogate-Key" | "Cache-Tag"));
    }
    let kind = response
        .headers
        .iter()
        .find(|(name, _)| *name == "Content-Type")
        .map_or("", |(_, value)| value.as_str());
    let made = response.headers.iter().any(|(name, _)| *name == "Vary");
    if response.status == 304 || response.file.is_some() || made || !encode::compressible(kind) {
        return response;
    }
    response.headers.push(("Vary", "Accept-Encoding".into()));
    if let Some((encoding, body)) = encode::now(&response.body, request.accepts) {
        encoded(&mut response, encoding, body);
    }
    if response.status == 200
        && response.headers.iter().any(|(name, value)| {
            *name == "ETag" && encode::none_match(request.if_none_match.as_deref(), value)
        })
    {
        response.status = 304;
    }
    response
}

/// `body` as `response`'s content in `encoding`, which its ETag names. A page
/// compressed as it was sent and one made at the best share their tag: they
/// decode to the same bytes, and the server sends no ranges.
fn encoded(response: &mut Response, encoding: encode::Encoding, body: Vec<u8>) {
    response.body = body;
    response
        .headers
        .push(("Content-Encoding", encoding.name().into()));
    for (name, value) in &mut response.headers {
        if *name == "ETag" {
            *value = format!("{}-{}\"", value.trim_end_matches('"'), encoding.name());
        }
    }
}

/// Of `locations`, the ones a crawler may index, as the build's sitemap
/// keeps them: each is answered as a request is (a cached page from the
/// origin's cache), and a page that is gone, failed, or says `noindex` is
/// left out. A page at its deadline stays: it exists, and asks to be read
/// again. Renders run `--renders` at a time.
fn indexed<D: DataSource>(shared: &Shared, data: fn() -> D, locations: Vec<String>) -> Vec<String> {
    let keep = |location: &String| {
        let request = Request {
            method: "GET".into(),
            target: location.clone(),
            if_none_match: None,
            revalidate: false,
            no_store: false,
            accepts: Accepts::default(),
            cdn: false,
        };
        let response = respond(&request, shared, data);
        let noindex = response.headers.iter().any(|(name, value)| {
            *name == "X-Robots-Tag" && value.to_ascii_lowercase().contains("noindex")
        });
        matches!(response.status, 200 | 503) && !noindex
    };
    let mut kept = Vec::with_capacity(locations.len());
    for batch in locations.chunks(shared.serve.renders.max(1)) {
        let verdicts: Vec<bool> = std::thread::scope(|scope| {
            let running: Vec<_> = batch
                .iter()
                .map(|location| scope.spawn(move || keep(location)))
                .collect();
            running
                .into_iter()
                .map(|handle| handle.join().unwrap_or(false))
                .collect()
        });
        kept.extend(
            batch
                .iter()
                .zip(verdicts)
                .filter(|(_, keep)| *keep)
                .map(|(location, _)| location.clone()),
        );
    }
    kept
}

/// Whether a path names a file (its last segment has an extension the
/// server knows as a type) rather than a page.
fn asset_shaped(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or("");
    last.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && extension != "html"
            && content_type(&extension.to_ascii_lowercase()) != "application/octet-stream"
    })
}

fn read_request(stream: &mut TcpStream) -> Result<Request, ()> {
    let mut bytes = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk).map_err(|_| ())?;
        if n == 0 || bytes.len() + n > 16 << 10 {
            return Err(());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8(bytes).map_err(|_| ())?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next().ok_or(())?.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (first.next(), first.next(), first.next(), first.next())
    else {
        return Err(());
    };
    if !version.starts_with("HTTP/1.")
        || !target.starts_with('/')
        || target.chars().any(char::is_control)
    {
        return Err(());
    }
    let headers: Vec<_> = lines
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_once(':'))
        .collect();
    let if_none_match = headers
        .iter()
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("if-none-match"))
        .map(|(_, value)| value.trim().to_string());
    let revalidate = headers.iter().any(|(name, value)| {
        name.trim().eq_ignore_ascii_case("cache-control")
            && value.split(',').any(|part| {
                matches!(
                    part.trim().to_ascii_lowercase().as_str(),
                    "no-cache" | "no-store" | "max-age=0"
                )
            })
    });
    let no_store = headers.iter().any(|(name, value)| {
        name.trim().eq_ignore_ascii_case("cache-control")
            && value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case("no-store"))
    });
    let accepts = headers
        .iter()
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("accept-encoding"))
        .map_or_else(Accepts::default, |(_, value)| Accepts::parse(value));
    let cdn = headers
        .iter()
        .any(|(name, _)| name.trim().eq_ignore_ascii_case("cdn-loop"));
    Ok(Request {
        method: method.to_string(),
        target: target.to_string(),
        if_none_match,
        revalidate,
        no_store,
        accepts,
        cdn,
    })
}

fn respond<D: DataSource>(request: &Request, shared: &Shared, data: fn() -> D) -> Response {
    let path = request
        .target
        .split_once('?')
        .map_or(request.target.as_str(), |(path, _)| path);
    // Do not reflect a URL spelling that browsers can reinterpret as an origin
    // into Location. Dot segments follow the canonicalizer; check escapes too.
    let safe = percent_decode(path, true)
        .is_some_and(|decoded| !decoded.contains('\\') && !decoded.chars().any(char::is_control));
    if !safe {
        return Response::text(400, "bad path\n");
    }
    if path == "/.exact/health" {
        return Response::text(200, "ok\n").header("Cache-Control", "no-store");
    }
    if path == "/sitemap.xml" {
        return sitemap(shared, data);
    }
    if path == "/robots.txt" {
        // Neutral until hosting decides a crawler policy (LLP 1048 §9.9).
        let sitemap = shared
            .serve
            .origin
            .as_deref()
            .map_or(String::new(), |origin| {
                format!("Sitemap: {}/sitemap.xml\n", origin.trim_end_matches('/'))
            });
        let lifetime = shared.serve.lifetime.as_secs();
        return Response::text(200, &format!("User-agent: *\nAllow: /\n{sitemap}")).header(
            "Cache-Control",
            format!("public, max-age=0, s-maxage={lifetime}"),
        );
    }
    if let Some((file, kind)) = static_file(&shared.serve.dist, path) {
        if std::fs::metadata(&file).is_ok_and(|meta| meta.len() > MAX_PAGE as u64) {
            return stream_file(&file, kind, request)
                .unwrap_or_else(|| Response::text(404, "not found\n"));
        }
        let compressible = encode::compressible(kind);
        let Some(served) = shared.variants.serve(&file, request.accepts, compressible) else {
            return Response::text(404, "not found\n");
        };
        let mut headers = vec![
            ("Content-Type", kind.into()),
            // Not content-addressed yet (D10): revalidated, by its ETag.
            ("Cache-Control", "no-cache".into()),
            ("ETag", served.etag.clone()),
        ];
        if compressible {
            headers.push(("Vary", "Accept-Encoding".into()));
        }
        if let Some(encoding) = served.encoding {
            headers.push(("Content-Encoding", encoding.name().into()));
        }
        let fresh = encode::none_match(request.if_none_match.as_deref(), &served.etag);
        return Response {
            status: if fresh { 304 } else { 200 },
            headers,
            body: served.body.to_vec(),
            file: None,
        };
    }
    // One URL per page: any other spelling redirects to the canonical one.
    let canonical = canonical_location(&request.target);
    if canonical != request.target {
        return Response::text(301, "moved\n").header("Location", canonical);
    }
    let route = route_at(&shared.plan, &request.target);
    let policy = route.map(|r| r.render);
    let notfound = route.is_some_and(|r| r.notfound);
    // A file the dist doesn't have (a browser's `/favicon.ico`, an old
    // script) is a plain 404: the not-found document is for readers, and
    // rendering it cost 22 ms and 36 KB a visit (Interview's measurement).
    if notfound && asset_shaped(path) {
        return Response::text(404, "not found\n")
            .header("Cache-Control", "public, max-age=0, s-maxage=60");
    }
    if !notfound && matches!(policy, None | Some(RenderPolicy::Client)) {
        return Response {
            status: 200,
            headers: vec![
                ("Content-Type", "text/html; charset=utf-8".into()),
                ("Cache-Control", "no-cache".into()),
            ],
            body: shared.shell.clone().into_bytes(),
            file: None,
        };
    }
    if policy != Some(RenderPolicy::Cached) || shared.serve.lifetime.is_zero() || request.no_store {
        return document(request, policy, notfound, shared, data);
    }
    {
        let mut pages = shared.pages.lock().unwrap();
        pages.retain(|page| page.created.elapsed() < shared.serve.lifetime);
        if !request.revalidate {
            if let Some(page) = pages.iter().find(|page| page.target == request.target) {
                let mut response = page
                    .response
                    .clone()
                    .header("Age", page.created.elapsed().as_secs().to_string());
                if let Some((encoding, body)) =
                    page.best.get().and_then(|best| best.pick(request.accepts))
                {
                    // Made: `finish` leaves a response with its `Vary` as it is,
                    // so its 304 is decided here.
                    response.headers.push(("Vary", "Accept-Encoding".into()));
                    encoded(&mut response, encoding, body.to_vec());
                    if response.headers.iter().any(|(name, value)| {
                        *name == "ETag"
                            && encode::none_match(request.if_none_match.as_deref(), value)
                    }) {
                        response.status = 304;
                    }
                    return response;
                }
                if response.headers.iter().any(|(name, value)| {
                    *name == "ETag" && request.if_none_match.as_ref() == Some(value)
                }) {
                    response.status = 304;
                }
                return response;
            }
        }
    }
    // Render without holding the cache lock. A concurrent miss may render too;
    // it never delays an unrelated route or holds a module realm in the cache.
    let unconditional = Request {
        method: request.method.clone(),
        target: request.target.clone(),
        if_none_match: None,
        revalidate: true,
        no_store: false,
        accepts: request.accepts,
        cdn: request.cdn,
    };
    let mut response = document(&unconditional, policy, notfound, shared, data);
    if response.status == 200 && response.body.len() <= MAX_CACHE_BYTES {
        let mut pages = shared.pages.lock().unwrap();
        pages.retain(|page| page.target != request.target);
        while pages.len() >= MAX_CACHED_PAGES
            || pages
                .iter()
                .map(|page| {
                    page.response.body.len() + page.best.get().map_or(0, encode::Best::bytes)
                })
                .sum::<usize>()
                + response.body.len()
                > MAX_CACHE_BYTES
        {
            pages.pop_front();
        }
        // This response goes out compressed as it is sent; later hits get the
        // best, made here once, off every request's path.
        let best = Arc::new(OnceLock::new());
        let (made, body) = (Arc::clone(&best), response.body.clone());
        let _ = std::thread::Builder::new()
            .name("exact-render-best".into())
            .spawn(move || made.set(encode::Best::of(&body)));
        pages.push_back(CachedPage {
            target: request.target.clone(),
            created: Instant::now(),
            response: response.clone(),
            best,
        });
    }
    if response.status == 200
        && response
            .headers
            .iter()
            .any(|(name, value)| *name == "ETag" && request.if_none_match.as_ref() == Some(value))
    {
        response.status = 304;
    }
    response
}

/// Large assets bypass compression and memory caches. Hash in a fixed buffer
/// for a content validator, rewind, then copy to the socket in a fixed buffer.
/// As with every response, slow clients occupy one bounded worker until timeout.
fn stream_file(path: &Path, kind: &str, request: &Request) -> Option<Response> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut remaining = size;
    while remaining > 0 {
        let capacity = remaining.min(buffer.len() as u64) as usize;
        let count = file.read(&mut buffer[..capacity]).ok()?;
        if count == 0 {
            return None;
        }
        hash.update(&buffer[..count]);
        remaining -= count as u64;
    }
    file.rewind().ok()?;
    let etag = format!("\"{:x}\"", hash.finalize());
    let fresh = encode::none_match(request.if_none_match.as_deref(), &etag);
    Some(Response {
        status: if fresh { 304 } else { 200 },
        headers: vec![
            ("Content-Type", kind.into()),
            ("Cache-Control", "no-cache".into()),
            ("ETag", etag),
        ],
        body: Vec::new(),
        file: Some((Arc::new(file), size)),
    })
}

fn document<D: DataSource>(
    request: &Request,
    policy: Option<RenderPolicy>,
    notfound: bool,
    shared: &Shared,
    data: fn() -> D,
) -> Response {
    let serve = &shared.serve;
    let started = Instant::now();
    let site = Site {
        name: &serve.name,
        origin: serve.origin.as_deref(),
    };
    let location = request.target.as_str();
    let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        render(
            &shared.plan,
            data,
            serve.viewport,
            location,
            &site,
            serve.deadline,
        )
        .and_then(|rendered| {
            let html = page(&shared.shell, &rendered)?;
            if html.len() > MAX_PAGE {
                return Err(format!("the page is {} bytes", html.len()));
            }
            Ok((rendered, html))
        })
    }))
    .unwrap_or_else(|_| Err("the render panicked".into()));
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    let (rendered, html) = match rendered {
        Ok(done) => done,
        Err(error) => {
            println!("render {location} 500 {ms:.1}ms error={error:?}");
            let _ = std::io::stdout().flush();
            return Response {
                status: 500,
                headers: vec![
                    ("Content-Type", "text/html; charset=utf-8".into()),
                    ("Cache-Control", "no-store".into()),
                ],
                file: None,
                body: b"<!doctype html>\n<title>Unavailable</title>\n<p>This page couldn't be rendered.</p>\n".to_vec(),
            };
        }
    };
    let status = rendered.status(notfound);
    println!(
        "render {location} {status} {ms:.1}ms answers={} pending={} bytes={}",
        rendered.state.answers.len(),
        rendered.state.pending.len(),
        html.len()
    );
    let _ = std::io::stdout().flush();
    let mut response = Response {
        status,
        headers: vec![("Content-Type", "text/html; charset=utf-8".into())],
        body: html.into_bytes(),
        file: None,
    };
    let lifetime = serve.lifetime.as_secs();
    match status {
        503 => {
            response.headers.push(("Cache-Control", "no-store".into()));
            response.headers.push(("Retry-After", "1".into()));
        }
        404 | 410 => {
            response
                .headers
                .push(("Cache-Control", "public, max-age=0, s-maxage=60".into()));
        }
        _ if policy == Some(RenderPolicy::Request) => {
            response.headers.push(("Cache-Control", "no-store".into()));
        }
        _ => response.headers.push((
            "Cache-Control",
            format!("public, max-age=0, s-maxage={lifetime}, stale-while-revalidate={lifetime}"),
        )),
    }
    if let Some(robots) = &rendered.document.head.robots {
        response.headers.push(("X-Robots-Tag", robots.clone()));
    }
    let keys = keys(&rendered);
    if !keys.is_empty() {
        response.headers.push(("Surrogate-Key", keys.join(" ")));
        response.headers.push(("Cache-Tag", keys.join(",")));
    }
    if status != 503 {
        let tag = format!("\"{}\"", &hex(&response.body)[..32]);
        if request.if_none_match.as_deref() == Some(tag.as_str()) {
            response.status = 304;
        }
        response.headers.push(("ETag", tag));
    }
    response
}

/// The sitemap (LLP 1048.000 D2): every rendered route's location — a
/// parameterized one's as its `pages=` source lists them now, a route
/// without one left out — absolute against the configured origin. Without
/// an origin there is none.
fn sitemap<D: DataSource>(shared: &Shared, data: fn() -> D) -> Response {
    let Some(origin) = shared.serve.origin.as_deref() else {
        return Response::text(404, "no origin, no sitemap\n");
    };
    let plan = &shared.plan;
    let mut locations = Vec::new();
    for row in plan
        .routes
        .iter()
        .filter(|r| r.render != RenderPolicy::Client && !r.notfound)
    {
        let pattern = plan.str(row.pattern);
        if !pattern.split('/').any(|s| s.starts_with(':')) {
            locations.push(pattern.to_string());
            continue;
        }
        let listed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::pages(plan, data(), row, shared.serve.deadline)
        }))
        .unwrap_or_else(|_| Err("the pages source panicked".into()));
        match listed {
            Ok(found) => locations.extend(found),
            Err(error) => {
                println!("sitemap 503 error={error:?}");
                return Response::text(503, "the sitemap's pages didn't answer\n")
                    .header("Cache-Control", "no-store")
                    .header("Retry-After", "1");
            }
        }
    }
    let locations = indexed(shared, data, locations);
    let xml = |t: &str| {
        t.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let origin = origin.trim_end_matches('/');
    let mut body = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for location in &locations {
        let _ = writeln!(
            body,
            "  <url><loc>{}</loc></url>",
            xml(&format!("{origin}{location}"))
        );
    }
    body.push_str("</urlset>\n");
    let lifetime = shared.serve.lifetime.as_secs();
    Response {
        status: 200,
        headers: vec![
            ("Content-Type", "application/xml; charset=utf-8".into()),
            (
                "Cache-Control",
                format!("public, max-age=0, s-maxage={lifetime}"),
            ),
        ],
        body: body.into_bytes(),
        file: None,
    }
}

/// The surrogate keys (D11): each answer's source, and its source with a
/// digest of its arguments — `post` purges every post page, `post:<hex>`
/// the pages that showed that one.
fn keys(rendered: &Rendered) -> Vec<String> {
    let mut keys = Vec::new();
    for (_, source, args, _) in &rendered.state.answers {
        let bytes = exact_plan::Value::list(args.clone()).to_bytes();
        for key in [source.clone(), format!("{source}:{}", &hex(&bytes)[..16])] {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    keys
}

fn hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// A file `dist` serves as it is: anything under `/.exact/`, and any other
/// file but a page (`.html`), which is rendered. Never outside `dist`.
fn static_file(dist: &Path, path: &str) -> Option<(PathBuf, &'static str)> {
    let decoded = percent_decode(path, false)?;
    if decoded.split('/').any(|part| part == ".." || part == ".") || decoded.contains('\\') {
        return None;
    }
    let mut file = dist.join(decoded.trim_start_matches('/'));
    let exact = decoded.starts_with("/.exact/");
    if exact && file.is_dir() {
        file = file.join("index.html");
    }
    let root = dist.canonicalize().ok()?;
    let file = file.canonicalize().ok()?;
    if !file.starts_with(&root) || !file.is_file() {
        return None;
    }
    let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("");
    if extension == "html" && !exact {
        return None;
    }
    Some((file.clone(), content_type(extension)))
}

fn percent_decode(path: &str, allow_slash: bool) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            let byte = u8::from_str_radix(hex, 16).ok()?;
            if (!allow_slash && byte == b'/') || byte == 0 {
                return None;
            }
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

pub(crate) fn content_type(extension: &str) -> &'static str {
    match extension {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" | "wgsl" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

/// The pages' Content-Security-Policy (D11): scripts only from the app's
/// origin (the only inline scripts are inert data), the wasm, and fetches
/// to the origins the app's grants name.
fn csp(grants: &str, dist: &Path) -> String {
    let mut connect = String::from("'self'");
    for line in grants.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("net.fetch") {
            continue;
        }
        if let Some(target) = words.next() {
            // An origin is scheme://host[:port]: the target up to its path.
            let origin = target.split_once("://").map(|(scheme, rest)| {
                format!("{scheme}://{}", rest.split('/').next().unwrap_or(""))
            });
            if let Some(origin) = origin {
                connect.push(' ');
                connect.push_str(&origin);
            }
        }
    }
    // The admitted TypeScript module and its host prelude run in a private
    // same-origin realm. Admit their exact baked bytes, never arbitrary inline JS.
    // A document's one inline script, the host's capture script, likewise.
    use sha2::{Digest, Sha256};
    let hash = |bytes: &[u8]| exact_data::envelope::base64(&Sha256::digest(bytes));
    let mut scripts = format!(
        "'self' 'wasm-unsafe-eval' 'sha256-{}'",
        hash(crate::page::capture().as_bytes())
    );
    for file in ["module-prelude.js", "app.js"] {
        if let Ok(bytes) = std::fs::read(dist.join(file)) {
            let _ = write!(scripts, " 'sha256-{}'", hash(&bytes));
        }
    }
    format!(
        "default-src 'self'; script-src {scripts}; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; media-src 'self' blob: https:; font-src 'self' data:; connect-src {connect}; worker-src 'self' blob:; frame-src 'self' https:; object-src 'none'; base-uri 'self'; frame-ancestors 'self'"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_response_headers_fail_before_any_bytes_are_written() {
        for status in [200, 304] {
            for value in [
                "noindex\r\n\r\ninjected",
                "noindex\nX-Fake: yes",
                "noindex\0",
            ] {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
                let (mut server, _) = listener.accept().unwrap();
                Response::text(status, "original")
                    .header("X-Robots-Tag", value)
                    .write(&mut server, false, "default-src 'self'");
                server.shutdown(std::net::Shutdown::Write).unwrap();
                let mut received = String::new();
                client.read_to_string(&mut received).unwrap();
                assert!(received.starts_with("HTTP/1.1 500 "), "{received}");
                assert!(!received.contains("X-Robots-Tag"));
                assert!(!received.contains("injected"));
            }
        }
    }
}
