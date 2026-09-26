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
        settled.iter().filter(|i| !now.contains(i)).count(),
        4,
        "retirement is bounded"
    );
    assert!(
        !now.contains(&90) && now.contains(&94),
        "the farthest rows retire first"
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
    };
    h.send_filled(h.feedback(3200.0), up);
    let now = rows(&h);
    assert!((100..110).all(|i| now.contains(&i)));
    assert_eq!(now.first(), Some(&99), "{now:?}");
}
