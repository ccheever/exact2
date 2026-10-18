use super::*;
use exact_kernel::{motion::motion_node, MonospaceMeasurer, TransformDragBinding, ViewId};
use exact_motion::Value;
use exact_runner::Event;

fn playing(transition: &str) -> (Host<()>, TransformDragBinding, ViewId) {
    let source = r#"component App
  state x = 0
  state y = 0
  state zoom = 1
  state transition = "TRANSITION"
  action geometry(w: number, h: number, pw: number, ph: number)
    x = x
  action release(x: number, y: number, s: number, vx: number, vy: number, vs: number)
    x = x
  action move
    x = 100
    y = -40
    zoom = 3
  action snap
    transition = "none"
    x = 50
    y = -20
    zoom = 2
  view
    box width=400 height=500
      box width=200 height=150 overflow="hidden"
        box id="target" testId="target" width="100%" height="100%" box-sizing="border-box" translate=`${x}px ${y}px` scale=zoom transition=transition
          box testId="handle" height=30 transformDragFor="target" transformgeometry=geometry transformrelease=release
      button testId="move" press=move
        text "Move"
      button testId="snap" press=snap
        text "Snap"
"#
    .replace("TRANSITION", transition);
    let (mut h, error) = Host::boot(
        &contract::compile(&source).unwrap().encode(),
        (),
        Box::new(MonospaceMeasurer::default()),
        400.,
        500.,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    h.lowering = true;
    let handle = h.kernel().find_by_test_id("handle")[0];
    let binding = h.transform_drag_binding(handle).unwrap();
    let target = h.kernel().node_by_key(binding.target).unwrap().id;
    let button = h.kernel().find_by_test_id("move")[0];
    let button = h.kernel().node_by_key(button).unwrap().id;
    assert!(h.dispatch_at(button, Event::Press, 0.).is_none());
    assert!(h
        .engine
        .is_played(motion_node(binding.target), Property::Translate));
    assert_eq!(h.lowered_mask(binding.target), LOWER_TRANSFORM);
    assert_eq!(h.presented(target).translate, (100., -40.));
    (h, binding, target)
}

#[test]
fn holds_retire_reader_tracks_before_presentation() {
    let (mut h, binding, target) = playing("translate 1s linear, scale 1s linear");
    let node = motion_node(binding.target);
    let epoch = h.lowered_epoch;
    let held = h
        .hold_begin(target, Property::Translate, 250.)
        .unwrap()
        .unwrap();
    assert_eq!(held.value, Value::new(25., -10.));
    assert!(!h.engine.is_played(node, Property::Translate));
    assert_eq!(
        h.played[&node].len(),
        1,
        "only scale still belongs to the reader"
    );
    assert_eq!(h.played[&node][0].0, Property::Scale);
    assert_eq!(h.lowered_mask(binding.target), LOWER_TRANSFORM);
    assert!(h.lowered_changed[&node] > epoch);

    let epoch = h.lowered_epoch;
    let held = h
        .hold_begin(target, Property::Scale, 250.)
        .unwrap()
        .unwrap();
    assert_eq!(held.value, Value::scalar(1.5));
    assert!(!h.played.contains_key(&node));
    assert_eq!(h.lowered_mask(binding.target), 0);
    assert!(h.lowered_changed[&node] > epoch);
    assert_eq!(h.presented(target).translate, (25., -10.));
    assert_eq!(h.presented(target).scale, 1.5);
}

#[test]
fn transform_drag_catches_reader_easing() {
    for (transition, at, progress) in [
        ("all 1s linear", 250., 0.25),
        ("all 1s linear 500ms", 250., 0.),
        ("all 1s linear 500ms", 750., 0.25),
        ("all 0s linear 500ms", 250., 0.),
        ("all 1s steps(5, jump-end)", 250., 0.2),
        ("all 1s linear", 1250., 1.),
    ] {
        let (mut h, binding, target) = playing(transition);
        let node = motion_node(binding.target);
        let epoch = h.lowered_epoch;
        let held = h.transform_drag_begin(binding, at).unwrap().unwrap();
        assert_eq!(
            held.translate().value,
            Value::new(100. * progress, -40. * progress),
            "{transition} at {at}"
        );
        assert_eq!(held.scale().value, Value::scalar(1. + 2. * progress));
        assert_eq!(
            h.presented(target).translate,
            ((100. * progress) as f32, (-40. * progress) as f32)
        );
        assert_eq!(h.presented(target).scale, (1. + 2. * progress) as f32);
        assert!(!h.engine.is_played(node, Property::Translate));
        assert!(!h.engine.is_played(node, Property::Scale));
        assert!(!h.played.contains_key(&node));
        assert_eq!(h.lowered_mask(binding.target), 0);
        assert!(h.lowered_changed[&node] > epoch);
    }
}

#[test]
fn transform_drag_catches_reader_spring_frames() {
    let (mut h, binding, target) = playing("translate -exact-spring(300,30,1)");
    let node = motion_node(binding.target);
    let track = &h.played[&node][0].1;
    let PlayedCurve::Frames { duration, values } = &track.curve else {
        panic!("the reader plays a spring as frames");
    };
    let at = (track.start + duration * 5.25 / (values.len() - 1) as f64) * 1000.;
    let expected = values[5].zip(values[6], |a, b| a * 0.75 + b * 0.25);
    let held = h.transform_drag_begin(binding, at).unwrap().unwrap();
    assert!((held.translate().value.x - expected.x).abs() < 1e-9);
    assert!((held.translate().value.y - expected.y).abs() < 1e-9);
    assert_eq!(held.scale().value, Value::scalar(3.));
    assert_eq!(
        h.presented(target).translate,
        (expected.x as f32, expected.y as f32)
    );
    assert!(!h.played.contains_key(&node));
    assert_eq!(h.lowered_mask(binding.target), 0);
}

#[test]
fn snapping_a_played_transition_retires_its_reader_tracks() {
    let (mut h, binding, target) = playing("translate 1s linear, scale 1s linear");
    let node = motion_node(binding.target);
    let epoch = h.lowered_epoch;
    let button = h.kernel().find_by_test_id("snap")[0];
    let button = h.kernel().node_by_key(button).unwrap().id;
    assert!(h.dispatch_at(button, Event::Press, 250.).is_none());
    assert!(!h.engine.is_played(node, Property::Translate));
    assert!(!h.engine.is_played(node, Property::Scale));
    assert!(!h.played.contains_key(&node));
    assert_eq!(h.lowered_mask(binding.target), 0);
    assert!(h.lowered_changed[&node] > epoch);
    assert_eq!(h.presented(target).translate, (50., -20.));
    assert_eq!(h.presented(target).scale, 2.);
}
