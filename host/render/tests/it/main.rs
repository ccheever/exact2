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

mod serve;

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
}
