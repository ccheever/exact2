//! Placed's owner queue and retirements (tests).
use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Slow {
    started: Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
    retired: Arc<Mutex<Vec<String>>>,
}

impl DataSource for Slow {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Number(0.0))
    }

    fn app_id(&self) -> &str {
        "com.exact.test"
    }

    fn grants(&self) -> &str {
        "secret.keep document"
    }

    fn answer(&mut self, _: &mut Store, source: &str, _: &[Value]) -> Result<Answer, DataError> {
        if source == "slow" {
            self.started.send(()).unwrap();
            self.release.recv().unwrap();
        }
        Ok(Answer::Now(Value::Number(0.0)))
    }

    fn forgotten(&mut self, store: &Store, _: &[InFlight<'_>]) {
        self.retired
            .lock()
            .unwrap()
            .push(store.get("document").unwrap_or("").to_string());
    }
}

fn send(dispatch: Dispatch) -> std::sync::mpsc::Receiver<Outcome> {
    let Dispatch::Run(Work::Later(work)) = dispatch else {
        panic!("an owner turn");
    };
    let (tx, rx) = channel();
    work(Reply::new(move |outcome| {
        tx.send(outcome).unwrap();
    }));
    rx
}

/// A source whose `abort` turn aborts the fetch Mutation(0) awaits (its
/// module's ticket 7).
#[derive(Default)]
struct Aborting {
    aborted: Vec<u64>,
}
impl DataSource for Aborting {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Number(0.0))
    }
    fn app_id(&self) -> &str {
        "com.exact.test"
    }
    fn grants(&self) -> &str {
        ""
    }
    fn answer(&mut self, _: &mut Store, source: &str, _: &[Value]) -> Result<Answer, DataError> {
        if source == "abort" {
            self.aborted.push(7);
        }
        Ok(Answer::Now(Value::Number(0.0)))
    }
    fn take_aborted(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.aborted)
    }
    /// A let-go call's `finally` aborts another call's fetch (its 9).
    fn forgotten(&mut self, _: &Store, _: &[InFlight<'_>]) {
        self.aborted.push(9);
    }
}

#[test]
fn an_abort_made_on_the_owner_is_handed_over_even_when_its_turns_reply_is_discarded() {
    let mut placed = Placed::new(Aborting::default(), Placement::Worker);
    placed.activate().unwrap();
    let mut store = Store::new("", []);
    let token = match placed.answer(&mut store, "abort", &[]).unwrap() {
        Answer::Later(request) => request.continuation.unwrap(),
        _ => panic!("a worker answer"),
    };
    // The turn runs on the owner; its reply is never applied, as a
    // superseded answer's is not.
    let (woke, wakes) = channel();
    let woke = Mutex::new(woke);
    placed.abort_wake(Arc::new(move || {
        let _ = woke.lock().unwrap().send(());
    }));
    let reply = send(placed.dispatch(token, &store));
    reply.recv_timeout(Duration::from_secs(5)).unwrap();
    // The host is woken too, as a discarded reply brings it nothing.
    wakes
        .recv_timeout(Duration::from_secs(5))
        .expect("the host was woken");
    assert_eq!(placed.take_aborted(), [7]);
    assert!(placed.take_aborted().is_empty(), "taken once");
}

#[test]
fn an_abort_made_in_a_let_go_on_the_owner_wakes_the_host() {
    let mut placed = Placed::new(Aborting::default(), Placement::Worker);
    placed.activate().unwrap();
    let (woke, wakes) = channel();
    let woke = Mutex::new(woke);
    placed.abort_wake(Arc::new(move || {
        let _ = woke.lock().unwrap().send(());
    }));
    // No reply follows a let-go: only the wake tells the host to collect.
    placed.forgotten(&Store::new("", []), &[]);
    wakes
        .recv_timeout(Duration::from_secs(5))
        .expect("the host was woken");
    assert_eq!(placed.take_aborted(), [9]);
}

#[test]
fn blocked_owner_keeps_only_latest_retirement_before_next_turn() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let retired = Arc::new(Mutex::new(Vec::new()));
    let mut placed = Placed::new(
        Slow {
            started: started_tx,
            release: release_rx,
            retired: retired.clone(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut store = Store::new("secret.keep document", []);
    let slow = match placed.answer(&mut store, "slow", &[]).unwrap() {
        Answer::Later(request) => request.continuation.unwrap(),
        _ => panic!("a worker answer"),
    };
    let first = send(placed.dispatch(slow, &store));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    for i in 0..128 {
        store
            .set("document", &format!("{i}:{}", "x".repeat(256 * 1024)))
            .unwrap();
        placed.forgotten(&store, &[]);
    }
    let queue = placed.owner.as_ref().unwrap().lock().unwrap();
    let slot = queue.pending.as_ref().unwrap().lock().unwrap();
    let snapshot = &slot.as_ref().unwrap().1;
    assert_eq!(snapshot.len(), 1);
    assert!(snapshot[0].1.len() < 257 * 1024);
    drop(slot);
    drop(queue);
    let next = match placed.answer(&mut store, "next", &[]).unwrap() {
        Answer::Later(request) => request.continuation.unwrap(),
        _ => panic!("a worker answer"),
    };
    let second = send(placed.dispatch(next, &store));
    store.set("document", "after-next").unwrap();
    placed.forgotten(&store, &[]);
    let barrier = match placed.answer(&mut store, "barrier", &[]).unwrap() {
        Answer::Later(request) => request.continuation.unwrap(),
        _ => panic!("a worker answer"),
    };
    let third = send(placed.dispatch(barrier, &store));
    release_tx.send(()).unwrap();
    first.recv_timeout(Duration::from_secs(5)).unwrap();
    second.recv_timeout(Duration::from_secs(5)).unwrap();
    third.recv_timeout(Duration::from_secs(5)).unwrap();
    let seen = retired.lock().unwrap();
    assert_eq!(
        seen.len(),
        2,
        "redundant retirement snapshots reached the owner"
    );
    assert!(seen[0].starts_with("127:"));
    assert_eq!(seen[1], "after-next");
}

/// What a worker instance heard was still in flight, by source.
struct Hears {
    started: Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
    views: Arc<Mutex<Vec<Vec<String>>>>,
    answered: Arc<Mutex<Vec<String>>>,
}

impl DataSource for Hears {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Number(0.0))
    }

    fn answer(&mut self, _: &mut Store, source: &str, _: &[Value]) -> Result<Answer, DataError> {
        self.answered.lock().unwrap().push(source.to_string());
        if source == "slow" {
            self.started.send(()).unwrap();
            self.release.recv().unwrap();
        }
        Ok(Answer::Now(Value::Number(0.0)))
    }

    fn forgotten(&mut self, _: &Store, in_flight: &[InFlight<'_>]) {
        let view = in_flight.iter().map(|f| f.source.to_string()).collect();
        self.views.lock().unwrap().push(view);
    }
}

/// A call let go while the owner is busy, its replacement (the same key)
/// dispatched but its turn not yet queued, and a second retirement then:
/// the two retirements coalesce, and the call the first let go must still
/// be let go on the owner (its key is not in flight for it).
#[test]
fn a_coalesced_retirement_still_lets_go_what_an_earlier_one_did() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let views = Arc::new(Mutex::new(Vec::new()));
    let mut placed = Placed::new(
        Hears {
            started: started_tx,
            release: release_rx,
            views: views.clone(),
            answered: Default::default(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut store = Store::new("", []);
    let token = |placed: &mut Placed<Hears>, store: &mut Store, source: &str| {
        let answer = placed.answer_for(Target::Resource(0), store, source, &[Value::Number(1.0)]);
        match answer.unwrap() {
            Answer::Later(request) => request.continuation.unwrap(),
            _ => panic!("a worker answer"),
        }
    };
    let slow = token(&mut placed, &mut store, "slow");
    let first = send(placed.dispatch(slow, &store));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    // A (`read`) is replaced by B (`read`, equal arguments), recorded and
    // not yet dispatched: the first retirement leaves `read` out.
    let replacement = token(&mut placed, &mut store, "read");
    let args = [Value::Number(1.0)];
    let in_flight = [InFlight {
        target: Target::Resource(0),
        source: "read",
        args: &args,
        continuation: Some(replacement),
    }];
    placed.forgotten(&store, &in_flight);
    // B is dispatched; its turn is not queued yet when a second
    // retirement comes, which coalesces with the first.
    let Dispatch::Run(Work::Later(turn)) = placed.dispatch(replacement, &store) else {
        panic!("an owner turn");
    };
    placed.forgotten(&store, &in_flight);
    let (tx, second) = channel();
    turn(Reply::new(move |outcome| tx.send(outcome).unwrap()));
    release_tx.send(()).unwrap();
    first.recv_timeout(Duration::from_secs(5)).unwrap();
    second.recv_timeout(Duration::from_secs(5)).unwrap();
    let views = views.lock().unwrap();
    assert!(
        views
            .first()
            .is_some_and(|view| !view.contains(&"read".to_string())),
        "the owner kept the let-go call's key in flight: {views:?}"
    );
}

/// A key both coalesced retirements keep, each for a different call (the
/// second's view after a newer call for the key was dispatched): the owner
/// still hears it in flight. Its views carry no tokens (`forget` sends
/// none: the proxy's mean nothing there), so a key both keep is kept.
#[test]
fn a_key_both_coalesced_retirements_keep_stays_in_flight() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let views = Arc::new(Mutex::new(Vec::new()));
    let mut placed = Placed::new(
        Hears {
            started: started_tx,
            release: release_rx,
            views: views.clone(),
            answered: Default::default(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut store = Store::new("", []);
    let args = [Value::Number(1.0)];
    let mut token = |placed: &mut Placed<Hears>, source: &str| {
        let answer = placed.answer_for(Target::Resource(0), &mut store, source, &args);
        match answer.unwrap() {
            Answer::Later(request) => request.continuation.unwrap(),
            _ => panic!("a worker answer"),
        }
    };
    let slow = token(&mut placed, "slow");
    let empty = Store::new("", []);
    let first = send(placed.dispatch(slow, &empty));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let view = |continuation| {
        [InFlight {
            target: Target::Resource(0),
            source: "read",
            args: &args,
            continuation: Some(continuation),
        }]
    };
    let mut turns = Vec::new();
    for _ in 0..2 {
        let call = token(&mut placed, "read");
        let Dispatch::Run(Work::Later(turn)) = placed.dispatch(call, &empty) else {
            panic!("an owner turn");
        };
        placed.forgotten(&empty, &view(call));
        turns.push(turn);
    }
    let mut replies = Vec::new();
    for turn in turns {
        let (tx, rx) = channel();
        turn(Reply::new(move |outcome| tx.send(outcome).unwrap()));
        replies.push(rx);
    }
    release_tx.send(()).unwrap();
    first.recv_timeout(Duration::from_secs(5)).unwrap();
    for reply in replies {
        reply.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    let views = views.lock().unwrap();
    assert_eq!(views.first(), Some(&vec!["read".to_string()]), "{views:?}");
}

/// A turn dispatched whose job the host hands over only after a
/// retirement let its call go is never sent: begun after the marker, it
/// would park on the owner for good.
#[test]
fn a_turn_handed_over_after_its_call_was_let_go_never_begins() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let answered = Arc::new(Mutex::new(Vec::new()));
    let mut placed = Placed::new(
        Hears {
            started: started_tx,
            release: release_rx,
            views: Default::default(),
            answered: answered.clone(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut store = Store::new("", []);
    let args = [Value::Number(1.0)];
    let mut token = |placed: &mut Placed<Hears>, source: &str| {
        let answer = placed.answer_for(Target::Resource(0), &mut store, source, &args);
        match answer.unwrap() {
            Answer::Later(request) => request.continuation.unwrap(),
            _ => panic!("a worker answer"),
        }
    };
    let empty = Store::new("", []);
    let slow = token(&mut placed, "slow");
    let first = send(placed.dispatch(slow, &empty));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let read = token(&mut placed, "read");
    let Dispatch::Run(Work::Later(turn)) = placed.dispatch(read, &empty) else {
        panic!("an owner turn");
    };
    // Let go (nothing in flight), then its job is handed over.
    placed.forgotten(&empty, &[]);
    let (tx, reply) = channel();
    turn(Reply::new(move |outcome| tx.send(outcome).unwrap()));
    // Its reply is dropped unsent: `Aborted`, so the ticket ends.
    let outcome = reply.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        matches!(
            outcome,
            Outcome::Failed {
                kind: exact_runner::FailureKind::Aborted,
                ..
            }
        ),
        "{outcome:?}"
    );
    // A later turn, then the owner drains: `read` never began.
    let barrier = token(&mut placed, "barrier");
    let last = send(placed.dispatch(barrier, &empty));
    release_tx.send(()).unwrap();
    first.recv_timeout(Duration::from_secs(5)).unwrap();
    last.recv_timeout(Duration::from_secs(5)).unwrap();
    let answered = answered.lock().unwrap();
    assert_eq!(
        *answered,
        ["slow", "barrier"],
        "the let-go call began on the owner"
    );
}

/// A turn dispatched and not yet handed over survives a retirement that
/// keeps its call: an untargeted one (a tool handler's, which the
/// instance never lets go), and a targeted one whose key is in flight
/// with no token (a wrapper that does not pass the token on). An
/// abandoned one leaves no entry.
#[test]
fn a_turn_not_yet_handed_over_survives_a_retirement_that_keeps_its_call() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let answered = Arc::new(Mutex::new(Vec::new()));
    let mut placed = Placed::new(
        Hears {
            started: started_tx,
            release: release_rx,
            views: Default::default(),
            answered: answered.clone(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut store = Store::new("", []);
    let args = [Value::Number(1.0)];
    let later = |answer: Result<Answer, DataError>| match answer.unwrap() {
        Answer::Later(request) => request.continuation.unwrap(),
        _ => panic!("a worker answer"),
    };
    let empty = Store::new("", []);
    let slow = later(placed.answer(&mut store, "slow", &args));
    let first = send(placed.dispatch(slow, &empty));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let handler = later(placed.answer(&mut store, "handler", &args));
    let read = later(placed.answer_for(Target::Resource(0), &mut store, "read", &args));
    let abandoned = later(placed.answer_for(Target::Resource(1), &mut store, "gone", &args));
    let turn = |dispatch| match dispatch {
        Dispatch::Run(Work::Later(turn)) => turn,
        _ => panic!("an owner turn"),
    };
    let handler_turn = turn(placed.dispatch(handler, &empty));
    let read_turn = turn(placed.dispatch(read, &empty));
    drop(turn(placed.dispatch(abandoned, &empty)));
    let queue = placed.owner.as_ref().unwrap().lock().unwrap();
    assert!(
        !queue.unqueued.contains_key(&abandoned),
        "an abandoned turn's entry stayed"
    );
    drop(queue);
    let in_flight = [InFlight {
        target: Target::Resource(0),
        source: "read",
        args: &args,
        continuation: None,
    }];
    placed.forgotten(&empty, &in_flight);
    let mut replies = Vec::new();
    for turn in [handler_turn, read_turn] {
        let (tx, rx) = channel();
        turn(Reply::new(move |outcome| tx.send(outcome).unwrap()));
        replies.push(rx);
    }
    release_tx.send(()).unwrap();
    first.recv_timeout(Duration::from_secs(5)).unwrap();
    for reply in replies {
        let outcome = reply.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(!matches!(outcome, Outcome::Failed { .. }), "{outcome:?}");
    }
    assert_eq!(*answered.lock().unwrap(), ["slow", "handler", "read"]);
    let queue = placed.owner.as_ref().unwrap().lock().unwrap();
    assert!(
        queue.unqueued.is_empty(),
        "an abandoned turn's entry stayed"
    );
}

/// The coalescing's claim, checked: random retirements and turns, the
/// owner draining now and then, let go of exactly the calls processing
/// every retirement on its own, in order, does.
#[test]
fn coalesced_retirements_let_go_what_retiring_each_in_order_does() {
    let keys: Vec<(Target, String, Vec<Vec<u8>>)> = (0..4)
        .map(|i| (Target::Resource(i), "r".to_string(), Vec::new()))
        .collect();
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move |n: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % n
    };
    // An owner that parks a call for each turn and lets go of the parked
    // calls whose key a retirement leaves out; the calls it let go, by id.
    fn drain(
        jobs: &std::sync::mpsc::Receiver<Job>,
        parked: &mut Vec<(usize, Key)>,
        gone: &mut Vec<usize>,
        ids: &mut usize,
    ) {
        while let Ok(job) = jobs.try_recv() {
            match job {
                Job::Forgotten(slot) => {
                    let Some((view, _)) = slot.lock().unwrap().take() else {
                        continue;
                    };
                    let live: Vec<Key> = view
                        .iter()
                        .map(|(t, s, a)| (Some(*t), s.clone(), a.concat()))
                        .collect();
                    parked.retain(|(id, k)| {
                        let keep = live.contains(k);
                        if !keep {
                            gone.push(*id);
                        }
                        keep
                    });
                }
                Job::Answer {
                    target,
                    source,
                    args,
                    ..
                } => {
                    *ids += 1;
                    parked.push((*ids, (target, source, args.concat())));
                }
                Job::Resume { .. } => {}
            }
        }
    }
    for _ in 0..500 {
        let (coalesce_tx, coalesce_rx) = channel();
        let (order_tx, order_rx) = channel();
        let mut coalesced = OwnerQueue {
            sender: coalesce_tx,
            pending: None,
            unqueued: HashMap::new(),
        };
        let mut ordered = OwnerQueue {
            sender: order_tx,
            pending: None,
            unqueued: HashMap::new(),
        };
        let (mut parked_c, mut gone_c, mut ids_c) = (Vec::new(), Vec::new(), 0);
        let (mut parked_o, mut gone_o, mut ids_o) = (Vec::new(), Vec::new(), 0);
        for _ in 0..30 {
            match next(3) {
                0 => {
                    let view: Vec<_> = keys.iter().filter(|_| next(2) == 0).cloned().collect();
                    coalesced.forgotten((view.clone(), Vec::new()));
                    // In order: each its own marker, never merged.
                    ordered.pending = None;
                    ordered.forgotten((view, Vec::new()));
                }
                1 => {
                    let (target, source, args) = keys[next(4) as usize].clone();
                    let job = |reply| Job::Answer {
                        target: Some(target),
                        source: source.clone(),
                        args: args.clone(),
                        snapshot: Vec::new(),
                        clock: 0.0,
                        reply,
                    };
                    let _ = coalesced.turn(job(Reply::new(|_| {})));
                    let _ = ordered.turn(job(Reply::new(|_| {})));
                }
                _ => {
                    drain(&coalesce_rx, &mut parked_c, &mut gone_c, &mut ids_c);
                    drain(&order_rx, &mut parked_o, &mut gone_o, &mut ids_o);
                }
            }
        }
        drain(&coalesce_rx, &mut parked_c, &mut gone_c, &mut ids_c);
        drain(&order_rx, &mut parked_o, &mut gone_o, &mut ids_o);
        // The same calls let go (one merged marker lets go at once what two did one after the other).
        gone_c.sort_unstable();
        gone_o.sort_unstable();
        assert_eq!(gone_c, gone_o);
        assert_eq!(parked_c, parked_o);
    }
}

/// LLP 1016.000 D8 in a worker: a long poll attached to the worker's own
/// work is refused before that work runs.
struct PollsOtherWork(Arc<Mutex<bool>>);

impl DataSource for PollsOtherWork {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Number(0.0))
    }

    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(
            Request::continuation(1).long_poll("since", "seq"),
        ))
    }

    fn continuation(&mut self, _: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let ran = self.0.clone();
        Some(Box::new(move || {
            *ran.lock().unwrap() = true;
            Outcome::Storage(vec![])
        }))
    }
}

#[test]
fn a_long_poll_on_a_worker_s_own_work_is_refused_before_it_runs() {
    let ran = Arc::new(Mutex::new(false));
    let mut placed = Placed::new(PollsOtherWork(ran.clone()), Placement::Worker);
    placed.activate().unwrap();
    let mut store = Store::new("", []);
    let token = match placed.answer(&mut store, "polled", &[]).unwrap() {
        Answer::Later(request) => request.continuation.unwrap(),
        _ => panic!("a worker answer"),
    };
    let outcome = send(placed.dispatch(token, &store))
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    let refused = crate::envelope::apply(outcome, &mut store, &mut Vec::new()).unwrap_err();
    assert!(
        format!("{refused:?}").contains("exactLongPoll"),
        "{refused:?}"
    );
    assert!(!*ran.lock().unwrap(), "its work never ran");
}
