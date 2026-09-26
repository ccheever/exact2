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
