//! The server (LLP 1048.000 D10, D11) over a small dist: each route's
//! policy, the statuses and headers of the HTTP contract, its files, and a
//! full queue.

use exact_plan::Value;
use exact_render::{Serve, Server};
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Response, Store};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SRC: &str = r#"
routes nav
  tab home "/" render=build
    post "/post/:post" render=cached
    live "/live/:post" render=request
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
"#;

/// A post by its id: `slow` answers long after any deadline, `boom`
/// refuses (a render that fails), `down` shows its failure (503).
#[derive(Default)]
struct Posts;

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
            ("post", "slow") => Ok(Answer::Later(Request::continuation(1))),
            ("post", "boom") => Err(DataError::Unavailable("boom".into())),
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
    std::fs::write(dir.join("assets/dot.png"), [0x89, b'P', b'N', b'G']).unwrap();
    dir
}

fn start(name: &str, renders: usize, queue: usize, deadline: u64) -> SocketAddr {
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
    std::thread::spawn(move || server.run::<Posts>());
    addr
}

/// Status, headers (lowercased names) and body of one request.
fn fetch(addr: SocketAddr, request: &str) -> (u16, Vec<(String, String)>, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap();
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
    (status, headers, body.to_string())
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
    let keys = header(&headers, "surrogate-key").unwrap();
    assert!(keys.split(' ').any(|k| k == "post"), "{keys}");
    assert!(keys.split(' ').any(|k| k.starts_with("post:")), "{keys}");
    assert_eq!(
        header(&headers, "cache-tag"),
        Some(keys.replace(' ', ",").as_str())
    );
    let csp = header(&headers, "content-security-policy").unwrap();
    assert!(
        csp.contains("connect-src 'self' https://api.blog.test;"),
        "{csp}"
    );
    assert_eq!(header(&headers, "x-content-type-options"), Some("nosniff"));
    assert!(header(&headers, "vary").is_none());
    // Canonical URLs come from the configured origin, not the request's Host.
    assert!(!body.contains("evil.test"));
    // An unchanged page is a 304 to a cache that has it.
    let etag = header(&headers, "etag").unwrap().to_string();
    let (status, _, body) = fetch(
        addr,
        &format!("GET /post/7 HTTP/1.1\r\nIf-None-Match: {etag}\r\n\r\n"),
    );
    assert_eq!((status, body.as_str()), (304, ""));
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
    assert_eq!(get(addr, "/assets/dot.png").0, 200);
    let (status, _, body) = get(addr, "/.exact/health");
    assert_eq!((status, body.as_str()), (200, "ok\n"));
    // A page is never a file: shell.html renders as a location.
    assert_eq!(get(addr, "/shell.html").0, 404);
    // Nothing outside the dist.
    let (status, _, body) = fetch(addr, "GET /../Cargo.toml HTTP/1.1\r\n\r\n");
    assert!(!body.contains("[package]"));
    assert_ne!(status, 200);
    assert_eq!(get(addr, "/%2e%2e/%2e%2e/etc/hosts").0, 404);
    // One URL per page.
    let (status, headers, _) = get(addr, "/post//7/?x=1");
    assert_eq!(
        (status, header(&headers, "location")),
        (301, Some("/post/7?x=1"))
    );
    assert_eq!(fetch(addr, "POST /post/7 HTTP/1.1\r\n\r\n").0, 405);
    assert_eq!(fetch(addr, "nonsense\r\n\r\n").0, 400);
}

#[test]
fn a_full_queue_answers_503_at_once() {
    // One render at a time, and nothing may wait for it.
    // The slow render holds the only worker for a second.
    let addr = start("queue", 1, 0, 1000);
    let slow = std::thread::spawn(move || get(addr, "/post/slow"));
    std::thread::sleep(Duration::from_millis(200));
    let (status, headers, body) = get(addr, "/post/7");
    assert_eq!((status, body.as_str()), (503, "busy\n"));
    assert_eq!(header(&headers, "retry-after"), Some("1"));
    // The slow one was rendered, at its deadline.
    let (status, _, body) = slow.join().unwrap();
    assert_eq!(status, 503);
    assert!(body.contains("\"pending\":[\"post\"]"), "{body}");
    assert_eq!(get(addr, "/post/7").0, 200);
}
