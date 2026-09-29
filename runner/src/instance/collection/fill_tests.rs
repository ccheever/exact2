//! Limited reports (@ref LLP 1050.000 §6): owed rows, lead order, retirement.
use super::*;

#[test]
fn a_limited_report_builds_what_it_owes_then_leads_in_travel_order() {
    let mut h = Harness::new(1_000, false, false);
    let rows = |h: &Harness| {
        h.snapshot()
            .rows
            .iter()
            .map(|r| r.index)
            .collect::<Vec<_>>()
    };
    h.send(h.feedback(3200.0));
    let settled = rows(&h);
    assert!(!h.snapshot().pending);
    assert!(settled.contains(&90) && settled.contains(&119) && !settled.contains(&120));
    // Two viewports down at 1,600 px/s, with room for two rows past what shows.
    let down = CollectionFill {
        velocity: 1_600.0,
        limit: Some(2),
        ..Default::default()
    };
    h.send_filled(h.feedback(3840.0), down);
    let now = rows(&h);
    assert!(
        (120..130).all(|i| now.contains(&i)),
        "the visible rows are owed: {now:?}"
    );
    assert_eq!(
        now.iter()
            .filter(|&&i| i >= 130)
            .copied()
            .collect::<Vec<_>>(),
        [130, 131]
    );
    assert_eq!(
        settled
            .iter()
            .copied()
            .filter(|i| !now.contains(i))
            .collect::<Vec<_>>(),
        (90..99).collect::<Vec<_>>(),
        "rows two viewports past what shows retire at once, past the cap"
    );
    assert!(h.snapshot().pending);
    assert!(
        now.windows(2).all(|w| w[0] < w[1]),
        "rows stay in logical order"
    );
    // Continuations with the same facts finish the led window, then stop.
    let mut reports = 0;
    while h.snapshot().pending {
        let mut f = h.feedback(3840.0);
        f.scroll_sequence -= 1;
        h.send_filled(f, down);
        reports += 1;
        assert!(reports < 40);
    }
    let led = rows(&h);
    let last = *led.last().unwrap();
    // A quarter second of travel (400 px) past the viewport's overscan.
    assert!((152..=153).contains(&last), "{led:?}");
    assert!(*led.first().unwrap() >= 109, "{led:?}");
    // Upward travel leads the other way: rows above are built first.
    let up = CollectionFill {
        velocity: -1_600.0,
        limit: Some(1),
        ..Default::default()
    };
    h.send_filled(h.feedback(3200.0), up);
    let now = rows(&h);
    assert!((100..110).all(|i| now.contains(&i)));
    assert_eq!(now.first(), Some(&99), "{now:?}");
}

#[test]
fn travel_inside_the_realized_window_moves_only_the_geometry() {
    let mut h = Harness::new(1_000, false, false);
    // A host reports every mounted row's height, every time.
    let report = |h: &Harness, top: f64, grow: f64| {
        let mut f = h.feedback(top);
        let rows = h.snapshot().rows;
        f.measurements = rows
            .iter()
            .map(|r| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                size: r.size + if r.index == rows[0].index { grow } else { 0.0 },
            })
            .collect();
        f
    };
    h.send(h.feedback(3216.0));
    h.send(report(&h, 3216.0, 0.0));
    let before = h.snapshot();
    let children = h.collection().children.clone();
    // Rows are 32 px: every top in (3200, 3232) leads to the same window.
    for top in [3217.0, 3220.5, 3231.0] {
        assert!(
            !h.send(h.feedback(top)),
            "no row, extent or correction changes"
        );
        assert!(
            !h.send(report(&h, top, 0.0)),
            "the same heights again change nothing"
        );
    }
    let after = h.snapshot();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.rows, before.rows);
    assert_eq!(h.collection().children, children);
    assert_eq!(h.collection().geometry.as_ref().unwrap().offset, 3231.0);
    // A new height is a measurement: the window realizes again.
    assert!(h.send(report(&h, 3231.0, 8.0)));
    assert_ne!(h.snapshot().revision, before.revision);
    // Past the window, a report realizes as one sent there directly does.
    let mut fresh = Harness::new(1_000, false, false);
    fresh.send(fresh.feedback(3216.0));
    fresh.send(report(&fresh, 3216.0, 0.0));
    fresh.send(report(&fresh, 3231.0, 8.0));
    h.send(h.feedback(3400.0));
    fresh.send(fresh.feedback(3400.0));
    let rows = |s: CollectionSnapshot| {
        s.rows
            .iter()
            .map(|r| (r.index, r.start, r.size))
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(h.snapshot()), rows(fresh.snapshot()));
}

#[test]
fn a_limited_report_retires_nearer_rows_at_its_cap() {
    let mut h = Harness::new(1_000, false, false);
    let rows = |h: &Harness| {
        h.snapshot()
            .rows
            .iter()
            .map(|r| r.index)
            .collect::<Vec<_>>()
    };
    h.send(h.feedback(3200.0));
    let settled = rows(&h);
    // One viewport down: rows 90..99 leave the window, none of them two
    // viewports away, so a report that may build two retires four.
    let down = CollectionFill {
        velocity: 1_600.0,
        limit: Some(2),
        ..Default::default()
    };
    h.send_filled(h.feedback(3520.0), down);
    let now = rows(&h);
    assert_eq!(
        settled.iter().filter(|i| !now.contains(i)).count(),
        4,
        "retirement is bounded"
    );
    assert!(
        !now.contains(&90) && now.contains(&94),
        "the farthest rows retire first"
    );
    assert!(h.snapshot().pending);
}

#[test]
fn rows_kept_past_the_window_never_outnumber_the_window() {
    let mut h = Harness::new(5_000, false, false);
    h.send(h.feedback(3200.0));
    let settled = h.snapshot().rows.len();
    // Travel that outruns the fill: every report is a rescue (limit 0),
    // and each lands a viewport and more past the last.
    let rescue = CollectionFill {
        velocity: 48_000.0,
        limit: Some(0),
        ..Default::default()
    };
    for step in 1..=40 {
        let top = 3200.0 + 400.0 * step as f64;
        h.send_filled(h.feedback(top), rescue);
        let rows: Vec<usize> = h.snapshot().rows.iter().map(|r| r.index).collect();
        let first = (top / 32.0) as usize;
        assert!(
            (first..first + 10).all(|i| rows.contains(&i)),
            "the visible rows are owed: {rows:?}"
        );
        assert!(
            rows.len() <= 2 * settled,
            "report {step}: {} rows mounted, the window settled at {settled}",
            rows.len()
        );
    }
    // At rest an unlimited report retires everything past the window.
    h.send(h.feedback(3200.0 + 400.0 * 40.0));
    assert_eq!(h.snapshot().rows.len(), settled);
}

/// @ref LLP 1072 §5 — a build-only report then a retire-only one leave the
/// rows one immediate report leaves, and the build-only one destroys none.
#[test]
fn a_build_only_report_then_a_retire_only_one_equal_one_immediate_report() {
    let rows = |h: &Harness| {
        h.snapshot()
            .rows
            .iter()
            .map(|r| r.index)
            .collect::<Vec<_>>()
    };
    let down = CollectionFill {
        velocity: 1_600.0,
        limit: Some(2),
        ..Default::default()
    };
    let mut once = Harness::new(1_000, false, false);
    let mut split = Harness::new(1_000, false, false);
    for h in [&mut once, &mut split] {
        h.send(h.feedback(3200.0));
    }
    for top in [3840.0, 4400.0, 5000.0] {
        let before = rows(&split);
        once.send_filled(once.feedback(top), down);
        split.send_filled(
            split.feedback(top),
            CollectionFill {
                create_only: true,
                ..down
            },
        );
        let built = rows(&split);
        assert!(
            before.iter().all(|i| built.contains(i)),
            "a build-only report destroys nothing: {before:?} -> {built:?}"
        );
        assert!(split.snapshot().pending, "it leaves the retirement owed");
        let mut f = split.feedback(top);
        f.scroll_sequence -= 1;
        split.send_filled(
            f,
            CollectionFill {
                no_build: true,
                ..down
            },
        );
        assert_eq!(rows(&split), rows(&once), "at {top}");
    }
}

#[test]
fn a_build_only_report_that_changes_the_port_is_refused() {
    let mut h = Harness::new(1_000, false, false);
    h.send(h.feedback(3200.0));
    let mut f = h.feedback(3300.0);
    f.port_main += 10.0;
    let bytes = f
        .encode_with(CollectionFill {
            create_only: true,
            ..Default::default()
        })
        .unwrap();
    assert!(CollectionFeedback::decode_with_fill(&bytes).is_ok());
    assert!(h
        .try_send_filled(
            f,
            CollectionFill {
                create_only: true,
                ..Default::default()
            }
        )
        .is_err());
}
