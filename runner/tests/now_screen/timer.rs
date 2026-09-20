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
