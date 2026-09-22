//! A response may not cost more than the caller agreed to.
//!
//! `Request::max_body` is only a promise until a transport is made to keep it
//! against a peer that does not cooperate, so every case here is a server that
//! answers and then refuses to stop: chunked with no declared length and no
//! terminator, and a declared length larger than any host should accept. The
//! ceilings are small so the test is fast; the mechanism is the same at 64 MB.
//!
//! @ref LLP 0057#3-the-boundary — the platform executes the ceiling, Rust sets it

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use ibex2::stdlib::fetch::{Request, Transport};

/// What the server does once it has read a request head.
#[derive(Clone, Copy)]
enum Answer {
    /// Chunked, no `Content-Length`, and no terminating chunk — the shape a
    /// byte ceiling exists for, because nothing in the response says how much
    /// is coming or when it stops.
    ChunkedForever,
    /// A declared length no client should agree to read.
    DeclaresGigabytes,
    /// An ordinary small body, to prove the ceiling does not break traffic.
    Small,
}

/// A one-shot local HTTP server. Returns its URL; the thread ends with it.
fn serve(answer: Answer) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        read_head(&mut stream);
        match answer {
            Answer::ChunkedForever => {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n",
                );
                let chunk = vec![b'x'; 8 * 1024];
                // Until the client hangs up, which is the outcome under test.
                loop {
                    if stream
                        .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                        .and_then(|_| stream.write_all(&chunk))
                        .and_then(|_| stream.write_all(b"\r\n"))
                        .and_then(|_| stream.flush())
                        .is_err()
                    {
                        return;
                    }
                }
            }
            Answer::DeclaresGigabytes => {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4294967296\r\n\r\n",
                );
                let _ = stream.write_all(&vec![b'x'; 4096]);
                let _ = stream.flush();
                // Hold the connection so a client that ignored the declared
                // length would sit here rather than see a tidy EOF.
                thread::sleep(Duration::from_secs(5));
            }
            Answer::Small => {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello",
                );
                let _ = stream.flush();
            }
        }
    });
    format!("http://127.0.0.1:{port}/")
}

fn read_head(stream: &mut TcpStream) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while stream.read(&mut byte).map(|n| n == 1).unwrap_or(false) {
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            return;
        }
    }
}

fn bounded(url: &str, limit: usize) -> Request {
    let mut request = Request::get(url);
    request.max_body = Some(limit);
    request
}

/// The transport this platform actually uses, alongside the development one:
/// the ceiling is a property of the boundary, not of one implementation.
fn platform_transport() -> Box<dyn Transport> {
    ibex2::transport::default_transport()
}

fn refused(error: &str, limit: usize) -> bool {
    error.contains(&format!("exceeded the {limit}-byte limit"))
}

#[test]
fn a_chunked_response_with_no_end_is_refused_at_the_ceiling() {
    let limit = 64 * 1024;
    for (name, transport) in named_transports() {
        let url = serve(Answer::ChunkedForever);
        let error = transport
            .send(&bounded(&url, limit))
            .err()
            .unwrap_or_else(|| panic!("{name} accepted an endless response"));
        let text = format!("{error}");
        assert!(refused(&text, limit), "{name}: unexpected error {text}");
    }
}

#[test]
fn a_declared_length_over_the_ceiling_is_refused() {
    let limit = 64 * 1024;
    for (name, transport) in named_transports() {
        let url = serve(Answer::DeclaresGigabytes);
        let error = transport
            .send(&bounded(&url, limit))
            .err()
            .unwrap_or_else(|| panic!("{name} accepted a 4 GB declaration"));
        let text = format!("{error}");
        assert!(refused(&text, limit), "{name}: unexpected error {text}");
    }
}

#[test]
fn a_response_under_the_ceiling_is_untouched() {
    for (name, transport) in named_transports() {
        let url = serve(Answer::Small);
        let response = transport
            .send(&bounded(&url, 64 * 1024))
            .unwrap_or_else(|e| panic!("{name} refused a 5-byte body: {e}"));
        assert_eq!(response.status, 200, "{name}");
        assert_eq!(response.text(), "hello", "{name}");
    }
}

/// The default ceiling applies to a request that names none — the guarantee
/// cannot depend on every caller remembering to ask for it.
#[test]
fn a_request_that_names_no_ceiling_still_has_one() {
    assert_eq!(
        Request::get("http://127.0.0.1/").body_limit(),
        ibex2::stdlib::fetch::DEFAULT_MAX_BODY
    );
}

fn named_transports() -> Vec<(&'static str, Box<dyn Transport>)> {
    vec![
        (
            "dev-tcp",
            Box::new(ibex2::transport::DevTcpTransport::new()) as Box<dyn Transport>,
        ),
        ("platform", platform_transport()),
    ]
}

#[cfg(all(feature = "hermes", feature = "loader"))]
mod common;

#[cfg(all(feature = "hermes", feature = "loader"))]
mod request_headers {
    use super::*;
    use ibex2::engine::hermes::{DynamicCode, Grants, Hermes};
    use ibex2::loader::{ModuleGrants, Root};

    fn listener() -> (TcpListener, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        (listener, origin)
    }

    // Exercise the shipped factory, with the same frozen intrinsics as an
    // effect runtime. The raw binding is exposed only by this test's boot.
    fn run(name: &str, origin: &str, allowed: bool, source: &str) -> Vec<String> {
        let spec = if allowed {
            format!("net.fetch {origin}")
        } else {
            String::new()
        };
        run_with_setup(name, &spec, source, "")
    }

    fn run_with_setup(name: &str, spec: &str, source: &str, setup: &str) -> Vec<String> {
        let project = super::common::Project::new(name);
        project.file("index.js", source);
        let mut rt = Hermes::new(DynamicCode::Closed).unwrap();
        assert!(rt.install_stdlib());
        rt.eval(setup).unwrap();
        rt.install_bindings().unwrap();
        let grants = Grants::parse(spec).unwrap();
        assert!(rt.install_fetch(&grants));
        rt.set_loader(
            Root::Declared(project.0.clone()),
            ModuleGrants::parse(&format!("[*]\n{spec}\n")).unwrap(),
        )
        .unwrap();
        rt.harden().unwrap();
        rt.run_entry("./index.js").unwrap();
        rt.run_to_quiescence(Duration::from_secs(30));
        rt.drain_console().into_iter().map(|r| r.message).collect()
    }

    fn capture(listener: TcpListener) -> thread::JoinHandle<(String, Vec<u8>)> {
        capture_with_response(
            listener,
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".into(),
        )
    }

    fn capture_with_response(
        listener: TcpListener,
        response: String,
    ) -> thread::JoinHandle<(String, Vec<u8>)> {
        thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "no request arrived");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
                assert!(head.len() < 64 * 1024, "unbounded request head");
            }
            let head = String::from_utf8(head).unwrap();
            let length = head
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            assert!(length < 1024);
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            (head, body)
        })
    }

    fn folded_headers(head: &str) -> Vec<(String, String)> {
        // Fold names because transports may recase them. Keep every value
        // byte, including the transport's single space after the colon.
        head.split("\r\n")
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.to_string()))
            .collect()
    }

    fn assert_authored_headers(head: &str) {
        let headers = folded_headers(head);
        for (name, value) in [
            ("content-type", "application/json"),
            ("idempotency-key", "effect-123"),
            ("authorization", "Bearer token"),
            ("x-repeat", "first, second"),
        ] {
            let values: Vec<_> = headers
                .iter()
                .filter(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
                .collect();
            assert_eq!(
                values,
                [format!(" {value}")],
                "unexpected {name} in {head:?}"
            );
        }
    }

    fn assert_authored(head: &str, body: &[u8]) {
        assert_authored_headers(head);
        assert!(head.starts_with("POST /submit HTTP/1.1\r\n"), "{head:?}");
        assert_eq!(body, br#"{"ok":true}"#);
    }

    const RECORD: &str = "{'Content-Type':' application/json ', 'Idempotency-Key':'effect-123', Authorization:'Bearer token', 'X-Repeat':'first, second'}";
    const PAIRS: &str = "[['Content-Type',' application/json '], ['Idempotency-Key','effect-123'], ['Authorization','Bearer token'], ['X-Repeat','first'], ['x-repeat','second']]";

    #[test]
    fn raw_fetch_sends_the_headers_handle() {
        let (listener, origin) = listener();
        let server = capture(listener);
        let out = run("fetch-raw-headers", &origin, true, &format!(
            "__ibex2_fetch('{origin}/submit', 'POST', new TextEncoder().encode('{{\"ok\":true}}'), 'manual', new Headers({PAIRS})._handle).then(() => console.log('ok'), e => console.log(e.message));"
        ));
        assert_eq!(out, ["ok"]);
        let (head, body) = server.join().unwrap();
        assert_authored(&head, &body);
    }

    fn authored(name: &str, init: &str) {
        let (listener, origin) = listener();
        let server = capture(listener);
        let out = run(name, &origin, true, &format!(
            "const headers = {init};
             const pending = fetch('{origin}/submit', {{method:'post', headers, body:'{{\"ok\":true}}', redirect:'manual'}});
             if (headers instanceof Headers) headers.set('Idempotency-Key', 'changed');
             else if (Array.isArray(headers)) {{
               headers[1][1] = 'changed';
               headers.push(['Idempotency-Key', 'appended']);
             }} else headers['Idempotency-Key'] = 'changed';
             pending.then(r => r.text()).then(console.log, e => console.log(e.message));"
        ));
        assert_eq!(out, ["ok"]);
        let (head, body) = server.join().unwrap();
        assert_authored(&head, &body);
    }

    #[test]
    fn fetch_snapshots_a_headers_record() {
        authored("fetch-header-record", RECORD);
    }

    #[test]
    fn fetch_snapshots_header_pairs() {
        authored("fetch-header-pairs", PAIRS);
    }

    #[test]
    fn fetch_snapshots_a_headers_instance() {
        authored("fetch-header-instance", &format!("new Headers({PAIRS})"));
    }

    #[test]
    fn default_follow_retains_authored_headers_across_granted_origins() {
        let (destination, destination_origin) = listener();
        let destination = capture(destination);
        let (redirector, origin) = listener();
        let redirector = capture_with_response(redirector, format!(
            "HTTP/1.1 302 Found\r\nLocation: {destination_origin}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ));
        let out = run_with_setup(
            "fetch-follow-headers",
            &format!("net.fetch {origin}\nnet.fetch {destination_origin}"),
            &format!(
                "fetch('{origin}/submit', {{method:'POST', headers:{PAIRS}, body:'{{\"ok\":true}}'}})
                 .then(r => {{ console.log(r.status, r.redirected, r.url); return r.text(); }})
                 .then(console.log, e => console.log(e.message));"
            ),
            "",
        );
        assert_eq!(
            out,
            [format!("200 true {destination_origin}/final"), "ok".into()]
        );
        let (head, body) = redirector.join().unwrap();
        assert_authored(&head, &body);
        let (head, body) = destination.join().unwrap();
        assert!(head.starts_with("GET /final HTTP/1.1\r\n"), "{head:?}");
        assert!(body.is_empty(), "redirect rewrite must drop the body");
        // Pin the current follower: credentials and Content-Type survive
        // even this cross-origin POST-to-GET rewrite. Both servers were hit.
        assert_authored_headers(&head);
    }

    #[test]
    fn request_guard_strips_authored_transport_headers_on_the_wire() {
        let (listener, origin) = listener();
        let server = capture(listener);
        let out = run("fetch-guarded-headers", &origin, true, &format!(
            "fetch('{origin}/', {{headers:{{Host:'evil.example', 'Content-Length':'999', Connection:'x-authored-connection', 'X-Fine':'yes'}}}})
             .then(r => console.log(r.status), e => console.log(e.message));"
        ));
        assert_eq!(out, ["200"]);
        let (head, body) = server.join().unwrap();
        let headers = folded_headers(&head);
        assert!(
            headers.contains(&("x-fine".into(), " yes".into())),
            "{head:?}"
        );
        for (name, authored) in [
            ("host", "evil.example"),
            ("content-length", "999"),
            ("connection", "x-authored-connection"),
        ] {
            assert!(
                !headers
                    .iter()
                    .any(|(key, value)| key == name && value.contains(authored)),
                "authored {name} escaped the guard: {head:?}"
            );
        }
        assert!(body.is_empty());
    }

    #[test]
    fn fetch_without_init_and_raw_without_headers_reach_the_wire() {
        for (name, call) in [
            ("wrapped", "fetch(url)"),
            ("omitted", "__ibex2_fetch(url, '', undefined, 'manual')"),
            (
                "undefined",
                "__ibex2_fetch(url, '', undefined, 'manual', undefined)",
            ),
        ] {
            let (listener, origin) = listener();
            let server = capture(listener);
            let out = run_with_setup(
                &format!("fetch-empty-headers-{name}"),
                &format!("net.fetch {origin}"),
                &format!(
                    "const url = '{origin}/ping';
                     {call}.then(r => console.log(typeof r === 'number' ? testResponseStatus(r) : r.status), e => console.log(e.message));"
                ),
                // Retain just the status accessor in this test's bootstrap.
                "(function () { const field = __ibex2_response_field; globalThis.testResponseStatus = h => field(h, 0); })();",
            );
            assert_eq!(out, ["200"], "{name}");
            // Joining a capture proves accept fired, not just a JS rejection.
            let (head, body) = server.join().unwrap();
            assert!(head.starts_with("GET /ping HTTP/1.1\r\n"), "{head:?}");
            for (key, _) in folded_headers(&head) {
                assert!(
                    ![
                        "content-type",
                        "authorization",
                        "idempotency-key",
                        "x-repeat"
                    ]
                    .contains(&key.as_str()),
                    "unexpected authored header: {head:?}"
                );
            }
            assert!(body.is_empty());
        }
    }

    #[test]
    fn raw_fetch_with_valid_headers_still_denies_before_accept() {
        let (listener, origin) = listener();
        let out = run(
            "fetch-denied-raw-headers",
            &origin,
            false,
            &format!(
                "const headers = new Headers({{x:'y'}});
             __ibex2_fetch('{origin}/', '', undefined, 'manual', headers._handle)
             .then(() => console.log('accepted'), e => console.log(e.message, headers.get('x')));"
            ),
        );
        assert_eq!(out, ["denied: net.fetch y"]);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn raw_fetch_keeps_a_callers_handle_alive_for_reuse() {
        let (first, origin) = listener();
        let second = first.try_clone().unwrap();
        let server = thread::spawn(move || {
            let first = capture(first).join().unwrap();
            let second = capture(second).join().unwrap();
            [first, second]
        });
        let out = run("fetch-reused-raw-headers", &origin, true, &format!(
            "const headers = new Headers({PAIRS});
             function send() {{ return __ibex2_fetch('{origin}/submit', 'POST', new TextEncoder().encode('{{\"ok\":true}}'), 'manual', headers._handle); }}
             send().then(send).then(() => console.log(headers.get('Idempotency-Key')), e => console.log(e.message));"
        ));
        assert_eq!(out, ["effect-123"]);
        for (head, body) in server.join().unwrap() {
            assert_authored(&head, &body);
        }
    }

    // Observe real registry allocations and liveness through existing ops,
    // captured before bindings remove them; no production test API is needed.
    const TRACK_HEADERS: &str = r#"(function () {
      const ops = __ibex2_headers, create = ops.create, handles = [];
      ops.create = function () { const h = create(); handles.push(h); return h; };
      globalThis.headerLiveness = function () {
        return handles.map(h => {
          try { ops.count(h); return true; }
          catch (e) { if (!e.message.includes('unknown headers handle')) throw e; return false; }
        }).join(',');
      };
    })();"#;

    #[test]
    fn fetch_releases_its_snapshot_after_success() {
        let (listener, origin) = listener();
        let server = capture(listener);
        let out = run_with_setup("fetch-release-success", &format!("net.fetch {origin}"), &format!(
            "if (typeof globalThis.__ibex2_headers_free !== 'undefined') throw new Error('free op exposed');
             const headers = new Headers({PAIRS});
             const pending = fetch('{origin}/submit', {{method:'POST', headers, body:'{{\"ok\":true}}'}});
             console.log(headerLiveness());
             pending.then(r => {{ console.log(headerLiveness(), headers.get('Idempotency-Key')); return r.text(); }})
             .then(console.log, e => console.log(e.message));"
        ), TRACK_HEADERS);
        assert_eq!(out, ["true,true", "true,false,true effect-123", "ok"]);
        let (head, body) = server.join().unwrap();
        assert_authored(&head, &body);
    }

    #[test]
    fn fetch_releases_its_snapshot_after_rejection() {
        let (listener, origin) = listener();
        let out = run_with_setup("fetch-release-rejection", "", &format!(
            "const headers = new Headers({RECORD});
             const pending = fetch('{origin}/', {{headers}});
             console.log(headerLiveness());
             pending.then(() => console.log('accepted'), e => console.log(e.message, headerLiveness(), headers.get('Idempotency-Key')));"
        ), TRACK_HEADERS);
        assert_eq!(
            out,
            ["true,true", "denied: net.fetch true,false effect-123"]
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn fetch_releases_a_partially_initialized_header_list() {
        let (listener, origin) = listener();
        let out = run_with_setup("fetch-release-invalid", &format!("net.fetch {origin}"), &format!(
            "fetch('{origin}/', {{headers:[['x','y'], ['bad name','value']]}})
             .then(() => console.log('accepted'), e => console.log(e.constructor.name, headerLiveness()));"
        ), TRACK_HEADERS);
        assert_eq!(out, ["TypeError false"]);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn fetch_releases_its_snapshot_when_raw_argument_conversion_throws() {
        let (listener, origin) = listener();
        let out = run_with_setup("fetch-release-sync-throw", &format!("net.fetch {origin}"), &format!(
            "fetch('{origin}/', {{headers:{{x:'y'}}, body:{{toString() {{ throw new Error('body conversion failed'); }}}}}})
             .then(() => console.log('accepted'), e => console.log(e.message, headerLiveness()));"
        ), TRACK_HEADERS);
        assert_eq!(out, ["body conversion failed false"]);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn fetch_without_a_url_rejects_with_a_missing_url_type_error() {
        let out = run("fetch-missing-url", "", false,
            "try { fetch().then(() => console.log('accepted'), e => console.log(e.constructor.name, e.message)); }
             catch (e) { console.log('sync throw', e.message); }");
        assert_eq!(out, ["TypeError fetch expects a URL"]);
    }

    #[test]
    fn fetch_rejects_invalid_header_lists_before_a_request() {
        let (listener, origin) = listener();
        let out = run(
            "fetch-invalid-headers",
            &origin,
            true,
            &format!(
                r#"Promise.all([
              [['x']], [['x', 'y', 'z']], {{'bad name':'value'}},
              {{x:'bad\r\ninjection'}}, {{x:'bad\nline'}}, {{x:'bad\0byte'}}, {{x:'\u0100'}}
            ].map(headers => {{
              try {{ return fetch('{origin}/', {{headers}}).then(() => 'accepted', e => e.constructor.name); }}
              catch (e) {{ return 'sync throw: ' + e.constructor.name; }}
            }})).then(results => console.log(results.join(',')));"#
            ),
        );
        assert_eq!(out, [["TypeError"; 7].join(",")]);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn raw_fetch_rejects_invalid_or_unknown_header_handles() {
        let (listener, origin) = listener();
        let out = run("fetch-invalid-handles", &origin, false, &format!(
            "const h = new Headers()._handle;
             Promise.all([null, true, '1', {{}}, [], NaN, Infinity, -1, 0, h + 0.5, 9007199254740992, 999999].map(handle =>
               __ibex2_fetch('{origin}/', '', undefined, 'manual', handle).then(() => 'accepted', e => e.message.startsWith('invalid argument:'))
             )).then(results => console.log(results.join(',')));"
        ));
        assert_eq!(out, [["true"; 12].join(",")]);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn authored_headers_do_not_bypass_the_origin_grant() {
        let (listener, origin) = listener();
        let out = run("fetch-denied-headers", &origin, false, &format!(
            "fetch('{origin}/', {{headers:{RECORD}}}).then(() => console.log('accepted'), e => console.log(e.message));"
        ));
        assert_eq!(out, ["denied: net.fetch"]);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
