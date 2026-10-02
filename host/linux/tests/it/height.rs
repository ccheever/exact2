//! Explicit single-owner layout motion, with no physical gesture claim.
use exact_kernel::{
    motion::motion_node, Dimension, MonospaceMeasurer, TextMeasureRequest, TextMeasurer,
    TextMetrics,
};
use exact_linux::Host;
use exact_motion::{HoldEnd, Property, Value};
use exact_runner::{DataError, DataSource, Event};
use std::{cell::Cell, rc::Rc};

struct Empty;
impl DataSource for Empty {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
fn boot(source: &str, measurer: Box<dyn TextMeasurer>) -> Host<Empty> {
    let (h, error) = Host::boot(
        &contract::compile(source).unwrap().encode(),
        Empty,
        measurer,
        400.,
        300.,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    h
}
const APP: &str = r#"component App
  state target = 180
  state showing = true
  state hidden = false
  action grow
    target = 420
  action shrink
    target = 100
  action hide
    hidden = true
  action toggle
    showing = !showing
  view
    box width=400 height="100%"
      button testId="grow" press=grow
        text "grow"
      button testId="shrink" press=shrink
        text "shrink"
      button testId="hide" press=hide
        text "hide"
      button testId="toggle" press=toggle
        text "toggle"
      box height="100%" position="absolute" top=0 display=(hidden ? "none" : "block") testId="ancestor"
        when showing
          box testId="panel" width=300 height=target position="absolute" bottom=0 max-height="100%" box-sizing="border-box" padding=8 border-width=2 border-style="solid" transition="height spring(300,30,1), translate spring(300,30,1)"
            text "panel text"
      box testId="other" height=70 box-sizing="border-box"
      box testId="content" height=80
      image testId="image"
"#;
fn fixture() -> Host<Empty> {
    boot(APP, Box::new(MonospaceMeasurer::default()))
}
fn id(h: &Host<Empty>, name: &str) -> u32 {
    h.kernel()
        .node_by_key(h.kernel().find_by_test_id(name)[0])
        .unwrap()
        .id
}
fn height(h: &Host<Empty>, view: u32) -> f32 {
    h.kernel().node(view).unwrap().frame.height
}
fn press(h: &mut Host<Empty>, name: &str, now: f64) {
    assert!(h.dispatch_at(id(h, name), Event::Press, now).is_none());
}

#[test]
fn no_implicit_height_adoption_and_invalid_registration_is_atomic() {
    let mut h = fixture();
    let panel = id(&h, "panel");
    let key = h.kernel().node(panel).unwrap().key;
    assert!(h
        .engine()
        .value(motion_node(key), Property::Height)
        .is_none());
    assert!(h.hold_begin(panel, Property::Height, 0.).unwrap().is_none());
    h.set_height_owner(Some(panel)).unwrap();
    let start = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    h.hold_update(start.token, Value::scalar(220.), 0.).unwrap();
    let before = h.kernel().export(None).unwrap();
    assert!(h.set_height_owner(Some(u32::MAX)).is_err());
    assert!(h.set_height_owner(Some(id(&h, "content"))).is_err());
    assert_eq!(h.height_owner(), Some(key));
    assert!(h.has_hold(start.token));
    assert_eq!(h.kernel().export(None).unwrap(), before);
    assert_eq!(height(&h, panel), 220.);
}

#[test]
fn runner_refuses_multiple_roots_before_registration() {
    let mut plan = contract::compile("component App\n  view\n    box height=100 box-sizing=\"border-box\" testId=\"a\"\n      box height=100 box-sizing=\"border-box\"\n").unwrap();
    plan.nodes[1].parent = None;
    plan.nodes[1].order = 1;
    assert!(matches!(
        Host::boot(
            &plan.encode(),
            Empty,
            Box::new(MonospaceMeasurer::default()),
            400.,
            300.
        ),
        Err(exact_linux::host::HostError::Runner(
            exact_runner::RunnerError::NotOneRoot(2)
        ))
    ));
}

#[test]
fn owner_switch_and_clear_retire_only_height_and_restore_latest_authoring() {
    let mut h = fixture();
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    let translate = h
        .hold_begin(panel, Property::Translate, 0.)
        .unwrap()
        .unwrap();
    h.hold_update(held.token, Value::scalar(230.), 0.).unwrap();
    h.hold_update(translate.token, Value::new(35., 0.), 0.)
        .unwrap();
    press(&mut h, "shrink", 1.);
    assert_eq!(height(&h, panel), 230.);
    let other = id(&h, "other");
    h.set_height_owner(Some(other)).unwrap();
    assert!(!h.has_hold(held.token));
    assert!(h.has_hold(translate.token));
    assert_eq!(height(&h, panel), 100.);
    assert!(!h
        .hold_update(held.token, Value::scalar(f64::NAN), f64::NAN)
        .unwrap());
    let second = h.hold_begin(other, Property::Height, 1.).unwrap().unwrap();
    h.hold_update(second.token, Value::scalar(120.), 1.)
        .unwrap();
    h.set_height_owner(None).unwrap();
    assert!(!h.has_hold(second.token));
    assert!(h.has_hold(translate.token));
    assert_eq!(height(&h, other), 70.);
}

#[test]
fn tick_projects_and_catch_uses_used_constrained_height_with_zero_displacement() {
    let mut h = fixture();
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    press(&mut h, "grow", 0.);
    h.tick(100.);
    assert!(height(&h, panel) > 180.);
    h.tick(1000.);
    assert_eq!(height(&h, panel), 300.);
    let held = h
        .hold_begin(panel, Property::Height, 1000.)
        .unwrap()
        .unwrap();
    assert_eq!(held.value, Value::scalar(300.));
    assert_eq!(
        h.engine().target(held.token.node(), Property::Height),
        Some(Value::scalar(420.))
    );
    h.hold_update(held.token, held.value, 1000.).unwrap();
    assert_eq!(height(&h, panel), 300.);
    h.hold_update(held.token, Value::scalar(held.value.x - 25.), 1010.)
        .unwrap();
    assert_eq!(height(&h, panel), 275.);
    assert_eq!(
        h.kernel().node(panel).unwrap().style.height,
        Dimension::Points(420.)
    );
    assert!(!h.motion(), "held-only layout is quiescent");
    press(&mut h, "shrink", 1010.);
    assert_eq!(height(&h, panel), 275.);
    h.hold_end(
        held.token,
        HoldEnd::Release {
            velocity: Value::scalar(-50.),
        },
        1010.,
    )
    .unwrap();
    assert!(h.motion());
    h.tick(10_000.);
    assert_eq!(height(&h, panel), 100.);
}

#[test]
fn hidden_and_deleted_owners_cannot_keep_stale_holds() {
    for action in ["hide", "toggle"] {
        let mut h = fixture();
        let panel = id(&h, "panel");
        h.set_height_owner(Some(panel)).unwrap();
        let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
        let translate = h
            .hold_begin(panel, Property::Translate, 0.)
            .unwrap()
            .unwrap();
        h.hold_update(held.token, Value::scalar(230.), 0.).unwrap();
        press(&mut h, action, 1.);
        assert_eq!(h.height_owner().is_none(), action == "toggle", "{action}");
        assert!(!h.has_hold(held.token));
        assert!(!h.hold_end(held.token, HoldEnd::Cancel, f64::NAN).unwrap());
        if action != "toggle" {
            assert!(h.has_hold(translate.token));
        } else {
            press(&mut h, "toggle", 2.);
            let replacement = id(&h, "panel");
            assert!(h
                .hold_begin(replacement, Property::Height, 2.)
                .unwrap()
                .is_none());
        }
    }
}

#[test]
fn resize_intrinsic_and_unrelated_commits_keep_held_projection() {
    let mut h = fixture();
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    h.hold_update(held.token, Value::scalar(250.), 0.).unwrap();
    assert!(h.resize(400., 200.).is_none());
    assert_eq!(height(&h, panel), 200.);
    assert!(h.set_intrinsic(id(&h, "image"), Some((60., 30.))).is_none());
    assert_eq!(height(&h, panel), 200.);
    press(&mut h, "grow", 0.);
    assert_eq!(height(&h, panel), 200.);
    assert!(h.resize(400., 500.).is_none());
    assert_eq!(height(&h, panel), 250.);
    assert_eq!(
        h.kernel().node(panel).unwrap().style.height,
        Dimension::Points(420.)
    );
}

#[test]
fn malformed_live_height_samples_are_atomic_and_stale_callbacks_do_nothing() {
    let mut h = fixture();
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let held = h.hold_begin(panel, Property::Height, 10.).unwrap().unwrap();
    let before = h.kernel().export(None).unwrap();
    for (value, now) in [
        (Value::scalar(-1.), 11.),
        (Value::scalar(-f64::MIN_POSITIVE), 11.),
        (Value::scalar(f64::NEG_INFINITY), 11.),
        (Value::scalar(f64::MAX), 11.),
        (Value::new(200., 1.), 11.),
        (Value::scalar(200.), 9.),
    ] {
        assert!(h.hold_update(held.token, value, now).is_err());
        assert_eq!(h.now(), 10.);
        assert_eq!(h.engine().now(), 0.01);
        assert_eq!(
            h.engine().value(held.token.node(), Property::Height),
            Some(held.value)
        );
        assert!(h.has_hold(held.token));
        assert_eq!(h.kernel().export(None).unwrap(), before);
    }
    let mut replacement = fixture();
    replacement
        .set_height_owner(Some(id(&replacement, "panel")))
        .unwrap();
    assert!(!replacement
        .hold_update(held.token, Value::scalar(f64::NAN), f64::NAN)
        .unwrap());
    assert_eq!(replacement.now(), 0.);
}

struct Counted(Rc<Cell<usize>>);
impl TextMeasurer for Counted {
    fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
        self.0.set(self.0.get() + 1);
        MonospaceMeasurer::default().measure(r)
    }
}
#[test]
fn translate_only_frames_do_not_relayout_or_remeasure() {
    let calls = Rc::new(Cell::new(0));
    let mut h = boot(APP, Box::new(Counted(calls.clone())));
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let held = h
        .hold_begin(panel, Property::Translate, 0.)
        .unwrap()
        .unwrap();
    h.hold_update(held.token, Value::new(80., 0.), 0.).unwrap();
    h.hold_end(held.token, HoldEnd::Cancel, 0.).unwrap();
    let bytes = h.kernel().export(None).unwrap();
    let count = calls.get();
    for i in 1..30 {
        assert!(!h.tick(i as f64 * 10.));
    }
    assert_eq!(calls.get(), count);
    assert_eq!(h.kernel().export(None).unwrap(), bytes);
}

#[test]
fn zero_target_negative_release_clamps_layout_without_changing_the_curve() {
    let mut h = boot(
        "component App\n  view\n    box width=400 height=300\n      box testId=\"panel\" position=\"absolute\" bottom=0 width=300 height=0 box-sizing=\"border-box\" transition=\"height spring(300,30,1)\"\n",
        Box::new(MonospaceMeasurer::default()),
    );
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    assert_eq!(held.value, Value::scalar(0.));
    h.hold_end(
        held.token,
        HoldEnd::Release {
            velocity: Value::scalar(-500.),
        },
        0.,
    )
    .unwrap();
    assert!(h.motion(), "nonzero velocity at the target must still run");
    h.tick(16.);
    let sample = h
        .engine()
        .value(held.token.node(), Property::Height)
        .unwrap();
    assert!(
        sample.x < 0.,
        "the engine curve must not be clamped: {sample:?}"
    );
    assert_eq!(height(&h, panel), 0.);
    assert_eq!(
        h.kernel().node(panel).unwrap().style.height,
        Dimension::Points(0.)
    );
    assert_eq!(
        h.engine().target(held.token.node(), Property::Height),
        Some(Value::scalar(0.))
    );
    // A resize at the negative sample must succeed, not merely preserve an old
    // zero frame after a rejected PresentedHeight.
    assert!(h.resize(400., 250.).is_none());
    assert_eq!(
        h.engine().value(held.token.node(), Property::Height),
        Some(sample)
    );
    assert!(!h.agent(r#"{"op":"logs"}"#).contains("layout:"));
    let caught = h.hold_begin(panel, Property::Height, 16.).unwrap().unwrap();
    assert_eq!(
        caught.value,
        Value::scalar(0.),
        "catch uses actual CSS height"
    );
    h.hold_end(caught.token, HoldEnd::Cancel, 16.).unwrap();
    h.tick(10_000.);
    assert_eq!(height(&h, panel), 0.);
    assert!(!h.motion());
}

#[test]
fn negative_release_lobe_clamps_only_height_presentation() {
    let mut h = boot(
        "component App\n  view\n    box width=400 height=300\n      box testId=\"panel\" width=300 height=20 box-sizing=\"border-box\" transition=\"height spring(180,12,1)\"\n",
        Box::new(MonospaceMeasurer::default()),
    );
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    let translate = h
        .hold_begin(panel, Property::Translate, 0.)
        .unwrap()
        .unwrap();
    h.hold_update(translate.token, Value::new(-50., 0.), 0.)
        .unwrap();
    assert!(h.hold_update(held.token, Value::scalar(-30.), 1.).is_err());
    assert_eq!(h.now(), 0.);
    assert_eq!(
        h.engine().value(held.token.node(), Property::Height),
        Some(Value::scalar(20.))
    );
    h.hold_end(
        held.token,
        HoldEnd::Release {
            velocity: Value::scalar(-2000.),
        },
        0.,
    )
    .unwrap();
    h.tick(50.);
    let sample = h
        .engine()
        .value(held.token.node(), Property::Height)
        .unwrap()
        .x;
    assert!((sample - -49.7162387436).abs() < 0.00001, "{sample}");
    assert_eq!(height(&h, panel), 0.);
    assert_eq!(h.presented(panel).translate, (-50., 0.));
    assert_eq!(
        h.engine().target(held.token.node(), Property::Height),
        Some(Value::scalar(20.))
    );
    assert!(!h.hold_end(held.token, HoldEnd::Cancel, f64::NAN).unwrap());
    assert!(h.resize(400., 250.).is_none());
    h.tick(10_000.);
    assert_eq!(height(&h, panel), 20.);
    assert_eq!(h.presented(panel).translate, (-50., 0.));
    assert!(h.has_hold(translate.token));
}

#[test]
fn inactive_retained_route_retires_hold_then_readopts_generation_on_return() {
    let mut h = boot(
        r#"component App
  state selected = "panel"
  action away
    selected = "away"
  action back
    selected = "panel"
  view
    main width=400 height=300 navigationKey=selected navigationBack="back"
      button press=away testId="away"
        text "away"
      button press=back testId="back"
        text "back"
      box navigationKey="panel"
        box testId="panel" height=180 box-sizing="border-box"
      box navigationKey="away"
        text "other"
"#,
        Box::new(MonospaceMeasurer::default()),
    );
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let key = h.height_owner();
    let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    h.hold_update(held.token, Value::scalar(250.), 0.).unwrap();
    press(&mut h, "away", 1.);
    assert_eq!(h.height_owner(), key);
    assert!(!h.has_hold(held.token));
    assert!(h
        .hold_begin(panel, Property::Height, f64::NAN)
        .unwrap()
        .is_none());
    assert!(!h.hold_update(held.token, Value::scalar(300.), 10.).unwrap());
    assert_eq!(h.now(), 1.);
    press(&mut h, "back", 2.);
    assert_eq!(h.height_owner(), key);
    let new = h.hold_begin(panel, Property::Height, 2.).unwrap().unwrap();
    assert_ne!(held.token, new.token);
    assert_eq!(new.value, Value::scalar(180.));
}

#[test]
fn overdue_height_timer_retargets_held_layout_without_rewinding_engine() {
    let mut h = boot(
        r#"component App
  state target = 180
  action step
    target = target + 10
  task ticker mount
    every(100, step)
  view
    box width=400 height=300
      box height=target box-sizing="border-box" transition="height spring(300,30,1)" testId="panel"
"#,
        Box::new(MonospaceMeasurer::default()),
    );
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let held = h
        .hold_begin(panel, Property::Height, 150.)
        .unwrap()
        .unwrap();
    h.hold_update(held.token, Value::scalar(250.), 150.)
        .unwrap();
    assert!(h.advance(250.).is_none());
    assert_eq!(h.now(), 250.);
    assert_eq!(h.engine().now(), 0.25);
    assert_eq!(height(&h, panel), 250.);
    assert_eq!(
        h.engine().target(held.token.node(), Property::Height),
        Some(Value::scalar(200.))
    );
    h.hold_end(held.token, HoldEnd::Cancel, 250.).unwrap();
    h.tick(10_000.);
    assert_eq!(height(&h, panel), 200.);
}

#[test]
fn negative_spring_height_catch_uses_padding_border_and_minimum_constraints() {
    for (minimum, expected) in [(0, 24.), (50, 50.)] {
        let source = format!("component App\n  view\n    box width=400 height=300\n      box testId=\"panel\" height=20 min-height={minimum} padding=10 border-width=2 border-style=\"solid\" box-sizing=\"border-box\" transition=\"height spring(180,12,1)\"\n");
        let mut h = boot(&source, Box::new(MonospaceMeasurer::default()));
        let panel = id(&h, "panel");
        h.set_height_owner(Some(panel)).unwrap();
        let old = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
        h.hold_end(
            old.token,
            HoldEnd::Release {
                velocity: Value::scalar(-2000.),
            },
            0.,
        )
        .unwrap();
        h.tick(50.);
        assert_eq!(height(&h, panel), expected);
        assert!(
            h.engine()
                .value(old.token.node(), Property::Height)
                .unwrap()
                .x
                < 0.
        );
        let new = h.hold_begin(panel, Property::Height, 50.).unwrap().unwrap();
        assert_eq!(new.value, Value::scalar(expected as f64));
        assert_eq!(height(&h, panel), expected);
        assert!(!h.hold_update(old.token, Value::scalar(300.), 1.).unwrap());
        assert_eq!(h.now(), 50.);
        assert_eq!(
            h.engine().target(new.token.node(), Property::Height),
            Some(Value::scalar(20.))
        );
    }
}
