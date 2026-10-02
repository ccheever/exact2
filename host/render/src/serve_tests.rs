//! The server's own unit tests (`serve.rs`).
use super::*;

#[test]
fn a_socket_grant_is_a_connect_source() {
    let policy = csp(
        "net.fetch https://api.test\nnet.websocket wss://jetstream.test\nfs.read /x",
        Path::new("/nonexistent"),
    );
    assert!(
        policy.contains("connect-src 'self' https://api.test wss://jetstream.test;"),
        "{policy}"
    );
}

#[test]
fn the_boot_swap_script_is_admitted_by_its_hash() {
    // @ref LLP 1048.005 — the one inline script a boot document's page adds.
    let policy = csp("", Path::new("/nonexistent"));
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(crate::direct::boot_swap_js().as_bytes());
    let admitted = format!("'sha256-{}'", exact_data::envelope::base64(&digest));
    assert!(policy.contains(&admitted), "{policy}");
}

#[test]
fn the_cache_budget_counts_each_page_s_variants() {
    let page = |body: usize| {
        CachedPage {
            target: "/p".into(),
            class: String::new(),
            created: Instant::now(),
            response: Response::text(200, "").header("ETag", "\"t\""),
            best: Arc::new(OnceLock::new()),
            hash: String::new(),
            pairs: HashMap::new(),
        }
        .with_body(body)
    };
    let mut pages = VecDeque::from([page(MAX_CACHE_BYTES - 100)]);
    assert!(!over(&pages, 0));
    // A pair against a dictionary counts, and a pair that didn't shrink doesn't.
    pages[0].pairs.insert("d".into(), Some(vec![0; 101]));
    pages[0].pairs.insert("e".into(), None);
    assert_eq!(pages[0].bytes(), MAX_CACHE_BYTES + 1);
    assert!(over(&pages, 0));
    // So do the best variants, once made.
    pages[0].pairs.clear();
    let body: Vec<u8> = (0..4096u32)
        .flat_map(|i| (i * 7919 % 251).to_le_bytes())
        .collect();
    let _ = pages[0].best.set(encode::Best::of(&body));
    let best = pages[0].best.get().unwrap().bytes();
    assert!(best > 0);
    assert_eq!(pages[0].bytes(), MAX_CACHE_BYTES - 100 + best);
    assert!(over(&pages, 0));
}

#[test]
fn invalid_response_headers_fail_before_any_bytes_are_written() {
    for status in [200, 304] {
        for value in [
            "noindex\r\n\r\ninjected",
            "noindex\nX-Fake: yes",
            "noindex\0",
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (mut server, _) = listener.accept().unwrap();
            Response::text(status, "original")
                .header("X-Robots-Tag", value)
                .write(&mut server, false, "default-src 'self'", "", false);
            server.shutdown(std::net::Shutdown::Write).unwrap();
            let mut received = String::new();
            client.read_to_string(&mut received).unwrap();
            assert!(received.starts_with("HTTP/1.1 500 "), "{received}");
            assert!(!received.contains("X-Robots-Tag"));
            assert!(!received.contains("injected"));
        }
    }
}

#[test]
fn a_page_carries_the_permissions_policy_its_grants_derive() {
    let policy = exact_runner::device::permissions_policy(
        "net.fetch https://x/\ndevice.microphone purpose.mic",
    );
    assert_eq!(policy, "microphone=(self), camera=(), geolocation=()");
    for (content_type, expected) in [("text/html; charset=utf-8", true), ("text/plain", false)] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        let mut page = Response::text(200, "<p>");
        page.headers[0].1 = content_type.into();
        let lines = format!("Permissions-Policy: {policy}\r\n");
        page.write(&mut server, false, "default-src 'self'", &lines, false);
        server.shutdown(std::net::Shutdown::Write).unwrap();
        let mut received = String::new();
        client.read_to_string(&mut received).unwrap();
        assert_eq!(
            received.contains(
                "\r\nPermissions-Policy: microphone=(self), camera=(), geolocation=()\r\n"
            ),
            expected,
            "{received}"
        );
    }
}

#[test]
fn a_request_head_that_drips_is_refused_at_its_bound() {
    // A hang bound, never a deadline.
    const BOUND: Duration = Duration::from_secs(60);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut server, _) = listener.accept().unwrap();
    // The start of a head, then a byte of it every 20 ms: each read gets
    // one long before any idle timeout, and the head never ends.
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    client.write_all(b"GET / HTTP/1.1\r\nX-Slow: ").unwrap();
    let dripping = std::thread::spawn(move || {
        let mut sent = 0;
        while !stopped.load(Ordering::SeqCst) && client.write_all(b"a").is_ok() {
            sent += 1;
            std::thread::sleep(Duration::from_millis(20));
        }
        sent
    });
    let (done, read) = std::sync::mpsc::channel();
    let reading = std::thread::spawn(move || {
        let _ = done.send(read_request(&mut server, Duration::from_millis(300)).is_err());
        server
    });
    let refused = read.recv_timeout(BOUND);
    // The peer was still sending when the read gave up on it.
    let still_sending = !dripping.is_finished();
    stop.store(true, Ordering::SeqCst);
    assert!(dripping.join().unwrap() > 0);
    assert_eq!(refused, Ok(true), "the head was still being read");
    assert!(still_sending);
    // A head that arrives whole inside the bound is read as before.
    drop(reading.join().unwrap());
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client.write_all(b"GET /a HTTP/1.1\r\n\r\n").unwrap();
    let (mut server, _) = listener.accept().unwrap();
    let request = read_request(&mut server, BOUND).unwrap();
    assert_eq!(
        (request.method.as_str(), request.target.as_str()),
        ("GET", "/a")
    );
}
