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
/// a followed end it was sent (4405.1667 shows as 4405.333 at 3x) is at it.
/// A commit that changes nothing asks for no correction there, so none stays
/// owed and a later message's follow is still the smooth one.
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
    // Sent once; the host lands on its pixel, a sixth short, and that is there.
    assert_eq!(h.snapshot().correction.map(|c| c.offset), Some(2880.0));
    h.send(h.feedback(2880.0 - 1.0 / 6.0));
    h.slots[1] = Value::Number(2.0);
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

/// A row anchor keeps the tight test: a row above it measured 0.4 pt taller
/// moves the port 0.4, and the next 0.4 again (half a point would drop each,
/// and they add up report on report).
#[test]
fn a_row_anchors_small_moves_still_correct() {
    let mut h = Harness::new(100, false, false);
    h.send(h.feedback(642.0));
    let mut offset = 642.0;
    for _ in 0..2 {
        let above = h
            .snapshot()
            .rows
            .into_iter()
            .filter(|r| (r.index as f64 + 1.0) * 32.0 <= 642.0)
            .min_by_key(|r| r.index)
            .expect("a mounted row above the port");
        let mut f = h.feedback(offset);
        let grown = h
            .snapshot()
            .rows
            .iter()
            .filter(|r| r.view == above.view)
            .count();
        assert_eq!(grown, 1);
        f.measurements = vec![RowMeasurement {
            view: above.view,
            epoch: above.epoch,
            size: 32.4 + (offset - 642.0),
        }];
        h.send(f);
        let c = h.snapshot().correction.expect("a 0.4 pt move is corrected");
        assert!(
            (c.offset - (offset + 0.4)).abs() < 1e-9,
            "{} after {}",
            c.offset,
            offset
        );
        offset = c.offset;
    }
}

/// A list that opens at its end, whose end lands on half a pixel, opens
/// when a 3x host's port rounds past it, and its next message then follows
/// smoothly (an opening never settled made every follow a jump).
#[test]
fn an_opening_at_a_half_pixel_end_settles_and_then_follows_smoothly() {
    let mut h = Harness::from_parts(
        plan_opening(100, false, true, true, true, false),
        vec![
            values(100),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(16.0),
            Value::Unit,
        ],
    );
    let end = h.snapshot().total_extent - 320.0;
    h.send(h.feedback(end));
    let report = |h: &Harness, top: f64| {
        let mut f = h.feedback(top);
        let last = h.snapshot().rows.iter().map(|r| r.index).max().unwrap();
        f.measurements = h
            .snapshot()
            .rows
            .iter()
            .map(|r| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                size: if r.index == last {
                    32.0 + 1.0 / 6.0
                } else {
                    32.0
                },
            })
            .collect();
        f
    };
    for _ in 0..4 {
        let top = h.snapshot().total_extent - 320.0;
        // The host's port, rounded to a third of a point.
        let shown = (top * 3.0).round() / 3.0;
        let f = report(&h, shown);
        h.send(f);
    }
    assert!(
        ((h.snapshot().total_extent - 320.0) * 3.0).fract().abs() > 0.1,
        "the end is on half a pixel"
    );
    h.slots[0] = values(101);
    h.update().unwrap();
    let c = h.snapshot().correction.expect("the new row is followed");
    assert!(c.smooth, "the opening settled, so the follow is smooth");
}

/// A followed end that moves by less than half a point is still sent, once:
/// the last row measured 32.4, then 32.8, keeps the port on the end, and a
/// new row after it is followed. On a row list a card's re-measurement lands
/// before the anchor is taken, so both axes.
#[test]
fn an_end_moved_by_less_than_half_a_point_is_still_followed() {
    for horizontal in [false, true] {
        let mut h = Harness::from_parts(
            plan_opening(100, false, true, false, false, horizontal),
            vec![
                values(100),
                Value::Number(0.0),
                Value::Number(0.0),
                Value::Number(16.0),
                Value::Unit,
            ],
        );
        h.send(h.feedback(2880.0));
        let mut offset = 2880.0;
        for size in [32.4, 32.8] {
            let last = h
                .snapshot()
                .rows
                .into_iter()
                .max_by_key(|r| r.index)
                .unwrap();
            let mut f = h.feedback(offset);
            f.measurements = h
                .snapshot()
                .rows
                .iter()
                .map(|r| RowMeasurement {
                    view: r.view,
                    epoch: r.epoch,
                    size: if r.view == last.view { size } else { 32.0 },
                })
                .collect();
            h.send(f);
            let end = h.snapshot().total_extent - 320.0;
            let c = h.snapshot().correction.expect("the moved end is sent");
            assert!(
                (c.offset - end).abs() < 1e-9,
                "horizontal={horizontal}: {} vs {end}",
                c.offset
            );
            // The host lands on its pixel.
            offset = (end * 3.0).round() / 3.0;
        }
        h.send(h.feedback(offset));
        h.slots[0] = values(101);
        h.update().unwrap();
        let end = h.snapshot().total_extent - 320.0;
        assert_eq!(
            h.snapshot().correction.map(|c| c.offset),
            Some(end),
            "horizontal={horizontal}"
        );
    }
}

#[test]
fn dom_integer_end_append_follows_but_a_reader_a_pixel_up_does_not() {
    for (at_end, follow) in [(true, true), (false, true), (true, false)] {
        let mut h = Harness::new(2, false, follow);
        let mut initial = h.feedback(0.0);
        initial.port_main = 519.0;
        h.send(initial); // establish width before accepting measurements
        let mut measured = h.feedback(0.0);
        measured.port_main = 519.0;
        measured.measurements = h
            .snapshot()
            .rows
            .iter()
            .zip([335_080.0, 297.078_125])
            .map(|(r, height)| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                size: height,
            })
            .collect();
        h.send(measured);
        assert_eq!(h.snapshot().total_extent, 335_377.078_125);
        let top = if at_end {
            334_858.0
        } else {
            // A whole pixel (END_SLACK) short of the true end: a reader, not a clamp.
            334_858.078_125 - 1.0
        };
        let mut tail = h.feedback(top);
        tail.port_main = 519.0;
        let sequence = tail.scroll_sequence;
        h.send(tail.clone());
        h.slots[0] = values(3);
        h.update().unwrap();
        let appended = h.snapshot();
        if at_end && follow {
            let correction = appended.correction.expect("DOM end must follow append");
            assert_eq!(correction.scroll_sequence, sequence);
            assert_eq!(correction.offset, appended.total_extent - 519.0);
        } else {
            assert!(appended.correction.is_none());
        }
        assert!(
            !h.send(tail),
            "old revision must not replace the accepted append"
        );
        assert_eq!(h.snapshot(), appended);
        let row = appended.rows.iter().find(|r| r.index == 2).unwrap();
        let mut growth = h.feedback(if at_end && follow {
            (appended.total_extent - 519.0).round()
        } else {
            top
        });
        growth.port_main = 519.0;
        growth.measurements = vec![RowMeasurement {
            view: row.view,
            epoch: row.epoch,
            size: 134.593_75,
        }];
        h.send(growth);
        let grown = h.snapshot();
        assert_eq!(grown.total_extent, 335_511.671_875);
        if at_end && follow {
            assert_eq!(grown.correction.unwrap().offset, grown.total_extent - 519.0);
        } else {
            assert!(grown.correction.is_none());
        }
    }
}
