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
