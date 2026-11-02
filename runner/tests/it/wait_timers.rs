use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{frame_at_or_after, DataError, DataSource, Runner, WaitKinds};
use std::{cell::Cell, rc::Rc};

struct Empty;
impl DataSource for Empty {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
fn boot(tasks: &str) -> Runner<Empty> {
    let source = format!("component App\n  state count = 0\n  state seen = 0\n  state enabled = true\n  action tick\n    count = count + 1\n    seen = performanceNow()\n  action disable\n    enabled = false\n{tasks}\n  view\n    text \"timer\"\n");
    boot_source(&source)
}
fn boot_source(source: &str) -> Runner<Empty> {
    Runner::boot(
        contract::compile(source).unwrap(),
        Empty,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn number(r: &Runner<Empty>, name: &str) -> f64 {
    match r.slot(name) {
        Some(Value::Number(n)) => *n,
        other => panic!("{name}: {other:?}"),
    }
}
fn live(r: &mut Runner<Empty>) -> Rc<Cell<f64>> {
    let time = Rc::new(Cell::new(0.0));
    let sample = time.clone();
    r.hold_waits(WaitKinds::ALL);
    r.set_live_clock(Some(Box::new(move || sample.get())));
    time
}
#[test]
fn measured_firings_record_nominal_admission_and_commit_times() {
    let mut r = boot("  task t mount\n    after(10, tick, wait=update)");
    let time = live(&mut r);
    r.measure(true, false);
    time.set(25.0);
    r.advance_timed(25.0);
    assert_eq!(number(&r, "count"), 0.0);
    assert!(!r.journal().any(|line| line.contains("timer row=")));
    time.set(40.0);
    r.release_waits(WaitKinds::UPDATE, 25.0);
    assert_eq!(number(&r, "count"), 1.0);
    assert_eq!(number(&r, "seen"), 40.0);
    let lines: Vec<_> = r
        .journal()
        .filter(|line| line.contains("timer row="))
        .collect();
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("row=0 kind=2 nominal_ms=10.000 cutoff_ms=25.000 commit_ms=40.000"),
        "{}",
        lines[0]
    );
}

#[test]
fn options_are_checked_and_round_trip() {
    for arguments in [
        "wait=frame",
        "wait=update",
        "wait=scrollend",
        "wait=idle",
        "wait=idle, max=1",
        "max=4294967295, wait=scrollend",
    ] {
        let r = boot(&format!("  task t mount\n    after(1, tick, {arguments})"));
        let bytes = r.plan().encode();
        assert!(exact_plan::Plan::decode(&bytes).is_ok());
    }
    for arguments in [
        "wait=unknown",
        "wait=1",
        "wait=frame + 1",
        "wait=frame, wait=idle",
        "wait=idle, max=1, max=2",
        "max=1",
        "wait=frame, max=1",
        "wait=update, max=1",
        "wait=idle, max=0",
        "wait=idle, max=-1",
        "wait=idle, max=1.5",
        "wait=idle, max=4294967296",
        "wait=idle, max=count",
    ] {
        let source = format!("component App\n  state count = 0\n  action tick\n    count = count + 1\n  task t mount\n    after(1, tick, {arguments})\n  view\n    text \"timer\"\n");
        assert!(contract::compile(&source).is_err(), "{arguments}");
    }
}
#[test]
fn frame_rounding_includes_exact_boundaries_and_adjacent_values() {
    for k in [1u64, 2, 3, 59, 60, 61, 1_000_000, u32::MAX as u64 + 1] {
        let frame = k as f64 * 1000.0 / 60.0;
        assert_eq!(frame_at_or_after(frame), frame);
        assert_eq!(frame_at_or_after(frame.next_down()), frame);
        assert!(frame_at_or_after(frame.next_up()) > frame);
    }
}
#[test]
fn seeks_merge_frame_beats_keep_phase_and_do_not_depend_on_seek_partition() {
    let tasks = "  task t mount\n    every(1, tick, wait=frame)";
    let mut whole = boot(tasks);
    let mut split = boot(tasks);
    whole.advance(100.0).unwrap();
    for i in 1..=100 {
        split.advance(i as f64).unwrap();
    }
    assert_eq!(number(&whole, "count"), 6.0);
    assert_eq!(number(&split, "count"), 6.0);
    assert_eq!(number(&whole, "seen"), 100.0);
    assert_eq!(whole.tasks(), split.tasks());
}
#[test]
fn same_execution_time_orders_by_nominal_deadline_with_idle_last() {
    let mut r = boot("  task f when enabled\n    after(49, tick, wait=frame)\n  task p mount\n    after(50, disable)");
    r.advance(50.0).unwrap();
    assert_eq!(number(&r, "count"), 1.0);
    let mut r = boot("  task i when enabled\n    after(50, tick, wait=idle)\n  task p mount\n    after(50, disable)");
    r.advance(50.0).unwrap();
    assert_eq!(number(&r, "count"), 0.0);
}
#[test]
fn caps_are_visible_from_arming_and_uncapped_waits_cannot_hide_them() {
    let mut r = boot("  task s mount\n    after(100, tick, wait=scrollend)\n  task bounded mount\n    after(200, tick, wait=scrollend, max=100)");
    let time = live(&mut r);
    assert_eq!(r.timer_due_ms(), Some(300.0));
    time.set(200.0);
    assert!(r.advance(200.0).unwrap().is_empty());
    assert!(r.waits().scrollend.held);
    assert_eq!(r.timer_due_ms(), Some(300.0));
    time.set(300.0);
    r.advance(300.0).unwrap();
    assert_eq!(number(&r, "count"), 1.0);
    r.release_waits(WaitKinds::SCROLLEND, 300.0);
    assert_eq!(number(&r, "count"), 2.0);
    assert_eq!(r.timer_due_ms(), None);
}
#[test]
fn unsupported_waits_remain_precise() {
    for kind in ["frame", "update", "scrollend", "idle"] {
        let mut r = boot(&format!("  task t mount\n    after(49, tick, wait={kind})"));
        r.set_live_clock(Some(Box::new(|| 49.0)));
        assert_eq!(r.timer_due_ms(), Some(49.0));
        r.advance(49.0).unwrap();
        assert_eq!(number(&r, "count"), 1.0);
    }
}
#[test]
fn ordinary_work_is_applied_before_admission_and_cancels_held_work() {
    let mut r = boot("  task f when enabled\n    after(49, tick, wait=frame)\n  task p mount\n    after(50, disable)");
    let time = live(&mut r);
    time.set(51.0);
    let first = r.release_waits(WaitKinds::FRAME, 51.0);
    assert!(first.continuing);
    assert_eq!(first.advanced.receipts.len(), 1);
    assert_eq!(number(&r, "count"), 0.0);
    let next = r.release_waits(WaitKinds::FRAME, 51.0);
    assert!(!next.continuing);
    assert!(next.advanced.receipts.is_empty());
}
#[test]
fn idle_is_admitted_alone_one_batch_at_a_time() {
    let mut r = boot("  task a mount\n    after(10, tick, wait=idle)\n  task b mount\n    after(10, tick, wait=idle)");
    let time = live(&mut r);
    time.set(10.0);
    r.release_waits(WaitKinds::ALL, 10.0);
    assert_eq!(number(&r, "count"), 0.0);
    r.release_waits(WaitKinds::IDLE, 10.0);
    assert_eq!(number(&r, "count"), 1.0);
    assert!(r.waits().idle.held);
    r.release_waits(WaitKinds::IDLE, 10.0);
    assert_eq!(number(&r, "count"), 2.0);
}
#[test]
fn idle_cap_merges_beats_and_keeps_the_original_phase() {
    let mut r = boot("  task t mount\n    every(250, tick, wait=idle)");
    let time = live(&mut r);
    assert_eq!(r.timer_due_ms(), Some(1250.0));
    time.set(1250.0);
    r.advance(1250.0).unwrap();
    assert_eq!(number(&r, "count"), 1.0);
    assert_eq!(r.tasks()[0].1, Some(1500.0));
}
#[test]
fn takeover_runs_overdue_firings_once_at_the_sample_without_rewinding() {
    let mut r = boot("  task f mount\n    every(49, tick, wait=frame)\n  task s mount\n    after(50, tick, wait=scrollend)\n  task p mount\n    after(51, tick)");
    let time = live(&mut r);
    time.set(60.0);
    r.set_live_clock(None);
    r.advance(0.0).unwrap();
    assert_eq!(number(&r, "count"), 3.0);
    assert_eq!(number(&r, "seen"), 60.0);
    assert_eq!(r.tasks()[0].1, Some(98.0));
    r.advance(60.0).unwrap();
    assert_eq!(number(&r, "count"), 3.0);
}

#[test]
fn tolerance_coalesces_wakes_without_changing_logical_deadlines() {
    let mut r = boot("  task a mount\n    after(100, tick, tolerance=100)\n  task b mount\n    after(150, tick, tolerance=100)");
    let time = live(&mut r);
    assert_eq!(r.timer_due_ms(), Some(100.0));
    assert_eq!(r.timer_tolerance_ms(), 100.0);
    assert_eq!(r.timer_wake_ms(), Some(200.0));
    time.set(200.0);
    r.advance(200.0).unwrap();
    assert_eq!(number(&r, "count"), 2.0);
    assert_eq!(r.timer_wake_ms(), None);
    let mut r = boot(
        "  task a mount\n    after(100, tick, tolerance=100)\n  task b mount\n    after(150, tick)",
    );
    live(&mut r);
    assert_eq!(r.timer_tolerance_ms(), 50.0);
    assert_eq!(r.timer_wake_ms(), Some(150.0));
    r.advance(100.0).unwrap();
    assert_eq!(
        number(&r, "count"),
        1.0,
        "another wake may run a tolerant timer at its nominal deadline"
    );
}

#[test]
fn tolerance_has_no_effect_on_seekable_clocks_or_repeat_phases() {
    let mut r = boot("  task a mount\n    every(7, tick, tolerance=4294967295)");
    assert_eq!(r.timer_wake_ms(), Some(7.0));
    assert_eq!(r.timer_tolerance_ms(), 0.0);
    r.advance(70.0).unwrap();
    assert_eq!(number(&r, "count"), 10.0);
    assert_eq!(number(&r, "seen"), 70.0);
    assert_eq!(r.tasks()[0].1, Some(77.0));
}

#[test]
fn every_idle_observation_term_is_sampled_with_an_inclusive_horizon() {
    use exact_runner::IdleObservation;
    let now = 100.0;
    assert!(IdleObservation::default().allows(now));
    for blocked in [
        IdleObservation {
            scrolling: true,
            ..Default::default()
        },
        IdleObservation {
            demand: true,
            ..Default::default()
        },
        IdleObservation {
            applying: true,
            ..Default::default()
        },
        IdleObservation {
            ordinary: true,
            ..Default::default()
        },
        IdleObservation {
            next_ms: Some(now + 1000.0 / 60.0),
            ..Default::default()
        },
    ] {
        assert!(!blocked.allows(now));
    }
    assert!(IdleObservation {
        next_ms: Some((now + 1000.0 / 60.0).next_up()),
        ..Default::default()
    }
    .allows(now));
}

#[test]
fn a_refused_held_firing_is_spent_and_the_next_firing_survives_rollback() {
    let mut r = boot_source(
        r#"component App
  state count = 0
  state divisor = 1
  state done = false
  action refuse
    count = count + 1
    divisor = 0
  action finish
    done = true
  task first when true key=1 / divisor
    after(10, refuse, wait=frame)
  task second mount
    after(10, finish, wait=frame)
  view
    text "timer"
"#,
    );
    let time = live(&mut r);
    time.set(10.0);
    let first = r.release_waits(WaitKinds::FRAME, 10.0);
    assert!(first.advanced.error.is_some());
    assert!(first.continuing);
    assert_eq!(number(&r, "count"), 0.0);
    assert_eq!(number(&r, "divisor"), 1.0);
    let second = r.release_waits(WaitKinds::FRAME, 10.0);
    assert!(second.advanced.error.is_none());
    assert_eq!(r.slot("done"), Some(&Value::Bool(true)));
    assert_eq!(r.tasks()[0].1, None);
    r.release_waits(WaitKinds::FRAME, 10.0);
    assert_eq!(r.timer_due_ms(), None);
}

#[test]
fn continuing_a_refused_frame_does_not_repeat_completed_frame_tasks() {
    let mut r = boot_source(
        r#"component App
  state first = 0
  state last = 0
  state divisor = 1
  action a
    first = first + 1
  action b
    divisor = 0
  action c
    last = last + 1
  task a mount
    every(frame, a)
  task b when true key=1 / divisor
    every(frame, b)
  task c mount
    every(frame, c)
  view
    text "timer"
"#,
    );
    live(&mut r);
    let first = r.frame_tasks(10.0);
    assert!(first.error.is_some());
    assert_eq!(number(&r, "first"), 1.0);
    assert_eq!(number(&r, "last"), 0.0);
    let rest = r.complete_frame_tasks(10.0);
    assert!(rest.error.is_none());
    assert_eq!(number(&r, "first"), 1.0);
    assert_eq!(number(&r, "last"), 1.0);
    let next = r.complete_frame_tasks(20.0);
    assert!(next.error.is_some());
    assert_eq!(number(&r, "first"), 2.0);
    assert_eq!(number(&r, "last"), 2.0);
}
