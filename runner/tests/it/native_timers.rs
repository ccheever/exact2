//! A host can delegate ordinary live timers without a second coalescing policy.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Event, Runner, WaitKinds};
use std::{cell::Cell, rc::Rc};

fn boot(tasks: &str) -> (Runner<()>, Rc<Cell<f64>>) {
    let source = format!("component App\n  state a = 0\n  state b = 0\n  state enabled = true\n  state key = 0\n  action first\n    a = a + 1\n  action second\n    b = b + 1\n  action toggle\n    enabled = !enabled\n  action rekey\n    key = key + 1\n{tasks}\n  view\n    column\n      button \"toggle\" press=toggle testId=\"toggle\"\n      button \"rekey\" press=rekey testId=\"rekey\"\n");
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        (),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let time = Rc::new(Cell::new(0.0));
    let sample = time.clone();
    r.use_native_timers(true);
    r.hold_waits(WaitKinds::ALL);
    r.set_live_clock(Some(Box::new(move || sample.get())));
    (r, time)
}
fn count(r: &Runner<()>, name: &str) -> f64 {
    match r.slot(name).unwrap() {
        Value::Number(n) => *n,
        v => panic!("{v:?}"),
    }
}
fn press(r: &mut Runner<()>, name: &str) {
    let key = r.kernel().find_first_by_test_id(name).unwrap();
    let id = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
}
#[test]
fn native_callbacks_own_independent_full_tolerances_and_delivery_order() {
    let (mut r, time) = boot("  task x mount\n    after(100, first, tolerance=100)\n  task y mount\n    after(140, second, tolerance=50)");
    let rows = r.native_timers();
    assert_eq!(
        rows.iter()
            .map(|t| (t.due_ms, t.tolerance_ms))
            .collect::<Vec<_>>(),
        [(100.0, 100), (140.0, 50)]
    );
    assert_eq!(r.timer_due_ms(), None);
    assert_eq!(r.timer_tolerance_ms(), 0.0);
    time.set(180.0);
    assert!(r.advance(180.0).unwrap().is_empty());
    assert!(r.frame(180.0).receipts.is_empty());
    press(&mut r, "rekey");
    assert_eq!((count(&r, "a"), count(&r, "b")), (0.0, 0.0));
    assert_eq!(
        r.fire_native_timer(1, rows[1].generation, 180.0)
            .receipts
            .len(),
        1
    );
    assert_eq!((count(&r, "a"), count(&r, "b")), (0.0, 1.0));
    assert_eq!(r.native_timers(), [rows[0].clone()]);
    assert_eq!(
        r.fire_native_timer(0, rows[0].generation, 200.0)
            .receipts
            .len(),
        1
    );
    assert_eq!(count(&r, "a"), 1.0);
    assert!(r.native_timers().is_empty());
    assert!(r
        .fire_native_timer(0, rows[0].generation, 250.0)
        .receipts
        .is_empty());
}
#[test]
fn zero_tolerance_is_also_native_and_repeats_keep_their_phase_and_identity() {
    let (mut r, time) = boot("  task x mount\n    every(100, first)");
    let original = r.native_timers()[0].clone();
    assert_eq!(original.tolerance_ms, 0);
    time.set(340.0);
    r.fire_native_timer(0, original.generation, 340.0);
    let next = &r.native_timers()[0];
    assert_eq!(next.generation, original.generation);
    assert_eq!(next.due_ms, 400.0);
    assert_eq!(count(&r, "a"), 1.0);
    assert!(r
        .fire_native_timer(0, original.generation, 340.0)
        .receipts
        .is_empty());
    time.set(400.0);
    r.fire_native_timer(0, original.generation, 400.0);
    assert_eq!(count(&r, "a"), 2.0);
    assert_eq!(r.native_timers()[0].due_ms, 500.0);
}
#[test]
fn cancellation_and_rearming_at_the_same_instant_reject_the_old_callback() {
    let (mut r, time) = boot("  task x when enabled key=key\n    after(100, first, tolerance=100)");
    let original = r.native_timers()[0].clone();
    press(&mut r, "toggle");
    assert!(r.native_timers().is_empty());
    press(&mut r, "toggle");
    let rearmed = r.native_timers()[0].clone();
    assert_eq!(rearmed.due_ms, original.due_ms);
    assert_ne!(rearmed.generation, original.generation);
    press(&mut r, "rekey");
    let rekeyed = r.native_timers()[0].clone();
    assert_ne!(rekeyed.generation, rearmed.generation);
    time.set(100.0);
    for stale in [original, rearmed] {
        assert!(r
            .fire_native_timer(0, stale.generation, 100.0)
            .receipts
            .is_empty());
    }
    assert_eq!(count(&r, "a"), 0.0);
    r.fire_native_timer(0, rekeyed.generation, 100.0);
    assert_eq!(count(&r, "a"), 1.0);
}
#[test]
fn early_invalid_and_stale_callbacks_do_not_move_the_clock() {
    let (mut r, _) = boot("  task x mount\n    after(100, first)");
    let original = r.native_timers();
    for (index, generation, cutoff) in [(0, 0, 99.0), (1, 0, 200.0), (0, 1, 200.0)] {
        let a = r.fire_native_timer(index, generation, cutoff);
        assert!(a.error.is_none());
        assert!(a.receipts.is_empty());
    }
    for cutoff in [
        f64::NAN,
        f64::INFINITY,
        exact_runner::MAX_CLOCK_MS.next_up(),
    ] {
        assert!(r.fire_native_timer(0, 0, cutoff).error.is_some());
    }
    assert_eq!(r.now_ms(), 0.0);
    assert_eq!(r.native_timers(), original);
}
#[test]
fn takeover_and_seek_bypass_native_delegation_without_losing_overdue_timers() {
    let (mut r, time) = boot("  task x mount\n    every(100, first, tolerance=100)");
    time.set(340.0);
    r.set_live_clock(None);
    assert!(r.native_timers().is_empty());
    assert!(r.fire_native_timer(0, 0, 500.0).receipts.is_empty());
    r.advance(0.0).unwrap();
    assert_eq!(count(&r, "a"), 3.0);
    assert_eq!(r.tasks()[0].1, Some(400.0));
    r.advance(600.0).unwrap();
    assert_eq!(count(&r, "a"), 6.0);
}
#[test]
fn frame_and_admission_timers_remain_on_their_explicit_paths() {
    let (mut r, time) = boot("  task x mount\n    every(frame, first)\n  task y mount\n    after(100, second, wait=frame)");
    r.present_frames(true);
    assert!(r.native_timers().is_empty());
    time.set(100.0);
    r.advance(100.0).unwrap();
    assert_eq!(count(&r, "b"), 0.0);
    r.release_waits(WaitKinds::FRAME, 100.0);
    assert_eq!(count(&r, "b"), 1.0);
    r.frame_tasks(100.0);
    assert_eq!(count(&r, "a"), 1.0);
}

#[test]
fn a_callback_can_cancel_or_rekey_its_own_native_timer() {
    for (action, cancelled) in [("toggle", true), ("rekey", false)] {
        let (mut r, time) = boot(&format!(
            "  task x when enabled key=key\n    every(100, {action})"
        ));
        let original = r.native_timers()[0].clone();
        time.set(100.0);
        assert!(r
            .fire_native_timer(0, original.generation, 100.0)
            .error
            .is_none());
        if cancelled {
            assert!(r.native_timers().is_empty());
        } else {
            let replacement = &r.native_timers()[0];
            assert_ne!(replacement.generation, original.generation);
            assert_eq!(replacement.due_ms, 200.0);
        }
        assert!(r
            .fire_native_timer(0, original.generation, 200.0)
            .receipts
            .is_empty());
    }
}
#[test]
fn a_refused_native_callback_rolls_back_writes_but_spends_the_firing() {
    let (mut r, time) = boot("  task x when true key=1 / (1 - a)\n    every(100, first)");
    let original = r.native_timers()[0].clone();
    time.set(100.0);
    assert!(r
        .fire_native_timer(0, original.generation, 100.0)
        .error
        .is_some());
    assert_eq!(count(&r, "a"), 0.0);
    let next = &r.native_timers()[0];
    assert_eq!(next.generation, original.generation);
    assert_eq!(next.due_ms, 200.0);
    assert!(r
        .journal()
        .any(|line| line.contains("native timer") && line.contains("refused")));
}
