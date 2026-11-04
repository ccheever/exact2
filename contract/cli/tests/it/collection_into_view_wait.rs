//! LLP 1070.000 §2.6: a `scrollIntoView` that meets the reader's scroll on a
//! host whose jump would not end it (a browser, AppKit) waits for the scroll
//! to end. Meanwhile the reader's window follows the port, nothing is
//! corrected or counted, and a gesture begun since cancels it; when the
//! scroll ends it lands from the layout as it is then.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    CollectionFeedback, CollectionFill, CollectionSnapshot, DataError, DataSource, RowMeasurement,
    Runner,
};

/// Eleven rows 100 tall in a port 100 tall: row `n` starts at `100 n`.
const APP: &str = r#"component App
  state ids = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
  action go(n: number)
    scrollIntoView("feed", n, block="start")
  action drop(n: number)
    ids = ids.filter(i => i != n)
  action add(n: number)
    ids = ids.concat([n])
  action glide(n: number)
    scrollIntoView("feed", n, block="start", behavior="smooth")
  view
    list id="feed" virtualized=true height=100 width=400 estimated-item-height=100
      for i in ids key=i
        text `${i}` height=100
"#;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}
fn boot() -> (Runner<NoData>, u32) {
    boot_app(APP)
}
fn boot_app(app: &str) -> (Runner<NoData>, u32) {
    let mut r = Runner::boot(
        contract::compile(app).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let feed = r.collections()[0].view;
    // At rest at the start, every row measured at its size.
    for _ in 0..4 {
        report(&mut r, feed, 0.0, CollectionFill::default());
    }
    (r, feed)
}
fn snapshot(r: &Runner<NoData>, view: u32) -> CollectionSnapshot {
    r.collections()
        .into_iter()
        .find(|c| c.view == view)
        .unwrap()
}
/// One report from the port at `offset`, every mounted row 100 tall.
fn report(r: &mut Runner<NoData>, view: u32, offset: f64, fill: CollectionFill) {
    report_sized(r, view, offset, (100.0, 100.0), fill);
}
/// One report from a port `port` tall at `offset`, every mounted row `size` tall.
fn report_sized(
    r: &mut Runner<NoData>,
    view: u32,
    offset: f64,
    (port, size): (f64, f64),
    fill: CollectionFill,
) {
    report_rows(r, view, offset, port, |_| size, fill);
}
/// One report from a port `port` tall at `offset`, mounted row `n` `size(n)` tall.
fn report_rows(
    r: &mut Runner<NoData>,
    view: u32,
    offset: f64,
    port: f64,
    size: impl Fn(usize) -> f64,
    fill: CollectionFill,
) {
    let c = snapshot(r, view);
    r.collection_feedback_filled(
        CollectionFeedback {
            view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset,
            port_main: port,
            port_cross: 400.0,
            cross: 400.0,
            padding: [0.0; 2],
            measurements: c
                .rows
                .iter()
                .map(|row| RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    size: size(row.index),
                })
                .collect(),
            focus_view: None,
            interaction_view: None,
        },
        fill,
    )
    .unwrap();
}
/// A report while the host animates a smooth correction.
fn animating() -> CollectionFill {
    CollectionFill {
        animating: true,
        ..Default::default()
    }
}
/// A report while the reader's scroll goes on (a fling's frames).
fn scrolling(velocity: f64) -> CollectionFill {
    CollectionFill {
        velocity,
        scrolling: true,
        ..Default::default()
    }
}
/// The host at rest: it takes each correction, then reports where it put
/// the port, until the list asks for nothing more. Where it ends.
fn rest(r: &mut Runner<NoData>, view: u32, mut offset: f64) -> f64 {
    for _ in 0..8 {
        let c = snapshot(r, view);
        if let Some(correction) = c.correction {
            offset = correction.offset;
        }
        report(r, view, offset, CollectionFill::default());
        let after = snapshot(r, view);
        if after.correction.is_none() && !after.pending {
            break;
        }
    }
    offset
}
fn status(r: &Runner<NoData>) -> String {
    exact_runner::agent::state(r)
}
/// The list's latest request ended, or stands, `want` (`"done"`, `"pending"`, …).
#[track_caller]
fn assert_status(r: &Runner<NoData>, want: &str) {
    let s = status(r);
    assert!(s.contains(&format!("\"status\":\"{want}\"")), "{s}");
}
fn shows(c: &CollectionSnapshot, row: usize) -> bool {
    c.rows.iter().any(|r| r.index == row)
}

#[test]
fn a_request_during_the_readers_scroll_waits_and_lands_when_it_ends() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 200.0, scrolling(3000.0));
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    let c = snapshot(&r, feed);
    assert!(c.waiting && !c.seeking, "it waits");
    assert!(c.correction.is_none(), "nothing moves: {:?}", c.correction);
    assert!(!c.pending, "it asks for no report of its own");
    assert!(shows(&c, 2), "the reader's window stays");
    assert_status(&r, "pending");
    // The momentum carries the port past the destination and back: none of
    // it is a correction, and no number of reports ends the request.
    for (i, offset) in (0..40)
        .map(|i| 200.0 + 20.0 * f64::from(i))
        .chain([900.0, 950.0, 300.0])
        .enumerate()
    {
        report(&mut r, feed, offset, scrolling(3000.0));
        let c = snapshot(&r, feed);
        assert!(c.correction.is_none(), "report {i}: {:?}", c.correction);
        let row = (offset / 100.0).floor() as usize;
        assert!(shows(&c, row), "report {i}: the port's row {row} is built");
        assert!(
            status(&r).contains("\"status\":\"pending\""),
            "report {i}: {}",
            status(&r)
        );
    }
    let at = rest(&mut r, feed, 300.0);
    assert_eq!(at, 900.0, "it lands where row 9 starts");
    assert_status(&r, "done");
    assert!(
        snapshot(&r, feed).correction.is_none(),
        "done leaves nothing owed"
    );
}

#[test]
fn a_gesture_begun_while_it_waits_cancels_it_where_the_reader_is() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 200.0, scrolling(3000.0));
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    report(&mut r, feed, 300.0, scrolling(2000.0));
    report(
        &mut r,
        feed,
        320.0,
        CollectionFill {
            gesture: true,
            ..scrolling(-500.0)
        },
    );
    assert_status(&r, "cancelled");
    assert_eq!(rest(&mut r, feed, 320.0), 320.0, "nothing lands afterwards");
}

/// A waiting request is anchored where the reader is: its row leaving
/// cancels it, and the list keeps the reader's place (the reader at 300 and
/// the destination 1000 must not make the deletion a jump to 900).
#[test]
fn its_row_leaving_while_it_waits_cancels_it_and_the_reader_stays() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 300.0, scrolling(3000.0));
    r.act("go", vec![Value::Number(10.0)]).unwrap();
    r.act("drop", vec![Value::Number(10.0)]).unwrap();
    assert!(
        status(&r).contains("\"status\":\"cancelled\""),
        "at once: {}",
        status(&r)
    );
    let c = snapshot(&r, feed);
    assert!(
        c.correction
            .is_none_or(|c| c.from.is_some() && c.offset == c.from.unwrap()),
        "no move: {:?}",
        c.correction
    );
    report(&mut r, feed, 300.0, scrolling(0.0));
    assert_status(&r, "cancelled");
    assert_eq!(rest(&mut r, feed, 300.0), 300.0);
}

/// CSSOM View skips a scroll to where the port already is: a request the
/// port satisfies when it is made, during a fling too, is done, moving
/// nothing, and never becomes a later jump back. One it does not satisfy
/// waits, and lands where the fling ends.
#[test]
fn a_request_the_port_already_satisfies_is_done_and_never_waits() {
    let (mut r, feed) = boot();
    // The rows the port reaches are built and measured before each request.
    for _ in 0..2 {
        report(&mut r, feed, 200.0, scrolling(3000.0));
    }
    r.act("go", vec![Value::Number(2.0)]).unwrap();
    let c = snapshot(&r, feed);
    assert!(!c.waiting && !c.seeking && c.correction.is_none(), "{c:?}");
    assert_status(&r, "done");
    for _ in 0..2 {
        report(&mut r, feed, 700.0, scrolling(3000.0));
    }
    r.act("go", vec![Value::Number(7.0)]).unwrap();
    assert_status(&r, "done");
    assert_eq!(
        rest(&mut r, feed, 820.0),
        820.0,
        "the fling carries the port past row 7, and nothing lands"
    );

    report(&mut r, feed, 820.0, scrolling(3000.0));
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    assert!(
        snapshot(&r, feed).waiting,
        "row 9 is not at the port's start"
    );
    assert_eq!(rest(&mut r, feed, 880.0), 900.0);
    assert_status(&r, "done");
}

/// During a fling a host builds its window in slices, so the window still
/// owes rows past what shows; a request the port already satisfies is done
/// all the same, judged on the rows that lay its target out now, and the
/// fling goes on.
#[test]
fn a_satisfied_request_is_done_while_a_fling_still_owes_rows() {
    let (mut r, feed) = boot();
    let sliced = CollectionFill {
        velocity: 3000.0,
        limit: Some(0),
        ..Default::default()
    };
    for _ in 0..3 {
        report(&mut r, feed, 300.0, sliced);
    }
    assert!(
        snapshot(&r, feed).pending,
        "the fling's window still owes rows"
    );
    r.act("go", vec![Value::Number(3.0)]).unwrap();
    let c = snapshot(&r, feed);
    assert!(c.correction.is_none() && !c.seeking, "{c:?}");
    assert_status(&r, "done");
}

/// Two hundred rows 50 tall, estimated at 80, in a port 300 tall: each
/// first measurement of a row above the anchor moves the port with it.
const LONG: &str = r#"component App
  state ids = range(200)
  action go(n: number)
    scrollIntoView("feed", n, block="start")
  action near(n: number)
    scrollIntoView("feed", n, block="nearest")
  view
    list id="feed" virtualized=true height=300 width=400 estimated-item-height=80
      for i in ids key=i
        text `${i}` height=50
"#;
/// A host that takes each correction (relative or absolute) and reports the
/// port `step` further each time with `fill`, `n` times: where it leaves the
/// port. At rest at row 150 first.
fn fling(r: &mut Runner<NoData>, feed: u32, step: f64, n: usize, fill: CollectionFill) -> f64 {
    let follow = |r: &Runner<NoData>, at: f64| match snapshot(r, feed).correction {
        Some(c) => c.from.map_or(c.offset, |from| at + c.offset - from),
        None => at,
    };
    r.act("go", vec![Value::Number(150.0)]).unwrap();
    let mut at = 0.0;
    for _ in 0..8 {
        at = follow(r, at);
        report_sized(r, feed, at, (300.0, 50.0), CollectionFill::default());
    }
    for _ in 0..n {
        at = follow(r, at) + step;
        report_sized(r, feed, at, (300.0, 50.0), fill);
    }
    follow(r, at)
}

/// A fling toward the start measures the rows it brings in above the port,
/// each report moving the port with them (an anchor's relative correction)
/// and mounting the next rows above unmeasured. A request the port already
/// satisfies is done all the same and moves nothing: those rows do not move
/// its target in the port.
#[test]
fn a_satisfied_request_is_done_during_a_fling_toward_the_start() {
    let (mut r, feed) = boot_app(LONG);
    let sliced = CollectionFill {
        velocity: -3000.0,
        limit: Some(4),
        ..Default::default()
    };
    let port = fling(&mut r, feed, -150.0, 12, sliced);
    let c = snapshot(&r, feed);
    let first = c
        .rows
        .iter()
        .filter(|x| x.start + x.size > port)
        .map(|x| x.index)
        .min()
        .unwrap();
    r.act("near", vec![Value::Number((first + 2) as f64)])
        .unwrap();
    let c = snapshot(&r, feed);
    assert_status(&r, "done");
    assert!(
        c.correction.is_none_or(|c| c.from.is_some()),
        "no move of its own: {:?}",
        c.correction
    );
}

/// On a row list an anchor's shift still owed is one the web does not make
/// while the reader moves the list (LLP 1070 H4): a request the port satisfies
/// both with and without that shift is done; one it satisfies with it only
/// is left to move.
#[test]
fn a_row_list_judges_a_request_with_and_without_an_anchors_shift() {
    let row = LONG.replace(
        "virtualized=true height=300 width=400 estimated-item-height=80",
        "virtualized=true display=\"flex\" width=300 height=50 overflow-x=\"scroll\" overflow-y=\"hidden\" estimated-item-width=80",
    )
    .replace("height=50\n", "width=50\n");
    let sliced = CollectionFill {
        velocity: -3000.0,
        limit: Some(4),
        ..Default::default()
    };
    // Cards whole where the port stands with the shift and without it
    let judge = |pick: &dyn Fn(bool, bool) -> bool| {
        let (mut r, feed) = boot_app(&row);
        let port = fling(&mut r, feed, -150.0, 12, sliced);
        let c = snapshot(&r, feed);
        let unshifted = c
            .correction
            .filter(|c| c.from.is_some())
            .map_or(port, |c| port - (c.offset - c.from.unwrap()));
        let whole = |row: &exact_runner::CollectionRow, at: f64| {
            row.measured && row.start >= at - 0.5 && row.start + row.size <= at + 300.5
        };
        let target = c
            .rows
            .iter()
            .find(|x| pick(whole(x, port), whole(x, unshifted)))
            .map(|x| x.index)?;
        r.act("near", vec![Value::Number(target as f64)]).unwrap();
        Some(status(&r))
    };
    let both = judge(&|a, b| a && b).expect("a card whole either way");
    assert!(both.contains("\"status\":\"done\""), "{both}");
    if let Some(one) = judge(&|a, b| a && !b) {
        assert!(one.contains("\"status\":\"pending\""), "{one}");
    }
}

/// A request made during a fling toward the start is judged when it is made,
/// where the last report's anchor correction leaves the port: that report
/// measured rows above the port, which moved it with them.
#[test]
fn a_request_made_during_a_fling_is_judged_where_the_last_reports_correction_leaves_the_port() {
    let flinging = CollectionFill {
        velocity: -3000.0,
        scrolling: true,
        limit: Some(4),
        ..Default::default()
    };
    // The request made after seven reports, the last of which measured rows above the port
    let run = |target: Option<usize>| {
        let (mut r, feed) = boot_app(LONG);
        let at = fling(&mut r, feed, -150.0, 6, flinging);
        report_sized(&mut r, feed, at - 150.0, (300.0, 50.0), flinging);
        let c = snapshot(&r, feed);
        if let Some(t) = target {
            r.act("near", vec![Value::Number(t as f64)]).unwrap();
        }
        let port = c.correction.map_or(at - 150.0, |c| {
            c.from.map_or(c.offset, |from| at - 150.0 + c.offset - from)
        });
        (r, c, at - 150.0, port)
    };
    let (_, c, reported, port) = run(None);
    assert!(
        port < reported,
        "the report's correction moves the port: {reported} to {port}"
    );
    let whole = |x: &CollectionSnapshot, i: usize, at: f64| {
        x.rows.iter().any(|row| {
            row.index == i
                && row.measured
                && row.start >= at - 0.5
                && row.start + row.size <= at + 300.5
        })
    };
    let shown = c
        .rows
        .iter()
        .map(|x| x.index)
        .find(|&i| whole(&c, i, port) && !whole(&c, i, reported))
        .unwrap();
    let below = c
        .rows
        .iter()
        .map(|x| x.index)
        .find(|&i| whole(&c, i, reported) && !whole(&c, i, port))
        .unwrap();
    let (r, ..) = run(Some(shown));
    assert!(
        status(&r).contains("\"status\":\"done\""),
        "row {shown} shows whole: {}",
        status(&r)
    );
    let (r, ..) = run(Some(below));
    assert!(
        status(&r).contains("\"status\":\"pending\""),
        "row {below} is below the port: {}",
        status(&r)
    );
}

/// A host makes every admission's move. A data update before its next
/// report replaces the admission's correction in kind, so the destination
/// moves with the rows.
#[test]
fn a_data_update_after_an_admission_anchors_where_the_host_moved_the_port() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 300.0, CollectionFill::default());
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    assert_eq!(snapshot(&r, feed).correction.map(|c| c.offset), Some(900.0));
    r.act("drop", vec![Value::Number(0.0)]).unwrap();
    // The admission's correction is not yet acknowledged, so the anchor's
    // replaces it in kind (absolute): row 9 now starts at 800.
    let c = snapshot(&r, feed).correction.unwrap();
    assert_eq!(c.offset, 800.0, "{c:?}");
    assert_eq!(rest(&mut r, feed, 800.0), 800.0);
    assert_status(&r, "done");
}

/// A list that follows its end does not follow it while a request is under
/// way: the request overrides the kept position (LLP 1070.000 §2.5), so a row
/// appended while it waits makes no move.
#[test]
fn a_list_that_follows_its_end_does_not_while_a_request_waits() {
    let (mut r, feed) =
        boot_app(&APP.replace("virtualized=true", "virtualized=true scrollFollowEnd=true"));
    report(&mut r, feed, 1000.0, scrolling(3000.0));
    r.act("go", vec![Value::Number(2.0)]).unwrap();
    r.act("add", vec![Value::Number(11.0)]).unwrap();
    let c = snapshot(&r, feed);
    assert!(c.waiting, "{c:?}");
    assert!(
        c.correction.is_none_or(|c| c.from.is_some()),
        "no absolute move: {:?}",
        c.correction
    );
    assert_eq!(rest(&mut r, feed, 1000.0), 200.0);
}

/// A host tells a new request from the one before by its number, so a
/// gesture it saw during the first is not taken for one during the second.
#[test]
fn each_request_has_its_own_number() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 200.0, scrolling(3000.0));
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    let first = snapshot(&r, feed);
    r.act("go", vec![Value::Number(8.0)]).unwrap();
    let second = snapshot(&r, feed);
    assert!(first.waiting && second.waiting);
    assert_ne!(first.request, second.request);
}

/// A request that replaces a smooth one before any report waits for where
/// the port is, and asks for that report: the host may have nothing else to
/// report (the animation never started, or never moved), so a request that
/// only waited would wait for good. So does a port left unknown with no
/// request under way, whose updates anchor nothing until a report.
#[test]
fn where_the_port_is_unknown_the_list_asks_for_a_report() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 300.0, CollectionFill::default());
    r.act("glide", vec![Value::Number(9.0)]).unwrap();
    r.act("go", vec![Value::Number(2.0)]).unwrap();
    let c = snapshot(&r, feed);
    assert!(c.waiting && c.pending && c.correction.is_none(), "{c:?}");
    report(&mut r, feed, 300.0, CollectionFill::default());
    assert_eq!(snapshot(&r, feed).correction.map(|c| c.offset), Some(200.0));
    assert_eq!(rest(&mut r, feed, 300.0), 200.0);

    r.act("glide", vec![Value::Number(9.0)]).unwrap();
    r.act("drop", vec![Value::Number(9.0)]).unwrap();
    let c = snapshot(&r, feed);
    assert!(
        status(&r).contains("\"status\":\"cancelled\"") && c.pending,
        "{c:?}"
    );
    report(&mut r, feed, 200.0, CollectionFill::default());
    assert!(
        !snapshot(&r, feed).pending,
        "a report said where the port is"
    );
}

/// A smooth request whose row leaves while its animation may still run ends,
/// and the update moves nothing: the host stops the animation where the port
/// is, and an anchor taken at the destination would move the reader by what
/// the rows did there.
/// Whether the host has yet reported the animation or not.
#[test]
fn a_smooth_request_whose_row_leaves_mid_animation_moves_nothing_after() {
    for reported in [true, false] {
        let (mut r, feed) = boot();
        report(&mut r, feed, 300.0, CollectionFill::default());
        r.act("glide", vec![Value::Number(10.0)]).unwrap();
        let c = snapshot(&r, feed).correction.unwrap();
        assert!(c.smooth, "{c:?}");
        if reported {
            report(&mut r, feed, c.offset, animating());
        }
        r.act("drop", vec![Value::Number(10.0)]).unwrap();
        assert_status(&r, "cancelled");
        assert!(
            snapshot(&r, feed).correction.is_none(),
            "{:?}",
            snapshot(&r, feed).correction
        );
        // Nor does the next update before a report says where the host stopped.
        r.act("drop", vec![Value::Number(9.0)]).unwrap();
        assert!(
            snapshot(&r, feed).correction.is_none(),
            "reported {reported}: {:?}",
            snapshot(&r, feed).correction
        );
    }
}

/// The host's animation is stopped, and where it left the port unknown,
/// whatever ends or replaces its request: a replacement made while the
/// reader's scroll is reported too, and a request that ends `unconverged`.
/// Until a report says where the port is, an update anchors nothing.
#[test]
fn a_stopped_animation_leaves_the_port_unknown_until_a_report() {
    let (mut r, feed) = boot();
    let both = CollectionFill {
        scrolling: true,
        ..animating()
    };
    report(&mut r, feed, 1000.0, both);
    r.act("go", vec![Value::Number(5.0)]).unwrap();
    r.act("drop", vec![Value::Number(0.0)]).unwrap();
    let c = snapshot(&r, feed);
    assert!(c.waiting && c.correction.is_none(), "{c:?}");
    report(&mut r, feed, 450.0, scrolling(0.0));
    r.act("drop", vec![Value::Number(1.0)]).unwrap();
    assert!(
        snapshot(&r, feed)
            .correction
            .is_some_and(|c| c.from.is_some()),
        "a report said where the port is: an update anchors again"
    );

    let (mut r, feed) = boot();
    report(&mut r, feed, 0.0, CollectionFill::default());
    r.act("glide", vec![Value::Number(9.0)]).unwrap();
    // The animation reports a destination the request never finds aligned.
    for _ in 0..8 {
        report(&mut r, feed, 500.0, animating());
    }
    assert_status(&r, "unconverged");
    r.act("drop", vec![Value::Number(0.0)]).unwrap();
    assert!(
        snapshot(&r, feed).correction.is_none(),
        "{:?}",
        snapshot(&r, feed).correction
    );
}

/// A request that has moved the port is not the one the port already
/// satisfies: landing at its own destination, it completes on its second
/// aligned report (§2.4), not as satisfied at its first.
#[test]
fn a_request_that_moved_the_port_is_done_on_its_second_aligned_report() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 300.0, CollectionFill::default());
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    report(&mut r, feed, 900.0, CollectionFill::default());
    assert_status(&r, "pending");
    report(&mut r, feed, 900.0, CollectionFill::default());
    assert_status(&r, "done");
}

/// A request made while the host animates is not judged at the animation's
/// destination, which is all its reports give: the host stops the animation
/// where the port is, and the next report says where that is (§2.5).
#[test]
fn a_request_made_during_the_hosts_animation_is_not_judged_at_its_destination() {
    let (mut r, feed) = boot();
    r.act("glide", vec![Value::Number(9.0)]).unwrap();
    for _ in 0..2 {
        report(&mut r, feed, 900.0, animating());
    }
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    assert!(snapshot(&r, feed).waiting, "where the port is is unknown");
    assert_eq!(rest(&mut r, feed, 700.0), 900.0);
    assert_status(&r, "done");
}

/// While the host animates, its reports give where the animation is headed
/// (§11): a request made then waits for a report of where the port is, and
/// no report made during the animation completes one (§2.6).
#[test]
fn reports_while_the_host_animates_complete_no_request() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 200.0, animating());
    r.act("go", vec![Value::Number(2.0)]).unwrap();
    report(&mut r, feed, 200.0, animating());
    assert_status(&r, "pending");
    let (mut r, feed) = boot();
    r.act("glide", vec![Value::Number(9.0)]).unwrap();
    for _ in 0..2 {
        report(&mut r, feed, 900.0, animating());
    }
    assert_status(&r, "pending");
    for _ in 0..2 {
        report(&mut r, feed, 900.0, CollectionFill::default());
    }
    assert_status(&r, "done");
}

/// A request whose wait ends where the port already satisfies it, after a
/// first report that did not, is done there and moves nothing: the reader's
/// motion resuming afterwards, with no gesture, never lands it later.
#[test]
fn a_request_whose_wait_ends_where_the_port_satisfies_it_is_done_there() {
    const LIST: &str = r#"component App
  state ids = range(100)
  action near(n: number)
    scrollIntoView("feed", n, block="nearest")
  view
    list id="feed" virtualized=true height=300 width=400 estimated-item-height=50
      for i in ids key=i
        text `${i}` height=50
"#;
    let (mut r, feed) = boot_app(LIST);
    let sized = |r: &mut Runner<NoData>, at: f64, fill: CollectionFill| {
        report_sized(r, feed, at, (300.0, 50.0), fill)
    };
    for at in [0.0, 200.0, 400.0] {
        sized(&mut r, at, scrolling(3000.0));
    }
    // Row 20 is 1000 to 1050: shown whole by a port start in [750, 1000]
    r.act("near", vec![Value::Number(20.0)]).unwrap();
    sized(&mut r, 600.0, scrolling(3000.0));
    assert!(
        snapshot(&r, feed).waiting,
        "unsatisfied at its first report"
    );
    sized(&mut r, 800.0, scrolling(500.0));
    sized(&mut r, 800.0, scrolling(500.0));
    sized(&mut r, 800.0, CollectionFill::default());
    assert!(
        snapshot(&r, feed).correction.is_none(),
        "satisfied where the wait ends: no move"
    );
    assert!(
        status(&r).contains("\"status\":\"done\""),
        "done in the report where the wait ends: {}",
        status(&r)
    );
    sized(&mut r, 1200.0, scrolling(2000.0));
    sized(&mut r, 1200.0, CollectionFill::default());
    let c = snapshot(&r, feed);
    assert!(
        c.correction.is_none_or(|c| c.from.is_some()),
        "nothing takes the port back from 1200: {:?}, {}",
        c.correction,
        status(&r)
    );
}

/// Once a request has moved the port, a report that says the reader's
/// scroll is under way ends it, though the host reports no travel while the
/// request's moves are its own: as for a request admitted in the frame
/// before its host's report said a finger was down (§2.6).
#[test]
fn the_readers_scroll_reported_after_an_admission_ends_it_with_no_travel() {
    let (mut r, feed) = boot();
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    report(&mut r, feed, 900.0, scrolling(0.0));
    assert_status(&r, "cancelled");
    assert_eq!(rest(&mut r, feed, 920.0), 920.0, "nothing lands afterwards");
}

/// A request whose wait ends where its estimates align it but its target is
/// not yet measured is admitted there, which counts as its move: the reader's
/// scroll after it ends it where the reader is.
#[test]
fn a_request_admitted_where_its_wait_ends_is_ended_by_the_readers_scroll_after() {
    const LIST: &str = r#"component App
  state ids = range(100)
  action near(n: number)
    scrollIntoView("feed", n, block="nearest")
  view
    list id="feed" virtualized=true height=300 width=400 estimated-item-height=50
      for i in ids key=i
        text `${i}` height=50
"#;
    let (mut r, feed) = boot_app(LIST);
    let sized = |r: &mut Runner<NoData>, at: f64, fill: CollectionFill| {
        report_sized(r, feed, at, (300.0, 50.0), fill)
    };
    // The reader's scroll under way, its window not led
    sized(&mut r, 0.0, scrolling(0.0));
    // Row 20 is 1000 to 1050: shown whole by a port start in [750, 1000]
    r.act("near", vec![Value::Number(20.0)]).unwrap();
    assert!(!shows(&snapshot(&r, feed), 20));
    // The fling ends at 950 in one report, before row 20 is mounted
    sized(&mut r, 950.0, CollectionFill::default());
    assert!(
        status(&r).contains("\"status\":\"pending\""),
        "admitted, not done: {}",
        status(&r)
    );
    sized(&mut r, 1200.0, scrolling(0.0));
    assert_status(&r, "cancelled");
    sized(&mut r, 1200.0, CollectionFill::default());
    let c = snapshot(&r, feed);
    assert!(
        c.correction.is_none_or(|c| c.from.is_some()),
        "nothing takes the port back from 1200: {:?}",
        c.correction
    );
}

/// A row between the port's start and the target that has come into view
/// unmeasured leaves the question open: the request is admitted and moves
/// once that row's size is known, not done where its estimate would have it.
#[test]
fn a_row_before_the_target_not_yet_measured_leaves_the_request_open() {
    const LIST: &str = r#"component App
  state ids = range(30)
  action near(n: number)
    scrollIntoView("feed", n, block="nearest")
  view
    list id="feed" virtualized=true height=300 width=400 estimated-item-height=100
      for i in ids key=i
        text `${i}` height=100
"#;
    let (mut r, feed) = boot_app(LIST);
    for _ in 0..3 {
        report_sized(
            &mut r,
            feed,
            2000.0,
            (300.0, 100.0),
            CollectionFill::default(),
        );
    }
    // The port moves up past the window's first row: the row it now starts
    // at is mounted by that report, and measured only by the next one.
    let first = snapshot(&r, feed)
        .rows
        .iter()
        .map(|row| row.index)
        .min()
        .unwrap();
    let start = (first - 1) as f64 * 100.0;
    report_sized(
        &mut r,
        feed,
        start,
        (300.0, 100.0),
        CollectionFill::default(),
    );
    assert!(
        shows(&snapshot(&r, feed), first - 1),
        "row {} is mounted",
        first - 1
    );
    // The row after the window's first shows whole by the estimate
    r.act("near", vec![Value::Number((first + 1) as f64)])
        .unwrap();
    assert_status(&r, "pending");
    // The new row is 150 tall, over its estimate
    let mut at = start;
    for _ in 0..8 {
        if let Some(c) = snapshot(&r, feed).correction {
            at = c.offset;
        }
        let size = |n: usize| if n == first - 1 { 150.0 } else { 100.0 };
        report_rows(&mut r, feed, at, 300.0, size, CollectionFill::default());
    }
    assert_eq!(
        at,
        start + 50.0,
        "the target ends 50 below the port until it moves"
    );
    assert_status(&r, "done");
}

/// The move a request makes as its wait ends is its first, which is not one
/// of its six corrections (§2.4), as a request made at rest's is not.
#[test]
fn a_request_landing_after_its_wait_still_has_six_corrections() {
    let (mut r, feed) = boot();
    report(&mut r, feed, 200.0, scrolling(3000.0));
    r.act("go", vec![Value::Number(9.0)]).unwrap();
    report(&mut r, feed, 300.0, CollectionFill::default());
    assert_eq!(
        snapshot(&r, feed).correction.map(|c| c.offset),
        Some(900.0),
        "it lands"
    );
    // A host that never takes a correction
    for n in 1..=6 {
        report(&mut r, feed, 300.0, CollectionFill::default());
        assert!(
            status(&r).contains("\"status\":\"pending\""),
            "correction {n}: {}",
            status(&r)
        );
    }
    report(&mut r, feed, 300.0, CollectionFill::default());
    assert_status(&r, "unconverged");
}
