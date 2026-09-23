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

use crate::{page, render, Rendered};
use exact_plan::{Plan, RenderPolicy};
use exact_runner::DataSource;
use exact_web::document::{route_at, Site};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The largest page the server sends (D10's bound on output bytes).
const MAX_PAGE: usize = 16 << 20;
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
}

struct Shared {
    serve: Serve,
    plan: Plan,
    shell: String,
    csp: String,
    pages: Mutex<VecDeque<CachedPage>>,
}

struct CachedPage {
    target: String,
    created: Instant,
    response: Response,
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
        Ok(Server {
            listener,
            shared: Shared {
                serve,
                plan,
                shell,
                csp,
                pages: Mutex::new(VecDeque::new()),
            },
        })
    }

    /// Where it listens.
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().expect("a bound listener")
    }

    /// Serve until the process ends. `D::default()` is each render's source.
    pub fn run<D: DataSource + Default + 'static>(self) -> std::io::Result<()> {
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
                    answered = Some(handle::<D>(stream, &shared));
                }
            });
        }
        for stream in self.listener.incoming() {
            let Ok(mut stream) = stream else { continue };
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
}

#[derive(Clone)]
struct Response {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    fn text(status: u16, body: &str) -> Response {
        Response {
            status,
            headers: vec![("Content-Type", "text/plain; charset=utf-8".into())],
            body: body.as_bytes().to_vec(),
        }
    }

    fn header(mut self, name: &'static str, value: impl Into<String>) -> Response {
        self.headers.push((name, value.into()));
        self
    }

    fn write(&self, stream: &mut TcpStream, head: bool, csp: &str) {
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
            self.body.len()
        );
        let _ = stream.write_all(out.as_bytes());
        if !head && self.status != 304 {
            let _ = stream.write_all(&self.body);
        }
        let _ = stream.flush();
    }
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
fn handle<D: DataSource + Default>(mut stream: TcpStream, shared: &Shared) -> TcpStream {
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
        respond::<D>(&request, shared)
    };
    response.write(&mut stream, head, &shared.csp);
    stream
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
    if !version.starts_with("HTTP/1.") || !target.starts_with('/') {
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
    Ok(Request {
        method: method.to_string(),
        target: target.to_string(),
        if_none_match,
        revalidate,
        no_store,
    })
}

fn respond<D: DataSource + Default>(request: &Request, shared: &Shared) -> Response {
    let (path, query) = match request.target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (request.target.as_str(), None),
    };
    if path == "/.exact/health" {
        return Response::text(200, "ok\n").header("Cache-Control", "no-store");
    }
    if path == "/sitemap.xml" {
        return sitemap::<D>(shared);
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
        return match std::fs::read(&file) {
            Ok(body) => Response {
                status: 200,
                headers: vec![
                    ("Content-Type", kind.into()),
                    // Not content-addressed yet (D10): revalidate.
                    ("Cache-Control", "no-cache".into()),
                ],
                body,
            },
            Err(_) => Response::text(404, "not found\n"),
        };
    }
    // One URL per page: repeated slashes collapse, no trailing slash but `/`.
    let canonical = canonical_path(path);
    if canonical != path {
        let location = match query {
            Some(query) => format!("{canonical}?{query}"),
            None => canonical,
        };
        return Response::text(301, "moved\n").header("Location", location);
    }
    let route = route_at(&shared.plan, &request.target);
    let policy = route.map(|r| r.render);
    let notfound = route.is_some_and(|r| r.notfound);
    if !notfound && matches!(policy, None | Some(RenderPolicy::Client)) {
        return Response {
            status: 200,
            headers: vec![
                ("Content-Type", "text/html; charset=utf-8".into()),
                ("Cache-Control", "no-cache".into()),
            ],
            body: shared.shell.clone().into_bytes(),
        };
    }
    if policy != Some(RenderPolicy::Cached) || shared.serve.lifetime.is_zero() || request.no_store {
        return document::<D>(request, policy, notfound, shared);
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
    };
    let mut response = document::<D>(&unconditional, policy, notfound, shared);
    if response.status == 200 && response.body.len() <= MAX_CACHE_BYTES {
        let mut pages = shared.pages.lock().unwrap();
        pages.retain(|page| page.target != request.target);
        while pages.len() >= MAX_CACHED_PAGES
            || pages
                .iter()
                .map(|page| page.response.body.len())
                .sum::<usize>()
                + response.body.len()
                > MAX_CACHE_BYTES
        {
            pages.pop_front();
        }
        pages.push_back(CachedPage {
            target: request.target.clone(),
            created: Instant::now(),
            response: response.clone(),
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

fn document<D: DataSource + Default>(
    request: &Request,
    policy: Option<RenderPolicy>,
    notfound: bool,
    shared: &Shared,
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
            D::default(),
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
fn sitemap<D: DataSource + Default>(shared: &Shared) -> Response {
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
            crate::pages(plan, D::default(), row, shared.serve.deadline)
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

/// Collapse repeated slashes; no trailing slash except the root's.
fn canonical_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for part in path.split('/').filter(|p| !p.is_empty()) {
        out.push('/');
        out.push_str(part);
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

/// A file `dist` serves as it is: anything under `/.exact/`, and any other
/// file but a page (`.html`), which is rendered. Never outside `dist`.
fn static_file(dist: &Path, path: &str) -> Option<(PathBuf, &'static str)> {
    let decoded = percent_decode(path)?;
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

fn percent_decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            let byte = u8::from_str_radix(hex, 16).ok()?;
            if byte == b'/' || byte == 0 {
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

fn content_type(extension: &str) -> &'static str {
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
    let mut scripts = String::from("'self' 'wasm-unsafe-eval'");
    for file in ["module-prelude.js", "app.js"] {
        if let Ok(bytes) = std::fs::read(dist.join(file)) {
            use sha2::{Digest, Sha256};
            let hash = exact_data::envelope::base64(&Sha256::digest(bytes));
            let _ = write!(scripts, " 'sha256-{hash}'");
        }
    }
    format!(
        "default-src 'self'; script-src {scripts}; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; media-src 'self' blob: https:; font-src 'self' data:; connect-src {connect}; worker-src 'self' blob:; frame-src 'self' https:; object-src 'none'; base-uri 'self'; frame-ancestors 'self'"
    )
}
