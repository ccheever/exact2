//! The display's clock owns time until a driver's takes it (the timers RFC,
//! D1): a running runner and every one a reload boots take the choice.
use super::super::{PainterChoice, Presenter};
use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == "observe" {
            return Ok(args[0].clone());
        }
        Err(DataError::UnknownSource(source.into()))
    }
}

const TICKER: &str = r#"component App
  state ticks = 0
  action tick
    ticks = ticks + 1
  task ticker mount
    every(1000, tick)
  view
    text "x"
"#;

fn ticks(p: &Presenter<NoData>) -> f64 {
    p.host()
        .runner()
        .slot("ticks")
        .and_then(Value::as_number)
        .unwrap()
}

#[test]
fn the_display_clock_reaches_every_host_until_a_driver_takes_time() {
    let plan = crate::test_plan(TICKER);
    let (mut p, _) = Presenter::boot_with(
        &plan,
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    let wall = Rc::new(Cell::new(3500.0));
    let read = wall.clone();
    p.set_live_clock(Some(Rc::new(move || read.get())));
    p.clock(3500.0);
    assert_eq!(ticks(&p), 1.0, "three missed beats, one fire");
    // A reload's runner, carrying the state and the timer, takes the
    // display's clock too.
    p.reload(&plan, NoData).unwrap();
    wall.set(7500.0);
    p.clock(7500.0);
    assert_eq!(ticks(&p), 2.0, "four missed beats, one fire");
    // A driver takes the clock: every beat fires again.
    p.set_live_clock(None);
    p.clock(10_500.0);
    assert_eq!(ticks(&p), 5.0);
}

#[test]
fn an_input_the_live_clock_overtook_commits_without_rewinding_motion() {
    let source = r#"component App
  state on = false
  action go
    on = true
  view
    button "Go" press=go testId="go" opacity=(on ? 0 : 1) transition="opacity 1s linear"
"#;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    p.set_live_clock(Some(Rc::new(|| 2600.0)));
    let k = p.host().kernel();
    let go = k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id;
    // The event says 2000; the runner observes 2600. The host presents the
    // commit at the input's time, and what follows it runs from there.
    let error = p.host.dispatch_at(go, exact_runner::Event::Press, 2000.0);
    assert!(error.is_none(), "{error:?}");
    assert_eq!(p.host().now(), 2000.0);
    assert_eq!(p.host().runner().now_ms(), 2600.0);
}

#[test]
fn retained_input_clock_runs_due_timer_before_action_and_arms_from_input() {
    let source = r#"component App
  state fired = 0
  state pressed = false
  action timer
    fired = 1
  action go
    pressed = true
  action later
    fired = 2
  task first when !pressed
    after(150, timer)
  task second when pressed
    after(50, later)
  view
    button "Go" press=go testId="go"
"#;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(p.host.advance(100.).is_none());
    let key = p.host.kernel().find_by_test_id("go")[0];
    let binding = p
        .host
        .runner()
        .capture_action_binding(key, exact_plan::EventKind::Press)
        .unwrap();
    assert_eq!(
        p.host
            .advance_retained_input(key, &binding, exact_plan::EventKind::Press, None, 200.),
        Some(true)
    );
    assert_eq!(
        p.host
            .dispatch_retained(key, &binding, exact_plan::EventKind::Press, None),
        Ok(true)
    );
    assert_eq!(p.host.runner().slot("fired"), Some(&Value::Number(1.)));
    assert_eq!(p.host.runner().timer_due_ms(), Some(250.));
    assert!(p.host.advance(250.).is_none());
    assert_eq!(p.host.runner().slot("fired"), Some(&Value::Number(2.)));
}

#[test]
fn retained_input_clock_refusals_do_not_sample_or_fire_timers() {
    let source = r#"component App
  state value = "old"
  state disabled = false
  state fired = false
  action timer
    fired = true
  action change
    value = "new"
  action disable
    disabled = true
  action go(value: string)
    fired = true
  task first mount
    after(150, timer)
  view
    button "Go" press=go(value) testId="go" disabled=disabled
"#;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(p.host.advance(100.).is_none());
    let key = p.host.kernel().find_by_test_id("go")[0];
    let binding = p
        .host
        .runner()
        .capture_action_binding(key, exact_plan::EventKind::Press)
        .unwrap();
    let samples = Rc::new(Cell::new(0));
    let reads = samples.clone();
    p.set_live_clock(Some(Rc::new(move || {
        reads.set(reads.get() + 1);
        200.
    })));
    for at in [f64::NAN, f64::INFINITY, 99.] {
        assert_eq!(
            p.host
                .advance_retained_input(key, &binding, exact_plan::EventKind::Press, None, at),
            None
        );
    }
    assert_eq!(samples.get(), 0);
    p.set_live_clock(None);
    assert_eq!(samples.get(), 1, "clock takeover samples once");
    let takeover = p.host.runner().now_ms();
    p.host.runner_mut().act("change", vec![]).unwrap();
    let before = p.host.runner().kernel().epoch();
    assert_eq!(
        p.host
            .advance_retained_input(key, &binding, exact_plan::EventKind::Press, None, 200.),
        None
    );
    assert_eq!(p.host.runner().kernel().epoch(), before);
    assert_eq!(p.host.runner().now_ms(), takeover);
    assert_eq!(p.host.now(), 100.);
    assert_eq!(p.host.runner().slot("fired"), Some(&Value::Bool(false)));
    let current = p
        .host
        .runner()
        .capture_action_binding(key, exact_plan::EventKind::Press)
        .unwrap();
    p.host.runner_mut().act("disable", vec![]).unwrap();
    assert_eq!(
        p.host
            .advance_retained_input(key, &current, exact_plan::EventKind::Press, None, 200.),
        None
    );
    assert_eq!(p.host.runner().now_ms(), takeover);
    assert_eq!(p.host.runner().slot("fired"), Some(&Value::Bool(false)));
}

#[test]
fn retained_input_clock_keeps_live_release_time_and_coalesces_missed_beats() {
    let source = r#"component App
  state ticks = 0
  state pressed = false
  action timer
    ticks = ticks + 1
  action go
    pressed = true
  task ticker mount
    every(100, timer)
  view
    box swiperight=go testId="go" transition="translate -exact-spring(300, 30, 1)"
      text "Swipe"
"#;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    let key = p.host.kernel().find_by_test_id("go")[0];
    let view = p.host.kernel().node_by_key(key).unwrap().id;
    let binding = p
        .host
        .runner()
        .capture_action_binding(key, exact_plan::EventKind::Swiperight)
        .unwrap();
    let held = p
        .host
        .hold_begin(view, exact_motion::Property::Translate, 0.)
        .unwrap()
        .unwrap();
    p.set_live_clock(Some(Rc::new(|| 2600.)));
    assert_eq!(
        p.host.advance_retained_input(
            key,
            &binding,
            exact_plan::EventKind::Swiperight,
            Some(held.token),
            2000.
        ),
        Some(true)
    );
    assert_eq!(
        p.host.dispatch_retained(
            key,
            &binding,
            exact_plan::EventKind::Swiperight,
            Some(held.token)
        ),
        Ok(true)
    );
    assert_eq!(ticks(&p), 1.);
    assert_eq!(p.host.runner().slot("pressed"), Some(&Value::Bool(true)));
    assert_eq!(p.host.now(), 2000.);
    assert_eq!(p.host.runner().now_ms(), 2600.);
    assert_eq!(
        p.host
            .hold_end(held.token, exact_motion::HoldEnd::Cancel, 2000.),
        Ok(true)
    );
    let before = p.host.runner().kernel().epoch();
    assert_eq!(
        p.host.advance_retained_input(
            key,
            &binding,
            exact_plan::EventKind::Swiperight,
            Some(held.token),
            3000.
        ),
        None
    );
    assert_eq!(p.host.runner().kernel().epoch(), before);
    assert_eq!(p.host.now(), 2000.);
}

#[test]
fn timer_waits_use_presented_frames_reports_and_the_idle_opportunity() {
    for kind in ["frame", "update", "scrollend", "idle"] {
        let source = TICKER.replace(
            "every(1000, tick)",
            &format!("after(10, tick, wait={kind})"),
        );
        let plan = crate::test_plan(&source);
        let (mut p, _) = Presenter::boot_with(
            &plan,
            NoData,
            (400., 400.),
            1.,
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
            PainterChoice::Cpu,
        )
        .unwrap();
        let wall = Rc::new(Cell::new(0.0));
        let read = wall.clone();
        p.set_live_clock(Some(Rc::new(move || read.get())));
        p.timer_observation(kind == "scrollend", false, kind == "idle");
        wall.set(10.0);
        p.advance(10.0);
        assert_eq!(
            ticks(&p),
            if kind == "update" { 1.0 } else { 0.0 },
            "{kind}"
        );
        match kind {
            "frame" => {
                wall.set(16.0);
                p.animation_frame(16.0);
            }
            "scrollend" => {
                p.timer_observation(false, false, false);
                p.advance(10.0);
            }
            "idle" => {
                p.admit_idle_timer(10.0);
                assert_eq!(ticks(&p), 0.0, "an unapplied publication prevents idle");
                p.timer_observation(false, true, false);
                p.admit_idle_timer(10.0);
                assert_eq!(ticks(&p), 0.0, "animation sampling prevents idle");
                p.timer_observation(false, false, false);
                p.admit_idle_timer(10.0);
            }
            _ => {}
        }
        assert_eq!(ticks(&p), 1.0, "{kind}");
        p.advance(110.0);
        assert_eq!(ticks(&p), 1.0, "the cap cannot fire a spent wait");
    }
}

#[test]
fn native_callbacks_keep_non_ui_work_off_frames_and_survive_reload_and_takeover() {
    let plan = crate::test_plan(TICKER);
    let (mut p, error) = Presenter::boot_with(
        &plan,
        NoData,
        (400., 400.),
        1.,
        PathBuf::from("."),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let wall = Rc::new(Cell::new(0.0));
    let read = wall.clone();
    p.use_native_timers(true);
    p.set_live_clock(Some(Rc::new(move || read.get())));
    let epoch = p.native_timer_generation();
    let timer = p.host().runner().native_timers()[0].clone();
    p.frame();
    assert!(!p.dirty());
    assert!(!p.wants_display_frames());
    assert_eq!(p.wake_due_ms(), None);
    wall.set(3500.0);
    assert!(p.advance(3500.0).is_none());
    assert!(p.animation_frame(3500.0).is_none());
    assert_eq!(ticks(&p), 0.0);
    assert!(p.fire_native_timer(epoch, &timer, 3500.0).is_none());
    assert_eq!(ticks(&p), 1.0);
    assert!(!p.dirty(), "A hidden state write needs no redraw");
    assert_eq!(p.host().runner().native_timers()[0].due_ms, 4000.0);
    p.reload(&plan, NoData).unwrap();
    assert_ne!(p.native_timer_generation(), epoch);
    let carried = ticks(&p);
    assert!(p.fire_native_timer(epoch, &timer, 4500.0).is_none());
    assert_eq!(
        ticks(&p),
        carried,
        "An old callback cannot fire a reloaded plan"
    );
    assert!(p.host().runner().uses_native_timers());
    assert_eq!(p.host().runner().native_timers().len(), 1);
    let reloaded = p.native_timer_generation();
    p.set_live_clock(None);
    assert_ne!(p.native_timer_generation(), reloaded);
    assert!(p.host().runner().native_timers().is_empty());
}

#[test]
fn native_actions_publish_their_visible_changes_through_the_normal_commit_path() {
    let plan = crate::test_plan(&TICKER.replace("text \"x\"", "text `${ticks}`"));
    let (mut p, _) = Presenter::boot_with(
        &plan,
        NoData,
        (400., 400.),
        1.,
        PathBuf::from("."),
        PainterChoice::Cpu,
    )
    .unwrap();
    p.use_native_timers(true);
    p.set_live_clock(Some(Rc::new(|| 1000.0)));
    let timer = p.host().runner().native_timers()[0].clone();
    p.frame();
    assert!(!p.dirty());
    assert!(p
        .fire_native_timer(p.native_timer_generation(), &timer, 1000.0)
        .is_none());
    assert_eq!(ticks(&p), 1.0);
    assert!(p.dirty());
}

#[test]
fn native_commands_run_without_a_presentation() {
    let source = TICKER
        .replace(
            "  action tick",
            "  resource observed = observe(ticks) as shape number\n  action tick",
        )
        .replace(
            "    ticks = ticks + 1",
            "    ticks = ticks + 1\n    startTrace_EXPERIMENTAL()",
        );
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(&source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from("."),
        PainterChoice::Cpu,
    )
    .unwrap();
    p.use_native_timers(true);
    p.set_live_clock(Some(Rc::new(|| 1000.0)));
    let timer = p.host().runner().native_timers()[0].clone();
    p.frame();
    assert!(p
        .fire_native_timer(p.native_timer_generation(), &timer, 1000.0)
        .is_none());
    assert!(p.pump(1000.0).is_none());
    p.run_commands(NoData::default);
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    assert!(
        journal.contains("startTrace_EXPERIMENTAL refused: trace recording is on Apple hosts only"),
        "{journal}"
    );
    assert!(!p.dirty());
    assert!(!p.wants_display_frames());
}

#[test]
fn native_deadlines_still_block_idle_admission_inside_the_frame_horizon() {
    let source = TICKER.replace(
        "  view",
        "  task idle mount\n    after(995, tick, wait=idle)\n  view",
    );
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(&source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from("."),
        PainterChoice::Cpu,
    )
    .unwrap();
    let time = Rc::new(Cell::new(0.0));
    let read = time.clone();
    p.use_native_timers(true);
    p.set_live_clock(Some(Rc::new(move || read.get())));
    time.set(995.0);
    p.advance(995.0);
    p.admit_idle_timer(995.0);
    assert_eq!(ticks(&p), 0.0);
    assert!(p.host().runner().waits().idle.held);
    let timer = p.host().runner().native_timers()[0].clone();
    time.set(1000.0);
    p.fire_native_timer(p.native_timer_generation(), &timer, 1000.0);
    p.admit_idle_timer(1000.0);
    assert_eq!(ticks(&p), 2.0);
    assert!(!p.host().runner().waits().idle.held);
}

#[test]
fn a_hidden_native_carrier_admits_waits_without_adding_frames() {
    let source = TICKER.replace("    every(1000, tick)", "    after(100, tick, wait=frame)");
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(&source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from("."),
        PainterChoice::Cpu,
    )
    .unwrap();
    p.use_native_timers(true);
    p.set_live_clock(Some(Rc::new(|| 100.0)));
    p.timer_visibility(true);
    p.advance(100.0);
    assert_eq!(ticks(&p), 1.0);
    assert!(!p.host().runner().waits().frame.held);
    assert!(!p.wants_display_frames());
}

#[test]
fn a_held_frame_wait_supplies_demand_for_an_update_wait() {
    let source = TICKER.replace(
        "    every(1000, tick)",
        "    after(10, tick, wait=frame)\n  task update mount\n    after(10, tick, wait=update)",
    );
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(&source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from("."),
        PainterChoice::Cpu,
    )
    .unwrap();
    p.set_live_clock(Some(Rc::new(|| 10.0)));
    assert!(p.advance(10.0).is_none());
    assert!(p.wants_frames());
    assert_eq!(ticks(&p), 0.0);
    assert!(p.animation_frame(10.0).is_none());
    assert_eq!(ticks(&p), 2.0);
}
