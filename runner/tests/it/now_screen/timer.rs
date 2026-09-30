use super::*;

// @ref LLP 1043.000 §3 D8 — the host sees the earliest live deadline,
// including an overdue deadline after bounded catch-up; it drops no commits.
#[test]
fn timer_deadline_tracks_ordered_catch_up_and_absence() {
    let (mut plan, _) = now_screen();
    let action = plan.timers[0].action;
    plan.timers.push(exact_plan::TimersRow {
        interval_ms: 16,
        action,
        once: false,
        frame: false,
    });
    let mut r = Runner::boot(
        plan,
        Schedule::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.timer_due_ms(), Some(16.0));
    let advanced = r.advance_timed(250.0);
    assert_eq!(advanced.receipts.len(), 15);
    assert_eq!(
        advanced
            .receipts
            .iter()
            .map(|r| r.at_ms)
            .collect::<Vec<_>>(),
        (1..=15).map(|n| f64::from(n * 16)).collect::<Vec<_>>()
    );
    assert_eq!(r.timer_due_ms(), Some(256.0));
    // Ticks that send nothing: advancing until a request is one advance.
    let held = r.advance_until_request(500.0);
    assert_eq!((held.receipts.len(), held.now_ms), (16, 500.0));
    assert!(r.journal().any(|l| l.contains("advance → 16 timers fired")));
    let target = 1_000_000.0;
    let advanced = r.advance_timed(target);
    assert_eq!(advanced.receipts.len(), TIMER_FIRE_LIMIT);
    assert!(matches!(
        advanced.error,
        Some(RunnerError::TimerFireLimit { .. })
    ));
    assert!(r.timer_due_ms().unwrap() < target);
    let (mut plan, _) = now_screen();
    plan.timers.clear();
    let r = Runner::boot(
        plan,
        Schedule::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.timer_due_ms(), None);
}

// A one-shot timer (`after(60, arrive)`) fires exactly once at boot+60 and is
// then spent: never due again, and no deadline reported for it, so an idle
// host has nothing to wake for.
#[test]
fn a_one_shot_timer_fires_once_then_owes_no_deadline() {
    let (mut plan, _) = now_screen();
    let action = plan.timers[0].action;
    plan.timers[0] = exact_plan::TimersRow {
        interval_ms: 60,
        action,
        once: true,
        frame: false,
    };
    let mut r = Runner::boot(
        plan,
        Schedule::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.timer_due_ms(), Some(60.0));
    let advanced = r.advance_timed(60.0);
    assert_eq!(advanced.receipts.len(), 1);
    assert_eq!(advanced.receipts[0].at_ms, 60.0);
    assert_eq!(r.timer_due_ms(), None);
    assert!(r.advance_timed(120.0).receipts.is_empty());
    assert!(r.advance_timed(180.0).receipts.is_empty());
    assert_eq!(r.timer_due_ms(), None);
    assert_eq!(r.now_ms(), 180.0);

    // Beside a repeating timer, the spent one is skipped: only the repeat's
    // deadline is reported, and a long seek fires it alone.
    let (mut plan, _) = now_screen();
    plan.timers.push(exact_plan::TimersRow {
        interval_ms: 60,
        action,
        once: true,
        frame: false,
    });
    let mut r = Runner::boot(
        plan,
        Schedule::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.timer_due_ms(), Some(60.0));
    let advanced = r.advance_timed(2_500.0);
    assert_eq!(
        advanced
            .receipts
            .iter()
            .map(|r| r.at_ms)
            .collect::<Vec<_>>(),
        vec![60.0, 1_000.0, 2_000.0]
    );
    assert_eq!(r.timer_due_ms(), Some(3_000.0));
    assert_eq!(r.advance_timed(4_500.0).receipts.len(), 2);
}

// @ref LLP 1073 D2–D4 — a frame task fires once per presented frame, at the
// frame's time and after the timers due by then, and is never caught up; the
// timeout of a host that presents frames fires none; any other advance fires
// each virtual frame, every
// 1000/60 ms after the task last fired, as a timer.
#[test]
fn a_frame_task_fires_once_per_presented_frame_and_each_virtual_frame_on_a_seek() {
    let (mut plan, _) = now_screen();
    let action = plan.timers[0].action;
    let interval = f64::from(plan.timers[0].interval_ms);
    plan.timers.push(exact_plan::TimersRow {
        interval_ms: 0,
        action,
        once: false,
        frame: true,
    });
    let mut r = Runner::boot(
        plan,
        Schedule::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(r.wants_frames());
    // Not presenting frames (tests, the agent): the next virtual frame is a deadline.
    assert_eq!(r.timer_due_ms(), Some(virtual_frame(0.0, 1)));
    // A host that presents frames: its frame source wakes it, not a deadline,
    // and its timeouts fire no frame task.
    r.present_frames(true);
    assert_eq!(r.timer_due_ms(), Some(interval));
    assert!(r.advance_timed(interval / 2.0).receipts.is_empty());
    // A stall: one frame, at its time.
    let at = |a: &exact_runner::Advanced| a.receipts.iter().map(|t| t.at_ms).collect::<Vec<_>>();
    let f = r.frame(interval - 100.0);
    assert_eq!(at(&f), vec![interval - 100.0]);
    // The timer due first, then the frame task, both in the frame.
    let f = r.frame(interval + 16.0);
    assert_eq!(at(&f), vec![interval, interval + 16.0]);
    assert_eq!(r.now_ms(), interval + 16.0);
    // The agent takes the clock: every virtual frame from the last presented
    // one, each at base + k·1000/60.
    r.present_frames(false);
    let seek = r.advance_until_request(interval + 120.0);
    let want = (1..)
        .map(|k| virtual_frame(interval + 16.0, k))
        .take_while(|t| *t <= interval + 120.0)
        .collect::<Vec<_>>();
    assert_eq!(want.len(), 6);
    assert_eq!(at(&seek), want);
    // Sixty frames are exactly a second, and one timer falls in it.
    assert_eq!(
        r.advance(virtual_frame(interval + 16.0, 66)).unwrap().len(),
        61
    );
}
