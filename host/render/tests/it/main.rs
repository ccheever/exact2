//! The async render host (LLP 1048.000 D9): a render waits for its answers,
//! stops at its deadline with placeholders, and refuses what its environment
//! doesn't hold — the source keeps its placeholder for the client.

use exact_plan::{Plan, Value};
use exact_render::{render, Anonymous, Rendered, Settled};
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Response, Store};
use exact_web::document::Site;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

mod boot;
mod direct;
mod overload;
mod serve;
mod viewport;

/// How the fixture's `post` source answers later.
#[derive(Clone, Copy, Default)]
enum Post {
    /// A continuation that answers after a moment.
    #[default]
    Soon,
    /// A continuation that answers long after any deadline.
    Never,
    /// Storage: a device capability.
    Storage,
    /// A fetch the grants don't cover.
    Elsewhere,
    /// A real fetch from a server on this machine.
    Local(u16),
}

#[derive(Clone, Default)]
struct Blog {
    post: Post,
    grants: String,
}

impl Blog {
    fn new(post: Post) -> Self {
        let fetch = match post {
            Post::Local(port) => format!("http://127.0.0.1:{port}/"),
            _ => "https://blog.test/".into(),
        };
        Blog {
            post,
            grants: format!("net.fetch {fetch}\nsecret.keep session\nsqlite.open app:/blog.db\n"),
        }
    }
}

fn post(id: &str, title: &str) -> Value {
    Value::record(vec![Value::str(id), Value::str(title)])
}

impl DataSource for Blog {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        Ok(match (source, self.post) {
            ("emptyPost", _) => Answer::Now(post("", "")),
            // A render's store holds nothing, and nothing can be kept in it:
            // a session would show one comment.
            ("comments", _) => {
                let held = store.get("session").is_some();
                let kept = store.set("session", "x").is_ok();
                let private = (held || kept).then(|| Value::str("private"));
                Answer::Now(Value::list(private.into_iter().collect()))
            }
            ("post", Post::Soon | Post::Never) => Answer::Later(Request::continuation(1)),
            ("post", Post::Storage) => Answer::Later(Request::storage(b"get".to_vec())),
            ("post", Post::Elsewhere) => {
                Answer::Later(Request::get("https://elsewhere.invalid/post"))
            }
            ("post", Post::Local(port)) => {
                Answer::Later(Request::get(&format!("http://127.0.0.1:{port}/post/7")))
            }
            (other, _) => return Err(DataError::UnknownSource(other.into())),
        })
    }

    fn continuation(&mut self, _: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let wait = match self.post {
            Post::Never => Duration::from_secs(5),
            _ => Duration::from_millis(20),
        };
        Some(Box::new(move || {
            std::thread::sleep(wait);
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: b"Hello".to_vec(),
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
        Ok(Answer::Now(match outcome {
            Outcome::Response(r) => post("7", &String::from_utf8_lossy(&r.body)),
            Outcome::Failed { message, .. } => post("7", &format!("failed: {message}")),
            _ => post("7", "?"),
        }))
    }

    fn grants(&self) -> &str {
        &self.grants
    }
}

fn plan() -> Plan {
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contract/corpus/placeholder.contract"),
    )
    .unwrap();
    contract::compile(&src).unwrap()
}

const SITE: Site<'static> = Site {
    name: "Blog",
    origin: None,
};

pub fn warm_transport() {
    // Server::bind prepares the platform transport before accepting requests.
    // The per-render deadlines below exercise that same running-server path.
    static WARM: std::sync::Once = std::sync::Once::new();
    WARM.call_once(|| drop(exact_render::Executor::start("")));
}

fn at(post: Post, deadline: Duration) -> Rendered {
    warm_transport();
    render(
        &plan(),
        || Blog::new(post),
        Default::default(),
        "/post/7",
        &SITE,
        deadline,
    )
    .unwrap()
}

#[test]
fn a_render_waits_for_its_answers() {
    let r = at(Post::Soon, Duration::from_secs(5));
    assert_eq!(r.settled, Settled::Complete);
    assert!(r.document.root.contains(">Hello<"), "{}", r.document.root);
    assert!(r.document.root.contains(">ready<"), "{}", r.document.root);
    assert!(r.checkpoint.contains("\"pending\":[]"), "{}", r.checkpoint);
    // The environment holds no secret and keeps none.
    assert!(
        r.document.root.contains(">0 comments<"),
        "{}",
        r.document.root
    );
}

#[test]
fn an_async_branch_is_adopted_from_the_same_checkpoint() {
    warm_transport();
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contract/corpus/placeholder.contract"),
    )
    .unwrap();
    let src = src.replace("      text post.title testId=\"title\"", "      when pending(post)\n        text \"Loading\"\n      else\n        column\n          text post.title testId=\"title\"");
    let plan = contract::compile(&src).unwrap();
    let rendered = render(
        &plan,
        || Blog::new(Post::Soon),
        Default::default(),
        "/post/7",
        &SITE,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(rendered.settled, Settled::Complete);
    let (_, batch) = exact_web::Host::boot_checkpoint(
        &plan.encode(),
        Blog::new(Post::Soon),
        &rendered.checkpoint,
        &rendered.digest,
        Vec::new(),
        None,
        Default::default(),
        "/post/7",
    )
    .unwrap();
    assert!(batch.contains("\"adopted\":true"), "{batch}");
}

#[test]
fn at_the_deadline_the_placeholder_stays_and_the_checkpoint_lists_it() {
    warm_transport();
    let started = Instant::now();
    let r = at(Post::Never, Duration::from_millis(100));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(r.settled, Settled::Deadline);
    // A deadline is a 503, but an unknown URL stays a 404 (LLP 1048.000 D11).
    assert_eq!((r.status(false), r.status(true)), (503, 404));
    assert!(r.document.root.contains(">loading<"), "{}", r.document.root);
    assert!(
        r.checkpoint.contains("\"pending\":[\"post\"]"),
        "{}",
        r.checkpoint
    );
}

#[test]
fn what_the_environment_does_not_hold_keeps_its_placeholder() {
    for post in [Post::Storage, Post::Elsewhere] {
        let r = at(post, Duration::from_secs(5));
        assert_eq!(r.settled, Settled::Complete);
        assert!(r.document.root.contains(">loading<"), "{}", r.document.root);
        assert!(!r.document.root.contains("failed"), "{}", r.document.root);
        assert!(
            r.checkpoint.contains("\"pending\":[\"post\"]"),
            "{}",
            r.checkpoint
        );
    }
    // Only the app's fetch grants reach the render.
    let data = Anonymous::new(Blog::new(Post::Soon));
    assert_eq!(DataSource::grants(&data), "net.fetch https://blog.test/");
}

#[test]
fn a_fetch_runs_through_the_native_executor() {
    // The waits here are hang bounds, not deadlines: on a loaded Mac the
    // platform transport has taken over ten seconds to open a loopback
    // connection.
    let bound = Duration::from_secs(60);
    warm_transport();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    // The test's server: every wait bounded, and a failure says what it
    // waited for (a blocking accept once held a test run for hours).
    let server = std::thread::spawn(move || -> Result<String, String> {
        let until = Instant::now() + bound;
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= until {
                        return Err(format!(
                            "the render never connected to the test server in {bound:?}"
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => return Err(format!("the test server's accept failed: {error}")),
            }
        };
        // An accepted socket inherits the listener's non-blocking mode on
        // macOS: the read below waits for the request, up to its bound.
        let failed = |what: &'static str| move |error: std::io::Error| format!("{what}: {error}");
        stream
            .set_nonblocking(false)
            .map_err(failed("blocking mode"))?;
        stream
            .set_read_timeout(Some(bound))
            .map_err(failed("read timeout"))?;
        stream
            .set_write_timeout(Some(bound))
            .map_err(failed("write timeout"))?;
        let mut request = [0u8; 4096];
        let n = stream
            .read(&mut request)
            .map_err(failed("the render's request never arrived"))?;
        let line = String::from_utf8_lossy(&request[..n])
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        let body = "From the server";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .map_err(failed("the answer couldn't be sent"))?;
        Ok(line)
    });
    let r = at(Post::Local(port), bound);
    let request = server.join().expect("the test server panicked");
    assert_eq!(request.as_deref(), Ok("GET /post/7 HTTP/1.1"));
    assert_eq!(r.settled, Settled::Complete);
    assert!(
        r.document.root.contains(">From the server<"),
        "{}",
        r.document.root
    );
    // Failure is the source's data: a refused connection is shaped by parse.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let r = at(Post::Local(port), bound);
    assert_eq!(r.settled, Settled::Complete);
    assert!(r.document.root.contains(">failed: "), "{}", r.document.root);
}

#[test]
fn a_keep_alive_answer_ends_at_its_framing_not_at_the_close() {
    // An API that keeps its connection open (HTTP/1.1's default): the
    // answer ends where its Content-Length or its last chunk says, never
    // at the server's close. The server holds the connection 30 s after its
    // answer; a render that waited for the close would take that long.
    let bound = Duration::from_secs(60);
    warm_transport();
    let body = "From a kept connection ".repeat(4096); // ~90 KB, several reads
    for chunked in [false, true] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (answered, when) = std::sync::mpsc::channel();
        let (release, hold) = std::sync::mpsc::channel::<()>();
        let text = body.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(bound)).unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            if chunked {
                write!(stream, "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n").unwrap();
                for part in text.as_bytes().chunks(16 * 1024) {
                    write!(stream, "{:x}\r\n", part.len()).unwrap();
                    stream.write_all(part).unwrap();
                    stream.write_all(b"\r\n").unwrap();
                }
                stream.write_all(b"0\r\n\r\n").unwrap();
            } else {
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: keep-alive\r\nKeep-Alive: timeout=30\r\n\r\n{text}", text.len()).unwrap();
            }
            stream.flush().unwrap();
            answered.send(Instant::now()).unwrap();
            let _ = hold.recv_timeout(Duration::from_secs(30));
        });
        let r = at(Post::Local(port), bound);
        let done = Instant::now();
        let sent = when.recv_timeout(bound).expect("the server never answered");
        release.send(()).unwrap();
        server.join().unwrap();
        assert_eq!(r.settled, Settled::Complete, "chunked {chunked}");
        assert!(
            r.document
                .root
                .contains("From a kept connection From a kept"),
            "chunked {chunked}"
        );
        assert!(
            done.duration_since(sent) < Duration::from_secs(10),
            "chunked {chunked}: the render ended {:?} after the answer",
            done.duration_since(sent)
        );
    }
}

#[test]
fn a_document_whose_ids_nobody_keeps_is_the_settled_tree_projected() {
    // `Ids::Any` (a JavaScript page) projects the tree the render settled
    // to, not a second one booted from its checkpoint: the same document,
    // view ids apart, answered or held at the deadline.
    warm_transport();
    let strip = |root: &str| {
        let (mut out, mut rest) = (String::new(), root);
        while let Some(at) = rest.find(" data-view=\"") {
            out.push_str(&rest[..at]);
            let end = rest[at + 12..].find('"').unwrap();
            rest = &rest[at + 13 + end..];
        }
        out + rest
    };
    for (post, deadline) in [(Post::Soon, 5_000), (Post::Never, 100)] {
        let [runtime, any] = [exact_render::Ids::Runtime, exact_render::Ids::Any].map(|ids| {
            exact_render::render_with_at(
                &plan(),
                &|| Blog::new(post),
                Default::default(),
                "/post/7",
                &SITE,
                Duration::from_millis(deadline),
                ids,
                exact_render::Projection::Auto,
                1_700_000_000_000.0,
            )
            .unwrap()
        });
        assert_eq!(strip(&runtime.document.root), strip(&any.document.root));
        assert_eq!(
            (
                runtime.settled,
                runtime.checkpoint.clone(),
                runtime.activate
            ),
            (any.settled, any.checkpoint.clone(), any.activate)
        );
    }
    assert!(exact_render::projects_as_booted(&plan()));
}

#[test]
fn a_page_is_the_shell_around_the_document() {
    let r = at(Post::Soon, Duration::from_secs(5));
    let shell =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/index.html"))
            .unwrap();
    let html = exact_render::page(&shell, &r).unwrap();
    assert!(html.contains(&format!("<div id=\"exact-root\">{}</div>", r.document.root)));
    assert!(html.contains(&format!(
        "data-digest=\"{}\" data-activate=\"{}\">{}</script>\n<script type=\"module\" src=\"./glue.js\"></script>",
        r.digest, r.activate.name(), r.checkpoint
    )));
    assert!(html.contains(&r.head));
    // Its one script that runs is the host's capture script, in the head,
    // so it hears a press from first parse; the rest load or are data.
    let capture = format!("<script>{}</script>", exact_render::capture());
    let scripts: Vec<&str> = html
        .match_indices("<script")
        .map(|(at, _)| &html[at..])
        .collect();
    assert_eq!(scripts.len(), 3, "{html}");
    assert!(scripts[0].starts_with(&capture));
    assert!(html.find(&capture) < html.find("<body").or(html.find("<div id=\"exact-root\"")));
    assert!(scripts[1].starts_with("<script type=\"application/vnd.exact.checkpoint\""));
    assert!(scripts[2].starts_with("<script type=\"module\" src=\"./glue.js\">"));
    assert!(exact_render::capture().len() <= 1024);
    assert_eq!(html.matches("<meta name=\"viewport\"").count(), 1);
    // An idle page's runtime downloads with the document (LLP 1048.000 D3).
    assert!(html.contains("<link rel=\"preload\" href=\"./app.wasm\" as=\"fetch\" crossorigin>"));
    assert!(html.contains("<link rel=\"modulepreload\" href=\"./navigation.js\">"));
    assert!(!html.contains("<title>Exact</title>"));
    assert!(exact_render::page("<!doctype html><title>x</title>\n", &r).is_err());
    let mut interaction = r;
    interaction.activate = exact_plan::ActivatePolicy::Interaction;
    let html = exact_render::page(&shell, &interaction).unwrap();
    assert!(html.contains("data-activate=\"interaction\""));
    assert!(html.contains("src=\"./document-glue.js\""));
    assert!(!html.contains("src=\"./glue.js\""));
    // Nothing fetches the wasm before intent: the checkpoint names the build
    // for document-glue.js, and the capture script's own code names it for
    // an idle page.
    let named = " data-activate=\"interaction\" data-wasm=\"./app.wasm\">";
    assert!(html.contains(named), "{html}");
    let rest = html
        .replace(exact_render::capture(), "")
        .replace(" data-wasm=\"./app.wasm\"", "");
    assert!(!rest.contains("app.wasm"));
    assert!(!rest.contains("navigation.js"));
    a_javascript_page_preloads_its_runtime_and_runs_it_after_first_paint(interaction);
}

/// The same document over the JavaScript runtime's shell (one render for
/// both: the render tests share an executor's limits).
fn a_javascript_page_preloads_its_runtime_and_runs_it_after_first_paint(mut r: Rendered) {
    // @ref LLP 1071 D6
    r.activate = exact_plan::ActivatePolicy::Inferred;
    // The shell as host/web-js/build.mjs writes it.
    let shell = "<!doctype html>\n<html lang=\"en\">\n<meta charset=\"utf-8\">\n<base href=\"/\">\n<title>Blog</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<link rel=\"modulepreload\" href=\"./app.js\">\n<link rel=\"modulepreload\" href=\"./shared-1.js\">\n<style>p{margin:0}</style>\n<div id=\"exact-root\"></div>\n<script type=\"module\" src=\"./app.js\"></script>\n";
    let html = exact_render::page(shell, &r).unwrap();
    // Undeclared is eager: the runtime downloads from the head, before the
    // stylesheet and the document, while the page streams.
    assert!(html.contains("data-activate=\"eager\""), "{html}");
    let head = &html[..html.find("<style>").unwrap()];
    assert!(head.contains("<link rel=\"modulepreload\" href=\"./app.js\">\n"));
    assert!(head.contains("<link rel=\"modulepreload\" href=\"./shared-1.js\">\n"));
    // First paint runs nothing of the app's: no module script is in the
    // page (the checkpoint takes the entry's place), and the one script
    // that runs is the capture script, in the head, which imports the entry
    // after the first paint entry, once the document is parsed.
    assert!(!html.contains("<script type=\"module\""), "{html}");
    let scripts: Vec<&str> = html
        .match_indices("<script")
        .map(|(at, _)| &html[at..])
        .collect();
    assert_eq!(scripts.len(), 2, "{html}");
    let capture = exact_render::capture_js();
    assert!(scripts[0].starts_with(&format!("<script>{capture}</script>")));
    assert!(html.find(capture) < html.find("<div id=\"exact-root\""));
    // The document as the runtime adopts it: no view ids (only the wasm
    // runtime reads them); this shell has no classes, so styles stay inline.
    let root = strip_view_ids(&r.document.root);
    assert!(r.document.root.contains(" data-view=\""));
    assert!(html.ends_with(&format!(
        "<div id=\"exact-root\">{root}</div>\n<script type=\"application/vnd.exact.checkpoint\" data-digest=\"{}\" data-activate=\"eager\">{}</script>\n",
        r.digest, r.checkpoint
    )), "{html}");
    // The head goes first, as a server flushes it before the render: the
    // capture script and the preloads before the stylesheet, and the
    // render's title and metas after it, still in the head.
    assert!(html.find(capture) < html.find("<link rel=\"modulepreload\""));
    assert!(html.find("<style>") < html.find(&r.head));
    assert!(html.find(&r.head) < html.find("<div id=\"exact-root\""));
    // An inline style the stylesheet has as a static class goes as that
    // class (the runtime gives it that class at adoption).
    let style = &r.document.root[r.document.root.find(" style=\"").unwrap() + 8..];
    let style = &style[..style.find('"').unwrap()];
    let classed = shell.replace(
        "<style>p{margin:0}</style>",
        &format!("<style>#exact-root#exact-root{{.c7{{{style}}}}}</style>"),
    );
    let html = exact_render::page(&classed, &r).unwrap();
    assert!(html.contains(" class=\"c7\""), "{html}");
    assert!(!html.contains(&format!(" style=\"{style}\"")));
    // A dynamic style row's element: the plain class that carries the rest
    // of its style goes as that class, and only the live row stays inline,
    // as the runtime writes it after adoption; a class another rule names
    // (a hover, a media variant) is only ever taken whole.
    let (live, fixed) = style.split_once(';').unwrap();
    assert!(fixed.contains(';'), "{style}");
    let split = shell.replace(
        "<style>p{margin:0}</style>",
        &format!("<style>#exact-root#exact-root{{.c9{{{fixed}}}}}</style>"),
    );
    let html = exact_render::page(&split, &r).unwrap();
    assert!(
        html.contains(&format!(" class=\"c9\" style=\"{live};\"")),
        "{html}"
    );
    let hovered = split.replace("}}</style>", "}.c9:hover{opacity:0.5}}</style>");
    let html = exact_render::page(&hovered, &r).unwrap();
    assert!(html.contains(&format!(" style=\"{style}\"")), "{html}");
    assert!(!html.contains(" data-view=\""));
    for link in html.match_indices("<a ") {
        let tag = &html[link.0..link.0 + html[link.0..].find('>').unwrap()];
        assert!(tag.contains(" data-view"), "{tag}");
    }
    for part in [
        "import(\"./app.js\")",
        "observe({type:\"paint\",buffered:!0})",
        "DOMContentLoaded",
        "requestIdleCallback",
    ] {
        assert!(capture.contains(part), "the capture script lacks {part}");
    }
    // `idle` keeps the preloads (the runtime runs when the browser is idle
    // after `load`); `interaction` fetches nothing before intent.
    r.activate = exact_plan::ActivatePolicy::Idle;
    let html = exact_render::page(shell, &r).unwrap();
    assert!(html.contains("data-activate=\"idle\""));
    assert_eq!(html.matches("rel=\"modulepreload\"").count(), 2);
    r.activate = exact_plan::ActivatePolicy::Interaction;
    let html = exact_render::page(shell, &r).unwrap();
    assert!(html.contains("data-activate=\"interaction\""));
    assert!(!html.replace(capture, "").contains(".js"), "{html}");
    // A shell that declares the plan's fonts (their faces with
    // `font-display`, their preloads early in the head) keeps its own: the
    // render's copies would replace the shell's rules.
    r.activate = exact_plan::ActivatePolicy::Inferred;
    let preload =
        "<link rel=\"preload\" href=\"assets/A.ttf\" as=\"font\" type=\"font/ttf\" crossorigin>";
    r.head.push_str(&format!("<style>@font-face{{font-family:\"ExactPlanStack0\";src:url(\"assets/A.ttf\");font-weight:400;font-style:normal}}</style>{preload}"));
    let fonted = shell.replace(
        "<style>p{margin:0}</style>",
        &format!("{preload}\n<style>@font-face{{font-family:\"ExactPlanStack0\";src:url(\"assets/A.ttf\");font-display:optional}}p{{margin:0}}</style>"),
    );
    let html = exact_render::page(&fonted, &r).unwrap();
    assert_eq!(html.matches("as=\"font\"").count(), 1, "{html}");
    assert_eq!(html.matches("@font-face").count(), 1, "{html}");
    assert!(html.find(preload) < html.find("<style>"));
}

/// `root` without its view ids: a link's `data-view` is empty.
fn strip_view_ids(root: &str) -> String {
    let mut out = String::new();
    let mut rest = root;
    while let Some(at) = rest.find(" data-view=\"") {
        out.push_str(&rest[..at]);
        let tag = &out[out.rfind('<').unwrap()..];
        if tag.starts_with("<a ") {
            out.push_str(" data-view");
        }
        rest = &rest[at + 12..];
        rest = &rest[rest.find('"').unwrap() + 1..];
    }
    out + rest
}

#[test]
fn a_route_lists_its_pages_with_its_source() {
    // @ref LLP 1048.000 D2
    #[derive(Default)]
    struct Listed {
        later: bool,
    }
    impl DataSource for Listed {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            args: &[Value],
        ) -> Result<Answer, DataError> {
            assert_eq!((source, args), ("posts", &[Value::str("public")][..]));
            Ok(if self.later {
                Answer::Later(Request::continuation(9))
            } else {
                Answer::Now(Value::list(vec![Value::str("1"), Value::str("two words")]))
            })
        }
        fn continuation(&mut self, _: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
            Some(Box::new(|| {
                Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: b"3".to_vec(),
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
            let Outcome::Response(r) = outcome else {
                return Err(DataError::Unavailable("no reply".into()));
            };
            Ok(Answer::Now(Value::list(vec![Value::str(
                &String::from_utf8_lossy(&r.body),
            )])))
        }
    }
    let plan = contract::compile(
        "routes nav\n  tab home \"/\" render=build\n    post \"/post/:post\" render=build pages=posts(\"public\")\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let row = plan
        .routes
        .iter()
        .find(|r| plan.str(r.name) == "post")
        .unwrap();
    let listed = exact_render::pages(&plan, Listed::default(), row, Duration::from_secs(5));
    assert_eq!(listed.unwrap(), ["/post/1", "/post/two%20words"]);
    let later = exact_render::pages(&plan, Listed { later: true }, row, Duration::from_secs(5));
    assert_eq!(later.unwrap(), ["/post/3"]);
    // The build's own list leaves the listed route to its source.
    assert_eq!(
        exact_web::document::build_locations(&plan).unwrap(),
        vec![("/".to_string(), false)]
    );
}

#[test]
fn rendered_document_sets_html_language_and_direction() {
    let mut rendered = at(Post::Soon, Duration::from_secs(5));
    rendered.document.lang = "ar".into();
    rendered.document.dir = "rtl".into();
    let shell =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/index.html"))
            .unwrap();
    let html = exact_render::page(&shell, &rendered).unwrap();
    assert!(html.contains(r#"<html lang="ar" dir="rtl">"#));
    rendered.document.lang = "en".into();
    rendered.document.dir = "ltr".into();
    assert!(exact_render::page(&shell, &rendered)
        .unwrap()
        .contains(r#"<html lang="en" dir="ltr">"#));
    // A page whose scroller is the page's marks its root for the shell's
    // rule (LLP 1048.003 D4), on either runtime's shell; a JavaScript page
    // sent whole needs no script for it.
    rendered.document.scroll_document = true;
    let js = "<!doctype html>\n<html lang=\"en\">\n<meta charset=\"utf-8\">\n<base href=\"/\">\n<title>Blog</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<link rel=\"modulepreload\" href=\"./app.js\">\n<style>p{margin:0}</style>\n<div id=\"exact-root\"></div>\n<script type=\"module\" src=\"./app.js\"></script>\n";
    for shell in [shell.as_str(), js] {
        let html = exact_render::page(shell, &rendered).unwrap();
        assert!(
            html.contains(r#"<html lang="en" dir="ltr" data-scrolldocument>"#),
            "{html}"
        );
        assert!(!html.contains(exact_render::scroll_document_js()), "{html}");
    }
}

#[test]
fn render_clock_seeds_initializers_resources_and_checkpoint_without_timers() {
    struct Clock;
    impl DataSource for Clock {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            Ok(Value::record(vec![args[0].clone()]))
        }
    }
    let plan = contract::compile(
        r#"
shape ClockAnswer
  at: number

component Clock
  state first = now()
  resource observed = clock(now()) as shape ClockAnswer
  view
    column
      text `First ${first}`
      text `Clock ${now()}`
      text `Resource ${observed.at}`
"#,
    )
    .unwrap();
    let now = 1_700_000_123_456.0;
    for projection in [
        exact_render::Projection::Kernel,
        exact_render::Projection::Direct,
    ] {
        let rendered = exact_render::render_with_at(
            &plan,
            &|| Clock,
            Default::default(),
            "/",
            &SITE,
            Duration::from_secs(2),
            exact_render::Ids::Any,
            projection,
            now,
        )
        .unwrap();
        for label in ["First", "Clock", "Resource"] {
            assert!(
                rendered
                    .document
                    .root
                    .contains(&format!("{label} 1700000123456")),
                "{}",
                rendered.document.root
            );
        }
        assert_eq!(rendered.state.now_ms, now);
    }
    let fresh = render(
        &plan,
        || Clock,
        Default::default(),
        "/",
        &SITE,
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(fresh.state.now_ms > now);
}
