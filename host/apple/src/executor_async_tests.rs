//! The asynchronous core (LLP 1120 D3–D5) over a scripted transport whose
//! exchanges the test answers by hand: the caps, the per-origin limit, the
//! ordered slot at the terminal transition, the later-of release, a parked
//! upload. The rest of the executor's suite runs on this core too
//! (`the_suite_passes_on_the_asynchronous_core`).
use super::*;
use ibex2::stdlib::fetch::Headers;
use ibex2::stdlib::fetch_async::{Exchange, Head, HopMetrics, HopRequest, Sink};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// One exchange the core started, to answer by hand, with what it sent
/// (an upload's buffer stays alive with it, as the platform's would).
struct Started {
    url: String,
    _request: HopRequest,
    sink: Box<dyn Sink>,
}

#[derive(Default)]
struct Script {
    refuse_next: std::sync::atomic::AtomicBool,
    started: Mutex<Vec<Started>>,
    changed: Condvar,
}

impl Script {
    /// Wait until `n` exchanges have started; how many did.
    fn wait(&self, n: usize) -> usize {
        let started = self.started.lock().unwrap();
        let (started, _) = self
            .changed
            .wait_timeout_while(started, Duration::from_secs(5), |s| s.len() < n)
            .unwrap();
        started.len()
    }

    /// How many have started, after a moment for any that would.
    fn settle(&self) -> usize {
        std::thread::sleep(Duration::from_millis(100));
        self.started.lock().unwrap().len()
    }

    /// Answer the first started exchange to `url` with 200 and `body`.
    fn answer(&self, url: &str, body: &[u8]) {
        let mut exchange = self.take(url);
        exchange.sink.head(Head {
            status: 200,
            headers: Headers::new(),
            declared_length: None,
        });
        exchange.sink.data(body);
        exchange.sink.end(Ok(()));
    }

    fn take(&self, url: &str) -> Started {
        let mut started = self.started.lock().unwrap();
        let at = started
            .iter()
            .position(|s| s.url == url)
            .unwrap_or_else(|| panic!("{url} did not start"));
        started.remove(at)
    }
}

struct Scripted(Arc<Script>);
impl ibex2::stdlib::fetch_async::AsyncTransport for Scripted {
    fn start(
        &self,
        request: HopRequest,
        sink: Box<dyn Sink>,
    ) -> Result<Exchange, ibex2::boundary::HostError> {
        let script = self.0.clone();
        if script.refuse_next.swap(false, Ordering::SeqCst) {
            return Err(ibex2::boundary::HostError::Failed(
                "transport capacity refused".into(),
            ));
        }
        let held = Arc::new(Mutex::new(Some((request, sink))));
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (on, flag) = (held.clone(), cancelled.clone());
        Ok(Exchange::new(
            move || {
                let Some((request, mut sink)) = on.lock().unwrap().take() else {
                    return;
                };
                if flag.load(Ordering::SeqCst) {
                    // Cancelled before activation: it ends, sending nothing,
                    // off this thread.
                    std::thread::spawn(move || {
                        sink.end(Err(ibex2::boundary::HostError::Failed("cancelled".into())))
                    });
                    return;
                }
                let url = request.url.clone();
                script.started.lock().unwrap().push(Started {
                    url,
                    _request: request,
                    sink,
                });
                script.changed.notify_all();
            },
            move || cancelled.store(true, Ordering::SeqCst),
        ))
    }
}

fn core(script: &Arc<Script>, grants: &str) -> (Core, Receiver<()>) {
    core_with_upload_copy(script, grants, false)
}

fn core_with_upload_copy(
    script: &Arc<Script>,
    grants: &str,
    portable: bool,
) -> (Core, Receiver<()>) {
    let owners =
        (0..2)
            .map(|_| {
                Some(ibex2::host::Host::new().endow(
                    ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants)).unwrap(),
                ))
            })
            .collect();
    let (wake, woke) = channel();
    let script = script.clone();
    let lanes: asynch::Lanes = Arc::new(move |_, _| (Arc::new(Scripted(script.clone())), portable));
    let core = Core::with_owners_lanes(
        owners,
        grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
        Arc::new(ibex2::host::Host::new),
        Some(lanes),
    );
    (core, woke)
}

fn job(ticket: u64, request: Request) -> RequestOut {
    RequestOut {
        ticket,
        target: "test".into(),
        request,
        forced: false,
    }
}

fn collect(core: &Core, woke: &Receiver<()>, count: usize) -> Vec<(u64, Outcome)> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut outcomes = vec![];
    while outcomes.len() < count {
        core.begin_pump();
        outcomes.extend(core.drain().into_iter().map(|(t, o, _)| (t, o)));
        if outcomes.len() < count {
            woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
        }
    }
    outcomes
}

fn bytes(core: &Core) -> [usize; 2] {
    core.shared.state.lock().unwrap().bytes
}

/// `until` within five seconds.
fn eventually(mut until: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !until() {
        assert!(Instant::now() < deadline, "not within five seconds");
        std::thread::sleep(Duration::from_millis(5));
    }
}

const ORIGINS: &str = "net.fetch https://*.example.test";

#[test]
fn independent_exchanges_overlap_up_to_the_in_flight_cap() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    let url = |n: u64| format!("https://o{n}.example.test/");
    for n in 0..=asynch::IN_FLIGHT as u64 {
        let request = Request::get(&url(n)).independent_http(1024);
        core.run(job(n, request), None).unwrap();
    }
    assert_eq!(script.wait(asynch::IN_FLIGHT), asynch::IN_FLIGHT);
    assert_eq!(script.settle(), asynch::IN_FLIGHT, "the cap holds the next");
    script.answer(&url(0), b"x");
    let last = url(asynch::IN_FLIGHT as u64);
    eventually(|| script.started.lock().unwrap().iter().any(|s| s.url == last));
    assert_eq!(collect(&core, &woke, 1)[0].0, 0);
}

#[test]
fn a_head_at_its_origins_limit_is_passed_over_until_it_reports_h2() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    for n in 0..7 {
        let request = Request::get(&format!("https://a.example.test/{n}")).independent_http(1024);
        core.run(job(n, request), None).unwrap();
    }
    let other = Request::get("https://b.example.test/").independent_http(1024);
    core.run(job(7, other), None).unwrap();
    // Six to one HTTP/1.1 origin, and the other origin behind them starts.
    assert_eq!(script.wait(7), 7);
    assert_eq!(script.settle(), 7);
    let started: Vec<String> = script
        .started
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.url.clone())
        .collect();
    assert!(started.contains(&"https://b.example.test/".to_string()));
    assert!(!started.contains(&"https://a.example.test/6".to_string()));
    // A report of h2 lifts the limit: the seventh starts.
    let mut first = script.take("https://a.example.test/0");
    first.sink.metrics(HopMetrics {
        protocol: Some("h2".into()),
        ..HopMetrics::default()
    });
    eventually(|| {
        script
            .started
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.url == "https://a.example.test/6")
    });
    assert!(script
        .started
        .lock()
        .unwrap()
        .iter()
        .any(|s| s.url == "https://a.example.test/6"));
    first
        .sink
        .end(Err(ibex2::boundary::HostError::Failed("gone".into())));
    assert_eq!(collect(&core, &woke, 1)[0].0, 0);
}

#[test]
fn a_timeout_frees_the_ordered_lane_while_its_transport_has_not_ended() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    let mut held = Request::get("https://a.example.test/held");
    held.timeout_ms = Some(50);
    core.run(job(1, held), None).unwrap();
    let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = ran.clone();
    core.run(
        job(2, Request::continuation(1)),
        Some(Box::new(move || {
            seen.store(true, Ordering::SeqCst);
            Outcome::Storage(vec![2])
        })),
    )
    .unwrap();
    assert_eq!(script.wait(1), 1);
    let outcomes = collect(&core, &woke, 2);
    assert!(matches!(
        outcomes[0],
        (
            1,
            Outcome::Failed {
                kind: FailureKind::Timeout,
                ..
            }
        )
    ));
    assert_eq!(outcomes[1], (2, Outcome::Storage(vec![2])));
    // Its cancelled exchange still holds its charge until it ends.
    assert!(bytes(&core)[0] > 0);
    script
        .take("https://a.example.test/held")
        .sink
        .end(Err(ibex2::boundary::HostError::Failed("cancelled".into())));
    eventually(|| bytes(&core) == [0, 0]);
}

#[test]
fn a_drained_timeout_keeps_its_bytes_until_its_exchange_ends() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    let mut held = Request::get("https://a.example.test/held").independent_http(4 << 20);
    held.timeout_ms = Some(50);
    core.run(job(1, held), None).unwrap();
    script.wait(1);
    assert!(matches!(
        collect(&core, &woke, 1)[0],
        (
            1,
            Outcome::Failed {
                kind: FailureKind::Timeout,
                ..
            }
        )
    ));
    // Drained, but the cancelled exchange still holds its buffers.
    assert!(bytes(&core)[1] >= 8 << 20);
    script
        .take("https://a.example.test/held")
        .sink
        .end(Err(ibex2::boundary::HostError::Failed("cancelled".into())));
    eventually(|| bytes(&core) == [0, 0]);
}

#[test]
fn an_exchange_that_ends_before_its_outcome_is_drained_keeps_it_until_drained() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    let request = Request::get("https://a.example.test/x").independent_http(4 << 20);
    core.run(job(1, request), None).unwrap();
    script.wait(1);
    script.answer("https://a.example.test/x", b"done");
    eventually(|| core.shared.state.lock().unwrap().running.is_empty());
    assert!(bytes(&core)[1] >= 8 << 20, "the outcome keeps its charge");
    assert_eq!(collect(&core, &woke, 1)[0].0, 1);
    assert_eq!(bytes(&core), [0, 0]);
}

#[test]
fn a_forgotten_running_write_keeps_the_ordered_slot_until_it_ends() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    let mut post = Request::get("https://a.example.test/post");
    post.method = "POST".into();
    core.run(job(1, post), None).unwrap();
    core.run(job(2, Request::get("https://a.example.test/next")), None)
        .unwrap();
    script.wait(1);
    core.forget(|ticket| ticket != 1);
    assert_eq!(script.settle(), 1, "the next waits for the write");
    script.answer("https://a.example.test/post", b"ok");
    script.wait(1);
    script.answer("https://a.example.test/next", b"ok");
    let outcomes = collect(&core, &woke, 1);
    assert_eq!(outcomes[0].0, 2, "the forgotten write's outcome is dropped");
}

#[test]
fn the_suite_passes_on_the_asynchronous_core() {
    // The executor's whole suite, again, on this core: the same binary,
    // asked for those tests with the core switched on.
    if std::env::var_os("EXACT_HTTP_ASYNC").is_some() {
        return;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["executor", "--skip", "async_tests", "-q"])
        .env("EXACT_HTTP_ASYNC", "1")
        .status()
        .unwrap();
    assert!(
        status.success(),
        "the executor's suite failed on the asynchronous core"
    );
}

/// Temporary app directories for a file body.
struct Files(std::path::PathBuf);
impl Files {
    fn new() -> Files {
        let root = std::env::temp_dir().join(format!(
            "exact-async-upload-{}-{}",
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

fn upload(url: &str, path: &str, ceiling: u32) -> Request {
    let mut request = Request::get(url).body_from(path).independent_http(ceiling);
    request.method = "POST".into();
    request
}

/// Round 7's sequence (LLP 1120 §6 question 8, provisional): an upload
/// parked behind a running request is not crowded out by one admitted
/// later, and starts once the bytes it waits for drain.
#[test]
fn a_parked_upload_starts_when_its_bytes_drain_and_is_not_crowded_out() {
    let files = Files::new();
    std::fs::write(files.0.join("tmp/big.bin"), vec![7u8; 20 << 20]).unwrap();
    let script = Arc::new(Script::default());
    let grants = "net.fetch https://*.example.test\nfs.read app:/tmp";
    let (core, woke) = core(&script, grants);
    core.set_app_roots(files.roots());
    // X holds 44 MiB of the 64 while it runs.
    let x = Request::get("https://x.example.test/").independent_http(22 << 20);
    core.run(job(1, x), None).unwrap();
    script.wait(1);
    // A: a 20 MiB file and a 1 MiB ceiling: 22 MiB, which doesn't fit beside X.
    core.run(
        job(
            2,
            upload("https://a.example.test/", "app:/tmp/big.bin", 1 << 20),
        ),
        None,
    )
    .unwrap();
    eventually(|| {
        core.shared
            .state
            .lock()
            .unwrap()
            .book
            .sized
            .contains_key(&2)
    });
    // B, 3 MiB of inline body, would leave too little for A: refused.
    let mut b = Request::get("https://b.example.test/").independent_http(1024);
    b.method = "POST".into();
    b.body = vec![0; 3 << 20];
    assert_eq!(
        core.run(job(3, b), None),
        Err("native executor admission limit reached")
    );
    assert_eq!(script.settle(), 1, "A waits for X");
    script.answer("https://x.example.test/", b"x");
    assert_eq!(collect(&core, &woke, 1)[0].0, 1);
    // X drained: A starts, its file sent whole.
    script.wait(1);
    let mut a = script.take("https://a.example.test/");
    a.sink.head(Head {
        status: 200,
        headers: Headers::new(),
        declared_length: None,
    });
    a.sink.end(Ok(()));
    drop(a);
    assert_eq!(collect(&core, &woke, 1)[0].0, 2);
    eventually(|| bytes(&core) == [0, 0]);
}

/// The same with B queued behind A before A is sized: A could never fit
/// beside it, so it is refused at its park rather than left waiting.
#[test]
fn an_upload_that_cannot_fit_beside_the_requests_behind_it_is_refused() {
    let files = Files::new();
    std::fs::write(files.0.join("tmp/big.bin"), vec![7u8; 60 << 20]).unwrap();
    let script = Arc::new(Script::default());
    let grants = "net.fetch https://*.example.test\nfs.read app:/tmp";
    let (core, woke) = core(&script, grants);
    core.set_app_roots(files.roots());
    // X holds 60 MiB, so W waits at the head for its 4, and A and B queue
    // behind it.
    let x = Request::get("https://x.example.test/").independent_http(30 << 20);
    core.run(job(1, x), None).unwrap();
    script.wait(1);
    let w = Request::get("https://w.example.test/").independent_http(2 << 20);
    core.run(job(2, w), None).unwrap();
    core.run(
        job(
            3,
            upload("https://a.example.test/", "app:/tmp/big.bin", 1 << 20),
        ),
        None,
    )
    .unwrap();
    // B's own ceiling keeps it queued while A is sized.
    let mut b = Request::get("https://b.example.test/").independent_http(30 << 20);
    b.method = "POST".into();
    b.body = vec![0; 3 << 20];
    core.run(job(4, b), None).unwrap();
    // X ends: W starts, then A is sized: 62 MiB with B's 3 behind it.
    script.answer("https://x.example.test/", b"x");
    let outcomes = collect(&core, &woke, 2);
    assert_eq!(outcomes[0].0, 1);
    assert!(
        matches!(&outcomes[1], (3, Outcome::Failed { kind: FailureKind::Refused, message }) if message.contains("queued behind it")),
        "{outcomes:?}"
    );
    script.wait(1);
    script.answer("https://w.example.test/", b"w");
    assert_eq!(collect(&core, &woke, 1)[0].0, 2);
    script.wait(1);
    script.answer("https://b.example.test/", b"b");
    assert_eq!(collect(&core, &woke, 1)[0].0, 4);
    eventually(|| bytes(&core) == [0, 0]);
}

#[test]
fn a_head_short_of_bytes_is_not_passed_over_at_its_origins_limit() {
    let script = Arc::new(Script::default());
    let (core, _woke) = core(&script, ORIGINS);
    // Six to `a`, at its limit, and 48 MiB of the 64 held between them.
    for n in 0..6 {
        let request =
            Request::get(&format!("https://a.example.test/{n}")).independent_http(4 << 20);
        core.run(job(n, request), None).unwrap();
    }
    assert_eq!(script.wait(6), 6);
    // The head, to `a`, needs 16 MiB more than is free; `b` behind it fits.
    let head = Request::get("https://a.example.test/big").independent_http(16 << 20);
    core.run(job(6, head), None).unwrap();
    core.run(
        job(
            7,
            Request::get("https://b.example.test/").independent_http(1024),
        ),
        None,
    )
    .unwrap();
    assert_eq!(script.settle(), 6, "nothing passes a head short of bytes");
}

#[test]
fn a_forgotten_ordered_read_frees_the_lane_before_its_transport_ends() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    core.run(job(1, Request::get("https://a.example.test/held")), None)
        .unwrap();
    core.run(job(2, Request::get("https://a.example.test/next")), None)
        .unwrap();
    script.wait(1);
    core.forget(|ticket| ticket != 1);
    // The next ordered read starts while the first's exchange is still open.
    eventually(|| {
        script
            .started
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.url.ends_with("/next"))
    });
    script.answer("https://a.example.test/next", b"ok");
    assert_eq!(collect(&core, &woke, 1)[0].0, 2);
    script
        .take("https://a.example.test/held")
        .sink
        .end(Err(ibex2::boundary::HostError::Failed("cancelled".into())));
    eventually(|| bytes(&core) == [0, 0]);
}

#[test]
fn an_http_1_1_report_puts_the_origin_back_at_six() {
    let script = Arc::new(Script::default());
    let (core, _woke) = core(&script, ORIGINS);
    let lift = |protocol: &str| {
        let mut s = script.started.lock().unwrap();
        let first = s.iter_mut().next().unwrap();
        first.sink.metrics(HopMetrics {
            protocol: Some(protocol.into()),
            ..HopMetrics::default()
        });
    };
    for n in 0..6 {
        let request = Request::get(&format!("https://a.example.test/{n}")).independent_http(1024);
        core.run(job(n, request), None).unwrap();
    }
    script.wait(6);
    lift("h2");
    let seventh = Request::get("https://a.example.test/6").independent_http(1024);
    core.run(job(6, seventh), None).unwrap();
    assert_eq!(script.wait(7), 7);
    lift("http/1.1");
    eventually(|| core.shared.state.lock().unwrap().book.multiplexed[1].is_empty());
    let eighth = Request::get("https://a.example.test/7").independent_http(1024);
    core.run(job(7, eighth), None).unwrap();
    assert_eq!(script.settle(), 7, "back at six, and over it");
}

#[test]
fn a_redirect_waits_for_an_in_flight_place_and_never_exceeds_the_cap() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    let url = |n: u64| format!("https://o{n}.example.test/");
    for n in 0..asynch::IN_FLIGHT as u64 {
        core.run(job(n, Request::get(&url(n)).independent_http(1024)), None)
            .unwrap();
    }
    script.wait(asynch::IN_FLIGHT);
    // A waiting job behind the full cap, then one exchange redirects.
    core.run(
        job(100, Request::get(&url(100)).independent_http(1024)),
        None,
    )
    .unwrap();
    let mut redirecting = script.take(&url(0));
    let mut headers = Headers::new();
    headers.set_response("location", &url(200));
    redirecting.sink.head(Head {
        status: 302,
        headers,
        declared_length: None,
    });
    redirecting
        .sink
        .end(Err(ibex2::boundary::HostError::Failed("skipped".into())));
    // One place came free: exactly one of the redirect and the job starts.
    eventually(|| script.started.lock().unwrap().len() == asynch::IN_FLIGHT);
    assert_eq!(script.settle(), asynch::IN_FLIGHT, "never over the cap");
    script.answer(&url(1), b"x");
    eventually(|| script.started.lock().unwrap().len() == asynch::IN_FLIGHT);
    assert_eq!(collect(&core, &woke, 1)[0].0, 1);
}

#[test]
fn a_refused_scope_never_runs_its_closure() {
    let script = Arc::new(Script::default());
    let (core, woke) = core(&script, ORIGINS);
    let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = ran.clone();
    let mut request = Request::continuation(1);
    request.grants = Some("net.fetch https://outside.test".into());
    core.run(
        job(1, request),
        Some(Box::new(move || {
            seen.store(true, Ordering::SeqCst);
            Outcome::Storage(vec![])
        })),
    )
    .unwrap();
    assert!(matches!(
        collect(&core, &woke, 1)[0].1,
        Outcome::Failed {
            kind: FailureKind::Refused,
            ..
        }
    ));
    assert!(!ran.load(Ordering::SeqCst));
}

/// An upload read whose first exchange waits for an in-flight place and
/// times out there returns its file's charge: no buffer was ever lent.
#[test]
fn an_upload_that_times_out_before_its_exchange_starts_returns_its_charge() {
    let files = Files::new();
    std::fs::write(files.0.join("tmp/small.bin"), vec![7u8; 1 << 20]).unwrap();
    let script = Arc::new(Script::default());
    let grants = "net.fetch https://*.example.test\nfs.read app:/tmp";
    let (core, woke) = core(&script, grants);
    core.set_app_roots(files.roots());
    let url = |n: u64| format!("https://o{n}.example.test/");
    for n in 0..asynch::IN_FLIGHT as u64 {
        core.run(job(n, Request::get(&url(n)).independent_http(1024)), None)
            .unwrap();
    }
    script.wait(asynch::IN_FLIGHT);
    let before = bytes(&core)[1];
    let mut a = upload("https://a.example.test/", "app:/tmp/small.bin", 1024);
    a.timeout_ms = Some(200);
    core.run(job(100, a), None).unwrap();
    let outcome = collect(&core, &woke, 1);
    assert!(
        matches!(
            outcome[0],
            (
                100,
                Outcome::Failed {
                    kind: FailureKind::Timeout,
                    ..
                }
            )
        ),
        "{outcome:?}"
    );
    eventually(|| bytes(&core)[1] == before);
}

#[test]
fn transport_admission_refusal_releases_the_lane_and_its_accounting() {
    let script = Arc::new(Script::default());
    script.refuse_next.store(true, Ordering::SeqCst);
    let (core, woke) = core(&script, ORIGINS);
    let url = "https://admission.example.test/";
    core.run(job(800, Request::get(url).independent_http(1024)), None)
        .unwrap();
    let refused = collect(&core, &woke, 1);
    assert!(matches!(&refused[0].1, Outcome::Failed { message, .. }
        if message.contains("transport capacity refused")));
    assert!(script.started.lock().unwrap().is_empty());
    eventually(|| bytes(&core) == [0, 0]);
    core.run(job(801, Request::get(url).independent_http(1024)), None)
        .unwrap();
    assert_eq!(script.wait(1), 1);
    script.answer(url, b"accepted");
    assert!(matches!(&collect(&core, &woke, 1)[0].1,
        Outcome::Response(response) if response.body == b"accepted"));
    eventually(|| bytes(&core) == [0, 0]);
}

#[test]
fn portable_upload_reserves_the_blocking_workers_additional_copy() {
    let files = Files::new();
    std::fs::write(files.0.join("tmp/large.bin"), vec![7u8; 40 << 20]).unwrap();
    let grants = "net.fetch https://*.example.test\nfs.read app:/tmp";
    for portable in [false, true] {
        let script = Arc::new(Script::default());
        let (core, woke) = core_with_upload_copy(&script, grants, portable);
        core.set_app_roots(files.roots());
        core.run(
            job(
                1,
                upload("https://a.example.test/", "app:/tmp/large.bin", 1024),
            ),
            None,
        )
        .unwrap();
        if portable {
            let outcomes = collect(&core, &woke, 1);
            assert!(
                matches!(&outcomes[0].1, Outcome::Failed { kind: FailureKind::Refused, message } if message.contains("byte budget")),
                "{outcomes:?}"
            );
            assert_eq!(
                script.settle(),
                0,
                "refused before reading or transport activation"
            );
        } else {
            script.wait(1);
            script.answer("https://a.example.test/", b"ok");
            assert!(matches!(
                &collect(&core, &woke, 1)[0].1,
                Outcome::Response(_)
            ));
        }
        eventually(|| bytes(&core) == [0, 0]);
    }
}
