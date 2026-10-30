use super::*;

#[test]
fn surface_scope_must_be_admitted_and_name_the_operation() {
    let admitted = "surface.read world\nsurface.write world\nsurface.read map";
    assert!(Request::capture_surface("world")
        .check_surface_grant(admitted)
        .is_ok());
    assert!(Request::restore_surface("world", vec![])
        .check_surface_grant(admitted)
        .is_ok());
    let mut narrowed = Request::capture_surface("world");
    narrowed.grants = Some("surface.read map".into());
    assert!(narrowed.check_surface_grant(admitted).is_err());
    narrowed.grants = Some("surface.read world\nsurface.write other".into());
    assert!(narrowed.check_surface_grant(admitted).is_err());
    let mut mixed = Request::capture_surface("world");
    mixed.storage = Some(Vec::new());
    assert!(mixed
        .check_surface_grant(admitted)
        .unwrap_err()
        .contains("multiple host-work kinds"));
    assert_eq!(
        io_grants(
            "fs.read app:/data\nsurface.read world\ndevice.microphone p\nruntime.allow-jit Helpers/node/bin/node\nsurface.write world\nauth.session https://x.test\nauth.callback a.b:/c"
        ),
        "fs.read app:/data"
    );
}

#[test]
fn a_body_from_a_file_is_http_with_no_other_body() {
    let post = |path: &str| Request::post_json("https://x.test", "").body_from(path);
    assert_eq!(post("app:/tmp/a.jpg").body_from_refusal(), None);
    assert_eq!(Request::get("https://x.test").body_from_refusal(), None);
    assert!(post("/tmp/a.jpg")
        .body_from_refusal()
        .unwrap()
        .contains("app:/"));
    let mut both = post("app:/tmp/a.jpg");
    both.body = b"x".to_vec();
    assert!(both.body_from_refusal().unwrap().contains("one body"));
    for method in ["GET", "head"] {
        let mut read = post("app:/tmp/a.jpg");
        read.method = method.into();
        assert!(read.body_from_refusal().unwrap().contains("GET or HEAD"));
    }
    let socket = Answer::stream(post("app:/tmp/a.jpg"));
    let Answer::Later(mut socket) = socket else {
        unreachable!()
    };
    socket.url = "WSS://x.test/feed".into();
    assert!(socket.body_from_refusal().unwrap().contains("WebSocket"));
    let native = Request::native(Vec::new()).body_from("app:/tmp/a.jpg");
    assert_eq!(
        native.body_from_refusal(),
        Some("only HTTP takes exactBodyFrom")
    );
    let mut storage = Request::storage(Vec::new()).body_from("app:/tmp/a.jpg");
    storage.method = "POST".into();
    assert_eq!(
        storage.body_from_refusal(),
        Some("only HTTP takes exactBodyFrom")
    );
}

#[test]
fn a_body_to_a_file_is_an_http_reply_written_by_the_host() {
    let get = |path: &str| Request::get("https://x.test").body_to(path);
    assert_eq!(get("app:/cache/a.jpg").body_to_refusal(), None);
    let mut post = Request::post_json("https://x.test", "{}").body_to("app:/cache/a.jpg");
    assert_eq!(post.body_to_refusal(), None);
    post.body_from = Some("app:/tmp/b".into());
    post.body.clear();
    assert_eq!(post.body_to_refusal(), None);
    for (path, why) in [
        ("/cache/a.jpg", "app:/"),
        ("app:/cache/../a", ". or .."),
        ("app:/cache/./a", ". or .."),
        ("app:/cache/a\0", ". or .."),
    ] {
        assert!(get(path).body_to_refusal().unwrap().contains(why), "{path}");
    }
    let long = format!("app:/cache/{}", "a".repeat(4096));
    assert!(get(&long).body_to_refusal().unwrap().contains("4096"));
    let mut head = get("app:/cache/a");
    head.method = "HEAD".into();
    assert!(head.body_to_refusal().unwrap().contains("HEAD"));
    let Answer::Later(stream) = Answer::stream(get("app:/cache/a")) else {
        unreachable!()
    };
    assert!(stream.body_to_refusal().unwrap().contains("stream"));
    assert_eq!(
        Request::native(Vec::new())
            .body_to("app:/cache/a")
            .body_to_refusal(),
        Some("only HTTP takes exactBodyTo")
    );
    // The ceiling: up to 1 GiB for a body written to a file, 64 MiB otherwise.
    let big = get("app:/cache/a").independent_http(MAX_BODY_TO_BYTES);
    assert_eq!(big.ceiling_refusal(), None);
    assert!(get("app:/cache/a")
        .independent_http(MAX_BODY_TO_BYTES + 1)
        .ceiling_refusal()
        .is_some());
    assert!(Request::get("https://x.test")
        .independent_http((64 << 20) + 1)
        .ceiling_refusal()
        .is_some());
    let written = Response {
        status: 200,
        headers: vec![(BODY_WRITTEN.into(), "17".into())],
        body: Vec::new(),
    };
    assert_eq!(written.body_written(), Some(17));
}

/// LLP 1146 D2: what may change its server runs to its end once its
/// answer is let go; what cannot (a safe method), or what is Exact's own
/// work, a stream or a held request, does not.
#[test]
fn a_request_has_effects_only_when_its_method_is_unsafe_over_http() {
    /// Whether a POST so changed has effects.
    fn with(change: impl FnOnce(&mut Request)) -> bool {
        let mut r = Request::post_json("https://example.test/x", "{}");
        change(&mut r);
        r.has_effects()
    }
    assert!(with(|_| {}));
    assert!(with(
        |r| (r.method, r.url) = ("delete".into(), "HTTP://example.test/x".into())
    ));
    for safe in ["GET", "HEAD", "OPTIONS", "TRACE"] {
        assert!(!with(|r| r.method = safe.into()), "{safe}");
    }
    assert!(!with(|r| r.url = crate::auth::AUTH_URL.into()));
    assert!(!with(|r| r.url = NATIVE_URL.into()));
    assert!(!with(|r| r.stream = true));
    assert!(!with(|r| r.held = true));
    assert!(!with(|r| r.continuation = Some(1)));
    assert!(!with(|r| r.socket = true));
    assert!(!with(|r| r.storage = Some(Vec::new())));
    assert!(!with(|r| r.surface = Request::capture_surface("x").surface));
    assert!(!Request::continuation(7).has_effects());
}
