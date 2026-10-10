use super::*;
use ibex2::{
    boundary::HostError,
    stdlib::{
        abort::AbortSignal,
        fetch::{Headers, StreamingResponse, Transport},
    },
};
use std::{
    sync::mpsc::{channel, Receiver, Sender},
    time::{Duration, Instant},
};

#[derive(Default)]
struct Fixture {
    state: Mutex<(usize, bool, Vec<String>)>,
    ready: Condvar,
}
impl Fixture {
    fn wait_held(&self, count: usize) {
        let guard = self.state.lock().unwrap();
        let (state, result) = self
            .ready
            .wait_timeout_while(guard, Duration::from_secs(5), |s| s.0 < count)
            .unwrap();
        assert!(
            !result.timed_out(),
            "only {} of {count} requests reached transport",
            state.0
        );
    }
    fn release(&self) {
        self.state.lock().unwrap().1 = true;
        self.ready.notify_all();
    }
}
struct Fake(Arc<Fixture>);
impl Transport for Fake {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        let state = self.0.clone();
        let _registration = signal.register(move || {
            let _guard = state.state.lock().unwrap();
            state.ready.notify_all();
        });
        let mut state = self.0.state.lock().unwrap();
        state.2.push(request.url.clone());
        if request.url.ends_with("/release") {
            state.1 = true;
            self.0.ready.notify_all();
        }
        if request.url.ends_with("/hold") {
            state.0 += 1;
            self.0.ready.notify_all();
            let (next, timeout) = self
                .0
                .ready
                .wait_timeout_while(state, Duration::from_secs(60), |s| {
                    !s.1 && !signal.aborted()
                })
                .unwrap();
            state = next;
            if timeout.timed_out() {
                return Err(HostError::Failed("test hold timed out".into()));
            }
        }
        drop(state);
        signal.check()?;
        // `/away` redirects to an origin the fixture's grants lack.
        let mut headers = Headers::default();
        let away = request.url.ends_with("/away");
        if away {
            headers.set_response("location", "https://elsewhere.test/feed");
        }
        Ok(ibex2::stdlib::fetch::Response {
            status: if away { 301 } else { 200 },
            status_text: "OK".into(),
            headers,
            body: b"done".to_vec(),
            url: request.url.clone(),
            redirected: false,
        }
        .into_stream(request.body_limit(), signal.clone()))
    }
}
fn setup() -> (Core, Arc<Fixture>, Receiver<()>) {
    let fixture = Arc::new(Fixture::default());
    let grants = "net.fetch https://example.test";
    let owners = (0..WORKERS)
        .map(|_| {
            Some(
                ibex2::host::Host::with_transport(Box::new(Fake(fixture.clone())))
                    .endow(ibex2::grant::GrantSet::parse(grants).unwrap()),
            )
        })
        .collect();
    let (wake, woke) = channel();
    (
        Core::with_owners(
            owners,
            grants,
            Box::new(move || {
                let _ = wake.send(());
            }),
        ),
        fixture,
        woke,
    )
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

#[test]
fn held_independent_http_does_not_block_the_ordered_releaser() {
    let (core, fixture, woke) = setup();
    for ticket in 1..=INDEPENDENT as u64 {
        core.run(
            job(
                ticket,
                Request::get("https://example.test/hold").independent_http(4096),
            ),
            None,
        )
        .unwrap();
    }
    fixture.wait_held(INDEPENDENT);
    core.run(
        job(
            100,
            Request::post_json("https://example.test/release", "{}"),
        ),
        None,
    )
    .unwrap();
    let outcomes = collect(&core, &woke, INDEPENDENT + 1);
    assert!(outcomes
        .iter()
        .all(|(_, o)| matches!(o, Outcome::Response(Response { status: 200, .. }))));
}

#[test]
fn unannotated_http_and_native_writes_remain_in_one_fifo() {
    let (core, fixture, woke) = setup();
    core.run(
        job(
            1,
            Request::post_json("https://example.test/hold", "write one"),
        ),
        None,
    )
    .unwrap();
    fixture.wait_held(1);
    let (writes, written) = channel();
    for ticket in 2..=3 {
        let writes: Sender<_> = writes.clone();
        core.run(
            job(ticket, Request::continuation(ticket)),
            Some(Box::new(move || {
                writes.send(ticket).unwrap();
                Outcome::Storage(vec![ticket as u8])
            })),
        )
        .unwrap();
    }
    assert!(written.try_recv().is_err());
    fixture.release();
    assert_eq!(collect(&core, &woke, 1)[0].0, 1);
    core.run(job(4, Request::get("https://example.test/read")), None)
        .unwrap();
    assert_eq!(
        collect(&core, &woke, 3)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [2, 3, 4]
    );
    assert_eq!(written.try_iter().collect::<Vec<_>>(), [2, 3]);
    assert_eq!(
        fixture.state.lock().unwrap().2,
        ["https://example.test/hold", "https://example.test/read"]
    );
}

/// Independent reads overlap up to the lane's workers, as a browser's six
/// connections a host: a source's `Promise.all` of 4 MiB-ceiling reads (the
/// Bluesky clone's) is not two at a time, and the next waits for a worker.
#[test]
fn independent_reads_overlap_up_to_the_lanes_workers() {
    let (core, fixture, woke) = setup();
    for ticket in 0..=INDEPENDENT as u64 {
        core.run(
            job(
                ticket,
                Request::get("https://example.test/hold").independent_http(4 << 20),
            ),
            None,
        )
        .unwrap();
    }
    fixture.wait_held(INDEPENDENT);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(fixture.state.lock().unwrap().0, INDEPENDENT);
    fixture.release();
    assert_eq!(
        collect(&core, &woke, INDEPENDENT + 1).len(),
        INDEPENDENT + 1
    );
    assert!(settled(&core));
}

#[test]
fn admission_counts_completed_results_until_consumed_and_preserves_control_space() {
    let (core, fixture, woke) = setup();
    for ticket in 0..128 {
        core.run(
            job(
                ticket,
                Request::get("https://example.test/hold").independent_http(4096),
            ),
            None,
        )
        .unwrap();
    }
    fixture.wait_held(2);
    assert!(core
        .run(
            job(
                129,
                Request::get("https://example.test/hold").independent_http(4096)
            ),
            None
        )
        .is_err());
    core.run(
        job(
            130,
            Request::post_json("https://example.test/release", "{}"),
        ),
        None,
    )
    .unwrap();
    // Wait for every result WITHOUT draining it: completed work still owns its reservation.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if core
            .shared
            .state
            .lock()
            .unwrap()
            .completed
            .iter()
            .map(VecDeque::len)
            .sum::<usize>()
            == 129
        {
            break;
        }
        woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        core.begin_pump();
    }
    assert!(core
        .run(
            job(
                131,
                Request::get("https://example.test/read").independent_http(4096)
            ),
            None
        )
        .is_err());
    assert_eq!(core.drain()[0].0, 130); // Control has a separate completion lane too.
    assert!(core
        .run(
            job(
                131,
                Request::get("https://example.test/read").independent_http(4096)
            ),
            None
        )
        .is_err());
    assert_eq!(core.drain().len(), 1);
    core.run(
        job(
            131,
            Request::get("https://example.test/read").independent_http(4096),
        ),
        None,
    )
    .unwrap();
}

#[test]
fn byte_budget_and_illegal_opt_ins_refuse_before_transport() {
    let (core, fixture, _) = setup();
    core.run(
        job(
            1,
            Request::get("https://example.test/hold").independent_http(8 << 20),
        ),
        None,
    )
    .unwrap();
    fixture.wait_held(1);
    for request in [
        Request::continuation(1).independent_http(1),
        Request::storage(vec![]).independent_http(1),
        Request::get("https://example.test/read").independent_http(0),
        Request::get("https://example.test/read").independent_http((64 << 20) + 1),
        // A ceiling the 64 MiB lane can never hold (twice the body, plus).
        Request::get("https://example.test/read").independent_http(32 << 20),
    ] {
        // Each is refused on its own, not held behind the one before it.
        assert_eq!(core.resume_ordered(), []);
        assert!(core.run(job(3, request), None).is_err());
    }
    let mut large = Request::get("https://example.test/read");
    large.body = Vec::with_capacity(MAX_REQUEST + 1);
    assert_eq!(core.resume_ordered(), []);
    assert!(core.run(job(4, large), None).is_err());
    fixture.release();
}

/// The Bluesky port's D5: a second 8 MiB read beside a held one was refused
/// because admission charged the whole ceiling. It now waits for the bytes
/// (16 MiB reads here, so that two outgrow the lane's 64 MiB).
#[test]
fn an_independent_read_over_the_budget_waits_instead_of_refusing() {
    let (core, fixture, woke) = setup();
    core.run(
        job(
            1,
            Request::get("https://example.test/hold").independent_http(16 << 20),
        ),
        None,
    )
    .unwrap();
    fixture.wait_held(1);
    core.run(
        job(
            2,
            Request::get("https://example.test/read").independent_http(16 << 20),
        ),
        None,
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        fixture.state.lock().unwrap().2,
        ["https://example.test/hold"]
    );
    fixture.release();
    let mut done: Vec<u64> = collect(&core, &woke, 2).into_iter().map(|v| v.0).collect();
    done.sort();
    assert_eq!(done, [1, 2]);
    assert!(settled(&core));
}

#[test]
fn retirement_aborts_held_http_discards_queued_effects_and_stops_wakes() {
    let (core, fixture, woke) = setup();
    core.run(job(1, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    struct Dropped(Sender<std::thread::ThreadId>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.send(std::thread::current().id()).unwrap();
        }
    }
    let (dropped, destroyed) = channel();
    let guard = Dropped(dropped);
    let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let check = executed.clone();
    core.run(
        job(2, Request::continuation(2)),
        Some(Box::new(move || {
            let _guard = guard;
            check.store(true, Ordering::SeqCst);
            Outcome::Storage(vec![])
        })),
    )
    .unwrap();
    let state = core.shared.clone();
    let start = Instant::now();
    drop(core);
    // Well before the held request's minute: retirement aborted it.
    assert!(start.elapsed() < Duration::from_secs(30));
    assert!(state.abort.signal().aborted());
    assert_ne!(
        destroyed.recv_timeout(Duration::from_secs(5)).unwrap(),
        std::thread::current().id()
    );
    assert!(!executed.load(Ordering::SeqCst));
    assert!(state
        .state
        .lock()
        .unwrap()
        .jobs
        .iter()
        .all(VecDeque::is_empty));
    assert!(woke.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(state
        .state
        .lock()
        .unwrap()
        .completed
        .iter()
        .all(VecDeque::is_empty));
}

#[test]
fn a_full_new_cohort_recovers_only_after_old_outcomes_are_drained() {
    let (core, fixture, woke) = setup();
    for ticket in 0..128 {
        core.run(
            job(
                ticket,
                Request::get("https://example.test/hold").independent_http(4096),
            ),
            None,
        )
        .unwrap();
    }
    fixture.wait_held(2);
    // Until the host forgets their tickets (`Core::forget`), old work keeps
    // its reservations: the next generation gets explicit refusals.
    for ticket in 128..256 {
        assert!(core
            .run(
                job(
                    ticket,
                    Request::get("https://example.test/read").independent_http(4096)
                ),
                None
            )
            .is_err());
    }
    core.run(
        job(
            1000,
            Request::post_json("https://example.test/release", "{}"),
        ),
        None,
    )
    .unwrap();
    let old = collect(&core, &woke, 129);
    assert_eq!(old.iter().filter(|(ticket, _)| *ticket < 128).count(), 128);
    for ticket in 128..256 {
        core.run(
            job(
                ticket,
                Request::get("https://example.test/read").independent_http(4096),
            ),
            None,
        )
        .unwrap();
    }
    let current = collect(&core, &woke, 128);
    assert!(current
        .iter()
        .all(|(ticket, outcome)| (128..256).contains(ticket)
            && matches!(outcome, Outcome::Response(Response { status: 200, .. }))));
    let state = core.shared.state.lock().unwrap();
    assert_eq!(state.counts, [0, 0]);
    assert_eq!(state.bytes, [0, 0]);
}

#[test]
fn response_ceiling_and_missing_continuations_fail_without_poisoning_the_lane() {
    let (core, fixture, woke) = setup();
    core.run(
        job(
            1,
            Request::get("https://example.test/read").independent_http(3),
        ),
        None,
    )
    .unwrap();
    // A response over its size limit is Refused (`failure(x)`'s `refused`),
    // with the web's words, as a stream's body over its ceiling is.
    assert_eq!(
        collect(&core, &woke, 1)[0].1,
        Outcome::Failed {
            kind: FailureKind::Refused,
            message: "HTTP response exceeds limit".into(),
        }
    );
    core.run(job(2, Request::continuation(1)), None).unwrap();
    assert!(matches!(
        collect(&core, &woke, 1)[0].1,
        Outcome::Failed {
            kind: FailureKind::Unsupported,
            ..
        }
    ));
    let mut denied = Request::get("https://example.test/read").independent_http(4096);
    denied.grants = Some("net.fetch https://outside.test".into());
    core.run(job(3, denied), None).unwrap();
    assert!(matches!(
        collect(&core, &woke, 1)[0].1,
        Outcome::Failed {
            kind: FailureKind::Refused,
            ..
        }
    ));
    assert_eq!(fixture.state.lock().unwrap().2.len(), 1);
    core.run(
        job(4, Request::continuation(2)),
        Some(Box::new(|| Outcome::Storage(vec![42]))),
    )
    .unwrap();
    assert_eq!(collect(&core, &woke, 1)[0].1, Outcome::Storage(vec![42]));

    // Storage retains priority over a continuation even for an async handoff.
    let mut storage = Request::continuation(3);
    storage.storage = Some(vec![]);
    let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = invoked.clone();
    core.run_owned(
        job(5, storage),
        Some(OwnedWork::Later(Box::new(move |_| {
            observed.store(true, Ordering::SeqCst);
        }))),
    )
    .unwrap();
    assert!(matches!(
        collect(&core, &woke, 1)[0].1,
        Outcome::Failed {
            kind: FailureKind::Unsupported,
            ..
        }
    ));
    assert!(!invoked.load(Ordering::SeqCst));
}

#[test]
fn asynchronous_module_reply_keeps_io_available_and_ordered_results_in_order() {
    let (core, _fixture, woke) = setup();
    let (handed, replies) = channel();
    core.run_owned(
        job(10, Request::continuation(1)),
        Some(OwnedWork::Later(Box::new(move |reply| {
            handed.send(reply).unwrap();
        }))),
    )
    .unwrap();
    let reply = replies.recv_timeout(Duration::from_secs(5)).unwrap();
    let (entered, ran) = channel();
    core.run(
        job(11, Request::continuation(2)),
        Some(Box::new(move || {
            entered.send(()).unwrap();
            Outcome::Storage(vec![2])
        })),
    )
    .unwrap();
    ran.recv_timeout(Duration::from_secs(5)).unwrap();
    // Independent I/O can finish while the module still owns its reply.
    core.run(
        job(
            12,
            Request::get("https://example.test/read").independent_http(4096),
        ),
        None,
    )
    .unwrap();
    assert_eq!(collect(&core, &woke, 1)[0].0, 12);
    assert!(
        core.drain().is_empty(),
        "later ordered answer overtook the module"
    );
    assert!(!core.ordered_idle());
    reply.send(Outcome::Storage(vec![1]));
    assert_eq!(
        collect(&core, &woke, 2)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [10, 11]
    );
    assert!(core.ordered_idle());
}

#[test]
fn grants_that_do_not_parse_are_named_in_every_refusal() {
    let (wake, woke) = channel();
    let core = Core::start(
        None,
        "net.fetch https://example.test\nsecret.keep jwtToken",
        Box::new(move || {
            let _ = wake.send(());
        }),
    );
    for (ticket, request) in [
        (1, Request::get("https://example.test/a")),
        (
            2,
            Request::get("https://example.test/b").independent_http(4096),
        ),
    ] {
        core.run(job(ticket, request), None).unwrap();
    }
    for (_, outcome) in collect(&core, &woke, 2) {
        let Outcome::Failed { kind, message } = outcome else {
            panic!("an unbound owner answered: {outcome:?}");
        };
        assert_eq!(kind, FailureKind::Refused);
        assert!(
            message.contains("did not parse") && message.contains("jwtToken"),
            "{message}"
        );
    }
}

/// A redirect that leaves the grants is refused naming the origin it led to
/// (podcast F5: a feed moved to another host read only "outside the app's
/// grants"), and the second hop is never sent.
#[test]
fn a_refused_redirect_names_where_it_led() {
    let (core, fixture, woke) = setup();
    core.run(job(1, Request::get("https://example.test/away")), None)
        .unwrap();
    let [(_, outcome)] = collect(&core, &woke, 1).try_into().unwrap();
    let Outcome::Failed { kind, message } = outcome else {
        panic!("the redirect was followed: {outcome:?}");
    };
    assert_eq!(kind, FailureKind::Refused);
    assert_eq!(
        message,
        "outside the app's grants (net.fetch): redirected to https://elsewhere.test"
    );
    assert_eq!(
        fixture.state.lock().unwrap().2,
        ["https://example.test/away"]
    );
}

fn settled(core: &Core) -> bool {
    let state = core.shared.state.lock().unwrap();
    state.counts == [0, 0]
        && state.bytes == [0, 0]
        && state.running.is_empty()
        && state.fenced.is_empty()
        && state.fenced_bytes == 0
        && state.discard.is_empty()
        && state.light == 0
        && state.agains == 0
        && state.waiting.is_empty()
        && state.waiting_set.is_empty()
}

fn until_settled(core: &Core) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(core) {
        assert!(
            Instant::now() < deadline,
            "forgotten work kept its reservation"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_forgotten_read_is_aborted_and_later_ordered_results_drain() {
    let (core, fixture, woke) = setup();
    core.run(job(1, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    for ticket in 2..=3 {
        core.run(job(ticket, Request::get("https://example.test/read")), None)
            .unwrap();
    }
    // A poll's new arguments superseded ticket 1 (Seth's Crew port, F3).
    core.forget(|ticket| ticket != 1);
    assert_eq!(
        collect(&core, &woke, 2)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    until_settled(&core);
    assert!(core.ordered_idle());
}

#[test]
fn a_forgotten_write_still_runs_but_its_outcome_is_not_drained() {
    let (core, fixture, woke) = setup();
    core.run(job(1, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    core.run(
        job(2, Request::post_json("https://example.test/write", "{}")),
        None,
    )
    .unwrap();
    core.run(job(3, Request::get("https://example.test/read")), None)
        .unwrap();
    core.forget(|ticket| ticket != 2 && ticket != 3);
    fixture.release();
    assert_eq!(collect(&core, &woke, 1)[0].0, 1);
    until_settled(&core);
    assert!(core.drain().is_empty());
    // The queued read was never sent; the write was, in its order.
    assert_eq!(
        fixture.state.lock().unwrap().2,
        ["https://example.test/hold", "https://example.test/write"]
    );
}

#[test]
fn forgotten_independent_reads_release_their_transports_and_admission() {
    let (core, fixture, _) = setup();
    for ticket in 0..128 {
        core.run(
            job(
                ticket,
                Request::get("https://example.test/hold").independent_http(4096),
            ),
            None,
        )
        .unwrap();
    }
    fixture.wait_held(2);
    core.forget(|_| false);
    until_settled(&core);
    core.run(
        job(
            200,
            Request::get("https://example.test/read").independent_http(4096),
        ),
        None,
    )
    .unwrap();
}

#[test]
fn ordered_reads_queue_in_order_and_count_undrained_results() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    for ticket in 1..128 {
        core.run(
            job(
                ticket,
                Request::get(&format!("https://example.test/read/{ticket}")),
            ),
            None,
        )
        .unwrap();
    }
    assert_eq!(
        core.run(job(128, Request::get("https://example.test/read")), None),
        Err("native executor admission limit reached")
    );
    fixture.release();
    let deadline = Instant::now() + Duration::from_secs(5);
    while core.shared.state.lock().unwrap().completed[0].len() < 128 {
        woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        core.begin_pump();
    }
    // Isolate the capacity check from the refusal fence; the Bridge test
    // proves that the host only lifts the fence after refusal settlement.
    core.resume_ordered();
    assert_eq!(
        core.run(job(129, Request::get("https://example.test/read")), None),
        Err("native executor admission limit reached"),
        "completed but undrained reads still count"
    );
    assert_eq!(
        collect(&core, &woke, 128)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        (0..128).collect::<Vec<_>>()
    );
    let mut sent = vec!["https://example.test/hold".to_string()];
    sent.extend((1..128).map(|ticket| format!("https://example.test/read/{ticket}")));
    assert_eq!(fixture.state.lock().unwrap().2, sent);
    assert!(settled(&core));
    core.resume_ordered();
    core.run(job(130, Request::get("https://example.test/read")), None)
        .unwrap();
    assert_eq!(collect(&core, &woke, 1)[0].0, 130);
    assert!(settled(&core));
}

#[test]
fn ordered_waiting_bytes_include_writes_and_leave_room_for_progress() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    // 15 writes can enter before the 16-ticket bound. Six more reads put
    // all waiting buffers just over 63 MiB, although reads alone are 18 MiB.
    for ticket in 1..=21 {
        let mut request = if ticket <= 15 {
            Request::post_json("https://example.test/write", "")
        } else {
            Request::get("https://example.test/read")
        };
        request.body = Vec::with_capacity(3 << 20);
        core.run(job(ticket, request), None).unwrap();
    }
    let mut request = Request::get("https://example.test/refused");
    request.body = Vec::with_capacity(3 << 20);
    assert_eq!(
        core.run(job(22, request), None),
        Err("native ordered queue byte limit reached")
    );
    {
        let state = core.shared.state.lock().unwrap();
        let waiting: usize = state.jobs[0].iter().map(|job| job.charge).sum();
        assert!(waiting > 63 << 20 && waiting <= 64 << 20);
        assert!(state.counts[0] < 128, "bytes refused before count");
        assert!(state.bytes[0] <= BYTES[0]);
    }
    fixture.release();
    assert_eq!(
        collect(&core, &woke, 22)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        (0..22).collect::<Vec<_>>()
    );
    assert_eq!(fixture.state.lock().unwrap().2.len(), 22);
    assert!(settled(&core));
    core.resume_ordered();
    core.run(job(23, Request::get("https://example.test/read")), None)
        .unwrap();
    assert_eq!(collect(&core, &woke, 1)[0].0, 23);
    assert!(settled(&core));
}

#[test]
fn writes_and_opaque_work_keep_the_sixteen_ticket_admission_bound() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    for ticket in 1..16 {
        core.run(job(ticket, Request::get("https://example.test/read")), None)
            .unwrap();
    }
    let mut native = Request::native(vec![]);
    native.http = HttpScheduling::Ordered;
    let mut auth = Request::auth(vec![]);
    auth.http = HttpScheduling::Ordered;
    for request in [
        Request::post_json("https://example.test/refused-write", "{}"),
        Request::continuation(1),
        Request::storage(vec![]),
        Request::capture_surface("surface"),
        native,
        auth,
    ] {
        // Each candidate must hit its own count check, not the previous
        // candidate's ordered refusal fence.
        core.resume_ordered();
        assert_eq!(
            core.run(job(16, request), None),
            Err("native executor admission limit reached")
        );
    }
    let called = Arc::new(AtomicUsize::new(0));
    let work_called = called.clone();
    core.resume_ordered();
    assert_eq!(
        core.run(
            job(16, Request::get("https://example.test/opaque")),
            Some(Box::new(move || {
                work_called.fetch_add(1, Ordering::SeqCst);
                Outcome::Storage(vec![])
            })),
        ),
        Err("native executor admission limit reached")
    );
    fixture.release();
    assert_eq!(collect(&core, &woke, 16).len(), 16);
    assert_eq!(called.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.state.lock().unwrap().2.len(), 16);
    assert!(settled(&core));
}

/// Sixteen writes in flight, a seventeenth refused at the limit: the ordered
/// requests after it wait behind the refusal instead of being refused with it
/// (the Bluesky clone, 2026-10-08: more than sixteen answers at boot, then
/// every later ordered request refused, and the app sat on skeletons).
#[test]
fn ordered_requests_after_a_capacity_refusal_wait_for_it_to_settle() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    for ticket in 1..16 {
        core.run(job(ticket, write(ticket)), None).unwrap();
    }
    assert_eq!(
        core.run(job(16, write(16)), None),
        Err("native executor admission limit reached")
    );
    // Later ordered work is held, not refused, while the refusal is retained.
    for ticket in 17..=18 {
        core.run(job(ticket, write(ticket)), None).unwrap();
    }
    core.run(job(19, Request::get("https://example.test/read")), None)
        .unwrap();
    fixture.release();
    let first = collect(&core, &woke, 16);
    assert_eq!(
        first.into_iter().map(|v| v.0).collect::<Vec<_>>(),
        (0..16).collect::<Vec<_>>()
    );
    assert!(
        core.ordered_idle(),
        "held requests don't hold the refusal back"
    );
    // The refusal settled; the host lifts the fence and the held requests go.
    assert!(core.resume_ordered().is_empty());
    assert_eq!(
        collect(&core, &woke, 3)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [17, 18, 19]
    );
    let sent = fixture.state.lock().unwrap().2.clone();
    assert_eq!(sent.len(), 19, "{sent:?}");
    assert!(!sent.iter().any(|url| url.ends_with("/write/16")));
    assert_eq!(
        &sent[16..],
        [
            "https://example.test/write/17",
            "https://example.test/write/18",
            "https://example.test/read"
        ]
    );
    assert!(settled(&core));
    // The lane is not poisoned: a later request is admitted at once.
    core.run(job(20, write(20)), None).unwrap();
    assert_eq!(collect(&core, &woke, 1)[0].0, 20);
}

/// Held requests past the limit when the fence lifts: the first one over is
/// refused (and named, for the host to record on its ticket), the rest wait
/// behind it again; a request the runner lets go is dropped unsent.
#[test]
fn held_ordered_requests_past_the_limit_refuse_one_and_wait_again() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    for ticket in 1..16 {
        core.run(job(ticket, write(ticket)), None).unwrap();
    }
    assert!(core.run(job(16, write(16)), None).is_err());
    for ticket in 17..=36 {
        core.run(job(ticket, write(ticket)), None).unwrap();
    }
    core.forget(|ticket| ticket != 20);
    fixture.release();
    assert_eq!(collect(&core, &woke, 16).len(), 16);
    // 17..=36 less 20 is 19 held: sixteen admitted, the seventeenth (34) refused.
    assert_eq!(
        core.resume_ordered(),
        [(34, "native executor admission limit reached")]
    );
    let next = collect(&core, &woke, 16);
    let expected: Vec<u64> = (17..=33).filter(|t| *t != 20).collect();
    assert_eq!(next.into_iter().map(|v| v.0).collect::<Vec<_>>(), expected);
    assert!(core.ordered_idle());
    assert_eq!(core.resume_ordered(), []);
    assert_eq!(
        collect(&core, &woke, 2)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [35, 36]
    );
    let sent = fixture.state.lock().unwrap().2.clone();
    assert!(!sent
        .iter()
        .any(|url| url.ends_with("/write/20") || url.ends_with("/write/34")));
    assert!(settled(&core));
}

/// Sixteen in flight from a hold and fifteen writes, the next write refused
/// at the limit: the fence is up over whatever comes next.
fn fenced_core() -> (Core, Arc<Fixture>, Receiver<()>) {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    for ticket in 1..16 {
        core.run(job(ticket, write(ticket)), None).unwrap();
    }
    assert_eq!(
        core.run(job(16, write(16)), None),
        Err("native executor admission limit reached")
    );
    (core, fixture, woke)
}

fn write(n: u64) -> Request {
    Request::post_json(&format!("https://example.test/write/{n}"), "{}")
}

fn tickets(outcomes: Vec<(u64, Outcome)>) -> Vec<u64> {
    outcomes.into_iter().map(|v| v.0).collect()
}

/// A request refused while work is held (here invalid: request buffers over
/// 4 MiB) keeps its place: refused only when the held work before it has
/// been admitted, so that its refusal settles after that work (Astra and
/// Grok, round 1: it had settled first, A, B, D, C).
#[test]
fn a_refusal_behind_held_work_settles_after_it() {
    let (core, fixture, woke) = fenced_core();
    core.run(job(17, write(17)), None).unwrap();
    let mut invalid = write(18);
    invalid.body = Vec::with_capacity(5 << 20);
    core.run(job(18, invalid), None).unwrap();
    core.run(job(19, write(19)), None).unwrap();
    fixture.release();
    assert_eq!(
        tickets(collect(&core, &woke, 16)),
        (0..16).collect::<Vec<_>>()
    );
    // 17 is admitted before 18 is refused; 19 waits behind 18.
    let refused = core.resume_ordered();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].0, 18);
    assert_eq!(tickets(collect(&core, &woke, 1)), [17]);
    assert!(core.ordered_idle());
    assert_eq!(core.resume_ordered(), []);
    assert_eq!(tickets(collect(&core, &woke, 1)), [19]);
    assert!(settled(&core));
}

/// The 129th request behind the fence is refused, in its place: after the
/// 128 held before it.
#[test]
fn the_request_past_the_held_count_is_refused_after_the_held_ones() {
    let (core, fixture, woke) = fenced_core();
    for ticket in 17..=146 {
        let request = Request::get(&format!("https://example.test/read/{ticket}"));
        core.run(job(ticket, request), None).unwrap();
    }
    assert!(core.shared.state.lock().unwrap().fenced_bytes > 0);
    fixture.release();
    assert_eq!(collect(&core, &woke, 16).len(), 16);
    let fence = "earlier ordered admission refusal must settle first";
    assert_eq!(core.resume_ordered(), [(145, fence)]);
    assert_eq!(
        tickets(collect(&core, &woke, 128)),
        (17..=144).collect::<Vec<_>>()
    );
    assert_eq!(core.resume_ordered(), [(146, fence)]);
    assert_eq!(core.resume_ordered(), []);
    assert!(settled(&core));
}

/// Held request buffers are capped at 64 MiB: the one over is refused in its
/// place, and a small one after it is still held.
#[test]
fn the_request_past_the_held_bytes_is_refused_in_its_place() {
    let (core, fixture, woke) = fenced_core();
    let read = |ticket: u64, bytes: usize| {
        let mut request = Request::get(&format!("https://example.test/read/{ticket}"));
        request.body = Vec::with_capacity(bytes);
        job(ticket, request)
    };
    for ticket in 17..=38 {
        core.run(read(ticket, 3 << 20), None).unwrap();
    }
    core.run(read(39, 0), None).unwrap();
    {
        let state = core.shared.state.lock().unwrap();
        assert!(state.fenced_bytes <= ORDERED_WAITING_BYTES);
        assert!(matches!(state.fenced[21], Fenced::Refused(38, _)));
        assert!(matches!(state.fenced[22], Fenced::Held(..)));
    }
    fixture.release();
    assert_eq!(collect(&core, &woke, 16).len(), 16);
    assert_eq!(
        core.resume_ordered(),
        [(38, "earlier ordered admission refusal must settle first")]
    );
    assert_eq!(
        tickets(collect(&core, &woke, 21)),
        (17..=37).collect::<Vec<_>>()
    );
    assert_eq!(core.resume_ordered(), []);
    assert_eq!(tickets(collect(&core, &woke, 1)), [39]);
    assert!(settled(&core));
}

/// The last forgotten ordered job ending wakes the host: the lane is idle,
/// so a retained refusal can settle and the work held behind it go (Astra,
/// round 1: it released its count without a wake).
#[test]
fn the_last_forgotten_ordered_job_ending_wakes_the_host() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    while woke.try_recv().is_ok() {}
    core.begin_pump();
    core.forget(|_| false);
    woke.recv_timeout(Duration::from_secs(5))
        .expect("no wake when the lane went idle");
    until_settled(&core);
    assert!(core.ordered_idle());
}

/// Records the thread its drop runs on.
struct DropsOn(Arc<Mutex<Option<std::thread::ThreadId>>>);
impl Drop for DropsOn {
    fn drop(&mut self) {
        *self.0.lock().unwrap() = Some(std::thread::current().id());
    }
}

fn held_work(core: &Core, ticket: u64) -> Arc<Mutex<Option<std::thread::ThreadId>>> {
    let dropped = Arc::new(Mutex::new(None));
    let guard = DropsOn(dropped.clone());
    core.run(
        job(ticket, Request::continuation(ticket)),
        Some(Box::new(move || {
            let _guard = &guard;
            Outcome::Storage(vec![])
        })),
    )
    .unwrap();
    dropped
}

fn dropped_off_this_thread(dropped: &Mutex<Option<std::thread::ThreadId>>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(on) = *dropped.lock().unwrap() {
            assert_ne!(
                on,
                std::thread::current().id(),
                "destroyed on the host thread"
            );
            return;
        }
        assert!(Instant::now() < deadline, "held work never destroyed");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Held work the runner lets go is destroyed by a worker, unrun, as a
/// queued job's is, not on the host thread that forgets it.
#[test]
fn forgotten_held_work_is_destroyed_by_a_worker_unrun() {
    let (core, fixture, woke) = fenced_core();
    let dropped = held_work(&core, 17);
    core.forget(|ticket| ticket != 17);
    dropped_off_this_thread(&dropped);
    fixture.release();
    assert_eq!(collect(&core, &woke, 16).len(), 16);
    assert_eq!(core.resume_ordered(), []);
    assert!(settled(&core));
}

/// Retiring the executor destroys held work on a retiring worker, while
/// something else (a native module's waker) still keeps its state alive.
#[test]
fn retirement_destroys_held_work_on_a_worker() {
    let (core, _fixture, _woke) = fenced_core();
    let dropped = held_work(&core, 17);
    let waker = core.waker();
    drop(core);
    dropped_off_this_thread(&dropped);
    drop(waker);
}

#[test]
fn forgotten_ordered_backlog_releases_queued_buffers_and_aborts_running_read() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    for ticket in 1..128 {
        let mut request = Request::get("https://example.test/never-sent");
        request.body = Vec::with_capacity(256 << 10);
        core.run(job(ticket, request), None).unwrap();
    }
    core.forget(|_| false);
    until_settled(&core);
    assert!(core.ordered_idle());
    core.run(job(128, Request::get("https://example.test/read")), None)
        .unwrap();
    assert_eq!(collect(&core, &woke, 1)[0].0, 128);
    assert_eq!(
        fixture.state.lock().unwrap().2,
        ["https://example.test/hold", "https://example.test/read"]
    );
    assert!(settled(&core));
}

#[test]
fn an_ordered_job_waits_for_retained_bytes_instead_of_refusing() {
    let (core, fixture, woke) = setup();
    // Four undrained outcomes retain 480 MiB of capacity (never touched).
    for ticket in 1..=4 {
        core.run(
            job(ticket, Request::continuation(ticket)),
            Some(Box::new(|| Outcome::Storage(Vec::with_capacity(120 << 20)))),
        )
        .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while core.shared.state.lock().unwrap().completed[0].len() < 4 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    // Admitted on its request buffers; its 128 MiB ceiling doesn't fit yet.
    core.run(job(5, Request::get("https://example.test/read")), None)
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(fixture.state.lock().unwrap().2.is_empty());
    core.begin_pump();
    assert_eq!(core.drain()[0].0, 1);
    let rest = collect(&core, &woke, 4);
    assert_eq!(
        rest.into_iter().map(|v| v.0).collect::<Vec<_>>(),
        [2, 3, 4, 5]
    );
    assert_eq!(
        fixture.state.lock().unwrap().2,
        ["https://example.test/read"]
    );
    assert!(settled(&core));
}

/// The platform transport, a loopback server and a body over its 64 KiB
/// handoff, behind and beside handed-off module turns as worker placement
/// queues them (the Crew port's F4/F6, not reproduced on macOS).
#[test]
#[ignore = "async lane: a real socket under the platform transport (URLSession), timing-sensitive on a loaded machine; bun scripts/async.mjs runs it"]
fn a_large_body_on_the_platform_transport_drains_behind_handed_off_turns() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let size = if head.contains("/big") { 190_785 } else { 24 };
                let body = format!("\"{}\"", "x".repeat(size - 2));
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
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
    // A handed-off turn that completes a little later, like the owner's.
    let turn = |ticket: u64| {
        let mut request = Request::continuation(ticket);
        request.http = HttpScheduling::Ordered;
        let work: OwnedWork = OwnedWork::Later(Box::new(|reply: Reply| {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                reply.send(Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: b"turn".to_vec(),
                }));
            });
        }));
        (job(ticket, request), work)
    };
    let (r, w) = turn(1);
    core.run_owned(r, Some(w)).unwrap();
    core.run_owned(
        job(2, Request::get(&format!("http://127.0.0.1:{port}/big"))),
        None,
    )
    .unwrap();
    let (r, w) = turn(3);
    core.run_owned(r, Some(w)).unwrap();
    core.run_owned(
        job(4, Request::get(&format!("http://127.0.0.1:{port}/small"))),
        None,
    )
    .unwrap();
    core.run_owned(
        job(5, Request::get(&format!("http://127.0.0.1:{port}/big"))),
        None,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut outcomes = vec![];
    while outcomes.len() < 5 {
        core.begin_pump();
        outcomes.extend(core.drain().into_iter().map(|(t, o, _)| (t, o)));
        if outcomes.len() < 5 {
            woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| panic!("stalled after {outcomes:?}"));
        }
    }
    let sizes: Vec<(u64, usize)> = outcomes
        .iter()
        .map(|(t, o)| match o {
            Outcome::Response(r) => (*t, r.body.len()),
            other => panic!("{t}: {other:?}"),
        })
        .collect();
    assert_eq!(sizes, [(1, 4), (2, 190_785), (3, 4), (4, 24), (5, 190_785)]);
}

/// A long native call is handed to the module on the independent lane: it
/// holds no worker, and ordered storage behind it finishes first.
#[test]
fn a_native_call_is_handed_off_and_never_holds_the_ordered_lane() {
    let (core, _fixture, woke) = setup();
    let mut native = Request::get(exact_runner::NATIVE_URL).independent_http(1 << 20);
    native.method = "POST".into();
    native.body = br#"{"op":"transcribe"}"#.to_vec();
    assert!(native.is_native());
    let (handed, replies) = channel();
    core.run_owned(
        job(20, native),
        Some(OwnedWork::Later(Box::new(move |reply| {
            handed.send(reply).unwrap();
        }))),
    )
    .unwrap();
    let reply = replies.recv_timeout(Duration::from_secs(5)).unwrap();
    core.run(
        job(21, Request::continuation(1)),
        Some(Box::new(|| Outcome::Storage(vec![1]))),
    )
    .unwrap();
    assert_eq!(collect(&core, &woke, 1)[0].0, 21);
    reply.send(Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: b"{}".to_vec(),
    }));
    assert_eq!(collect(&core, &woke, 1)[0].0, 20);
}

#[test]
fn completed_latency_is_measured_before_the_ui_drains_it() {
    let (core, fixture, woke) = setup();
    let before = Instant::now();
    core.run(job(991, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    fixture.release();
    woke.recv_timeout(Duration::from_secs(5)).unwrap();
    let completed_by = before.elapsed().as_millis() as u64;
    // Simulate a UI owner busy after the reply; its wait is not network latency.
    std::thread::sleep(Duration::from_millis(20));
    let (ticket, outcome, elapsed) = core.drain().pop().unwrap();
    assert_eq!(ticket, 991);
    assert!(matches!(outcome, Outcome::Response(_)));
    assert!(elapsed.unwrap() <= completed_by);
}

#[path = "executor_timeout_tests.rs"]
mod timeout;

#[path = "executor_again_tests.rs"]
mod again;

#[path = "executor_body_tests.rs"]
mod body_from;
