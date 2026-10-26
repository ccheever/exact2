//! `fetch(url, {exactBodyTo})` from TypeScript (LLP 1108 D6 R2, 2026-10-26):
//! Hermes and the prelude send the path alone, and the native executor core
//! (the one the Apple, Linux and render hosts share, here the render host's)
//! writes a 2xx body into the file as it arrives. The bytes never cross the
//! module; the response names how many were written.
#![cfg(all(exact_js_engine, unix))]
use super::storage::{args, text, Root};
use exact_runner::{Answer, DataSource, FailureKind, Outcome, RequestOut, Store};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// 17 MiB and a few bytes, every value of a byte, in no simple repeat.
fn big() -> Vec<u8> {
    (0..(17u32 << 20) + 5)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect()
}

/// A server on 127.0.0.1: `/big` answers 200 with [`big`] in 64 KiB chunks
/// (chunked transfer coding), anything else 404 with a short body. Each
/// request's first line is sent on `seen`.
struct Server {
    port: u16,
    seen: Receiver<String>,
}

impl Server {
    fn start() -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, seen) = channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                if first.starts_with("GET /big ") {
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n",
                    );
                    for chunk in big().chunks(64 << 10) {
                        let _ = write!(stream, "{:x}\r\n", chunk.len());
                        let _ = stream.write_all(chunk);
                        let _ = stream.write_all(b"\r\n");
                    }
                    let _ = stream.write_all(b"0\r\n\r\n");
                } else {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\ncontent-length: 13\r\nconnection: close\r\n\r\nno such thing",
                    );
                }
                let _ = tx.send(first);
            }
        });
        Server { port, seen }
    }
}

/// The executor core under the module's file grant and this server's origin,
/// with the app's files at `root` (or none).
fn executor(server: &Server, root: Option<&Root>) -> exact_render::Executor {
    let grants = format!(
        "fs.write app:/data\nnet.fetch http://127.0.0.1:{}",
        server.port
    );
    let executor = exact_render::Executor::start(&grants);
    if let Some(root) = root {
        executor.set_app_roots(["data", "cache", "tmp"].map(|d| root.0.join(d)));
    }
    executor
}

/// Run `request` against `server`'s `path` and wait for its outcome.
fn run(
    executor: &exact_render::Executor,
    server: &Server,
    path: &str,
    mut request: exact_runner::Request,
) -> Outcome {
    request.url = format!("http://127.0.0.1:{}{path}", server.port);
    let out = RequestOut {
        ticket: 1,
        target: "download".into(),
        request,
        forced: false,
    };
    executor.run(out, None).unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some((_, outcome, _)) = executor.drain().pop() {
            return outcome;
        }
        assert!(Instant::now() < until, "no outcome");
        executor.wait(until);
    }
}

#[test]
fn a_2xx_body_goes_to_the_file_and_a_reply_that_is_not_2xx_to_the_module() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(super::storage::GRANTS, Vec::<(String, Vec<u8>)>::new());
    std::fs::write(root.0.join("data/big.bin"), b"old").unwrap();
    let server = Server::start();
    let executor = executor(&server, Some(&root));

    let a = args("download", "app:/data/big.bin");
    let Answer::Later(request) = m.answer(&mut s, "work", &a).unwrap() else {
        panic!("a download is a request")
    };
    assert_eq!(request.body_to.as_deref(), Some("app:/data/big.bin"));
    let outcome = run(&executor, &server, "/big", request);
    assert!(
        matches!(&outcome, Outcome::Response(r) if r.body.is_empty()),
        "{outcome:?}"
    );
    let Answer::Now(value) = m.parse(&mut s, "work", &a, outcome).unwrap() else {
        panic!("the reply answers")
    };
    let size = big().len();
    assert_eq!(
        text(value),
        format!(
            r#"200 {{"path":"app:/data/big.bin","bytes":{size}}}  connection,transfer-encoding"#
        )
    );
    let written = std::fs::read(root.0.join("data/big.bin")).unwrap();
    assert!(written == big(), "the file differs from the body");

    // A 404's body is the module's, and the file is untouched.
    std::fs::write(root.0.join("data/kept.bin"), b"old").unwrap();
    let a = args("download-missing", "app:/data/kept.bin");
    let Answer::Later(request) = m.answer(&mut s, "work", &a).unwrap() else {
        panic!("a download is a request")
    };
    let outcome = run(&executor, &server, "/missing", request);
    let Answer::Now(value) = m.parse(&mut s, "work", &a, outcome).unwrap() else {
        panic!("the reply answers")
    };
    assert_eq!(
        text(value),
        "404 null no such thing connection,content-length"
    );
    assert_eq!(std::fs::read(root.0.join("data/kept.bin")).unwrap(), b"old");
    let names: Vec<_> = std::fs::read_dir(root.0.join("data"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n.to_string_lossy().starts_with(".exact-"))
        .collect();
    assert!(names.is_empty(), "a staging file was left: {names:?}");
}

/// What `fetch` refuses itself, before any request: each a TypeError.
#[test]
fn a_body_to_fetch_refuses_is_a_type_error_before_any_request() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(super::storage::GRANTS, Vec::<(String, Vec<u8>)>::new());
    let refused = super::storage::call(&mut m, &mut s, "download-refusals", "");
    assert_eq!(
        refused.lines().collect::<Vec<_>>(),
        [
            "TypeError: exactBodyTo must be an app:/ path",
            "TypeError: exactBodyTo must be an app:/ path",
            "TypeError: exactBodyTo: an app:/ path has no . or .. segment",
            "TypeError: exactBodyTo: a path is at most 4096 bytes",
            "TypeError: exactBodyTo: a HEAD reply has no body to write",
            "TypeError: exactBodyTo: a stream's body is its messages, not a file",
            "TypeError: exactIndependentHttp.maxResponseBytes must be an integer from 1 to 1073741824",
            "TypeError: exactIndependentHttp.maxResponseBytes must be an integer from 1 to 67108864",
        ],
        "{refused}"
    );
    assert_eq!(m.in_flight(), 0);
}

/// What the host refuses before anything is sent: a path outside the
/// `fs.write` grant, a missing parent, and a host with no app files.
#[test]
fn the_host_refuses_a_file_it_may_not_write_before_sending() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(super::storage::GRANTS, Vec::<(String, Vec<u8>)>::new());
    let server = Server::start();
    for (path, roots, kind, says) in [
        (
            "app:/tmp/a.bin",
            true,
            FailureKind::Refused,
            "denied: fs.write app:/tmp/a.bin",
        ),
        (
            "app:/data/none/a.bin",
            true,
            FailureKind::Refused,
            "No such file",
        ),
        (
            "app:/data/a.bin",
            false,
            FailureKind::Unsupported,
            "this host has no app files",
        ),
    ] {
        let a = args("download", path);
        let Answer::Later(request) = m.answer(&mut s, "work", &a).unwrap() else {
            panic!("{path}: a request")
        };
        let executor = executor(&server, roots.then_some(&root));
        let outcome = run(&executor, &server, "/big", request);
        match &outcome {
            Outcome::Failed { kind: k, message } => {
                assert_eq!(*k, kind, "{path}: {message}");
                assert!(
                    message.contains(&format!("exactBodyTo {path}: ")),
                    "{message}"
                );
                assert!(message.contains(says), "{path}: {message}");
            }
            other => panic!("{path}: {other:?}"),
        }
        let Answer::Now(value) = m.parse(&mut s, "work", &a, outcome).unwrap() else {
            panic!("{path}: the refusal answers")
        };
        assert!(text(value).starts_with("FetchError "));
    }
    assert!(
        server.seen.try_recv().is_err(),
        "a refused download was sent"
    );
}
