//! A route that paints its boot document first (LLP 1048.005): the page
//! with its answers still to come goes before the render waits, its head's
//! fields in the head; the settled page follows, hiding it and removing it
//! before the runtime adopts.

use super::serve::{dist, get, header, unchunk, BOUND, JS_SHELL};
use exact_plan::Value;
use exact_render::{Serve, Server};
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Response, Store};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const SRC: &str = r#"
routes nav
  tab home "/" render=request paint=boot
    fast "/fast/:speed" render=request paint=boot
    kept "/kept/:speed" render=cached paint=boot
  notfound render=build

component Cards
  resource cards = cards(params(nav, "speed")) as shape list<string>
  view
    column
      head title="Cards"
      when length(cards) == 0
        text "Loading cards" testId="loading"
      each c in cards key = c
        text c
"#;

/// How long a slow answer takes.
const SLOW: Duration = Duration::from_millis(600);

/// `fast` answers at once; anything else a moment later.
#[derive(Default)]
struct Cards;

impl DataSource for Cards {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let fast = matches!(args.first(), Some(Value::List(speed)) if speed.first().and_then(Value::as_str) == Some("fast"));
        match source {
            "cards" if fast => Ok(Answer::Now(Value::list(vec![Value::str("quick")]))),
            "cards" => Ok(Answer::Later(Request::continuation(1))),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn continuation(&mut self, _: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        Some(Box::new(|| {
            std::thread::sleep(SLOW);
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: b"first second".to_vec(),
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

/// A browser's navigation of `target`: the page's bytes as they came, and
/// how long until its boot document (the bytes up to `data-boot`'s `</div>`)
/// had arrived, if it had one.
fn navigate(addr: std::net::SocketAddr, target: &str) -> (String, Option<Duration>) {
    let started = Instant::now();
    let mut stream = TcpStream::connect_timeout(&addr, BOUND).unwrap();
    stream.set_read_timeout(Some(BOUND)).unwrap();
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nSec-Fetch-Dest: document\r\nSec-Fetch-Mode: navigate\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut bytes = Vec::new();
    let mut boot = None;
    loop {
        let mut chunk = [0u8; 4096];
        let n = stream.read(&mut chunk).unwrap();
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&bytes);
        if boot.is_none() {
            if let Some(at) = text.find("data-boot=\"\"") {
                if text[at..].contains("Loading cards</div></div></div>") {
                    boot = Some(started.elapsed());
                }
            }
        }
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let at = text.rfind("HTTP/1.1 200 OK\r\n").expect("a 200");
    let body_at = text[at..].find("\r\n\r\n").unwrap() + at + 4;
    let page = String::from_utf8(unchunk(&bytes[body_at..])).unwrap();
    (page, boot)
}

#[test]
fn a_boot_document_goes_before_the_answers_and_the_settled_page_replaces_it() {
    super::warm_transport();
    let dir = dist("boot");
    std::fs::write(dir.join("shell.html"), JS_SHELL).unwrap();
    let serve = Serve {
        dist: dir,
        port: 0,
        name: "Cards".into(),
        origin: None,
        deadline: Duration::from_secs(10),
        renders: 2,
        queue: 8,
        viewport: Default::default(),
        lifetime: Duration::from_secs(120),
        generations: None,
    };
    let plan = contract::compile(SRC).unwrap();
    let served =
        super::serve::serving(Server::bind(serve, plan, Cards.grants()).unwrap(), || Cards);
    let addr = served.addr;

    // The boot document arrives before the answer could have.
    let (page, boot) = navigate(addr, "/");
    let boot = boot.expect("a boot document");
    assert!(boot < SLOW, "the boot document after {boot:?}");
    // In order: the head's fields (the boot document's), the boot document,
    // the style that hides it, the settled root, the script that removes
    // the boot document, then the checkpoint.
    let order = [
        "<title>Cards</title>",
        "<div id=\"exact-root\" data-boot=\"\">",
        ">Loading cards<",
        "<style>#exact-root[data-boot]{display:none}</style>",
        "<div id=\"exact-root\">",
        ">first<",
        &format!("</div><script>{}</script>", exact_render::boot_swap_js()),
        "application/vnd.exact.checkpoint",
    ];
    let mut from = 0;
    for part in order {
        let at = page[from..]
            .find(part)
            .unwrap_or_else(|| panic!("{part:?} after byte {from}: {page}"));
        from += at + part.len();
    }
    // Nothing of the head lands in the body: one title, one viewport meta.
    assert_eq!(page.matches("<title>").count(), 1, "{page}");
    assert_eq!(page.matches("<meta name=\"viewport\"").count(), 1, "{page}");
    // The settled root holds no placeholder.
    let settled = &page[page.find("<div id=\"exact-root\">").unwrap()..];
    assert!(!settled.contains(">Loading cards<"), "{settled}");

    // A page with nothing to wait for has no boot document.
    let (page, boot) = navigate(addr, "/fast/fast");
    assert_eq!(boot, None);
    assert!(!page.contains("data-boot"), "{page}");
    assert!(page.contains(">quick<"), "{page}");

    // A kept page is the settled page alone: no boot document, no swap.
    let (page, boot) = navigate(addr, "/kept/slow");
    assert!(boot.is_some(), "{page}");
    let (status, headers, kept) = get(addr, "/kept/slow");
    assert_eq!(status, 200);
    assert!(header(&headers, "age").is_some(), "{headers:?}");
    assert!(!kept.contains("data-boot"), "{kept}");
    assert!(!kept.contains(exact_render::boot_swap_js()), "{kept}");
    assert!(kept.contains("<title>Cards</title>") && kept.contains(">second<"));
}
