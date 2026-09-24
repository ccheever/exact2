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
                .wait_timeout_while(state, Duration::from_secs(5), |s| !s.1 && !signal.aborted())
                .unwrap();
            state = next;
            if timeout.timed_out() {
                return Err(HostError::Failed("test hold timed out".into()));
            }
        }
        drop(state);
        signal.check()?;
        Ok(ibex2::stdlib::fetch::Response {
            status: 200,
            status_text: "OK".into(),
            headers: Headers::default(),
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
        outcomes.extend(core.drain());
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
    for ticket in 1..=2 {
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
    core.run(
        job(3, Request::post_json("https://example.test/release", "{}")),
        None,
    )
    .unwrap();
    let outcomes = collect(&core, &woke, 3);
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
    assert!(core
        .run(
            job(
                2,
                Request::get("https://example.test/read").independent_http(8 << 20)
            ),
            None
        )
        .is_err());
    for request in [
        Request::continuation(1).independent_http(1),
        Request::storage(vec![]).independent_http(1),
        Request::get("https://example.test/read").independent_http(0),
        Request::get("https://example.test/read").independent_http((64 << 20) + 1),
    ] {
        assert!(core.run(job(3, request), None).is_err());
    }
    let mut large = Request::get("https://example.test/read");
    large.body = Vec::with_capacity(MAX_REQUEST + 1);
    assert!(core.run(job(4, large), None).is_err());
    fixture.release();
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
    assert!(start.elapsed() < Duration::from_secs(1));
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
    assert!(matches!(
        collect(&core, &woke, 1)[0].1,
        Outcome::Failed {
            kind: FailureKind::Network,
            ..
        }
    ));
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

fn settled(core: &Core) -> bool {
    let state = core.shared.state.lock().unwrap();
    state.counts == [0, 0] && state.bytes == [0, 0] && state.running.is_empty()
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
fn ordered_admission_charges_request_buffers_until_a_job_runs() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    // Six asks at boot were Crew's; the count is the limit now, not bytes.
    for ticket in 1..16 {
        core.run(job(ticket, Request::get("https://example.test/read")), None)
            .unwrap();
    }
    assert!(core
        .run(job(16, Request::get("https://example.test/read")), None)
        .is_err());
    fixture.release();
    assert_eq!(
        collect(&core, &woke, 16)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        (0..16).collect::<Vec<_>>()
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
