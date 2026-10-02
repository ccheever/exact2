//! A page laid out for its reader's viewport (LLP 1048.006): the client
//! hints, else the class the user agent names; a kept page kept per class.

use super::serve::{dist, header, BOUND};
use exact_plan::Value;
use exact_render::{Serve, Server};
use exact_runner::{Answer, DataError, DataSource, Store};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const SRC: &str = r#"
routes nav
  tab home "/" render=cached
    plain "/plain" render=cached
  notfound render=build

shape Viewport
  width: number

component Page
  resource viewport = exactViewport() as shape Viewport
  derive narrow = viewport.width <= 700
  view
    column flex-direction=(narrow ? "column" : "row")
      when narrow
        text "narrow" testId="narrow"
      when not narrow
        text "wide" testId="wide"
"#;

struct Nothing;

impl DataSource for Nothing {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, _: &[Value]) -> Result<Answer, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// Status, headers (names lowercased) and body of a GET with `headers`.
fn get(addr: SocketAddr, target: &str, headers: &str) -> (u16, Vec<(String, String)>, String) {
    let mut stream = TcpStream::connect_timeout(&addr, BOUND).unwrap();
    stream.set_read_timeout(Some(BOUND)).unwrap();
    write!(
        stream,
        "GET {target} HTTP/1.1\r\n{headers}Connection: close\r\n\r\n"
    )
    .unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    let mut lines = head.lines();
    let status = lines.next().unwrap()[9..12].parse().unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(": "))
        .map(|(n, v)| (n.to_ascii_lowercase(), v.to_string()))
        .collect();
    (status, headers, body.to_string())
}

#[test]
fn a_page_is_rendered_for_its_reader_s_viewport_and_kept_per_class() {
    super::warm_transport();
    let serve = Serve {
        dist: dist("viewport"),
        port: 0,
        name: "Page".into(),
        origin: None,
        deadline: Duration::from_secs(5),
        renders: 2,
        queue: 8,
        viewport: Default::default(),
        lifetime: Duration::from_secs(120),
        generations: None,
    };
    let plan = contract::compile(SRC).unwrap();
    let server = Server::bind(serve, plan, "").unwrap();
    let addr = server.addr();
    std::thread::spawn(move || server.run(|| Nothing));

    // A client that says nothing: the page viewport (390 wide), narrow.
    let (status, headers, body) = get(addr, "/", "");
    assert_eq!(status, 200);
    assert!(body.contains(">narrow<"), "{body}");
    // The page asks for the width and says what it varies by; its CSP stays.
    assert_eq!(header(&headers, "accept-ch"), Some("Sec-CH-Viewport-Width"));
    let vary: Vec<_> = headers
        .iter()
        .filter(|(n, _)| n == "vary")
        .map(|(_, v)| v.as_str())
        .collect();
    assert!(
        vary.contains(&"Sec-CH-Viewport-Width, Sec-CH-UA-Mobile, User-Agent"),
        "{vary:?}"
    );
    assert!(header(&headers, "content-security-policy").is_some());

    // A width hint: laid out for it.
    let (_, _, body) = get(addr, "/plain", "Sec-CH-Viewport-Width: 1440\r\n");
    assert!(body.contains(">wide<"), "{body}");
    // Another width of the same class is the kept page, a third class is not.
    let (_, headers, again) = get(addr, "/plain", "Sec-CH-Viewport-Width: 1024\r\n");
    assert!(header(&headers, "age").is_some(), "{headers:?}");
    assert_eq!(again, body);
    let (_, headers, narrow) = get(addr, "/plain", "Sec-CH-Viewport-Width: 600\r\n");
    assert!(header(&headers, "age").is_none(), "{headers:?}");
    assert!(narrow.contains(">narrow<"), "{narrow}");
    // A desktop browser without the width hint: its class, wide — the kept
    // page of that class.
    let (_, headers, desktop) = get(addr, "/plain", "Sec-CH-UA-Mobile: ?0\r\n");
    assert!(desktop.contains(">wide<"), "{desktop}");
    assert!(header(&headers, "age").is_some(), "{headers:?}");
    let safari = "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15\r\n";
    let (_, _, body) = get(addr, "/", safari);
    assert!(body.contains(">wide<"), "{body}");
}

#[test]
fn a_page_that_reads_no_viewport_asks_for_no_hints() {
    let (status, headers, _) = super::serve::get(super::serve::start("nohints", 1, 4, 5).addr, "/");
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "accept-ch"), None);
    assert!(headers
        .iter()
        .all(|(n, v)| n != "vary" || !v.contains("Sec-CH")));
}
