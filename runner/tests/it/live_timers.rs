//! A live clock (the host's own, not a seek's): a repeating timer that
//! missed beats fires once, ordered as scheduled and observing when it runs,
//! and is rescheduled after its callback (the timers RFC, D2 and D3). A seek
//! clock still fires every beat at its own time (D4).
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
}

fn boot(source: &str) -> Runner<NoData> {
    let plan = contract::compile(source).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// A live sampler that reads `times` in turn, then keeps the last.
fn samples(times: &[f64]) -> impl FnMut() -> f64 + '_ {
    let mut at = 0;
    move || {
        let t = times[at.min(times.len() - 1)];
        at += 1;
        t
    }
}

fn number(r: &Runner<NoData>, slot: &str) -> f64 {
    match r.slot(slot) {
        Some(Value::Number(n)) => *n,
        other => panic!("{slot}: {other:?}"),
    }
}

const TICKER: &str = r#"component App
  state ticks = 0
  state seen = 0
  action tick
    ticks = ticks + 1
    seen = performanceNow()
  task ticker mount
    every(1000, tick)
  view
    text "x"
"#;

#[test]
fn missed_beats_fire_once_observing_when_they_run() {
    let mut r = boot(TICKER);
    let a = r.advance_live(3500.0, &mut samples(&[3500.0]));
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "ticks"), 1.0, "three beats missed, one fire");
    assert_eq!(
        number(&r, "seen"),
        3500.0,
        "it observes when it ran, not its first missed beat"
    );
    assert_eq!(r.timer_due_ms(), Some(4000.0), "and keeps its phase");
}

#[test]
fn any_number_of_missed_beats_is_one_fire() {
    let mut r = boot(TICKER);
    let a = r.advance_live(10_000_000.0, &mut samples(&[10_000_000.0]));
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "ticks"), 1.0);
}

#[test]
fn a_repeat_is_rescheduled_after_its_callback_completes() {
    let mut r = boot(TICKER);
    // Delivered at 1000; its action and commit end at 4500.
    r.advance_live(1000.0, &mut samples(&[1000.0, 4500.0]));
    assert_eq!(number(&r, "ticks"), 1.0);
    assert_eq!(
        r.timer_due_ms(),
        Some(5000.0),
        "the first beat of its phase after 4500"
    );
}

#[test]
fn the_cutoff_is_fixed_and_each_callback_observes_its_own_sample() {
    let mut r = boot(
        r#"component App
  state ticks = 0
  state snapshot = -1
  state snapshotSeen = 0
  state late = false
  action tick
    ticks = ticks + 1
  action snap
    snapshot = ticks
    snapshotSeen = performanceNow()
  action lateOne
    late = true
  task ticker mount
    every(1000, tick)
  task snapper mount
    after(1500, snap)
  task later mount
    after(5000, lateOne)
  view
    text "x"
"#,
    );
    // The repeat (scheduled 1000) runs first, from 3500 to 6500; the
    // snapshot (1500) still runs in this advance and sees one increment.
    r.advance_live(3500.0, &mut samples(&[3500.0, 6500.0, 6500.0, 6600.0]));
    assert_eq!(number(&r, "ticks"), 1.0);
    assert_eq!(number(&r, "snapshot"), 1.0, "the repeat ran first");
    assert_eq!(
        number(&r, "snapshotSeen"),
        6500.0,
        "resampled for each callback"
    );
    assert_eq!(
        r.slot("late"),
        Some(&Value::Bool(false)),
        "due after the cutoff: not this advance"
    );
}

#[test]
fn a_rekeyed_timer_arms_from_its_callbacks_completion() {
    let mut r = boot(
        r#"component App
  state k = 0
  action tick
    k = k + 1
  task ticker when true key=k
    every(1000, tick)
  view
    text "x"
"#,
    );
    r.advance_live(1000.0, &mut samples(&[1000.0, 4500.0]));
    assert_eq!(number(&r, "k"), 1.0);
    assert_eq!(
        r.timer_due_ms(),
        Some(5500.0),
        "not already overdue on return"
    );
}

#[test]
fn a_timer_its_callback_cancels_stays_cancelled() {
    let mut r = boot(
        r#"component App
  state on = true
  action stop
    on = false
  task ticker when on
    every(1000, stop)
  view
    text "x"
"#,
    );
    r.advance_live(3500.0, &mut samples(&[3500.0, 4500.0]));
    assert_eq!(r.slot("on"), Some(&Value::Bool(false)));
    assert_eq!(r.timer_due_ms(), None);
}

#[test]
fn a_refused_callback_still_consumes_its_firing() {
    let mut r = boot(
        r#"component App
  state ticks = 0
  state d = 1
  action tick
    ticks = ticks + 1
    d = 0
  task ticker when true key=1 / d
    every(1000, tick)
  view
    text "x"
"#,
    );
    // The commit makes the key no key: refused, rolled back, and the
    // firing consumed, its successor after the completion sample.
    let a = r.advance_live(3500.0, &mut samples(&[3500.0, 4200.0]));
    assert!(a.error.is_some(), "the key refusal");
    assert_eq!(number(&r, "ticks"), 0.0, "rolled back");
    assert_eq!(r.timer_due_ms(), Some(5000.0));
}

#[test]
fn a_seek_clock_still_fires_every_beat_at_its_own_time() {
    let mut r = boot(TICKER);
    let a = r.advance_timed(3500.0);
    assert_eq!(number(&r, "ticks"), 3.0);
    assert_eq!(
        a.receipts.iter().map(|t| t.at_ms).collect::<Vec<_>>(),
        [1000.0, 2000.0, 3000.0]
    );
    assert_eq!(number(&r, "seen"), 3000.0);
}

#[test]
fn every_live_callback_ends_with_a_sample_of_the_clock() {
    let mut r = boot(
        r#"component App
  state done = false
  action finish
    done = true
  task once mount
    after(1000, finish)
  view
    text "x"
"#,
    );
    let a = r.advance_live(1000.0, &mut samples(&[1000.0, 4500.0]));
    assert_eq!(r.slot("done"), Some(&Value::Bool(true)));
    assert_eq!(a.now_ms, 4500.0, "a one-shot's completion is read too");
}

#[test]
fn a_sample_that_is_no_clock_time_moves_nothing() {
    let mut r = boot(TICKER);
    let a = r.advance_live(3500.0, &mut samples(&[f64::INFINITY]));
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "ticks"), 1.0);
    assert_eq!(a.now_ms, 3500.0, "the cutoff, not the bad sample");
    assert_eq!(r.timer_due_ms(), Some(4000.0));
}

#[test]
fn a_repeat_keeps_its_phase_whatever_the_rounding() {
    let mut r = boot(
        r#"component App
  state k = 0
  state ticks = 0
  action bump
    k = 1
  action tick
    ticks = ticks + 1
  task ticker when true key=k
    every(1, tick)
  task bumper mount
    after(5, bump)
  view
    text "x"
"#,
    );
    // The rekey ends at 6.13, so the timer's phase is 7.13 + n, which no
    // float subtraction may shift a beat.
    r.advance_live(5.0, &mut samples(&[5.0, 6.13]));
    assert_eq!(r.timer_due_ms(), Some(7.13));
    r.advance_live(9.5, &mut samples(&[9.5, 44.129999999999995]));
    assert_eq!(
        r.timer_due_ms(),
        Some(44.13),
        "the first beat after, none skipped"
    );
}
