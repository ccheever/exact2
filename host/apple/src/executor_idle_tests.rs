//! An idle executor costs nothing: no worker thread before the first
//! request, and no worker woken by the host's per-commit calls while none
//! has anything to do.
use super::*;
use std::sync::mpsc::channel;
use std::time::Duration;

fn core() -> (Core, std::sync::mpsc::Receiver<()>) {
    let (wake, woke) = channel();
    let core = Core::with_owners(
        (0..WORKERS).map(|_| None).collect(),
        "",
        Box::new(move || {
            let _ = wake.send(());
        }),
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
    // Every worker back in its wait.
    std::thread::sleep(Duration::from_millis(100));
    let before = core.shared.wakes.load(Ordering::Relaxed);
    for _ in 0..200 {
        // What a host does after every commit and at every pump.
        core.forget(|_| true);
        core.begin_pump();
        assert!(core.drain().is_empty());
        core.notify();
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        core.shared.wakes.load(Ordering::Relaxed),
        before,
        "idle workers were woken"
    );
}
