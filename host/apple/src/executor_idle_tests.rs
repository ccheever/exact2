//! An idle executor costs nothing: no worker thread before the first
//! request, and no worker notified by the host's per-commit calls while none
//! has anything to do.
use super::*;
use std::sync::mpsc::channel;
use std::time::Duration;

fn core() -> (Core, std::sync::mpsc::Receiver<()>) {
    let (wake, woke) = channel();
    // Exercise the pool even when EXACT_HTTP_ASYNC selects the async core.
    let core = Core::with_owners_on(
        (0..WORKERS).map(|_| None).collect(),
        "",
        Box::new(move || {
            let _ = wake.send(());
        }),
        Arc::new(ibex2::host::Host::new),
    );
    (core, woke)
}

#[test]
fn workers_start_with_the_first_request() {
    let (core, woke) = core();
    assert!(core.idle.lock().unwrap().is_some(), "started at boot");
    // The host's per-commit and per-pump calls start nothing.
    core.forget(|_| true);
    core.begin_pump();
    assert!(core.drain().is_empty());
    assert!(core.idle.lock().unwrap().is_some());
    core.run(
        RequestOut {
            ticket: 1,
            target: "test".into(),
            request: Request::continuation(1),
            forced: false,
        },
        Some(Box::new(|| Outcome::Storage(vec![7]))),
    )
    .unwrap();
    assert!(core.idle.lock().unwrap().is_none());
    woke.recv_timeout(Duration::from_secs(5)).unwrap();
    core.begin_pump();
    let done = core.drain();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].0, 1);
}

#[test]
fn an_idle_pool_is_not_woken() {
    let (core, woke) = core();
    core.run(
        RequestOut {
            ticket: 1,
            target: "test".into(),
            request: Request::continuation(1),
            forced: false,
        },
        Some(Box::new(|| Outcome::Storage(vec![]))),
    )
    .unwrap();
    woke.recv_timeout(Duration::from_secs(5)).unwrap();
    core.begin_pump();
    assert_eq!(core.drain().len(), 1);
    // Count notification calls: wait returns can be delayed or spurious.
    let before = core.shared.worker_notifications.load(Ordering::Relaxed);
    assert!(before > 0, "the first request must notify workers");
    for _ in 0..200 {
        // What a host does after every commit and at every pump.
        core.forget(|_| true);
        core.begin_pump();
        assert!(core.drain().is_empty());
        core.notify();
    }
    assert_eq!(
        core.shared.worker_notifications.load(Ordering::Relaxed),
        before,
        "idle workers were notified"
    );
}
