use super::*;

fn lowered(m: &mut Motion, now: f64) -> Vec<f64> {
    let mut out = Vec::new();
    m.lower(now, &mut out).unwrap();
    out
}

#[test]
fn a_released_hold_springs_home_as_frames_once() {
    let mut m = Motion::new();
    assert!(m.transitions(7, "translate spring(180, 12, 1)"));
    m.observe(7, [0.0, 0.0, 1.0, 0.0, 1.0], 0.0).unwrap();
    assert!(
        lowered(&mut m, 0.0).is_empty(),
        "a first observation is taken as it is"
    );
    let (serial, value) = m
        .begin(7, Property::Translate, Value::new(0.0, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(value, Value::new(0.0, 0.0));
    for i in 1..6 {
        assert!(m
            .update(
                serial,
                Value::new(20.0 * i as f64, 0.0),
                0.1 + 0.016 * i as f64
            )
            .unwrap());
    }
    assert!(m.measured(serial, 0.18).x > 500.0);
    let velocity = m.measured(serial, 0.18);
    assert!(m.end(serial, Some(velocity), 0.18).unwrap());
    assert!(!m.live(serial));
    let ops = lowered(&mut m, 0.18);
    assert_eq!(
        ops[..4],
        [1.0, 7.0, 0.0, 0.18],
        "a translate spring on 7 at 0.18 s"
    );
    let n = ops[6] as usize;
    assert_eq!(ops.len(), 7 + 2 * n);
    assert_eq!(
        ops[ops.len() - 2..],
        [0.0, 0.0],
        "the last frame is the target"
    );
    assert!(
        lowered(&mut m, 0.2).is_empty(),
        "an unchanged spring is not compiled again"
    );
    assert!(m.settle_time().is_some());
}

#[test]
fn an_unknown_node_takes_no_hold_and_a_removed_one_forgets_its_holds() {
    let mut m = Motion::new();
    assert!(m
        .begin(3, Property::Translate, Value::ZERO, 0.0)
        .unwrap()
        .is_none());
    m.observe(3, [5.0, 0.0, 1.0, 0.0, 1.0], 0.0).unwrap();
    let (serial, _) = m
        .begin(3, Property::Translate, Value::new(5.0, 0.0), 0.0)
        .unwrap()
        .unwrap();
    assert_eq!(m.held(serial), Some((3, Property::Translate)));
    m.remove(3);
    assert!(!m.live(serial));
    assert!(!m.update(serial, Value::ZERO, 0.1).unwrap());
}

#[test]
fn a_new_target_under_an_eased_transition_lowers_nothing() {
    let mut m = Motion::new();
    assert!(m.transitions(1, "translate 300ms ease-out"));
    m.observe(1, [0.0, 0.0, 1.0, 0.0, 1.0], 0.0).unwrap();
    m.observe(1, [100.0, 0.0, 1.0, 0.0, 1.0], 0.5).unwrap();
    assert!(
        lowered(&mut m, 0.5).is_empty(),
        "the browser plays CSS transitions"
    );
    assert!(
        !m.transitions(1, "translate bogus"),
        "refused text is no transition"
    );
}

#[test]
fn a_pan_releases_at_its_speed_once() {
    let mut m = Motion::new();
    for i in 0..6 {
        m.pan_sample(
            7,
            i == 0,
            100.0 + 20.0 * i as f64,
            50.0,
            1000.0 + 16.0 * i as f64,
        );
    }
    let v = m.pan_release(7, 1080.0);
    assert!((v.x - 1250.0).abs() < 1.0, "{v:?}");
    assert_eq!(m.pan_release(7, 1080.0), Value::ZERO);
}

#[test]
fn a_held_height_springs_to_its_new_target() {
    let mut m = Motion::new();
    assert!(m.transitions(4, "height spring(300, 30, 1)"));
    m.height(4, 400.0, 0.0).unwrap();
    let (serial, value) = m
        .begin(4, Property::Height, Value::scalar(400.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(value, Value::scalar(400.0));
    assert!(m.update(serial, Value::scalar(300.0), 0.2).unwrap());
    m.height(4, 640.0, 0.2).unwrap();
    assert!(m.end(serial, Some(Value::ZERO), 0.2).unwrap());
    let ops = lowered(&mut m, 0.2);
    assert_eq!(ops[..3], [1.0, 4.0, 4.0], "a height spring on 4");
    assert_eq!(ops[ops.len() - 2], 640.0, "to the new target");
    assert!(m.retire_height(4));
}

#[test]
fn a_transform_pair_moves_together_and_ends_apart() {
    let mut m = Motion::new();
    assert!(m.transitions(9, "translate spring(200, 20, 1), scale spring(200, 20, 1)"));
    m.observe(9, [0.0, 0.0, 1.0, 0.0, 1.0], 0.0).unwrap();
    let (t, s, v) = m.begin_pair(9, [0.0, 0.0, 1.0], 0.1).unwrap().unwrap();
    assert_eq!(v, [0.0, 0.0, 1.0]);
    assert!(m.update_pair(t, [30.0, 10.0, 2.0], 0.2).unwrap());
    assert_eq!(m.held(t), Some((9, Property::Translate)));
    assert_eq!(m.held(s), Some((9, Property::Scale)));
    assert!(m.end(t, Some(Value::ZERO), 0.2).unwrap());
    assert!(
        !m.update_pair(t, [0.0, 0.0, 1.0], 0.3).unwrap(),
        "half a pair is no pair"
    );
    assert!(m.end(s, None, 0.3).unwrap());
}
