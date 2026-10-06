//! A request's deadline (`Request::timeout_ms`, TypeScript's `exactTimeout`):
//! the executor cancels the exchange when it passes, replies `Timeout`, and
//! frees the ordered lane for what waits behind it.
use super::*;
use std::io::{Read, Write};

fn timed_out(outcome: &Outcome, ms: u32) -> bool {
    matches!(outcome, Outcome::Failed { kind: FailureKind::Timeout, message }
        if message == &format!("the request timed out after {ms} ms"))
}

#[test]
fn a_held_request_times_out_and_the_ordered_request_behind_it_runs() {
    let (core, fixture, woke) = setup();
    let started = Instant::now();
    core.run(
        job(1, Request::get("https://example.test/hold").timeout(100)),
        None,
    )
    .unwrap();
    core.run(job(2, Request::get("https://example.test/next")), None)
        .unwrap();
    let outcomes = collect(&core, &woke, 2);
    assert!(timed_out(&outcomes[0].1, 100), "{outcomes:?}");
    assert_eq!(outcomes[0].0, 1);
    assert!(matches!(&outcomes[1], (2, Outcome::Response(r)) if r.body == b"done"));
    // Well before the fixture's own minute-long hold gives up.
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(fixture.state.lock().unwrap().2.len(), 2);
}

#[test]
fn a_request_that_answers_in_time_is_unaffected_by_its_deadline() {
    let (core, _fixture, woke) = setup();
    core.run(
        job(1, Request::get("https://example.test/read").timeout(5_000)),
        None,
    )
    .unwrap();
    assert!(matches!(&collect(&core, &woke, 1)[0], (1, Outcome::Response(r)) if r.body == b"done"));
}

#[test]
fn a_deadline_out_of_range_or_on_work_that_is_not_http_is_refused() {
    // Refused at admission, before any path runs it (the host settles the
    // refusal on the runner's ticket), and nothing reaches the server.
    let (core, fixture, _woke) = setup();
    let mut zero = Request::get("https://example.test/read");
    zero.timeout_ms = Some(0);
    assert_eq!(
        core.run(job(1, zero), None),
        Err("a request timeout must be 1 to 3600000 ms")
    );
    let (over, _, _) = setup();
    assert_eq!(
        over.run(
            job(
                2,
                Request::get("https://example.test/read").timeout(exact_runner::MAX_TIMEOUT_MS + 1)
            ),
            None
        ),
        Err("a request timeout must be 1 to 3600000 ms")
    );
    let (streams, _, _) = setup();
    let mut stream = Request::get("https://example.test/events").timeout(10);
    stream.stream = true;
    assert_eq!(
        streams.run(job(3, stream), None),
        Err("a stream has no timeout")
    );
    assert!(fixture.state.lock().unwrap().2.is_empty());
    assert_eq!(
        Request::native(vec![]).timeout(10).timeout_refusal(),
        Some("only HTTP takes a timeout")
    );
    let mut stream = Request::get("https://example.test/events").timeout(10);
    stream.stream = true;
    assert_eq!(stream.timeout_refusal(), Some("a stream has no timeout"));
}

/// The platform transport (URLSession on Apple) against servers that never
/// finish: one accepts and never answers, one sends its headers and then a
/// byte every 50 ms, which an idle timeout alone would never end. Each times
/// out on its deadline, and the trickling exchange is torn down.
#[test]
#[ignore = "async lane: a real socket under the platform transport (URLSession), timing-sensitive on a loaded machine; bun scripts/async.mjs runs it"]
fn the_platform_transport_cancels_a_silent_and_a_trickling_server_at_the_deadline() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (closed, closes) = channel::<&'static str>();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let closed = closed.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let which = if head.contains("/trickle") {
                    "trickle"
                } else {
                    "silent"
                };
                if which == "trickle" {
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\n");
                    while stream.write_all(b"x").and_then(|()| stream.flush()).is_ok() {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                } else {
                    // Blocks until the client closes the connection.
                    stream
                        .set_read_timeout(Some(Duration::from_secs(60)))
                        .unwrap();
                    while matches!(stream.read(&mut buf), Ok(n) if n > 0) {}
                }
                let _ = closed.send(which);
            });
        }
    });
    let grants = format!("net.fetch http://127.0.0.1:{port}");
    let owners = (0..WORKERS)
        .map(|_| {
            Some(ibex2::host::Host::new().endow(ibex2::grant::GrantSet::parse(&grants).unwrap()))
        })
        .collect();
    let (wake, woke) = channel();
    let core = Core::with_owners(
        owners,
        &grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
    );
    let started = Instant::now();
    core.run(
        job(
            1,
            Request::get(&format!("http://127.0.0.1:{port}/silent")).timeout(300),
        ),
        None,
    )
    .unwrap();
    core.run(
        job(
            2,
            Request::get(&format!("http://127.0.0.1:{port}/trickle")).timeout(400),
        ),
        None,
    )
    .unwrap();
    let outcomes = collect(&core, &woke, 2);
    assert!(timed_out(&outcomes[0].1, 300), "{outcomes:?}");
    assert!(timed_out(&outcomes[1].1, 400), "{outcomes:?}");
    // Ordered: 300 ms, then 400 ms more. The bounds only have to tell the
    // deadline from the servers, which hold for a minute: a heavily loaded
    // machine (load 200+) took over 5 s to see a cancelled socket close.
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(700) && elapsed < Duration::from_secs(30),
        "{elapsed:?}"
    );
    // The trickling exchange is torn down: its server's writes fail. The
    // silent one's socket is URLSession's to close when it likes (it has
    // been seen to keep one past 45 s after the cancel, unread), so only the
    // outcome above is asserted for it.
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut seen = Vec::new();
    while !seen.contains(&"trickle") {
        let left = deadline.saturating_duration_since(Instant::now());
        seen.push(
            closes
                .recv_timeout(left)
                .expect("the trickling connection left open"),
        );
    }
}
