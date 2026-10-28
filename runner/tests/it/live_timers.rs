//! A live clock (the host's own, not a seek's): a repeating timer that
//! missed beats fires once, ordered as scheduled and observing when it runs,
//! and is rescheduled after its callback (the timers RFC, D2 and D3); what a
//! callback arms takes its place, and every commit observes the clock. A
//! seek clock still fires every beat at its own time (D4).
use exact_kernel::{Kernel, ViewId};
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Event, FailureKind, InFlight, Outcome, Request, Runner,
    RunnerError, Store,
};
use std::cell::Cell;
use std::rc::Rc;

/// Answers `save` at once (`n` 1) and `later` by a request (`n` 0, whatever
/// its outcome); `forgotten` sets the clock to `forgotten_at`, if any.
#[derive(Default)]
struct Desk {
    forgotten_at: Option<Rc<Cell<f64>>>,
}
impl DataSource for Desk {
    fn forgotten(&mut self, _: &Store, _: &[InFlight<'_>]) {
        if let Some(clock) = &self.forgotten_at {
            clock.set(4500.0);
        }
    }
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        match name {
            "save" => Ok(Value::record(vec![Value::Number(1.0)])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(&mut self, _: &mut Store, name: &str, args: &[Value]) -> Result<Answer, DataError> {
        match name {
            "later" => Ok(Answer::Later(Request::post_json(
                "https://desk.example/",
                "",
            ))),
            _ => self.query(name, args).map(Answer::Now),
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(Value::record(vec![Value::Number(0.0)])))
    }
}

fn boot(source: &str) -> Runner<Desk> {
    let plan = contract::compile(source).unwrap();
    Runner::boot(
        plan,
        Desk::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// Advance to `cutoff_ms` on a live clock that reads `times` in turn, then
/// keeps the last.
fn live(r: &mut Runner<Desk>, cutoff_ms: f64, times: &[f64]) -> exact_runner::Advanced {
    clock(r, times);
    r.advance_timed(cutoff_ms)
}

/// Give `r` a live clock that reads `times` in turn, then keeps the last.
fn clock(r: &mut Runner<Desk>, times: &[f64]) {
    let times = times.to_vec();
    let mut at = 0;
    r.set_live_clock(Some(Box::new(move || {
        let t = times[at.min(times.len() - 1)];
        at += 1;
        t
    })));
}

fn view(r: &Runner<Desk>, test_id: &str) -> ViewId {
    let key = r.kernel().find_first_by_test_id(test_id).unwrap();
    r.kernel().node_by_key(key).unwrap().id
}

fn number(r: &Runner<Desk>, slot: &str) -> f64 {
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
    let a = live(&mut r, 3500.0, &[3500.0]);
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
    let a = live(&mut r, 10_000_000.0, &[10_000_000.0]);
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "ticks"), 1.0);
}

#[test]
fn a_repeat_is_rescheduled_after_its_callback_completes() {
    let mut r = boot(TICKER);
    // Delivered at 1000; its action and commit end at 4500.
    live(&mut r, 1000.0, &[1000.0, 4500.0]);
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
    live(&mut r, 3500.0, &[3500.0, 6500.0, 6500.0, 6600.0]);
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
    live(&mut r, 1000.0, &[1000.0, 4500.0]);
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
    live(&mut r, 3500.0, &[3500.0, 4500.0]);
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
    let a = live(&mut r, 3500.0, &[3500.0, 4200.0]);
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
    let a = live(&mut r, 1000.0, &[1000.0, 4500.0]);
    assert_eq!(r.slot("done"), Some(&Value::Bool(true)));
    assert_eq!(a.now_ms, 4500.0, "a one-shot's completion is read too");
}

#[test]
fn a_sample_that_is_no_clock_time_moves_nothing() {
    let mut r = boot(TICKER);
    let a = live(&mut r, 3500.0, &[f64::INFINITY]);
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
    live(&mut r, 5.0, &[5.0, 6.13]);
    assert_eq!(r.timer_due_ms(), Some(7.13));
    live(&mut r, 9.5, &[9.5, 44.129999999999995]);
    assert_eq!(
        r.timer_due_ms(),
        Some(44.13),
        "the first beat after, none skipped"
    );
}

/// A late repeat, its `then` and a later-scheduled snapshot: under the old
/// order the snapshot ran first, between the repeat and what it caused.
const CAUSAL: &str = r#"shape Ack
  n: number
component App
  state followed = 0
  state snapshot = -1
  state log = ""
  mutation m as shape Ack then follow
  mutation q as shape Ack queue then queued
  action tick
    send m = save()
    send q = save()
    send q = save()
  action follow
    followed = followed + 1
  action queued
    log = `${log}q`
  action snap
    snapshot = followed
    log = `${log}s`
  task ticker mount
    every(1000, tick)
  task snapper mount
    after(1500, snap)
  view
    text "x"
"#;

#[test]
fn what_a_callback_arms_takes_its_place_ahead_of_later_work() {
    let mut r = boot(CAUSAL);
    // The repeat (scheduled 1000) runs from 3500 to 6500, past the cutoff.
    let a = live(&mut r, 3500.0, &[3500.0, 6500.0]);
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "snapshot"), 1.0, "its `then` ran first");
    assert_eq!(
        r.slot("log"),
        Some(&Value::str("qqs")),
        "and its queue's `then`s and `next`"
    );
}

#[test]
fn a_reply_takes_its_place_when_it_arrives() {
    let mut r = boot(
        r#"shape Ack
  n: number
component App
  state followed = 0
  state snapshot = -1
  mutation m as shape Ack then follow
  action go
    send m = later()
  action follow
    followed = followed + 1
  action snap
    snapshot = followed
  task snapper mount
    after(1500, snap)
  view
    button "go" press=go testId="go"
"#,
    );
    clock(&mut r, &[1000.0, 1000.0, 2000.0]);
    r.dispatch(view(&r, "go"), Event::Press).unwrap();
    let request = r.take_requests().remove(0);
    // It arrives at 2000, after the snapshot was due.
    r.fulfill(
        request.ticket,
        Outcome::Failed {
            kind: FailureKind::Network,
            message: "any outcome answers".into(),
        },
    )
    .unwrap();
    assert_eq!(r.now_ms(), 2000.0);
    r.advance_timed(2000.0);
    assert_eq!(number(&r, "snapshot"), 0.0, "the snapshot was due first");
    assert_eq!(number(&r, "followed"), 1.0);
}

#[test]
fn an_input_observes_the_clock_as_it_runs() {
    let mut r = boot(
        r#"component App
  state seen = 0
  action go
    seen = performanceNow()
  view
    button "go" press=go testId="go"
"#,
    );
    clock(&mut r, &[2600.0]);
    let a = r.dispatch_at(view(&r, "go"), Event::Press, 2000.0);
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "seen"), 2600.0);
    assert_eq!(a.receipts.last().unwrap().at_ms, 2600.0);
}

#[test]
fn a_frame_ahead_of_the_clock_fires_no_timer_early() {
    let mut r = boot(TICKER);
    clock(&mut r, &[900.0]);
    r.frame(1016.0);
    assert_eq!(number(&r, "ticks"), 0.0, "due at 1000, the clock reads 900");
    clock(&mut r, &[1010.0]);
    r.frame(1016.0);
    assert_eq!(number(&r, "ticks"), 1.0);
}

#[test]
fn missed_virtual_frames_fire_once() {
    let mut r = boot(
        r#"component App
  state n = 0
  action step
    n = n + 1
  task ticker mount
    every(frame, step)
  view
    text "x"
"#,
    );
    let a = live(&mut r, 5000.0, &[5000.0]);
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "n"), 1.0, "not 300 frames");
    assert!(
        r.timer_due_ms().unwrap() > 5000.0,
        "the next after the completion"
    );
}

#[test]
fn a_next_already_due_keeps_its_place() {
    let mut r = boot(
        r#"shape Ack
  n: number
component App
  state log = ""
  mutation q as shape Ack queue
  derive n = match q { case some(a) => a.n, case none => -1 }
  action go
    send q = later()
    send q = save()
  action a
    log = `${log}a${n}`
  action b
    log = `${log}b${n}`
  task early mount
    after(1500, a)
  task late mount
    after(1800, b)
  view
    button "go" press=go testId="go"
"#,
    );
    clock(&mut r, &[1000.0, 1000.0, 2000.0]);
    r.dispatch(view(&r, "go"), Event::Press).unwrap();
    let request = r.take_requests().remove(0);
    // The reply at 2000 makes the second send due then; the callback at
    // 1500 does not move it ahead of the one at 1800, which sees the reply
    // and not the second send's answer.
    r.fulfill(
        request.ticket,
        Outcome::Failed {
            kind: FailureKind::Network,
            message: "any outcome answers".into(),
        },
    )
    .unwrap();
    r.advance_timed(2000.0);
    assert_eq!(r.slot("log"), Some(&Value::str("a0b0")));
}

#[test]
fn a_replys_then_takes_its_arrival_not_its_completion() {
    let mut r = boot(
        r#"shape Ack
  n: number
component App
  state followed = 0
  state snapshot = -1
  mutation m as shape Ack then follow
  action go
    send m = later()
  action follow
    followed = followed + 1
  action snap
    snapshot = followed
  task snapper mount
    after(1500, snap)
  view
    button "go" press=go testId="go"
"#,
    );
    r.dispatch(view(&r, "go"), Event::Press).unwrap();
    let request = r.take_requests().remove(0);
    // It arrives at 1000 and its commit ends at 3500.
    clock(&mut r, &[1000.0, 3500.0]);
    r.fulfill(
        request.ticket,
        Outcome::Failed {
            kind: FailureKind::Network,
            message: "any outcome answers".into(),
        },
    )
    .unwrap();
    r.advance_timed(3500.0);
    assert_eq!(number(&r, "snapshot"), 1.0, "its `then` was due first");
}

#[test]
fn a_handlers_arguments_observe_the_events_time() {
    let mut r = boot(
        r#"component App
  state argument = 0
  state body = 0
  action go(at: number)
    argument = at
    body = performanceNow()
  view
    button "go" press=go(performanceNow()) testId="go"
"#,
    );
    clock(&mut r, &[2600.0]);
    r.dispatch(view(&r, "go"), Event::Press).unwrap();
    assert_eq!(number(&r, "argument"), 2600.0);
    assert_eq!(number(&r, "body"), 2600.0);
}

#[test]
fn a_callbacks_completion_is_read_after_its_refusal_is_undone() {
    let source = r#"component App
  state ticks = 0
  state d = 1
  action tick
    ticks = ticks + 1
    d = 0
  task ticker when true key=1 / d
    every(1000, tick)
  view
    text "x"
"#;
    let plan = contract::compile(source).unwrap();
    let time = Rc::new(Cell::new(1000.0));
    let desk = Desk {
        forgotten_at: Some(time.clone()),
    };
    let mut r = Runner::boot(
        plan,
        desk,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_live_clock(Some(Box::new(move || time.get())));
    // The refusal's cleanup runs until 4500.
    let a = r.advance_timed(1000.0);
    assert!(a.error.is_some(), "the key refusal");
    assert_eq!(r.timer_due_ms(), Some(5000.0));
}

#[test]
fn a_refused_frame_reports_the_time_it_ended() {
    let mut r = boot(
        r#"component App
  state d = 1
  action step
    d = 0
  task ticker when true key=1 / d
    every(frame, step)
  view
    text "x"
"#,
    );
    r.present_frames(true);
    clock(&mut r, &[1000.0, 1000.0, 4500.0]);
    let a = r.frame(1000.0);
    assert!(a.error.is_some(), "the key refusal");
    assert_eq!(a.now_ms, 4500.0);
    assert_eq!(a.now_ms, r.now_ms());
}

#[test]
fn an_inputs_then_takes_its_arrival_not_its_completion() {
    let mut r = boot(
        r#"shape Ack
  n: number
component App
  state followed = 0
  state snapshot = -1
  mutation m as shape Ack then follow
  action go
    send m = save()
  action follow
    followed = followed + 1
  action snap
    snapshot = followed
  task snapper mount
    after(1500, snap)
  view
    button "go" press=go testId="go"
"#,
    );
    // It arrives at 1000; its commit starts at 3500 and ends then.
    clock(&mut r, &[1000.0, 3500.0]);
    r.dispatch(view(&r, "go"), Event::Press).unwrap();
    r.advance_timed(3500.0);
    assert_eq!(number(&r, "snapshot"), 1.0, "its `then` was due first");
}

#[test]
fn a_rekeyed_virtual_frame_restarts_from_its_completion() {
    let mut r = boot(
        r#"component App
  state k = 0
  action step
    k = k + 1
  task ticker when true key=k
    every(frame, step)
  view
    text "x"
"#,
    );
    let a = live(&mut r, 1000.0, &[1000.0, 4500.0]);
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "k"), 1.0);
    assert!(r.timer_due_ms().unwrap() > 4500.0, "{:?}", r.timer_due_ms());
}

/// An answer's `then` and a timer: `go` sends at 1000 and its reply lands
/// at 2000, while the snapshot falls due at `snap_ms`.
fn then_and_timer(snap_ms: u32) -> Runner<Desk> {
    let mut r = boot(&format!(
        r#"shape Ack
  n: number
component App
  state followed = 0
  state snapshot = -1
  mutation m as shape Ack then follow
  action go
    send m = later()
  action follow
    followed = followed + 1
  action snap
    snapshot = followed
  task snapper mount
    after({snap_ms}, snap)
  view
    button "go" press=go testId="go"
"#
    ));
    clock(&mut r, &[1000.0, 1000.0, 2000.0]);
    r.dispatch(view(&r, "go"), Event::Press).unwrap();
    let request = r.take_requests().remove(0);
    let answer = Outcome::Failed {
        kind: FailureKind::Network,
        message: "any outcome answers".into(),
    };
    r.fulfill(request.ticket, answer).unwrap();
    r
}

#[test]
fn a_then_that_is_the_earliest_work_due_lands_alone() {
    let mut r = then_and_timer(5000);
    let a = r.land_then_first();
    assert!(a.error.is_none(), "{:?}", a.error);
    assert_eq!(number(&r, "followed"), 1.0);
    assert_eq!(number(&r, "snapshot"), -1.0, "no timer fired");
    assert_eq!(r.timer_due_ms(), Some(5000.0));
}

#[test]
fn a_then_behind_an_earlier_timer_waits_for_the_advance() {
    let mut r = then_and_timer(1500);
    let a = r.land_then_first();
    assert!(
        a.receipts.is_empty(),
        "the timer due first is not overtaken"
    );
    assert_eq!(number(&r, "followed"), 0.0);
    r.advance_timed(2000.0);
    assert_eq!(number(&r, "snapshot"), 0.0, "the timer, then the `then`");
    assert_eq!(number(&r, "followed"), 1.0);
}

/// LLP 1072.000 §7.1 (a): an input whose target a due commit removed is
/// refused and logged, and that commit stands; one on a row the commit
/// changed runs against the current state.
#[test]
fn an_input_on_a_row_a_due_commit_removed_is_refused_and_logged() {
    let mut r = boot(
        r#"component App
  state rows = ["a", "b"]
  state price = 1
  state log = ""
  action tick
    rows = ["b"]
    price = price + 1
  action tap(name: string, p: number)
    log = `${log}${name}@${p} `
  task ticker mount
    after(1000, tick)
  view
    column
      for r in rows key=r
        button `${r} ${price}` press=tap(r, price) testId=`row-${r}`
"#,
    );
    clock(&mut r, &[1500.0]);
    let (a, b) = (view(&r, "row-a"), view(&r, "row-b"));
    let refused = r.dispatch_at(a, Event::Press, 1500.0);
    assert!(
        matches!(refused.error, Some(RunnerError::UnknownView(v)) if v == a),
        "{:?}",
        refused.error
    );
    assert_eq!(refused.receipts.len(), 1, "the timer's commit stands");
    assert_eq!(number(&r, "price"), 2.0);
    assert!(r.kernel().find_first_by_test_id("row-a").is_none());
    assert!(
        r.journal()
            .any(|l| l.contains("refused") && l.contains("UnknownView")),
        "the refusal is logged"
    );
    assert_eq!(r.slot("log"), Some(&Value::str("")));
    let ran = r.dispatch_at(b, Event::Press, 1600.0);
    assert!(ran.error.is_none(), "{:?}", ran.error);
    assert_eq!(
        r.slot("log"),
        Some(&Value::str("b@2 ")),
        "the current price"
    );
}
