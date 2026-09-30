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
fn the_cache_budget_counts_each_page_s_variants() {
    let page = |body: usize| {
        CachedPage {
            target: "/p".into(),
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
        page.write(&mut server, false, "default-src 'self'", &policy, false);
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
