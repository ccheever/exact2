//! LLP 1070.000 §6.2: which corrections a collection asks the host to
//! animate.
use super::*;

/// LLP 1070.000 §6.2: a `scroll-behavior: smooth` list following its end
/// asks the host to animate the follow; one that is not smooth, or an
/// anchor kept in place, does not.
#[test]
fn a_smooth_list_following_its_end_asks_for_an_animated_correction() {
    for smooth in [true, false] {
        let plan = plan_with(100, false, true, smooth);
        let slots = vec![
            values(100),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(16.0),
            Value::Unit,
        ];
        let mut h = Harness::from_parts(plan, slots);
        h.send(h.feedback(2880.0));
        h.slots[0] = values(101);
        h.update().unwrap();
        let c = h.snapshot().correction.unwrap();
        assert_eq!(c.offset, 2912.0);
        assert_eq!(c.smooth, smooth, "smooth={smooth}");
        assert!(c.from.is_none());
    }
    // A reader away from the end keeps its row, never animated.
    let plan = plan_with(100, false, true, true);
    let slots = vec![
        values(100),
        Value::Number(0.0),
        Value::Number(0.0),
        Value::Number(16.0),
        Value::Unit,
    ];
    let mut h = Harness::from_parts(plan, slots);
    h.send(h.feedback(642.0));
    h.slots[0] = Value::list(
        std::iter::once(Value::Number(-1.0))
            .chain((0..100).map(|i| Value::Number(i as f64)))
            .collect(),
    );
    h.update().unwrap();
    assert!(!h.snapshot().correction.unwrap().smooth);
}

/// A host rounds its port to device pixels: one a sixth of a point short of
/// a followed end (4405.1667 shows as 4405.333 at 3x) is at it. A commit
/// that changes nothing asks for no correction there, so none stays owed
/// and a later message's follow is still the smooth one.
#[test]
fn a_port_rounded_to_a_device_pixel_is_at_its_followed_end() {
    let plan = plan_with(100, false, true, true);
    let slots = vec![
        values(100),
        Value::Number(0.0),
        Value::Number(0.0),
        Value::Number(16.0),
        Value::Unit,
    ];
    let mut h = Harness::from_parts(plan, slots);
    h.send(h.feedback(2880.0 - 1.0 / 6.0));
    h.slots[1] = Value::Number(1.0);
    h.update().unwrap();
    assert!(
        h.snapshot().correction.is_none(),
        "no correction for a sixth of a point"
    );
    h.slots[0] = values(101);
    h.update().unwrap();
    let c = h.snapshot().correction.unwrap();
    assert_eq!(c.offset, 2912.0);
    assert!(c.smooth);
}

/// The same where the extent itself ends on half a pixel: a row measured at
/// 32 1/6 puts the end at 2880.1667, which a 3x host shows as 2880.333.
#[test]
fn a_half_pixel_extent_is_reached_by_a_rounded_port() {
    let plan = plan_with(100, false, true, true);
    let slots = vec![
        values(100),
        Value::Number(0.0),
        Value::Number(0.0),
        Value::Number(16.0),
        Value::Unit,
    ];
    let mut h = Harness::from_parts(plan, slots);
    h.send(h.feedback(2880.0));
    let last = h
        .snapshot()
        .rows
        .into_iter()
        .max_by_key(|r| r.index)
        .unwrap();
    let mut measured = h.feedback(2880.0);
    measured.measurements = vec![RowMeasurement {
        view: last.view,
        epoch: last.epoch,
        size: 32.0 + 1.0 / 6.0,
    }];
    h.send(measured);
    let end = h.snapshot().total_extent - 320.0;
    assert!((end - (2880.0 + 1.0 / 6.0)).abs() < 1e-9);
    // The host lands on the nearest third of a point, past the end.
    h.send(h.feedback(2880.0 + 1.0 / 3.0));
    h.slots[1] = Value::Number(1.0);
    h.update().unwrap();
    assert!(
        h.snapshot().correction.is_none(),
        "a port half a pixel off its end is at it"
    );
    h.slots[0] = values(101);
    h.update().unwrap();
    let c = h.snapshot().correction.unwrap();
    assert!(c.smooth, "a later message still follows smoothly");
}
