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
        Err("exactBodyFrom: a request has one body, body or exactBodyFrom")
    );
    assert!(sent.lock().unwrap().is_empty(), "a refused body was sent");
}
