//! The server (LLP 1048.000 D10, D11) over a small dist: each route's
//! policy, the statuses and headers of the HTTP contract, its files, and a
//! full queue.

use exact_plan::Value;
use exact_render::{Serve, Server};
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Response, Store};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const SRC: &str = r#"
routes nav
  tab home "/" render=build
    post "/post/:post" render=cached pages=posts("public")
    live "/live/:post" render=request
    draft "/draft/:post" render=cached pages=posts("drafts")
  tab app "/app"
  notfound render=build

shape Post
  title: string

component Blog
  resource post = post(params(nav, "post")) as shape Post else emptyPost()
  view
    column
      each e in stack(nav) key = e.id
        column
          when e.name == "notfound"
            head title="Not found" robots="noindex" status=404
            text "Nothing here"
          when e.name == "post" or e.name == "live"
            head title=post.title
            text post.title testId="title"
          when e.name == "draft"
            head title=post.title robots="noindex"
            text post.title
"#;

/// A post by its id: `slow` answers long after any deadline, `boom`
/// refuses (a render that fails), `down` shows its failure (503).
#[derive(Default)]
struct Posts;
static CACHE_READS: AtomicUsize = AtomicUsize::new(0);

fn post(title: &str) -> Value {
    Value::record(vec![Value::str(title)])
}

impl DataSource for Posts {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let id = match args.first() {
            Some(Value::List(ids)) => ids
                .first()
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            _ => String::new(),
        };
        match (source, id.as_str()) {
            ("emptyPost", _) => Ok(Answer::Now(post(""))),
            ("posts", _) if args == [Value::str("public")] => Ok(Answer::Now(Value::list(vec![
                Value::str("7"),
                Value::str("an idea"),
            ]))),
            ("posts", _) if args == [Value::str("drafts")] => {
                Ok(Answer::Now(Value::list(vec![Value::str("d1")])))
            }
            ("post", "slow") => Ok(Answer::Later(Request::continuation(1))),
            ("post", "boom") => Err(DataError::Unavailable("boom".into())),
            ("post", "cache-probe") => Ok(Answer::Now(post(&format!(
                "Read {}",
                CACHE_READS.fetch_add(1, Ordering::SeqCst) + 1
            )))),
            ("post", id) => Ok(Answer::Now(post(&format!("Post {id}")))),
            (other, _) => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn continuation(&mut self, _: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        Some(Box::new(|| {
            std::thread::sleep(Duration::from_secs(3));
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: Vec::new(),
            })
        }))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(post("Late")))
    }

    fn grants(&self) -> &str {
        "net.fetch https://api.blog.test/\n"
    }
}

/// A dist: the web host's shell, a script and an image.
fn dist(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("exact-render-serve-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    let shell = Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/index.html");
    std::fs::copy(shell, dir.join("shell.html")).unwrap();
    std::fs::write(dir.join("glue.js"), "// the glue\n").unwrap();
    std::fs::write(dir.join("module-prelude.js"), "prelude").unwrap();
    std::fs::write(dir.join("app.js"), "module").unwrap();
    std::fs::write(
        dir.join("big.js"),
        "export const words = ['the same words, again'];\n".repeat(200),
    )
    .unwrap();
    std::fs::write(dir.join("assets/dot.png"), [0x89, b'P', b'N', b'G']).unwrap();
    dir
}

fn start(name: &str, renders: usize, queue: usize, deadline: u64) -> SocketAddr {
    super::warm_transport();
    let serve = Serve {
        dist: dist(name),
        port: 0,
        name: "Blog".into(),
        origin: Some("https://blog.test".into()),
        deadline: Duration::from_millis(deadline),
        renders,
        queue,
        viewport: Default::default(),
        lifetime: Duration::from_secs(120),
    };
    let plan = contract::compile(SRC).unwrap();
    let server = Server::bind(serve, plan, Posts.grants()).unwrap();
    let addr = server.addr();
    std::thread::spawn(move || server.run(|| Posts));
    addr
}

/// Status, headers (lowercased names) and body of one request.
fn fetch(addr: SocketAddr, request: &str) -> (u16, Vec<(String, String)>, String) {
    let (status, headers, body) = fetch_bytes(addr, request);
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

/// How long a test waits on the server before it fails, naming the wait: a
/// hang bound, not a deadline (a loaded Mac is slow, never this slow).
const BOUND: Duration = Duration::from_secs(60);

/// The same, the body as bytes.
fn fetch_bytes(addr: SocketAddr, request: &str) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let what = request.lines().next().unwrap_or("");
    let mut stream = TcpStream::connect_timeout(&addr, BOUND)
        .unwrap_or_else(|e| panic!("no connection for {what:?}: {e}"));
    stream.set_read_timeout(Some(BOUND)).unwrap();
    stream.set_write_timeout(Some(BOUND)).unwrap();
    stream
        .write_all(request.as_bytes())
        .unwrap_or_else(|e| panic!("the server took no request {what:?}: {e}"));
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("no whole answer to {what:?} within {BOUND:?}: {e}"));
    let at = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&bytes[..at]).into_owned();
    let body = bytes[at + 4..].to_vec();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
        .collect();
    (status, headers, body)
}

fn get(addr: SocketAddr, target: &str) -> (u16, Vec<(String, String)>, String) {
    fetch(
        addr,
        &format!("GET {target} HTTP/1.1\r\nHost: evil.test\r\n\r\n"),
    )
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn each_route_answers_by_its_policy() {
    let addr = start("policy", 2, 8, 300);
    // A cached route: the document, kept by shared caches for its lifetime.
    let (status, headers, body) = get(addr, "/post/7");
    assert_eq!(status, 200);
    assert!(body.contains(">Post 7<"), "{body}");
    assert!(body.contains("application/vnd.exact.checkpoint"));
    assert_eq!(
        header(&headers, "cache-control"),
        Some("public, max-age=0, s-maxage=120, stale-while-revalidate=120")
    );
    // Its surrogate keys go only to a CDN (RFC 8586's `CDN-Loop`), with the
    // same page, validator and lifetime a browser gets.
    assert_eq!(header(&headers, "surrogate-key"), None);
    assert_eq!(header(&headers, "cache-tag"), None);
    let (status, cdn, cdn_body) =
        fetch(addr, "GET /post/7 HTTP/1.1\r\nCDN-Loop: cloudflare\r\n\r\n");
    assert_eq!((status, cdn_body.as_str()), (200, body.as_str()));
    assert_eq!(header(&cdn, "etag"), header(&headers, "etag"));
    assert_eq!(
        header(&cdn, "cache-control"),
        header(&headers, "cache-control")
    );
    let keys = header(&cdn, "surrogate-key").unwrap();
    assert!(keys.split(' ').any(|k| k == "post"), "{keys}");
    assert!(keys.split(' ').any(|k| k.starts_with("post:")), "{keys}");
    assert_eq!(
        header(&cdn, "cache-tag"),
        Some(keys.replace(' ', ",").as_str())
    );
    let csp = header(&headers, "content-security-policy").unwrap();
    assert!(
        csp.contains("connect-src 'self' https://api.blog.test;"),
        "{csp}"
    );
    // The page's one inline script that runs, the capture script, by hash.
    let capture = {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(exact_render::capture().as_bytes());
        exact_data::envelope::base64(&digest)
    };
    assert!(csp.contains(&format!("'sha256-{capture}'")), "{csp}");
    assert!(body.contains(&format!("<script>{}</script>", exact_render::capture())));
    let scripts = csp
        .split(';')
        .find(|d| d.trim().starts_with("script-src"))
        .unwrap();
    assert!(!scripts.contains("unsafe-inline"), "{scripts}");
    for source in ["prelude", "module"] {
        use sha2::{Digest, Sha256};
        let hash = exact_data::envelope::base64(&Sha256::digest(source.as_bytes()));
        assert!(csp.contains(&format!("'sha256-{hash}'")), "{csp}");
    }
    assert_eq!(header(&headers, "x-content-type-options"), Some("nosniff"));
    // Only its encoding varies; nothing else of the request shapes a page.
    assert_eq!(header(&headers, "vary"), Some("Accept-Encoding"));
    // Canonical URLs come from the configured origin, not the request's Host.
    assert!(!body.contains("evil.test"));
    // An unchanged page is a 304 to a cache that has it, a browser's or a
    // CDN's, and carries no keys: the stored response has them.
    let etag = header(&headers, "etag").unwrap().to_string();
    for via in ["", "CDN-Loop: cloudflare\r\n"] {
        let (status, fresh, body) = fetch(
            addr,
            &format!("GET /post/7 HTTP/1.1\r\n{via}If-None-Match: {etag}\r\n\r\n"),
        );
        assert_eq!((status, body.as_str()), (304, ""));
        assert_eq!(header(&fresh, "etag"), Some(etag.as_str()));
        assert_eq!(header(&fresh, "surrogate-key"), None);
    }
    // HEAD: the same headers, no body.
    let (status, headers, body) = fetch(addr, "HEAD /post/7 HTTP/1.1\r\n\r\n");
    assert_eq!((status, body.as_str()), (200, ""));
    assert_eq!(header(&headers, "etag"), Some(etag.as_str()));
    // A per-request route isn't kept.
    let (status, headers, _) = get(addr, "/live/7");
    assert_eq!(
        (status, header(&headers, "cache-control")),
        (200, Some("no-store"))
    );
    // A client route is the shell.
    let (status, _, body) = get(addr, "/app");
    assert_eq!(status, 200);
    assert!(body.contains("<div id=\"exact-root\"></div>"));
    // Anywhere else is the not-found document, never the shell's 200.
    let (status, headers, body) = get(addr, "/no/such/page");
    assert_eq!(status, 404);
    assert!(body.contains(">Nothing here<"), "{body}");
    assert_eq!(header(&headers, "x-robots-tag"), Some("noindex"));
    assert_eq!(
        header(&headers, "cache-control"),
        Some("public, max-age=0, s-maxage=60")
    );
}

#[test]
fn a_render_that_misses_its_deadline_or_fails_is_not_kept() {
    let addr = start("failure", 2, 8, 300);
    let (status, headers, body) = get(addr, "/post/slow");
    assert_eq!(status, 503);
    assert_eq!(header(&headers, "retry-after"), Some("1"));
    assert_eq!(header(&headers, "cache-control"), Some("no-store"));
    assert!(body.contains("\"pending\":[\"post\"]"), "{body}");
    let (status, headers, _) = get(addr, "/post/boom");
    assert_eq!(
        (status, header(&headers, "cache-control")),
        (500, Some("no-store"))
    );
    // …and the server keeps serving.
    assert_eq!(get(addr, "/post/8").0, 200);
}

#[test]
fn files_health_and_the_edges_of_http() {
    let addr = start("edges", 1, 8, 300);
    let (status, headers, body) = get(addr, "/glue.js");
    assert_eq!(status, 200);
    assert_eq!(body, "// the glue\n");
    assert_eq!(
        header(&headers, "content-type"),
        Some("text/javascript; charset=utf-8")
    );
    // The CSP is a page's: on a script it would govern only a worker made
    // from it, and the module worker evaluates the module it verified.
    assert!(header(&headers, "content-security-policy").is_none());
    assert_eq!(get(addr, "/assets/dot.png").0, 200);
    let (status, _, body) = get(addr, "/.exact/health");
    assert_eq!((status, body.as_str()), (200, "ok\n"));
    // A page is never a file: shell.html renders as a location.
    assert_eq!(get(addr, "/shell.html").0, 404);
    // Nothing outside the dist.
    let (status, _, body) = fetch(addr, "GET /../Cargo.toml HTTP/1.1\r\n\r\n");
    assert!(!body.contains("[package]"));
    assert_ne!(status, 200);
    // An encoded climb is a location like any other: it resolves inside
    // the site, to a page that isn't there.
    let (status, headers, _) = get(addr, "/%2e%2e/%2e%2e/etc/hosts");
    assert_eq!(
        (status, header(&headers, "location")),
        (301, Some("/etc/hosts"))
    );
    assert_eq!(get(addr, "/etc/hosts").0, 404);
    // One URL per page: the router's canonical form.
    for (target, canonical) in [
        ("/post//7/?x=1", "/post/7?x=1"),
        ("/post/./7", "/post/7"),
        ("/post/7?a='b'", "/post/7?a=%27b%27"),
    ] {
        let (status, headers, _) = get(addr, target);
        assert_eq!(
            (status, header(&headers, "location")),
            (301, Some(canonical)),
            "{target}"
        );
    }
    assert_eq!(fetch(addr, "POST /post/7 HTTP/1.1\r\n\r\n").0, 405);
    assert_eq!(fetch(addr, "nonsense\r\n\r\n").0, 400);
}

#[test]
fn a_full_queue_answers_503_at_once() {
    // One render at a time, and nothing may wait for it.
    // The slow render holds the only worker for a second. A probe that
    // reaches the worker first (a loaded machine) is simply served.
    let addr = start("queue", 1, 0, 1000);
    // A probe on the worker can turn the slow request away too: it asks
    // again until it holds the worker.
    let slow = std::thread::spawn(move || {
        let until = std::time::Instant::now() + BOUND;
        loop {
            let answer = get(addr, "/post/slow");
            if answer.2 != "busy\n" {
                break answer;
            }
            assert!(
                std::time::Instant::now() < until,
                "the slow request never got the worker in {BOUND:?}"
            );
        }
    });
    // Probes until the slow render is over: while it holds the worker, a
    // probe finds the queue full.
    let (headers, body) = (0..)
        .take_while(|_| !slow.is_finished())
        .find_map(|_| {
            std::thread::sleep(Duration::from_millis(20));
            let (status, headers, body) = get(addr, "/post/7");
            (status == 503).then_some((headers, body))
        })
        .expect("a probe found the queue full");
    assert_eq!(body, "busy\n");
    assert_eq!(header(&headers, "retry-after"), Some("1"));
    // The slow one was rendered, at its deadline.
    let (status, _, body) = slow.join().unwrap();
    assert_eq!(status, 503);
    assert!(body.contains("\"pending\":[\"post\"]"), "{body}");
    assert_eq!(get(addr, "/post/7").0, 200);
}

#[test]
fn cached_pages_reuse_public_answers_and_honor_request_cache_controls() {
    let addr = start("origin-cache", 1, 8, 1000);
    let (status, _, first) = get(addr, "/post/cache-probe");
    assert_eq!(status, 200);
    assert!(first.contains("Read 1"));
    let (_, headers, second) = get(addr, "/post/cache-probe");
    assert_eq!(second, first);
    assert!(header(&headers, "age").is_some());
    assert_eq!(CACHE_READS.load(Ordering::SeqCst), 1);
    let etag = header(&headers, "etag").unwrap();
    let (status, _, _) = fetch(
        addr,
        &format!("GET /post/cache-probe HTTP/1.1\r\nIf-None-Match: {etag}\r\n\r\n"),
    );
    assert_eq!(status, 304);
    assert_eq!(CACHE_READS.load(Ordering::SeqCst), 1);
    let (_, _, refreshed) = fetch(
        addr,
        "GET /post/cache-probe HTTP/1.1\r\nCache-Control: no-cache\r\n\r\n",
    );
    assert!(refreshed.contains("Read 2"));
    let (_, _, unstored) = fetch(
        addr,
        "GET /post/cache-probe HTTP/1.1\r\nCache-Control: no-store\r\n\r\n",
    );
    assert!(unstored.contains("Read 3"));
    assert_eq!(get(addr, "/post/cache-probe").2, refreshed);
    assert!(get(addr, "/live/cache-probe").2.contains("Read 4"));
    assert!(get(addr, "/live/cache-probe").2.contains("Read 5"));
}

#[test]
fn the_sitemap_lists_each_rendered_route_and_its_listed_pages() {
    let addr = start("sitemap", 1, 8, 300);
    let (status, headers, body) = get(addr, "/sitemap.xml");
    assert_eq!(status, 200);
    assert_eq!(
        header(&headers, "content-type"),
        Some("application/xml; charset=utf-8")
    );
    for url in [
        "https://blog.test/",
        "https://blog.test/post/7",
        "https://blog.test/post/an%20idea",
    ] {
        assert!(body.contains(&format!("<loc>{url}</loc>")), "{url}\n{body}");
    }
    // A route without a pages source isn't in it; neither is the shell's,
    // nor a page that says `noindex`, as the build's sitemap leaves them.
    assert!(!body.contains("/live/") && !body.contains("/app"), "{body}");
    assert!(!body.contains("/draft/"), "{body}");
    assert_eq!(get(addr, "/draft/d1").0, 200);
    let (status, _, body) = get(addr, "/robots.txt");
    assert_eq!(status, 200);
    assert_eq!(
        body,
        "User-agent: *\nAllow: /\nSitemap: https://blog.test/sitemap.xml\n"
    );
}

/// A list that answers a moment after it is asked, before a sibling: the
/// runner that settles builds the sibling first and the cards when they
/// land; the runtime, booting with the answer, builds them in page order.
/// And a list the render's environment refuses (storage), still pending.
const FEED: &str = r#"
routes nav
  tab home "/" render=request

component Feed
  resource cards = cards() as shape list<string>
  resource saved = saved() as shape list<string>
  view
    column
      each c in cards key = c
        text c
      text "the end" testId="end"
      each s in saved key = s
        text s
"#;

#[derive(Default)]
struct Feed;

impl DataSource for Feed {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, _: &[Value]) -> Result<Answer, DataError> {
        match source {
            "cards" => Ok(Answer::Later(Request::continuation(1))),
            "saved" => Ok(Answer::Later(Request::storage(b"get".to_vec()))),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn continuation(&mut self, _: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        Some(Box::new(|| {
            std::thread::sleep(Duration::from_millis(20));
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: b"first second third".to_vec(),
            })
        }))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Outcome::Response(response) = outcome else {
            return Err(DataError::Unavailable("no response".into()));
        };
        let body = String::from_utf8_lossy(&response.body).into_owned();
        Ok(Answer::Now(Value::list(
            body.split(' ').map(Value::str).collect(),
        )))
    }

    fn grants(&self) -> &str {
        ""
    }
}

#[test]
fn a_page_whose_data_answered_later_is_adopted_with_what_is_pending() {
    super::warm_transport();
    let serve = Serve {
        dist: dist("feed"),
        port: 0,
        name: "Feed".into(),
        origin: None,
        deadline: Duration::from_secs(10),
        renders: 1,
        queue: 8,
        viewport: Default::default(),
        lifetime: Duration::from_secs(120),
    };
    let plan = contract::compile(FEED).unwrap();
    let server = Server::bind(serve, plan.clone(), Feed.grants()).unwrap();
    let addr = server.addr();
    std::thread::spawn(move || server.run(|| Feed));
    let (status, _, body) = get(addr, "/");
    assert_eq!(status, 200);
    assert!(body.contains(">third<"), "{body}");
    assert!(body.contains("\"pending\":[\"saved\"]"), "{body}");
    // The checkpoint and its digest, as the glue hands them to the runtime.
    let open = "<script type=\"application/vnd.exact.checkpoint\" data-digest=\"";
    let at = body.find(open).expect("a checkpoint") + open.len();
    let digest = body[at..].split('"').next().unwrap();
    let (_, rest) = body[at..].split_once('>').unwrap();
    let (checkpoint, _) = rest.split_once("</script>").unwrap();
    // The runtime boots from it and adopts the document: its first tree,
    // view ids included, is the document's.
    exact_web::link(exact_web_capabilities::ALL);
    let (_, batch) = exact_web::Host::boot_checkpoint(
        &plan.encode(),
        Feed,
        checkpoint,
        digest,
        Vec::new(),
        None,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(
        batch.contains("{\"op\":\"adopt\",\"adopted\":true}"),
        "{batch}"
    );
}

/// A body as it was before its `Content-Encoding`.
fn decoded(headers: &[(String, String)], body: &[u8]) -> String {
    let mut out = Vec::new();
    match header(headers, "content-encoding") {
        Some("br") => brotli::Decompressor::new(body, 4096)
            .read_to_end(&mut out)
            .map(drop)
            .unwrap(),
        Some("gzip") => flate2::read::GzDecoder::new(body)
            .read_to_end(&mut out)
            .map(drop)
            .unwrap(),
        None => out.extend_from_slice(body),
        Some(other) => panic!("{other}"),
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn pages_and_files_go_compressed_as_the_client_accepts() {
    let addr = start("encoding", 1, 8, 300);
    let page = |accept: &str| {
        fetch_bytes(
            addr,
            &format!("GET /live/7 HTTP/1.1\r\nAccept-Encoding: {accept}\r\n\r\n"),
        )
    };
    // Brotli first; gzip when brotli is refused; none when neither is taken.
    for (accept, encoding) in [
        ("gzip, deflate, br", Some("br")),
        ("br;q=0, gzip", Some("gzip")),
        ("identity", None),
    ] {
        let (status, headers, body) = page(accept);
        assert_eq!(status, 200);
        assert_eq!(header(&headers, "content-encoding"), encoding, "{accept}");
        assert_eq!(header(&headers, "vary"), Some("Accept-Encoding"));
        let text = decoded(&headers, &body);
        assert!(text.contains(">Post 7<"), "{text}");
        if let Some(encoding) = encoding {
            assert!(body.len() < text.len() / 2, "{encoding}: {}", body.len());
        }
    }
    // A cache holding the brotli page revalidates it by its own ETag.
    let (_, headers, _) = fetch_bytes(addr, "GET /post/7 HTTP/1.1\r\nAccept-Encoding: br\r\n\r\n");
    let etag = header(&headers, "etag").unwrap().to_string();
    assert!(etag.ends_with("-br\""), "{etag}");
    let (status, headers, _) = fetch_bytes(
        addr,
        &format!("GET /post/7 HTTP/1.1\r\nAccept-Encoding: br\r\nIf-None-Match: {etag}\r\n\r\n"),
    );
    assert_eq!(status, 304);
    assert_eq!(header(&headers, "etag"), Some(etag.as_str()));
    // A dist file goes as it is until its variant is made, off the request
    // path; then brotli's best.
    let script = "export const words = ['the same words, again'];\n".repeat(200);
    let started = std::time::Instant::now();
    let (headers, body) = loop {
        let (status, headers, body) = fetch_bytes(
            addr,
            "GET /big.js HTTP/1.1\r\nAccept-Encoding: br, gzip\r\n\r\n",
        );
        assert_eq!(status, 200);
        assert_eq!(header(&headers, "vary"), Some("Accept-Encoding"));
        assert_eq!(decoded(&headers, &body), script);
        if header(&headers, "content-encoding").is_some() {
            break (headers, body);
        }
        assert!(started.elapsed() < Duration::from_secs(60), "no variant");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(header(&headers, "content-encoding"), Some("br"));
    assert!(body.len() < script.len() / 10, "{}", body.len());
    // An image is sent as it is, and says nothing of encodings.
    let (_, headers, _) = fetch_bytes(
        addr,
        "GET /assets/dot.png HTTP/1.1\r\nAccept-Encoding: br\r\n\r\n",
    );
    assert_eq!(header(&headers, "content-encoding"), None);
    assert_eq!(header(&headers, "vary"), None);
}

#[test]
fn a_cached_page_goes_at_the_best_compression_once_it_is_made() {
    let addr = start("best", 1, 8, 300);
    let request = "GET /post/7 HTTP/1.1\r\nAccept-Encoding: br\r\n\r\n";
    // The render's own response is compressed as it is sent.
    let (status, first_headers, first) = fetch_bytes(addr, request);
    assert_eq!(status, 200);
    assert_eq!(header(&first_headers, "content-encoding"), Some("br"));
    // A later hit gets the page made at the best, off the request path.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let (headers, best) = loop {
        let (_, headers, body) = fetch_bytes(addr, request);
        if body.len() < first.len() || std::time::Instant::now() > deadline {
            break (headers, body);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        best.len() < first.len(),
        "{} !< {}",
        best.len(),
        first.len()
    );
    assert_eq!(decoded(&headers, &best), decoded(&first_headers, &first));
    assert_eq!(header(&headers, "content-encoding"), Some("br"));
    assert_eq!(header(&headers, "vary"), Some("Accept-Encoding"));
    // One representation, one tag: a client holding either gets a 304.
    let etag = header(&headers, "etag").unwrap();
    assert_eq!(Some(etag), header(&first_headers, "etag"));
    let (status, _, body) = fetch_bytes(
        addr,
        &format!("GET /post/7 HTTP/1.1\r\nAccept-Encoding: br\r\nIf-None-Match: {etag}\r\n\r\n"),
    );
    assert_eq!((status, body.len()), (304, 0));
    // A render that isn't kept goes as it is sent.
    let (_, _, fresh) = fetch_bytes(
        addr,
        "GET /post/7 HTTP/1.1\r\nAccept-Encoding: br\r\nCache-Control: no-store\r\n\r\n",
    );
    assert_eq!(fresh.len(), first.len());
}

#[test]
fn a_kept_page_goes_against_a_dictionary_the_browser_holds() {
    use sha2::{Digest, Sha256};
    const USE: &str = "match=\"/*\", match-dest=(\"document\")";
    let addr = start("dictionary", 1, 8, 300);
    // A kept page says a browser may keep it as a dictionary; its hash names it.
    let (status, headers, dictionary) = fetch_bytes(addr, "GET /post/7 HTTP/1.1\r\n\r\n");
    assert_eq!(
        (status, header(&headers, "use-as-dictionary")),
        (200, Some(USE))
    );
    let hash = exact_data::envelope::base64(&Sha256::digest(&dictionary));
    let ask = |extra: &str| {
        fetch_bytes(
            addr,
            &format!("GET /post/8 HTTP/1.1\r\nAccept-Encoding: gzip, deflate, br, zstd, dcb, dcz\r\nAvailable-Dictionary: :{hash}:\r\n{extra}\r\n"),
        )
    };
    // A browser that holds it gets the next page against it: the render that
    // keeps the page, then a hit.
    for _ in 0..2 {
        let (status, headers, body) = ask("");
        assert_eq!(status, 200);
        assert_eq!(header(&headers, "content-encoding"), Some("dcb"));
        assert_eq!(
            header(&headers, "vary"),
            Some("Accept-Encoding, Available-Dictionary")
        );
        assert_eq!(header(&headers, "use-as-dictionary"), Some(USE));
        assert!(header(&headers, "etag").unwrap().ends_with("-dcb\""));
        assert_eq!(body[..4], [0xff, 0x44, 0x43, 0x42]);
        assert_eq!(body[4..36], Sha256::digest(&dictionary)[..]);
        let mut decoded = Vec::new();
        brotli::Decompressor::new_with_custom_dict(&body[36..], 4096, dictionary.clone().into())
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, fetch_bytes(addr, "GET /post/8 HTTP/1.1\r\n\r\n").2);
        assert!(
            body.len() * 2 < decoded.len() / 4,
            "{} of {}",
            body.len(),
            decoded.len()
        );
    }
    // Its own ETag gets a 304.
    let etag = header(&ask("").1, "etag").unwrap().to_string();
    let (status, _, body) = ask(&format!("If-None-Match: {etag}\r\n"));
    assert_eq!((status, body.len()), (304, 0));
    // So does the tag of the page as brotli, which a browser coming back holds
    // when it now asks with a dictionary: every encoding decodes to the page.
    let (_, headers, _) = fetch_bytes(addr, "GET /post/8 HTTP/1.1\r\nAccept-Encoding: br\r\n\r\n");
    let br = header(&headers, "etag").unwrap().to_string();
    assert!(br.ends_with("-br\""), "{br}");
    let (status, headers, body) = ask(&format!("If-None-Match: {br}\r\n"));
    assert_eq!((status, body.len()), (304, 0));
    assert_eq!(header(&headers, "etag"), Some(br.as_str()));
    // A hash this server doesn't keep falls back to brotli.
    let unknown = exact_data::envelope::base64(&[0u8; 32]);
    let (_, headers, _) = fetch_bytes(
        addr,
        &format!("GET /post/8 HTTP/1.1\r\nAccept-Encoding: br, dcb\r\nAvailable-Dictionary: :{unknown}:\r\n\r\n"),
    );
    assert_eq!(header(&headers, "content-encoding"), Some("br"));
    assert_eq!(header(&headers, "vary"), Some("Accept-Encoding"));
    // A CDN's request hears of no dictionary and gets none.
    let (_, headers, _) = ask("CDN-Loop: cloudflare\r\n");
    assert_eq!(header(&headers, "content-encoding"), Some("br"));
    assert_eq!(header(&headers, "use-as-dictionary"), None);
    // Nor do pages that aren't kept: one per request, or one asked not to be stored.
    for target in [
        "/live/7 HTTP/1.1\r\n",
        "/post/8 HTTP/1.1\r\nCache-Control: no-store\r\n",
    ] {
        let (status, headers, _) = fetch_bytes(
            addr,
            &format!(
                "GET {target}Accept-Encoding: br, dcb\r\nAvailable-Dictionary: :{hash}:\r\n\r\n"
            ),
        );
        assert_eq!(status, 200);
        assert_eq!(header(&headers, "use-as-dictionary"), None);
        assert_eq!(header(&headers, "content-encoding"), Some("br"));
    }
}

#[test]
fn a_file_the_dist_lacks_is_a_plain_404() {
    let addr = start("favicon", 1, 8, 300);
    // A browser's icon request doesn't render the not-found document…
    let (status, headers, body) = get(addr, "/favicon.ico");
    assert_eq!((status, body.as_str()), (404, "not found\n"));
    assert_eq!(
        header(&headers, "cache-control"),
        Some("public, max-age=0, s-maxage=60")
    );
    assert_eq!(get(addr, "/old/app.js").0, 404);
    // …but an unknown page does, dots and all.
    for target in ["/no/such/page", "/v1.2", "/notes/readme.md"] {
        let (status, _, body) = get(addr, target);
        assert_eq!(status, 404, "{target}");
        assert!(body.contains(">Nothing here<"), "{target}: {body}");
    }
}

#[test]
fn a_drained_server_answers_what_it_took_and_stops() {
    super::warm_transport();
    let serve = Serve {
        dist: dist("drain"),
        port: 0,
        name: "Blog".into(),
        origin: None,
        deadline: Duration::from_millis(1000),
        renders: 1,
        queue: 0,
        viewport: Default::default(),
        lifetime: Duration::from_secs(120),
    };
    let server = Server::bind(serve, contract::compile(SRC).unwrap(), Posts.grants()).unwrap();
    let (addr, stopper) = (server.addr(), server.stopper());
    let running = std::thread::spawn(move || server.run(|| Posts));
    // The worker is up (before it is, every request finds the queue full)…
    let until = std::time::Instant::now() + BOUND;
    while get(addr, "/.exact/health").0 != 200 {
        assert!(
            std::time::Instant::now() < until,
            "the server's worker never came up in {BOUND:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // …then the slow render holds it, and a probe finds it busy.
    let slow = std::thread::spawn(move || {
        let until = std::time::Instant::now() + BOUND;
        loop {
            let answer = get(addr, "/post/slow");
            if answer.2 != "busy\n" {
                break answer;
            }
            assert!(
                std::time::Instant::now() < until,
                "the slow request never got the worker in {BOUND:?}"
            );
        }
    });
    while !slow.is_finished() && get(addr, "/.exact/health").0 != 503 {
        std::thread::sleep(Duration::from_millis(10));
    }
    stopper.stop();
    // What it took is answered: the render in flight ends at its deadline.
    let (status, _, body) = slow.join().unwrap();
    assert_eq!(status, 503);
    assert!(body.contains("\"pending\":[\"post\"]"), "{body}");
    // It takes nothing new, and returns.
    let started = std::time::Instant::now();
    while !running.is_finished() {
        assert!(started.elapsed() < Duration::from_secs(30), "still running");
        std::thread::sleep(Duration::from_millis(10));
    }
    running.join().unwrap().unwrap();
    assert!(TcpStream::connect(addr).is_err());
}

#[test]
fn a_dist_file_revalidates_by_its_etag() {
    let addr = start("validators", 1, 8, 300);
    let conditional = |target: &str, accept: &str, etag: &str| {
        fetch_bytes(
            addr,
            &format!(
                "GET {target} HTTP/1.1\r\nAccept-Encoding: {accept}\r\nIf-None-Match: {etag}\r\n\r\n"
            ),
        )
    };
    // As it is: a validator of its bytes, and a 304 for a client that has them.
    let (status, headers, body) = fetch_bytes(addr, "GET /glue.js HTTP/1.1\r\n\r\n");
    assert_eq!(
        (status, body.as_slice()),
        (200, b"// the glue\n".as_slice())
    );
    assert_eq!(header(&headers, "cache-control"), Some("no-cache"));
    let etag = header(&headers, "etag").unwrap().to_string();
    let (status, headers, body) = conditional("/glue.js", "identity", &etag);
    assert_eq!((status, body.len()), (304, 0));
    assert_eq!(header(&headers, "etag"), Some(etag.as_str()));
    // Only what updates the stored copy.
    assert_eq!(header(&headers, "cache-control"), Some("no-cache"));
    assert!(
        header(&headers, "content-type").is_none() && header(&headers, "content-length").is_none()
    );
    // Each encoding is its own representation, with its own validator.
    let started = std::time::Instant::now();
    let br = loop {
        let (_, headers, _) =
            fetch_bytes(addr, "GET /big.js HTTP/1.1\r\nAccept-Encoding: br\r\n\r\n");
        if header(&headers, "content-encoding") == Some("br") {
            break header(&headers, "etag").unwrap().to_string();
        }
        assert!(started.elapsed() < Duration::from_secs(60), "no variant");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(br.ends_with("-br\""), "{br}");
    assert_eq!(conditional("/big.js", "br", &br).0, 304);
    let (_, headers, _) = fetch_bytes(addr, "GET /big.js HTTP/1.1\r\n\r\n");
    let identity = header(&headers, "etag").unwrap().to_string();
    assert_eq!(br, format!("{}-br\"", identity.trim_end_matches('"')));
    assert_eq!(conditional("/big.js", "br", &identity).0, 200);
    // An image too.
    let (_, headers, _) = fetch_bytes(addr, "GET /assets/dot.png HTTP/1.1\r\n\r\n");
    let image = header(&headers, "etag").unwrap().to_string();
    assert_eq!(conditional("/assets/dot.png", "br", &image).0, 304);
}

#[test]
fn unsafe_canonical_paths_are_refused_before_redirecting() {
    let addr = start("unsafe-paths", 1, 8, 300);
    for target in [
        "/\\evil.com/",
        "//\\evil.com",
        "/%5cevil.com/",
        "/bad\nheader/",
    ] {
        let (status, headers, _) = get(addr, target);
        assert_eq!(status, 400, "{target:?}");
        assert_eq!(header(&headers, "location"), None);
    }
    let (status, headers, _) = get(addr, "//post//7/");
    assert_eq!(status, 301);
    assert_eq!(header(&headers, "location"), Some("/post/7"));
    // An encoded slash in a route parameter is valid, unlike a file traversal.
    assert_ne!(get(addr, "/post/a%2Fb").0, 400);
}

#[test]
fn large_static_files_stream_with_lengths_validators_and_head() {
    let addr = start("large-stream", 1, 8, 300);
    let path = std::env::temp_dir()
        .join(format!(
            "exact-render-serve-large-stream-{}",
            std::process::id()
        ))
        .join("large.wasm");
    let file = std::fs::File::create(&path).unwrap();
    let size = (16 << 20) + 1;
    file.set_len(size).unwrap();
    let (status, headers, body) = fetch_bytes(
        addr,
        "GET /large.wasm HTTP/1.1\r\nAccept-Encoding: br\r\n\r\n",
    );
    assert_eq!(status, 200);
    assert_eq!(body.len() as u64, size);
    assert!(body.iter().all(|b| *b == 0));
    assert_eq!(
        header(&headers, "content-length"),
        Some(size.to_string().as_str())
    );
    assert_eq!(header(&headers, "content-encoding"), None);
    let etag = header(&headers, "etag").unwrap();
    let (status, head, body) = fetch_bytes(addr, "HEAD /large.wasm HTTP/1.1\r\n\r\n");
    assert_eq!(status, 200);
    assert!(body.is_empty());
    assert_eq!(
        header(&head, "content-length"),
        Some(size.to_string().as_str())
    );
    assert_eq!(header(&head, "etag"), Some(etag));
    let (status, _, body) = fetch_bytes(
        addr,
        &format!("GET /large.wasm HTTP/1.1\r\nIf-None-Match: {etag}\r\n\r\n"),
    );
    assert_eq!(status, 304);
    assert!(body.is_empty());
    assert_eq!(get(addr, "/glue.js").0, 200);
}
