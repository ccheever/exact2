//! A request body read from an app file (`Request::body_from`, TypeScript's
//! `exactBodyFrom`; LLP 1108 D6 R2): the worker reads it under the request's
//! `fs.read` grant just before the request goes out, and refuses, sending
//! nothing, when it may not or cannot.
use super::*;

/// Each body that reached the transport, with its headers.
type Sent = Arc<Mutex<Vec<(Vec<u8>, Vec<(String, String)>)>>>;

/// Records each body that reached the transport, with its headers.
struct Recorder(Sent);
impl Transport for Recorder {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        self.0.lock().unwrap().push((
            request.body.clone().unwrap_or_default(),
            request.headers.entries().to_vec(),
        ));
        Ok(ibex2::stdlib::fetch::Response {
            status: 200,
            status_text: "OK".into(),
            headers: Headers::default(),
            body: b"sent".to_vec(),
            url: request.url.clone(),
            redirected: false,
        }
        .into_stream(request.body_limit(), signal.clone()))
    }
}

struct Files(std::path::PathBuf);
impl Files {
    fn new() -> Files {
        let root = std::env::temp_dir().join(format!(
            "exact-body-from-{}-{}",
            std::process::id(),
            ibex2::stdlib::crypto::random_uuid().unwrap()
        ));
        for dir in ["data", "cache", "tmp"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        Files(root)
    }
    fn roots(&self) -> [std::path::PathBuf; 3] {
        ["data", "cache", "tmp"].map(|dir| self.0.join(dir))
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn recording(grants: &str) -> (Core, Sent, Receiver<()>) {
    let sent = Sent::default();
    let owners = (0..WORKERS)
        .map(|_| {
            Some(
                ibex2::host::Host::with_transport(Box::new(Recorder(sent.clone()))).endow(
                    ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants)).unwrap(),
                ),
            )
        })
        .collect();
    let (wake, woke) = channel();
    let core = Core::with_owners(
        owners,
        grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
    );
    (core, sent, woke)
}

fn upload(path: &str) -> Request {
    let mut request = Request::get("https://example.test/upload")
        .header("content-type", "image/jpeg")
        .body_from(path);
    request.method = "POST".into();
    request
}

const GRANTS: &str = "net.fetch https://example.test\nfs.read app:/tmp";

#[test]
fn a_body_from_an_app_file_reaches_the_transport_byte_for_byte() {
    let files = Files::new();
    let bytes: Vec<u8> = (0..300_000u32).map(|i| (i * 7 + i / 255) as u8).collect();
    std::fs::write(files.0.join("tmp/photo.jpg"), &bytes).unwrap();
    let (core, sent, woke) = recording(GRANTS);
    core.set_app_roots(files.roots());
    // Ordered, and on the independent lane.
    core.run(job(1, upload("app:/tmp/photo.jpg")), None)
        .unwrap();
    core.run(
        job(2, upload("app:/tmp/photo.jpg").independent_http(1024)),
        None,
    )
    .unwrap();
    let outcomes = collect(&core, &woke, 2);
    for (_, outcome) in &outcomes {
        assert!(
            matches!(outcome, Outcome::Response(r) if r.body == b"sent"),
            "{outcome:?}"
        );
    }
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 2);
    for (body, headers) in sent.iter() {
        assert!(*body == bytes, "the body differs from the file");
        // The author's Content-Type, and none added.
        let types: Vec<_> = headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .collect();
        assert_eq!(
            types,
            [&("content-type".to_string(), "image/jpeg".to_string())]
        );
    }
}

#[test]
fn a_body_from_a_file_is_refused_unsent_past_its_grant_scope_or_with_another_body() {
    let files = Files::new();
    std::fs::write(files.0.join("tmp/photo.jpg"), b"photo").unwrap();
    std::fs::write(files.0.join("data/note"), b"note").unwrap();
    let (core, sent, woke) = recording(GRANTS);
    core.set_app_roots(files.roots());
    // Outside the app's grants; and a source scope without `fs.read`.
    core.run(job(1, upload("app:/data/note")), None).unwrap();
    let mut scoped = upload("app:/tmp/photo.jpg");
    scoped.grants = Some("net.fetch https://example.test".into());
    core.run(job(2, scoped), None).unwrap();
    core.run(job(3, upload("app:/tmp/../data/note")), None)
        .unwrap();
    for (ticket, outcome) in collect(&core, &woke, 3) {
        match outcome {
            Outcome::Failed { kind, message } => {
                assert_eq!(kind, FailureKind::Refused, "{ticket}: {message}");
                assert!(
                    message.starts_with("exactBodyFrom app:/"),
                    "{ticket}: {message}"
                );
                if ticket < 3 {
                    assert!(
                        message.contains(": denied: fs.read "),
                        "{ticket}: {message}"
                    );
                }
            }
            other => panic!("{ticket}: {other:?}"),
        }
    }
    // Two bodies, or a body on a GET, are refused at admission.
    let mut both = upload("app:/tmp/photo.jpg");
    both.body = b"x".to_vec();
    assert_eq!(
        core.run(job(4, both), None),
        Err("fetch: a request has one body: body or exactBodyFrom")
    );
    assert!(sent.lock().unwrap().is_empty(), "a refused body was sent");
}

/// The app's directories are opened when the host names them and pinned: a
/// root replaced later by a symlink to another tree is not followed, as
/// storage's own handles do not follow it.
#[cfg(unix)]
#[test]
fn a_root_replaced_after_the_host_named_it_is_not_followed() {
    let files = Files::new();
    let other = Files::new();
    std::fs::write(files.0.join("tmp/photo.jpg"), b"the app's").unwrap();
    std::fs::write(other.0.join("tmp/photo.jpg"), b"another's").unwrap();
    let (core, sent, woke) = recording(GRANTS);
    core.set_app_roots(files.roots());
    std::fs::rename(files.0.join("tmp"), files.0.join("tmp-moved")).unwrap();
    std::os::unix::fs::symlink(other.0.join("tmp"), files.0.join("tmp")).unwrap();
    core.run(job(1, upload("app:/tmp/photo.jpg")), None)
        .unwrap();
    let outcomes = collect(&core, &woke, 1);
    assert!(
        matches!(&outcomes[0].1, Outcome::Response(_)),
        "{outcomes:?}"
    );
    assert_eq!(sent.lock().unwrap()[0].0, b"the app's");
}

/// A read that outlasts the request's deadline does not hold it: the worker
/// settles `Timeout` when the deadline passes, tells the read, which stops
/// at its next chunk, and drops its late bytes (Astra, round 3).
#[test]
fn a_stalled_read_is_settled_by_the_deadline_and_told_to_stop() {
    static READERS: super::body::Readers = super::body::Readers::new();
    let passed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = passed.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        flag.store(true, Ordering::Release);
    });
    let started = Instant::now();
    let result = super::body::acquire(
        &READERS,
        |cancel| {
            // A read of many chunks, each slow, that looks between them.
            for _ in 0..300 {
                if cancel.load(Ordering::Acquire) {
                    return Err(failed(FailureKind::Refused, "the request ended"));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(b"late".to_vec())
        },
        &|| {
            passed
                .load(Ordering::Acquire)
                .then(|| failed(FailureKind::Timeout, "the request timed out after 30 ms"))
        },
    );
    assert!(
        matches!(
            &result,
            Err(Outcome::Failed {
                kind: FailureKind::Timeout,
                ..
            })
        ),
        "{result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    // Told, the reader stops at its next chunk and gives its place back.
    let until = Instant::now() + Duration::from_secs(2);
    while READERS.running() > 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(READERS.running(), 0);
    // A read that finishes after its request ended is not taken either.
    let result = super::body::acquire(&READERS, |_| Ok(b"bytes".to_vec()), &|| {
        Some(failed(FailureKind::Aborted, "native request aborted"))
    });
    assert!(matches!(
        result,
        Err(Outcome::Failed {
            kind: FailureKind::Aborted,
            ..
        })
    ));
}

/// Readers stuck on a stalled disk keep their places after their requests
/// were settled; past the bound a new file body is refused, unsent, until
/// one returns (Astra, post-landing).
#[test]
fn stuck_readers_are_bounded_and_keep_their_place_until_they_return() {
    static READERS: super::body::Readers = super::body::Readers::new();
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stuck = |release: Arc<std::sync::atomic::AtomicBool>| {
        // A read blocked in one call, as on a stalled disk: it cannot look.
        move |_: &std::sync::atomic::AtomicBool| {
            while !release.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok(Vec::new())
        }
    };
    let deadline_after_start = |first: &std::cell::Cell<bool>| {
        let fired = !first.get();
        first.set(false);
        fired.then(|| failed(FailureKind::Timeout, "the request timed out after 1 ms"))
    };
    for _ in 0..super::body::MAX_READERS {
        let first = std::cell::Cell::new(true);
        let result = super::body::acquire(&READERS, stuck(release.clone()), &|| {
            deadline_after_start(&first)
        });
        assert!(matches!(
            result,
            Err(Outcome::Failed {
                kind: FailureKind::Timeout,
                ..
            })
        ));
    }
    assert_eq!(READERS.running(), super::body::MAX_READERS);
    let first = std::cell::Cell::new(true);
    let refused = super::body::acquire(&READERS, |_| Ok(b"never".to_vec()), &|| {
        deadline_after_start(&first)
    });
    assert!(
        matches!(&refused, Err(Outcome::Failed { kind: FailureKind::Refused, message })
            if message.contains("file reads are still running")),
        "{refused:?}"
    );
    release.store(true, Ordering::Release);
    let until = Instant::now() + Duration::from_secs(2);
    while READERS.running() > 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(READERS.running(), 0);
    let ok = super::body::acquire(&READERS, |_| Ok(b"bytes".to_vec()), &|| None);
    assert_eq!(ok.unwrap(), b"bytes");
}
