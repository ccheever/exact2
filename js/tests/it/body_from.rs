//! `fetch(url, {exactBodyFrom})` from TypeScript (LLP 1108 D6 R2): Hermes
//! and the prelude send the path alone, and the native executor core (the
//! one the Apple, Linux and render hosts share, here the render host's)
//! reads the file into the body when it runs the request. The bytes never
//! cross the module, so a 2 MB upload's step stays far under its budget.
#![cfg(all(exact_js_engine, unix))]
use super::storage::{args, text, Root};
use exact_runner::{Answer, DataSource, FailureKind, Outcome, RequestOut, Store};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// What a server on 127.0.0.1 received: each request's head and body.
struct Server {
    port: u16,
    received: Receiver<(String, Vec<u8>)>,
}

impl Server {
    fn start() -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, received) = channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let (mut head, mut length) = (String::new(), 0usize);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = n.trim().parse().unwrap();
                    }
                    head.push_str(&line);
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 8\r\nconnection: close\r\n\r\nreceived",
                );
                let _ = tx.send((head, body));
            }
        });
        Server { port, received }
    }
}

/// The executor core under the module's file grant and this server's origin,
/// with the app's files at `root` (or none).
fn executor(server: &Server, root: Option<&Root>) -> exact_render::Executor {
    let grants = format!(
        "fs.read app:/data\nnet.fetch http://127.0.0.1:{}",
        server.port
    );
    let executor = exact_render::Executor::start(&grants);
    if let Some(root) = root {
        executor.set_app_roots([
            root.0.join("data"),
            root.0.join("cache"),
            root.0.join("tmp"),
        ]);
    }
    executor
}

/// Run `request` (sent to `server` in place of the fixture's origin) and
/// wait for its outcome.
fn run(
    executor: &exact_render::Executor,
    server: &Server,
    mut request: exact_runner::Request,
) -> Outcome {
    request.url = format!("http://127.0.0.1:{}/upload", server.port);
    executor
        .run(
            RequestOut {
                ticket: 1,
                target: "upload".into(),
                request,
                forced: false,
            },
            None,
        )
        .unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some((_, outcome, _)) = executor.drain().pop() {
            return outcome;
        }
        assert!(Instant::now() < until, "no outcome");
        executor.wait(until);
    }
}

/// 2,000,000 bytes, every value of a byte, in no simple repeat.
fn photo() -> Vec<u8> {
    (0..2_000_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect()
}

#[test]
fn a_two_megabyte_body_from_a_file_arrives_byte_for_byte_and_its_step_is_quick() {
    let root = Root::new();
    let mut m = root.module();
    // The budget a source's step has on every host (js/src/lib.rs).
    m.set_budget_ms(100.0);
    m.activate().unwrap();
    let mut s = Store::new(super::storage::GRANTS, Vec::<(String, String)>::new());
    let bytes = photo();
    std::fs::write(root.0.join("data/photo.jpg"), &bytes).unwrap();
    let a = args("upload", "app:/data/photo.jpg");
    let started = Instant::now();
    let answer = m.answer(&mut s, "work", &a).unwrap();
    let step = started.elapsed();
    let Answer::Later(request) = answer else {
        panic!("the upload is a request")
    };
    // The path alone crossed: no bytes, and no Content-Type but the author's.
    assert_eq!(request.body_from.as_deref(), Some("app:/data/photo.jpg"));
    assert!(request.body.is_empty());
    assert_eq!(
        request.headers,
        [("content-type".to_string(), "image/jpeg".to_string())]
    );
    eprintln!("a 2,000,000-byte exactBodyFrom upload's step: {step:?}");
    // It was 106.8 ms with the bytes read and base64'd in JavaScript.
    assert!(step < Duration::from_millis(25), "{step:?}");
    let server = Server::start();
    let outcome = run(&executor(&server, Some(&root)), &server, request);
    let (head, body) = server
        .received
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    assert!(head.starts_with("POST /upload "), "{head}");
    assert_eq!(body.len(), bytes.len());
    assert!(body == bytes, "the body differs from the file");
    let types: Vec<_> = head
        .lines()
        .filter(|l| l.to_ascii_lowercase().starts_with("content-type:"))
        .collect();
    assert_eq!(types, ["content-type: image/jpeg"], "{head}");
    let Answer::Now(value) = m.parse(&mut s, "work", &a, outcome).unwrap() else {
        panic!("the reply answers")
    };
    assert_eq!(text(value), "200 received");
}

/// What `fetch` refuses itself, before any request: each a TypeError.
#[test]
fn a_body_from_fetch_refuses_is_a_type_error_before_any_request() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(super::storage::GRANTS, Vec::<(String, String)>::new());
    let refused = super::storage::call(&mut m, &mut s, "upload-refusals", "");
    assert_eq!(
        refused.lines().collect::<Vec<_>>(),
        [
            "TypeError: exactBodyFrom must be an app:/ path",
            "TypeError: exactBodyFrom must be an app:/ path",
            "TypeError: exactBodyFrom: an app:/ path has no . or .. segment",
            "TypeError: fetch: a request has one body: body or exactBodyFrom",
            "TypeError: fetch: a GET request cannot have a body",
            "TypeError: fetch: a HEAD request cannot have a body",
        ],
        "{refused}"
    );
    assert_eq!(m.in_flight(), 0);
}

/// What the host refuses when it runs the request, before anything is sent:
/// a path outside the `fs.read` grant, a missing file, one over 64 MiB, and
/// a host with no app files. Each rejects the fetch, naming why.
#[test]
fn the_host_refuses_a_file_it_may_not_or_cannot_send_before_sending() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(super::storage::GRANTS, Vec::<(String, String)>::new());
    std::fs::create_dir_all(root.0.join("tmp")).unwrap();
    std::fs::write(root.0.join("tmp/secret.jpg"), b"secret").unwrap();
    // A sparse file one byte over the bound: nothing is read to refuse it.
    let big = std::fs::File::create(root.0.join("data/big.mov")).unwrap();
    big.set_len((64 << 20) + 1).unwrap();
    drop(big);
    std::fs::write(root.0.join("data/photo.jpg"), b"photo").unwrap();
    let server = Server::start();
    for (path, roots, kind, says) in [
        (
            "app:/tmp/secret.jpg",
            true,
            FailureKind::Refused,
            "denied: fs.read app:/tmp/secret.jpg",
        ),
        (
            "app:/data/absent.jpg",
            true,
            FailureKind::Refused,
            "No such file",
        ),
        (
            "app:/data/big.mov",
            true,
            FailureKind::Refused,
            "too-large: 67108865 bytes is over 67108864",
        ),
        (
            "app:/data/photo.jpg",
            false,
            FailureKind::Unsupported,
            "this host has no app files",
        ),
    ] {
        let a = args("upload", path);
        let Answer::Later(request) = m.answer(&mut s, "work", &a).unwrap() else {
            panic!("{path}: a request")
        };
        let executor = executor(&server, roots.then_some(&root));
        let outcome = run(&executor, &server, request);
        match &outcome {
            Outcome::Failed { kind: k, message } => {
                assert_eq!(*k, kind, "{path}: {message}");
                assert!(
                    message.contains(&format!("exactBodyFrom {path}: ")),
                    "{message}"
                );
                assert!(message.contains(says), "{path}: {message}");
            }
            other => panic!("{path}: {other:?}"),
        }
        let Answer::Now(value) = m.parse(&mut s, "work", &a, outcome).unwrap() else {
            panic!("{path}: the refusal answers")
        };
        let said = text(value);
        assert!(said.starts_with("FetchError "), "{said}");
        assert!(said.contains(says), "{said}");
    }
    assert!(
        server.received.try_recv().is_err(),
        "a refused body reached the server"
    );
}
