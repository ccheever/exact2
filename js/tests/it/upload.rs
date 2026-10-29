//! `fetch(url, {exactStream, exactUploadProgress: true})` from TypeScript
//! (LLP 1016.000 D10): the prelude sends the request as an upload, the
//! native executor core (the one the Apple, Linux and render hosts share,
//! here the render host's, whose transport is the portable one, rustls, on a
//! Mac too: LLP 1048.000 D10) tells the body's bytes sent as they go, and the
//! mapper gets each as `progress` and then the reply as `load`. URLSession's
//! is `exact-apple`'s `an_upload_through_urlsession_…`.
#![cfg(all(exact_js_engine, unix))]
use super::storage::{args, call, text, Root};
use exact_runner::{Answer, DataSource, Outcome, RequestOut, Store};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// The upload's size: past what loopback's socket buffers take at once.
const SIZE: usize = 16 << 20;

/// A server on 127.0.0.1 that takes a request's body 256 KiB at a time,
/// 4 ms apart, and answers 201 with the bytes it took, `wait` after the
/// body. Its port.
fn server(wait: Duration) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = n.trim().parse().unwrap();
                }
            }
            let (mut took, mut chunk) = (0, vec![0; 256 << 10]);
            while took < length {
                let n = reader
                    .read(&mut chunk[..(length - took).min(256 << 10)])
                    .unwrap_or(0);
                if n == 0 {
                    break;
                }
                took += n;
                std::thread::sleep(Duration::from_millis(4));
            }
            std::thread::sleep(wait);
            let body = format!("{{\"took\":{took}}}");
            let _ = write!(
                stream,
                "HTTP/1.1 201 Created\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    port
}

/// The fixture's `op` uploads `body` (`app:/data/up.bin`) to `server(wait)`:
/// its deadline, and what the mapper answers to each outcome, until the one
/// that is no message.
fn mapped(op: &str, body: &[u8], wait: Duration) -> (Option<u32>, Vec<String>) {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(super::storage::GRANTS, Vec::<(String, Vec<u8>)>::new());
    std::fs::write(root.0.join("data/up.bin"), body).unwrap();
    let port = server(wait);
    let executor = exact_render::Executor::start(&format!(
        "fs.read app:/data\nnet.fetch http://127.0.0.1:{port}"
    ));
    executor.set_app_roots(["data", "cache", "tmp"].map(|d| root.0.join(d)));

    let a = args(op, "app:/data/up.bin");
    let Answer::Later(mut request) = m.answer(&mut s, "work", &a).unwrap() else {
        panic!("an upload is a request")
    };
    assert!(request.stream && request.upload_progress && !request.bytes);
    assert_eq!(request.body_from.as_deref(), Some("app:/data/up.bin"));
    let deadline = request.timeout_ms;
    request.url = format!("http://127.0.0.1:{port}/upload");
    let out = RequestOut {
        ticket: 1,
        target: "upload".into(),
        request,
        forced: false,
    };
    executor.run(out, None).unwrap();

    let (mut said, until) = (Vec::new(), Instant::now() + wait + Duration::from_secs(30));
    loop {
        let Some((_, outcome, _)) = executor.drain().pop() else {
            assert!(Instant::now() < until, "no end after {said:?}");
            executor.wait(until);
            continue;
        };
        let ended = !matches!(outcome, Outcome::Message(_));
        let Answer::Now(value) = m.parse(&mut s, "work", &a, outcome).unwrap() else {
            panic!("the mapper answers now")
        };
        said.push(text(value));
        if ended {
            return (deadline, said);
        }
    }
}

#[test]
fn an_upload_maps_its_bytes_sent_as_they_go_then_its_reply() {
    let (deadline, said) = mapped("upload-progress", &vec![7; SIZE], Duration::ZERO);
    assert_eq!(deadline, Some(30_000), "an upload takes a deadline");
    let (end, progress) = said.split_last().unwrap();
    assert_eq!(end, &format!("load 201 {{\"took\":{SIZE}}}"), "{said:?}");
    let sent: Vec<u64> = progress
        .iter()
        .map(|line| {
            let (loaded, rest) = line
                .strip_prefix("progress ")
                .and_then(|l| l.split_once('/'))
                .unwrap_or_else(|| panic!("{line}"));
            assert_eq!(rest, format!("{SIZE} true"), "{line}");
            loaded.parse().unwrap()
        })
        .collect();
    assert!(sent.windows(2).all(|w| w[0] <= w[1]), "{sent:?} went back");
    assert!(
        sent.iter().any(|&n| n < SIZE as u64),
        "nothing before the whole: {sent:?}"
    );
    assert_eq!(
        sent.last(),
        Some(&(SIZE as u64)),
        "the last progress is the whole body"
    );
}

/// An empty body has no bytes to tell (XMLHttpRequest's `upload` fires no
/// computable `progress` for one): its reply alone.
#[test]
fn an_empty_upload_maps_no_progress_then_its_reply() {
    let (_, said) = mapped("upload-progress", &[], Duration::ZERO);
    assert_eq!(said, ["load 201 {\"took\":0}"]);
}

/// A reply 32 s after the body, past the portable transport's own limits
/// (15 s for the response, 30 s for the whole), which the upload's deadline
/// of 60 s replaces.
#[test]
#[ignore = "async lane: a real socket and a reply 32 s after its body; bun scripts/async.mjs runs it"]
fn an_upload_waits_out_a_reply_past_the_portable_transports_own_limits() {
    let (deadline, said) = mapped("upload-patient", &[7; 1 << 20], Duration::from_secs(32));
    assert_eq!(deadline, Some(60_000));
    assert_eq!(
        said.last().map(String::as_str),
        Some("load 201 {\"took\":1048576}"),
        "{said:?}"
    );
}

/// What `fetch` refuses itself, before any request: each a TypeError.
#[test]
fn an_upload_fetch_refuses_is_a_type_error_before_any_request() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(super::storage::GRANTS, Vec::<(String, Vec<u8>)>::new());
    let refused = call(&mut m, &mut s, "upload-progress-refusals", "");
    assert_eq!(
        refused.lines().collect::<Vec<_>>(),
        [
            "TypeError: exactUploadProgress is true or false",
            "TypeError: exactUploadProgress reports an upload's progress to its stream: give exactStream too",
            "TypeError: exactUploadProgress: a GET or HEAD request sends no body",
            "TypeError: exactUploadProgress: a WebSocket sends no body",
            "TypeError: exactUploadProgress: a long poll sends no body",
            "TypeError: exactUploadProgress: an upload follows its redirects",
            "TypeError: exactTimeout: a stream has no timeout",
        ],
        "{refused}"
    );
    assert_eq!(m.in_flight(), 0);
}
