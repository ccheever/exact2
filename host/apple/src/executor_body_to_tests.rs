//! A reply's body written to an app file (`Request::body_to`, TypeScript's
//! `exactBodyTo`; LLP 1108 D6 R2, 2026-10-26): the worker stages the file
//! under `fs.write` before sending, writes a 2xx body as it arrives, and
//! replaces the target only when the body is complete.
use super::*;
use exact_runner::Answer;
use ibex2::stdlib::fetch::{Body, BodySource};

/// The requests that reached the transport, by URL.
type Opened = Arc<Mutex<Vec<String>>>;

/// A deterministic byte at `offset` of a served body.
fn byte(offset: usize) -> u8 {
    (offset.wrapping_mul(31) ^ (offset >> 9)) as u8
}

/// A body of `size` bytes made as it is read, at most 7919 bytes a read
/// (chunked); `stall` makes every read after the first 256 KiB slow.
struct Made {
    at: usize,
    size: usize,
    stall: bool,
}
impl BodySource for Made {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, HostError> {
        if self.stall && self.at >= 256 << 10 {
            std::thread::sleep(Duration::from_millis(20));
            out[0] = 0;
            return Ok(1);
        }
        let n = out.len().min(7919).min(self.size - self.at);
        for (i, b) in out[..n].iter_mut().enumerate() {
            *b = byte(self.at + i);
        }
        self.at += n;
        Ok(n)
    }
}

/// Serves `/bytes/<n>` as 200 with n made bytes, `/stall` as 200 with a body
/// that never ends, and anything else as 404 with a short body.
struct Server(Opened);
impl Transport for Server {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        self.0.lock().unwrap().push(request.url.clone());
        let path = request.url.trim_start_matches("https://example.test");
        let (status, source): (u16, Box<dyn BodySource>) =
            match path.strip_prefix("/bytes/").map(str::parse) {
                Some(Ok(size)) => (
                    200,
                    Box::new(Made {
                        at: 0,
                        size,
                        stall: false,
                    }),
                ),
                _ if path == "/stall" => (
                    200,
                    Box::new(Made {
                        at: 0,
                        size: usize::MAX,
                        stall: true,
                    }),
                ),
                _ => (
                    404,
                    Box::new(std::io::Cursor::new(b"no such thing".to_vec())),
                ),
            };
        let mut headers = Headers::default();
        headers.set_response("content-type", "application/octet-stream");
        Ok(StreamingResponse {
            status,
            status_text: String::new(),
            headers,
            body: Body::new(source, request.body_limit(), signal.clone()),
            url: request.url.clone(),
            redirected: false,
        })
    }
}

struct Files(std::path::PathBuf);
impl Files {
    fn new() -> Files {
        let root = std::env::temp_dir().join(format!(
            "exact-body-to-{}-{}",
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
    /// The names in `app:/cache`: no staging file is left behind.
    fn cache(&self) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(self.0.join("cache"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const GRANTS: &str = "net.fetch https://example.test\nfs.write app:/cache";

/// A core on workers, whatever `EXACT_HTTP_ASYNC` says (the asynchronous
/// core refuses a body to a file: below), its transports [`Server`]s.
fn serving(grants: &str) -> (Core, Opened, Receiver<()>) {
    serving_on(grants, false)
}

fn serving_on(grants: &str, switched: bool) -> (Core, Opened, Receiver<()>) {
    let opened = Opened::default();
    let owners = (0..WORKERS)
        .map(|_| {
            Some(
                ibex2::host::Host::with_transport(Box::new(Server(opened.clone()))).endow(
                    ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants)).unwrap(),
                ),
            )
        })
        .collect();
    let (wake, woke) = channel();
    let wake: Wake = Box::new(move || {
        let _ = wake.send(());
    });
    let core = if switched {
        Core::with_owners(owners, grants, wake)
    } else {
        Core::with_owners_on(owners, grants, wake, Arc::new(ibex2::host::Host::new))
    };
    (core, opened, woke)
}

fn download(path: &str, to: &str) -> Request {
    Request::get(&format!("https://example.test{path}")).body_to(to)
}

#[test]
fn a_2xx_body_is_written_whole_to_the_file_and_the_response_names_its_size() {
    let files = Files::new();
    std::fs::write(files.0.join("cache/big"), b"old").unwrap();
    let (core, _opened, woke) = serving(GRANTS);
    core.set_app_roots(files.roots());
    // Over 16 MiB on the ordered lane (no ceiling named: 1 GiB), and a
    // smaller one on the independent lane, a chunk at a time.
    let big = (20 << 20) + 3;
    core.run(
        job(1, download(&format!("/bytes/{big}"), "app:/cache/big")),
        None,
    )
    .unwrap();
    core.run(
        job(
            2,
            download("/bytes/100000", "app:/cache/small").independent_http(100_000),
        ),
        None,
    )
    .unwrap();
    for (ticket, outcome) in collect(&core, &woke, 2) {
        let Outcome::Response(r) = outcome else {
            panic!("{ticket}: {outcome:?}")
        };
        assert_eq!(r.status, 200);
        assert!(r.body.is_empty(), "{ticket}: the body entered the response");
        let (name, size) = if ticket == 1 {
            ("big", big)
        } else {
            ("small", 100_000)
        };
        assert_eq!(r.body_written(), Some(size as u64));
        let written = std::fs::read(files.0.join("cache").join(name)).unwrap();
        assert_eq!(written.len(), size);
        assert!(
            written.iter().enumerate().all(|(i, b)| *b == byte(i)),
            "{name} differs"
        );
    }
    assert_eq!(files.cache(), ["big", "small"]);
}

#[test]
fn a_reply_that_is_not_2xx_is_the_response_and_leaves_the_file() {
    let files = Files::new();
    std::fs::write(files.0.join("cache/a"), b"old").unwrap();
    let (core, _opened, woke) = serving(GRANTS);
    core.set_app_roots(files.roots());
    core.run(job(1, download("/missing", "app:/cache/a")), None)
        .unwrap();
    let outcomes = collect(&core, &woke, 1);
    let Outcome::Response(r) = &outcomes[0].1 else {
        panic!("{outcomes:?}")
    };
    assert_eq!(
        (r.status, &r.body[..], r.body_written()),
        (404, &b"no such thing"[..], None)
    );
    assert_eq!(std::fs::read(files.0.join("cache/a")).unwrap(), b"old");
    assert_eq!(files.cache(), ["a"]);
}

#[test]
fn a_body_to_a_file_is_refused_unsent_without_its_grant_parent_or_app_files() {
    let files = Files::new();
    let (core, opened, woke) = serving(GRANTS);
    core.set_app_roots(files.roots());
    // No `fs.write` over app:/data; a source scope without it; no parent.
    core.run(job(1, download("/bytes/10", "app:/data/a")), None)
        .unwrap();
    let mut scoped = download("/bytes/10", "app:/cache/a");
    scoped.grants = Some("net.fetch https://example.test".into());
    core.run(job(2, scoped), None).unwrap();
    core.run(job(3, download("/bytes/10", "app:/cache/none/a")), None)
        .unwrap();
    for (ticket, outcome) in collect(&core, &woke, 3) {
        let Outcome::Failed { kind, message } = outcome else {
            panic!("{ticket}: {outcome:?}")
        };
        assert_eq!(kind, FailureKind::Refused, "{ticket}: {message}");
        assert!(
            message.starts_with("exactBodyTo app:/"),
            "{ticket}: {message}"
        );
        if ticket < 3 {
            assert!(
                message.contains(": denied: fs.write "),
                "{ticket}: {message}"
            );
        }
    }
    // A host that names no app files.
    let (bare, _, bare_woke) = serving(GRANTS);
    bare.run(job(4, download("/bytes/10", "app:/cache/a")), None)
        .unwrap();
    let outcomes = collect(&bare, &bare_woke, 1);
    assert!(
        matches!(
            &outcomes[0].1,
            Outcome::Failed {
                kind: FailureKind::Unsupported,
                ..
            }
        ),
        "{outcomes:?}"
    );
    // A path with a `..` segment, and a stream, are refused at admission.
    assert!(core
        .run(job(5, download("/bytes/10", "app:/cache/../data/a")), None)
        .unwrap_err()
        .contains(". or .."));
    let Answer::Later(stream) = Answer::stream(download("/bytes/10", "app:/cache/a")) else {
        unreachable!()
    };
    assert!(core
        .run(job(6, stream), None)
        .unwrap_err()
        .contains("stream"));
    assert!(
        opened.lock().unwrap().is_empty(),
        "a refused request was sent"
    );
    assert!(files.cache().is_empty(), "a refused request made a file");
}

#[test]
fn a_body_over_its_ceiling_leaves_the_target() {
    let files = Files::new();
    std::fs::write(files.0.join("cache/a"), b"old").unwrap();
    let (core, _opened, woke) = serving(GRANTS);
    core.set_app_roots(files.roots());
    core.run(
        job(
            1,
            download("/bytes/2000000", "app:/cache/a").independent_http(1 << 20),
        ),
        None,
    )
    .unwrap();
    let outcomes = collect(&core, &woke, 1);
    assert!(
        matches!(&outcomes[0].1, Outcome::Failed { kind: FailureKind::Refused, message }
            if message == "HTTP response exceeds limit"),
        "{outcomes:?}"
    );
    assert_eq!(std::fs::read(files.0.join("cache/a")).unwrap(), b"old");
    assert_eq!(files.cache(), ["a"], "a staging file was left");
}

#[test]
#[ignore = "async lane: waits on the wall clock (a 300 ms deadline ends a body mid-way); bun scripts/async.mjs runs it"]
fn a_body_ended_mid_way_leaves_the_target() {
    let files = Files::new();
    std::fs::write(files.0.join("cache/a"), b"old").unwrap();
    let (core, _opened, woke) = serving(GRANTS);
    core.set_app_roots(files.roots());
    core.run(
        job(1, download("/stall", "app:/cache/a").timeout(300)),
        None,
    )
    .unwrap();
    let outcomes = collect(&core, &woke, 1);
    assert!(
        matches!(
            &outcomes[0].1,
            Outcome::Failed {
                kind: FailureKind::Timeout,
                ..
            }
        ),
        "{outcomes:?}"
    );
    assert_eq!(std::fs::read(files.0.join("cache/a")).unwrap(), b"old");
    assert_eq!(files.cache(), ["a"], "a staging file was left");
}

/// The lane is charged what the request can hold in memory, not the 1 GiB it
/// may write, so a large download fits beside open sockets
/// (issues/20261026-independent-http-refuses-ceilings-over-32-mib-on-apple.md).
#[test]
fn a_body_to_a_file_is_charged_a_bounded_allowance() {
    let request = download("/bytes/1", "app:/cache/a").independent_http(1 << 30);
    let (lane, _, limit) = reservation(&request).unwrap();
    assert_eq!(lane, 1);
    assert!(limit < 3 << 20, "charged {limit}");
    let plain = Request::get("https://example.test/bytes/1").independent_http(1 << 30);
    assert!(reservation(&plain).is_err());
}

/// The asynchronous core (LLP 1120; `EXACT_HTTP_ASYNC`, off) does not write a
/// body to a file yet: it refuses one, sending nothing, never reading it into
/// memory instead. Run in the executor suite's rerun on that core.
#[test]
fn the_asynchronous_core_refuses_a_body_to_a_file() {
    if !asynch::asynchronous() {
        return;
    }
    let files = Files::new();
    let (core, opened, woke) = serving_on(GRANTS, true);
    core.set_app_roots(files.roots());
    core.run(job(1, download("/bytes/10", "app:/cache/a")), None)
        .unwrap();
    let outcomes = collect(&core, &woke, 1);
    assert!(
        matches!(
            &outcomes[0].1,
            Outcome::Failed {
                kind: FailureKind::Unsupported,
                ..
            }
        ),
        "{outcomes:?}"
    );
    assert!(opened.lock().unwrap().is_empty());
    assert!(files.cache().is_empty());
}
