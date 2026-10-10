//! A re-ask (`Dispatch::Again`) and the sixteen effects (LLP 1041 §8.4,
//! amended 2026-10-09): a re-ask is never worker work and never counts
//! against the sixteen; it takes its place in the ordered sequence complete,
//! charged one place in the markers' own window of 128; with that full it
//! waits pending, oldest first, never refused. Finished reads leave the sixteen;
//! finished effects stay in it until drained. Completion is driven by the
//! fixture's hold and release, never by sleeping.
use super::*;

fn again(core: &Core, ticket: u64) {
    core.again(ticket)
        .expect("a re-ask is never refused for room");
}

fn state(core: &Core) -> std::sync::MutexGuard<'_, State> {
    core.shared.state.lock().unwrap()
}

/// Wait, pumping wakes but draining nothing, until `n` ordered outcomes are
/// complete and undrained.
fn until_complete(core: &Core, woke: &Receiver<()>, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while state(core).completed[0].len() < n {
        core.begin_pump();
        woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("only {} of {n} completed", state(core).completed[0].len()));
    }
}

/// Queued and running jobs.
fn work_in_flight(core: &Core) -> (usize, usize) {
    let state = state(core);
    (state.jobs[0].len(), state.running.len())
}

fn effect(core: &Core, ticket: u64, request: Request) -> Result<(), &'static str> {
    // Each candidate meets its own count check, not an earlier refusal's fence.
    core.resume_ordered();
    core.run(job(ticket, request), None)
}

/// POST, GET, re-ask, POST: they settle in the order they entered; the
/// re-ask makes no transport call and adds no job or running work.
#[test]
fn a_re_ask_takes_no_worker_and_settles_in_its_place() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    core.run(job(1, write(1)), None).unwrap();
    let before = work_in_flight(&core);
    again(&core, 2);
    assert_eq!(work_in_flight(&core), before);
    core.run(job(3, write(3)), None).unwrap();
    assert!(
        !core.ordered_idle(),
        "an undrained re-ask keeps the lane busy"
    );
    fixture.release();
    let outcomes = collect(&core, &woke, 4);
    assert_eq!(tickets(outcomes.clone()), [0, 1, 2, 3]);
    assert!(matches!(
        &outcomes[2].1,
        Outcome::Response(Response { status: 200, body, .. }) if body.is_empty()
    ));
    assert_eq!(
        fixture.state.lock().unwrap().2,
        [
            "https://example.test/hold",
            "https://example.test/write/1",
            "https://example.test/write/3"
        ]
    );
    assert!(settled(&core));
}

/// Sixteen ordered requests in flight (a held GET, fifteen queued GETs):
/// a seventeenth GET and a re-ask wait; a POST, a storage step, opaque work,
/// an ordered native call, a handed-off turn and auth are each refused.
#[test]
fn past_sixteen_reads_and_re_asks_wait_and_effects_are_refused() {
    let (core, fixture, woke) = setup();
    core.run(job(0, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    for ticket in 1..16 {
        core.run(job(ticket, Request::get("https://example.test/read")), None)
            .unwrap();
    }
    core.run(job(16, Request::get("https://example.test/read")), None)
        .unwrap();
    let mut head = Request::get("https://example.test/read");
    head.method = "HEAD".into();
    core.run(job(17, head), None).unwrap();
    again(&core, 18);
    let mut native = Request::native(vec![]);
    native.http = HttpScheduling::Ordered;
    let mut auth = Request::auth(vec![]);
    auth.http = HttpScheduling::Ordered;
    let limit = Err("native executor admission limit reached");
    for request in [write(19), Request::storage(vec![]), native, auth] {
        assert_eq!(effect(&core, 19, request), limit);
    }
    core.resume_ordered();
    let opaque = core.run(
        job(19, Request::continuation(19)),
        Some(Box::new(|| Outcome::Storage(vec![]))),
    );
    assert_eq!(opaque, limit);
    core.resume_ordered();
    let handed: Option<OwnedWork> = Some(OwnedWork::Later(Box::new(|reply: Reply| {
        reply.send(Outcome::Storage(vec![]))
    })));
    assert_eq!(
        core.run_owned(job(19, Request::continuation(19)), handed),
        limit
    );
    core.resume_ordered();
    fixture.release();
    assert_eq!(
        tickets(collect(&core, &woke, 19)),
        (0..19).collect::<Vec<_>>()
    );
    assert!(settled(&core));
}

/// Sixteen finished, undrained POSTs still hold the sixteen: the next is
/// refused until one drains. Sixteen finished, undrained GETs do not: a POST
/// is admitted beside them, and they keep their bytes until drained.
#[test]
fn finished_effects_hold_the_sixteen_until_drained_and_finished_reads_do_not() {
    let (core, _fixture, woke) = setup();
    for ticket in 0..16 {
        core.run(job(ticket, write(ticket)), None).unwrap();
    }
    until_complete(&core, &woke, 16);
    assert_eq!(state(&core).light, 0);
    assert_eq!(
        core.run(job(16, write(16)), None),
        Err("native executor admission limit reached")
    );
    core.resume_ordered();
    assert_eq!(core.drain()[0].0, 0);
    core.run(job(17, write(17)), None).unwrap();
    assert_eq!(
        tickets(collect(&core, &woke, 16)),
        (1..16).chain([17]).collect::<Vec<_>>()
    );
    assert!(settled(&core));

    let (core, _fixture, woke) = setup();
    for ticket in 0..16 {
        core.run(job(ticket, Request::get("https://example.test/read")), None)
            .unwrap();
    }
    until_complete(&core, &woke, 16);
    assert_eq!(state(&core).light, 16);
    let retained = state(&core).bytes[0];
    assert!(retained >= 16 * "done".len());
    core.run(job(16, write(16)), None).unwrap();
    assert_eq!(
        tickets(collect(&core, &woke, 17)),
        (0..17).collect::<Vec<_>>()
    );
    assert!(settled(&core));
}

/// 128 finished, undrained GETs fill the window: the next GET is refused.
#[test]
fn a_full_window_of_finished_reads_refuses_the_next_read() {
    let (core, _fixture, woke) = setup();
    for ticket in 0..128 {
        core.run(job(ticket, Request::get("https://example.test/read")), None)
            .unwrap();
    }
    until_complete(&core, &woke, 128);
    assert_eq!(
        core.run(job(128, Request::get("https://example.test/read")), None),
        Err("native executor admission limit reached")
    );
    core.resume_ordered();
    assert_eq!(tickets(collect(&core, &woke, 128)).len(), 128);
    assert!(settled(&core));
}

/// More eligible re-asks than their window holds, and no I/O at all: each
/// waits pending, one record per ticket, and is placed oldest first as each
/// drain frees room. None is refused; the markers never pass 128, and they
/// never take the room real work needs (the shared load's next fetch).
#[test]
fn re_asks_past_the_window_wait_and_settle_in_order_with_no_delivery() {
    let (core, _fixture, woke) = setup();
    for ticket in 0..300 {
        again(&core, ticket);
    }
    // One record per ticket: asking again for one already placed or waiting
    // adds nothing.
    again(&core, 5);
    again(&core, 200);
    assert_eq!(state(&core).counts[0], 128);
    assert_eq!(state(&core).waiting.len(), 172);
    // A full markers' window takes none of the real tickets' room: a write
    // and a read are admitted, and enter the sequence ahead of the re-asks
    // still waiting.
    core.run(job(1000, write(1000)), None).unwrap();
    core.run(job(1001, Request::get("https://example.test/read")), None)
        .unwrap();
    let mut order = Vec::new();
    while order.len() < 302 {
        assert!(state(&core).agains <= 128);
        let drained = core.drain();
        if drained.is_empty() {
            woke.recv_timeout(Duration::from_secs(5))
                .expect("a wake for room");
            core.begin_pump();
            continue;
        }
        order.extend(drained.into_iter().map(|v| v.0));
    }
    let expected: Vec<u64> = (0..128).chain([1000, 1001]).chain(128..300).collect();
    assert_eq!(order, expected);
    assert!(settled(&core));
}

/// The last room freed by a forget, with no delivery: the waiting re-ask is
/// placed and wakes the host.
#[test]
fn room_a_forget_frees_places_a_waiting_re_ask() {
    let (core, _fixture, woke) = setup();
    for ticket in 0..129 {
        again(&core, ticket);
    }
    assert_eq!(state(&core).waiting.len(), 1);
    while woke.try_recv().is_ok() {}
    core.begin_pump();
    core.forget(|ticket| ticket != 0);
    assert!(state(&core).waiting.is_empty());
    assert_eq!(*state(&core).ordered.back().unwrap(), 128);
    woke.recv_timeout(Duration::from_secs(5)).expect("a wake");
    assert_eq!(
        tickets(collect(&core, &woke, 128)),
        (1..129).collect::<Vec<_>>()
    );
    assert!(settled(&core));
}

/// Forgetting a re-ask in each state it can be in — complete in the window,
/// waiting behind a refusal, waiting for room — drops it unsettled and
/// returns its ticket and bytes.
#[test]
fn a_forgotten_re_ask_is_dropped_in_every_state() {
    let (core, fixture, woke) = fenced_core();
    again(&core, 17); // behind the refusal: waiting
    assert_eq!(state(&core).waiting, [17]);
    fixture.release();
    assert_eq!(
        tickets(collect(&core, &woke, 16)),
        (0..16).collect::<Vec<_>>()
    );
    core.forget(|ticket| ticket != 17);
    assert!(state(&core).waiting.is_empty());
    assert!(core.resume_ordered().is_empty());
    assert!(
        core.drain().is_empty(),
        "the forgotten re-ask never settles"
    );
    // Waiting for room, then forgotten.
    for ticket in 100..229 {
        again(&core, ticket);
    }
    assert_eq!(state(&core).waiting, [228]);
    core.forget(|ticket| ticket != 228);
    assert!(state(&core).waiting.is_empty());
    // Complete in the window, then forgotten.
    assert_eq!(state(&core).light, 128);
    core.forget(|_| false);
    assert!(settled(&core));
    assert!(core.drain().is_empty());
}

/// Re-asks made while a refusal is retained wait (never in the fence) and
/// enter the sequence when it lifts, after the requests held before them:
/// none settles before the refusal, and the fence keeps its 128 for real
/// requests.
#[test]
fn re_asks_behind_a_refusal_wait_and_enter_after_it() {
    let (core, fixture, woke) = fenced_core();
    again(&core, 17);
    core.run(job(18, write(18)), None).unwrap();
    for ticket in 100..226 {
        core.run(job(ticket, Request::get("https://example.test/read")), None)
            .unwrap();
    }
    again(&core, 300);
    assert_eq!(state(&core).waiting, [17, 300]);
    assert_eq!(state(&core).fenced.len(), 127);
    fixture.release();
    assert_eq!(
        tickets(collect(&core, &woke, 16)),
        (0..16).collect::<Vec<_>>()
    );
    assert!(
        core.ordered_idle(),
        "the refusal may settle now, and only now"
    );
    // The host settles ticket 16's refusal, then lifts the fence.
    assert!(core.resume_ordered().is_empty());
    let expected: Vec<u64> = [18].into_iter().chain(100..226).chain([17, 300]).collect();
    assert_eq!(tickets(collect(&core, &woke, expected.len())), expected);
    let sent = fixture.state.lock().unwrap().2.clone();
    assert_eq!(sent[16], "https://example.test/write/18");
    assert!(settled(&core));
}

/// A wave of re-asks while a refusal is retained, more than the fence's
/// 128, then a real read: the read is held behind the refusal, not refused,
/// and runs when it lifts (Grok, code review round 1: re-asks had filled
/// the fence and the read was refused for good with read capacity free).
#[test]
fn re_asks_behind_a_refusal_leave_the_fence_to_real_requests() {
    let (core, fixture, woke) = fenced_core();
    for ticket in 100..300 {
        again(&core, ticket);
    }
    core.run(job(17, Request::get("https://example.test/read")), None)
        .unwrap();
    assert!(
        state(&core)
            .fenced
            .iter()
            .all(|f| matches!(f, Fenced::Held(..))),
        "the read is held, not refused in place"
    );
    fixture.release();
    assert_eq!(
        tickets(collect(&core, &woke, 16)),
        (0..16).collect::<Vec<_>>()
    );
    assert!(core.resume_ordered().is_empty());
    let expected: Vec<u64> = [17].into_iter().chain(100..300).collect();
    assert_eq!(tickets(collect(&core, &woke, expected.len())), expected);
    assert!(fixture
        .state
        .lock()
        .unwrap()
        .2
        .ends_with(&["https://example.test/read".to_string()]));
    assert!(settled(&core));
}

/// More re-asks than any cap the first build had (128 markers and 1024
/// waiting records), then an effect: none is refused, the effect is admitted
/// and runs, and everything settles in entry order (Astra and Grok, code
/// review round 1: the 1,153rd had been refused for good, outside the fence).
#[test]
fn thousands_of_re_asks_and_a_following_effect_all_settle() {
    let (core, fixture, woke) = setup();
    for ticket in 0..2000 {
        again(&core, ticket);
    }
    assert_eq!(state(&core).waiting.len(), 2000 - 128);
    core.run(job(5000, write(5000)), None).unwrap();
    let expected: Vec<u64> = (0..128).chain([5000]).chain(128..2000).collect();
    assert_eq!(tickets(collect(&core, &woke, expected.len())), expected);
    assert_eq!(
        fixture.state.lock().unwrap().2,
        ["https://example.test/write/5000"]
    );
    assert!(settled(&core));
}

/// The fence lifted while the markers' window is full and a refusal held
/// before re-asks A and C (Apple and Linux lift it when the runner lets the
/// refusal go, drained or not): the refusal is returned, the held write
/// after it is held again, and A and C keep their order and their single
/// records (Astra, code review round 1: A had gone after C, and the
/// waiting records could overfill).
#[test]
fn lifting_the_fence_with_the_window_full_keeps_re_asks_in_order() {
    let (core, _fixture, woke) = setup();
    for ticket in 0..128 {
        again(&core, ticket);
    }
    let invalid = |n: u64| {
        let mut request = write(n);
        request.body = Vec::with_capacity(5 << 20);
        request
    };
    assert!(core.run(job(500, invalid(500)), None).is_err());
    again(&core, 600);
    core.run(job(501, invalid(501)), None).unwrap();
    again(&core, 602);
    core.run(job(503, write(503)), None).unwrap();
    assert_eq!(state(&core).waiting, [600, 602]);
    // The runner let refusal 500 go: the host lifts the fence at once.
    let refused = core.resume_ordered();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].0, 501);
    assert_eq!(state(&core).waiting, [600, 602]);
    assert_eq!(state(&core).waiting_set.len(), 2);
    assert_eq!(
        tickets(collect(&core, &woke, 128)),
        (0..128).collect::<Vec<_>>()
    );
    // Refusal 501 settled; the fence lifts again.
    assert!(core.resume_ordered().is_empty());
    assert_eq!(tickets(collect(&core, &woke, 3)), [503, 600, 602]);
    assert!(settled(&core));
}

/// A module's background round is a storage continuation, an effect: beside
/// twenty undrained re-asks it is admitted; behind sixteen outstanding
/// writes it is refused (and settles through its own completion on a host,
/// as built in 2eca0ad80).
#[test]
fn a_background_round_beside_re_asks_is_admitted_and_behind_writes_refused() {
    let background = || Request::continuation(exact_runner::BACKGROUND);
    let (core, _fixture, woke) = setup();
    for ticket in 0..20 {
        again(&core, ticket);
    }
    core.run(
        job(20, background()),
        Some(Box::new(|| Outcome::Storage(vec![]))),
    )
    .unwrap();
    assert_eq!(
        tickets(collect(&core, &woke, 21)),
        (0..21).collect::<Vec<_>>()
    );
    let (core, _fixture, _woke) = fenced_core();
    core.resume_ordered();
    assert_eq!(
        core.run(
            job(30, background()),
            Some(Box::new(|| Outcome::Storage(vec![])))
        ),
        Err("native executor admission limit reached")
    );
}

/// Retirement drops complete and waiting re-asks with the other outcomes,
/// and refuses a re-ask made after it.
#[test]
fn retirement_drops_re_asks_and_refuses_later_ones() {
    let (core, _fixture, _woke) = setup();
    for ticket in 0..130 {
        again(&core, ticket);
    }
    let shared = core.shared.clone();
    drop(core);
    let state = shared.state.lock().unwrap();
    assert!(state.waiting.is_empty());
    assert!(state.completed.iter().all(VecDeque::is_empty));
    drop(state);
    let (core, _fixture, _woke) = setup();
    core.shared.state.lock().unwrap().retired = true;
    assert_eq!(core.again(1), Err("native executor retired"));
}
