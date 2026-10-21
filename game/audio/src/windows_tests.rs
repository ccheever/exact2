use super::*;
use std::time::{Duration, Instant};

#[test]
fn coalesced_suspend_resume_requires_worker_acknowledgement() {
    let control = Control::new().unwrap();
    control.ready.store(RUN, Ordering::Release);
    assert!(control.ready());
    control.request(0);
    control.request(RUN);
    assert!(
        !control.ready(),
        "coalesced transitions cannot revive an old stream"
    );
    let revision = control.requested.load(Ordering::Acquire);
    control.ready.store(revision, Ordering::Release);
    assert!(control.ready());
    control.error.store(-1, Ordering::Release);
    assert!(!control.ready());
}

#[test]
fn drop_joins_worker_before_releasing_retained_pcm() {
    let (mut pending, mut mixer) = Pending::new();
    let pcm = Pcm::F32(vec![0.25; 48].into());
    assert!(pending.start(1, &pcm, 48_000, true, 0, 1.0));
    pending.set(1, 1., 1.);
    pending.flush();
    let weak = match &pcm {
        Pcm::F32(p) => Arc::downgrade(p),
        _ => unreachable!(),
    };
    drop(pcm);
    let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_completed = completed.clone();
    let after_join = weak.clone();
    let control = Arc::new(Control::new().unwrap());
    let worker_control = control.clone();
    let worker = thread::spawn(move || {
        mixer.commands();
        while worker_control.requested.load(Ordering::Acquire) & EXIT == 0 {
            thread::yield_now();
        }
        assert_eq!(
            weak.strong_count(),
            1,
            "PCM still owned during worker shutdown"
        );
        for _ in 0..1000 {
            assert!(mixer.frame().0.is_finite());
        }
        worker_completed.store(true, Ordering::Release);
    });
    drop(WindowsOutput {
        control,
        worker: Some(worker),
        pending,
        suspended: false,
    });
    assert!(completed.load(Ordering::Acquire));
    assert_eq!(after_join.strong_count(), 0);
}

#[path = "windows_probe.rs"]
mod probe;
